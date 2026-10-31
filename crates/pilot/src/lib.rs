pub mod action;
pub mod brain;
pub mod digest;
pub mod reflex;

use std::net::{SocketAddr, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::app::{App, PostUpdate};
use bevy::math::Vec2;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::netcode::{
    ClientAuthentication, MemorySocketClient, NativeSocket, NetcodeClientTransport,
    in_memory_server_addr,
};
use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetClient};
use bevy_replicon_renet2::{RenetChannelsExt, RepliconRenetPlugins};
use sv_data::{ContentPack, PilotDef};
use sv_protocol::{
    AccountState, ActiveQuests, BuyCommodity, BuyShip, ChiefEncounter, DockableKind, DockableTag,
    DockedAt, Fuel, Heading, Hull, LoginRequest, LoginResponse, Meteorite, Notice, PROTOCOL_ID,
    PlanetTag, PlayerDied, PortMarket, Position, RequestDock, RequestLand, RequestLiftOff,
    RespondToEncounter, SellCommodity, SetFiring, SetThrottle, SetTurn, SetWarp, ShipInfo,
    ShipKind, SystemMembership,
};

pub use action::{PilotAction, TradeDirection, ValidationContext, parse_and_validate};
pub use brain::{DecisionWorker, MIN_DECISION_INTERVAL, MockBrain, OllamaBrain, PilotBrain};
pub use digest::{DIGEST_MAX_TOKENS_EST, DecisionPoint, DigestInput, ThreatSummary, render_digest};
pub use reflex::{
    HazardDisc, StandingOrder, collect_hazards, compute_flee_target, next_hazard_safe_waypoint,
    steer, steer_approach,
};

pub const RECONNECT_COOLDOWN: Duration = Duration::from_secs(5);

#[derive(Resource, Clone)]
pub struct PilotConfig {
    pub pilot: PilotDef,
    pub client_id: u64,
    pub server_addr: Option<SocketAddr>,
    pub min_decision_interval: Duration,
    pub verbose: bool,
}

#[derive(Resource)]
pub struct PilotContent {
    pub pack: ContentPack,
    pub hazards: Vec<HazardDisc>,
}

#[derive(Resource)]
pub struct PilotRuntime {
    pub logged_in: bool,
    pub login_sent: bool,
    pub standing_order: StandingOrder,
    pub last_docked: Option<(String, String)>,
    pub last_encounter_key: Option<String>,
    pub last_completed_count: usize,
    pub last_hostile_alert: bool,
    pub last_low_fuel_alert: bool,
    pub pending_death: bool,
    pub was_alive: bool,
    pub docked_settled_ticks: u32,
    pub post_action_cooldown_ticks: u32,
    pub dock_request_cooldown_ticks: u32,
    pub idle_ticks: u32,
    pub last_reconnect: Instant,
    pub decisions_made: usize,
    pub last_action: Option<PilotAction>,
    pub notices: Vec<String>,
    pub worker: DecisionWorker,
    pub memory_socket: Option<MemorySocketClient>,
}

pub fn build_udp_transport(server_addr: SocketAddr) -> (u64, NetcodeClientTransport) {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after the epoch");
    let client_id = current_time.as_nanos() as u64;
    let socket = UdpSocket::bind("0.0.0.0:0").expect("client socket must bind");
    let transport = NetcodeClientTransport::new(
        current_time,
        ClientAuthentication::Unsecure {
            client_id,
            protocol_id: PROTOCOL_ID,
            socket_id: 0,
            server_addr,
            user_data: None,
        },
        NativeSocket::new(socket).expect("client socket must be valid"),
    )
    .expect("client transport must be valid");
    (client_id, transport)
}

pub fn build_memory_reconnect_transport(
    client_id: u64,
    socket: &MemorySocketClient,
) -> NetcodeClientTransport {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after the epoch");
    NetcodeClientTransport::new(
        current_time,
        ClientAuthentication::Unsecure {
            client_id,
            protocol_id: PROTOCOL_ID,
            socket_id: 0,
            server_addr: in_memory_server_addr(),
            user_data: None,
        },
        socket.clone(),
    )
    .expect("memory client transport must be valid")
}

pub fn load_persona_and_pack(
    persona_path: &Path,
    content_dir: &Path,
) -> Result<(PilotDef, ContentPack), String> {
    let pack = ContentPack::load_dir(content_dir).map_err(|err| {
        format!(
            "failed to load content pack at {}: {err}",
            content_dir.display()
        )
    })?;
    let pilot = ContentPack::load_pilot(persona_path).map_err(|err| {
        format!(
            "failed to load pilot persona at {}: {err}",
            persona_path.display()
        )
    })?;
    pack.validate_pilot(&pilot)
        .map_err(|err| format!("invalid pilot persona '{}': {err}", pilot.id))?;
    Ok((pilot, pack))
}

pub fn build_pilot_app(
    config: PilotConfig,
    pack: ContentPack,
    transport: NetcodeClientTransport,
    memory_socket: Option<MemorySocketClient>,
    brain: Box<dyn PilotBrain>,
) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(RepliconPlugins.set(ServerPlugin::new(PostUpdate)));
    app.add_plugins(RepliconRenetPlugins);
    sv_protocol::register(&mut app);

    let (server_channels, client_channels) = {
        let channels = app.world().resource::<RepliconChannels>();
        (channels.server_configs(), channels.client_configs())
    };
    app.insert_resource(RenetClient::new(
        ConnectionConfig::from_channels(server_channels, client_channels),
        false,
    ));
    app.insert_resource(transport);

    let hazards = collect_hazards(&pack, 40.0);
    let worker_pack = pack.clone();
    let worker = DecisionWorker::spawn(
        brain,
        config.pilot.clone(),
        worker_pack,
        config.min_decision_interval,
    );

    let initial_order = StandingOrder::FlyTo {
        system: config.pilot.home_system.clone(),
        location: Some(config.pilot.home_port.clone()),
        dock_on_arrival: true,
    };

    app.insert_resource(PilotContent { pack, hazards });
    app.insert_resource(config);
    app.insert_resource(PilotRuntime {
        logged_in: false,
        login_sent: false,
        standing_order: initial_order,
        last_docked: None,
        last_encounter_key: None,
        last_completed_count: 0,
        last_hostile_alert: false,
        last_low_fuel_alert: false,
        pending_death: false,
        was_alive: false,
        docked_settled_ticks: 0,
        post_action_cooldown_ticks: 0,
        dock_request_cooldown_ticks: 0,
        idle_ticks: 0,
        last_reconnect: Instant::now(),
        decisions_made: 0,
        last_action: None,
        notices: Vec::new(),
        worker,
        memory_socket,
    });

    app.add_observer(on_login_response);
    app.add_observer(on_notice);
    app.add_observer(on_player_died);
    app.add_systems(
        Update,
        (
            ensure_connected_system,
            send_login_system,
            poll_decision_worker_system,
            detect_decision_points_system,
            execute_reflex_system,
        )
            .chain(),
    );
    app.finish();
    app
}

fn on_login_response(
    trigger: On<LoginResponse>,
    config: Res<PilotConfig>,
    mut runtime: ResMut<PilotRuntime>,
) {
    let ev = trigger.event();
    if ev.success {
        runtime.logged_in = true;
        if config.verbose {
            eprintln!(
                "sv-pilot [{}]: logged in ({} cr, system={})",
                config.pilot.id, ev.credits, ev.system
            );
        }
    } else {
        eprintln!(
            "sv-pilot [{}]: login rejected: {}",
            config.pilot.id, ev.message
        );
    }
}

fn on_notice(trigger: On<Notice>, config: Res<PilotConfig>, mut runtime: ResMut<PilotRuntime>) {
    let text = trigger.event().text.clone();
    if config.verbose {
        eprintln!("sv-pilot [{}] notice: {text}", config.pilot.id);
    }
    if runtime.notices.len() >= 128 {
        runtime.notices.remove(0);
    }
    runtime.notices.push(text);
}

fn on_player_died(
    _trigger: On<PlayerDied>,
    config: Res<PilotConfig>,
    mut runtime: ResMut<PilotRuntime>,
) {
    if config.verbose {
        eprintln!(
            "sv-pilot [{}]: ship destroyed; awaiting respawn",
            config.pilot.id
        );
    }
    runtime.pending_death = true;
    runtime.was_alive = false;
}

fn ensure_connected_system(world: &mut World) {
    let connected = world
        .get_resource::<NetcodeClientTransport>()
        .is_some_and(|t| t.is_connected());
    if connected {
        return;
    }
    let since = world.resource::<PilotRuntime>().last_reconnect.elapsed();
    if since < RECONNECT_COOLDOWN {
        return;
    }

    let server_addr = world.resource::<PilotConfig>().server_addr;
    let memory_socket = world.resource::<PilotRuntime>().memory_socket.clone();
    if server_addr.is_none() && memory_socket.is_none() {
        return;
    }

    world.resource_mut::<PilotRuntime>().last_reconnect = Instant::now();
    world.resource_mut::<PilotRuntime>().login_sent = false;
    world.resource_mut::<PilotRuntime>().logged_in = false;

    let (server_configs, client_configs) = {
        let channels = world.resource::<RepliconChannels>();
        (channels.server_configs(), channels.client_configs())
    };
    world.remove_resource::<NetcodeClientTransport>();
    world.remove_resource::<RenetClient>();
    world.insert_resource(RenetClient::new(
        ConnectionConfig::from_channels(server_configs, client_configs),
        false,
    ));

    if let Some(addr) = server_addr {
        let (new_id, transport) = build_udp_transport(addr);
        world.resource_mut::<PilotConfig>().client_id = new_id;
        world.insert_resource(transport);
    } else if let Some(sock) = memory_socket {
        let id = world.resource::<PilotConfig>().client_id;
        let transport = build_memory_reconnect_transport(id, &sock);
        world.insert_resource(transport);
    }

    let mut remotes = world.query_filtered::<Entity, With<Remote>>();
    let entities: Vec<Entity> = remotes.iter(world).collect();
    for entity in entities {
        world.entity_mut(entity).despawn();
    }
}

fn send_login_system(
    config: Res<PilotConfig>,
    mut runtime: ResMut<PilotRuntime>,
    client: Res<RenetClient>,
    mut commands: Commands,
) {
    if runtime.login_sent || !client.is_connected() {
        return;
    }
    runtime.login_sent = true;
    commands.client_trigger(LoginRequest {
        username: config.pilot.username.clone(),
        password: config.pilot.password.clone(),
    });
}

type MyShipQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ShipInfo,
        &'static Position,
        &'static Heading,
        &'static Hull,
        &'static Fuel,
        &'static SystemMembership,
        Option<&'static DockedAt>,
        Option<&'static AccountState>,
        Option<&'static ChiefEncounter>,
        Option<&'static ActiveQuests>,
    ),
    With<Remote>,
>;

fn poll_decision_worker_system(
    config: Res<PilotConfig>,
    mut runtime: ResMut<PilotRuntime>,
    mut commands: Commands,
) {
    let Some((reason, action)) = runtime.worker.try_recv() else {
        return;
    };
    if config.verbose {
        eprintln!(
            "sv-pilot [{}]: decision ({}) -> {:?}",
            config.pilot.id,
            reason.as_str(),
            action
        );
    }
    runtime.decisions_made += 1;
    runtime.last_action = Some(action.clone());
    runtime.post_action_cooldown_ticks = 15;
    runtime.idle_ticks = 0;

    match action {
        PilotAction::Respond { choice } => {
            commands.client_trigger(RespondToEncounter { choice });
        }
        PilotAction::Trade {
            direction,
            commodity,
            qty,
        } => match direction {
            TradeDirection::Buy => {
                commands.client_trigger(BuyCommodity { commodity, qty });
            }
            TradeDirection::Sell => {
                commands.client_trigger(SellCommodity { commodity, qty });
            }
        },
        PilotAction::BuyShip { ship } => {
            commands.client_trigger(BuyShip { ship });
        }
        PilotAction::FlyTo { system, location } => {
            runtime.standing_order = StandingOrder::FlyTo {
                system,
                location,
                dock_on_arrival: false,
            };
        }
        PilotAction::Dock { system, location } => {
            runtime.standing_order = StandingOrder::FlyTo {
                system,
                location: Some(location),
                dock_on_arrival: true,
            };
        }
        PilotAction::Flee { system } => {
            runtime.standing_order = StandingOrder::Flee {
                target_system: system,
            };
        }
        PilotAction::Idle => {
            runtime.standing_order = StandingOrder::Idle;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn detect_decision_points_system(
    config: Res<PilotConfig>,
    content: Res<PilotContent>,
    mut runtime: ResMut<PilotRuntime>,
    ships: MyShipQuery,
    other_ships: Query<(&ShipInfo, &Position, &SystemMembership), With<Remote>>,
    meteorites: Query<(&Meteorite, &Position, &SystemMembership), With<Remote>>,
    ports: Query<(&DockableTag, &PortMarket), With<Remote>>,
) {
    if runtime.post_action_cooldown_ticks > 0 {
        runtime.post_action_cooldown_ticks -= 1;
    }

    let Some((
        info,
        position,
        _heading,
        hull,
        fuel,
        membership,
        docked,
        account,
        encounter,
        quests,
    )) = ships
        .iter()
        .find(|(info, ..)| info.owner == Some(config.client_id))
    else {
        runtime.was_alive = false;
        return;
    };

    let just_respawned =
        runtime.pending_death || (!runtime.was_alive && runtime.decisions_made > 0);
    runtime.was_alive = true;

    let current_docked = docked.map(|d| (d.system.clone(), d.location.clone()));
    if current_docked.is_some() {
        runtime.docked_settled_ticks = runtime.docked_settled_ticks.saturating_add(1);
    } else {
        runtime.docked_settled_ticks = 0;
    }

    let encounter_key = encounter.map(|e| {
        let opts = e
            .options
            .iter()
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!("{}:{:?}:{}", e.chief_name, e.mood, opts)
    });

    let completed_count = quests.map_or(0, |q| q.completed.len());

    // Gather threats (pirates, police, close meteorites):
    let approaching_dock = matches!(
        runtime.standing_order,
        StandingOrder::FlyTo {
            dock_on_arrival: true,
            ..
        }
    );
    let mut threats = Vec::new();
    let mut hostile_close = false;
    for (other_info, other_pos, _other_sys) in &other_ships {
        if other_info.owner == Some(config.client_id) {
            continue;
        }
        let dist = position.0.distance(other_pos.0);
        if other_info.kind == ShipKind::Pirate {
            if dist <= config.pilot.flee_range && !approaching_dock {
                hostile_close = true;
            }
            if dist <= 1200.0 {
                threats.push(ThreatSummary {
                    kind: "pirate".into(),
                    name: other_info.name.clone(),
                    distance: dist,
                });
            }
        }
    }
    for (met, met_pos, met_sys) in &meteorites {
        if met_sys.0 == membership.0 {
            let dist = position.0.distance(met_pos.0);
            if dist <= 400.0 {
                threats.push(ThreatSummary {
                    kind: "meteorite".into(),
                    name: met.size.as_str().into(),
                    distance: dist,
                });
            }
        }
    }
    threats.sort_by_key(|t| t.distance.to_bits());
    threats.truncate(3);

    let fuel_ratio = if fuel.max > 0.0 {
        fuel.current / fuel.max
    } else {
        1.0
    };
    let low_fuel = current_docked.is_none() && fuel_ratio < config.pilot.low_fuel_ratio;

    runtime.idle_ticks = runtime.idle_ticks.saturating_add(1);
    let idle_threshold_ticks = (config.pilot.idle_interval_secs * 60.0).max(60.0) as u32;

    // Determine if a decision point fired:
    let mut trigger_reason: Option<DecisionPoint> = None;

    if just_respawned {
        trigger_reason = Some(DecisionPoint::RespawnedAfterDeath);
    } else if encounter_key.is_some() && encounter_key != runtime.last_encounter_key {
        trigger_reason = Some(DecisionPoint::EncounterOffered);
    } else if completed_count > runtime.last_completed_count {
        trigger_reason = Some(DecisionPoint::QuestComplete);
    } else if current_docked.is_some()
        && current_docked != runtime.last_docked
        && (encounter.is_some() || runtime.docked_settled_ticks >= 3)
    {
        trigger_reason = Some(DecisionPoint::Docked);
    } else if hostile_close && !runtime.last_hostile_alert && current_docked.is_none() {
        trigger_reason = Some(DecisionPoint::HostileClose);
    } else if low_fuel && !runtime.last_low_fuel_alert {
        trigger_reason = Some(DecisionPoint::LowFuel);
    } else if current_docked.is_some()
        && encounter.is_none()
        && matches!(
            runtime.standing_order,
            StandingOrder::FlyTo {
                ref system,
                location: Some(ref loc),
                dock_on_arrival: true,
            } if current_docked.as_ref() == Some(&(system.clone(), loc.clone()))
        )
        && runtime.post_action_cooldown_ticks == 0
    {
        // Just finished an encounter or trade while docked and still need departure orders:
        trigger_reason = Some(DecisionPoint::Docked);
    } else if runtime.idle_ticks >= idle_threshold_ticks {
        trigger_reason = Some(DecisionPoint::IdleHeartbeat);
    }

    if !hostile_close {
        runtime.last_hostile_alert = false;
    }
    if !low_fuel {
        runtime.last_low_fuel_alert = false;
    }

    let Some(reason) = trigger_reason else {
        if encounter_key.is_none() {
            runtime.last_encounter_key = None;
        }
        if current_docked.is_none() {
            runtime.last_docked = None;
        }
        return;
    };

    if !runtime.worker.can_request() || runtime.post_action_cooldown_ticks > 0 {
        return;
    }

    let market: Vec<(String, u32)> = current_docked
        .as_ref()
        .and_then(|(sys, loc)| {
            ports
                .iter()
                .find(|(tag, _)| &tag.system == sys && &tag.id == loc)
                .map(|(_, pm)| {
                    pm.prices
                        .iter()
                        .map(|p| (p.commodity.clone(), p.price))
                        .collect()
                })
        })
        .unwrap_or_default();

    let shipyard: Vec<(String, u32, u32)> = current_docked
        .as_ref()
        .and_then(|(sys, loc)| content.pack.dockable(sys, loc))
        .and_then(|port| port.shipyard())
        .map(|yard| {
            yard.ships
                .iter()
                .filter_map(|id| content.pack.ship(id))
                .map(|s| (s.id.clone(), s.price, s.cargo))
                .collect()
        })
        .unwrap_or_default();

    let acc = account.cloned().unwrap_or(AccountState {
        credits: 0,
        cargo: Vec::new(),
        cargo_capacity: 20,
    });

    let digest_input = DigestInput {
        reason,
        ship_name: info.name.clone(),
        ship_id: info.def_id.clone(),
        shields: hull.shields,
        max_shields: hull.max_shields,
        armor: hull.armor,
        max_armor: hull.max_armor,
        fuel: fuel.current,
        max_fuel: fuel.max,
        credits: acc.credits,
        cargo: acc.cargo,
        cargo_capacity: acc.cargo_capacity,
        system: membership.0.clone(),
        docked: current_docked.clone(),
        market,
        shipyard,
        encounter: encounter.cloned(),
        quests: quests.map(|q| q.quests.clone()).unwrap_or_default(),
        threats,
    };

    if runtime.worker.request(digest_input) {
        runtime.pending_death = false;
        runtime.last_docked = current_docked;
        runtime.last_encounter_key = encounter_key;
        runtime.last_completed_count = completed_count;
        if reason == DecisionPoint::HostileClose {
            runtime.last_hostile_alert = true;
        }
        if reason == DecisionPoint::LowFuel {
            runtime.last_low_fuel_alert = true;
        }
        runtime.idle_ticks = 0;
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_reflex_system(
    config: Res<PilotConfig>,
    content: Res<PilotContent>,
    mut runtime: ResMut<PilotRuntime>,
    ships: MyShipQuery,
    other_ships: Query<(&ShipInfo, &Position), With<Remote>>,
    planets: Query<(&PlanetTag, &Position), With<Remote>>,
    dockables: Query<(&DockableTag, &Position), With<Remote>>,
    mut commands: Commands,
) {
    if runtime.dock_request_cooldown_ticks > 0 {
        runtime.dock_request_cooldown_ticks -= 1;
    }

    let Some((_, position, heading, _, fuel, membership, docked, _, encounter, _)) = ships
        .iter()
        .find(|(info, ..)| info.owner == Some(config.client_id))
    else {
        return;
    };

    // Never maneuver while an encounter dialog is open:
    if encounter.is_some() {
        return;
    }

    // Immediate emergency flee reflex if undocked and a pirate is within flee_range
    // (unless we are actively flying toward a docking port with enough shields/armor to make berth):
    let has_docking_order = matches!(
        runtime.standing_order,
        StandingOrder::FlyTo {
            dock_on_arrival: true,
            ..
        }
    );

    let pirate_threats: Vec<Vec2> = other_ships
        .iter()
        .filter(|(info, pos)| {
            info.kind == ShipKind::Pirate && position.0.distance(pos.0) <= config.pilot.flee_range
        })
        .map(|(_, pos)| pos.0)
        .collect();

    if docked.is_none()
        && !has_docking_order
        && !pirate_threats.is_empty()
        && !matches!(runtime.standing_order, StandingOrder::Flee { .. })
    {
        let safe_sys = content
            .pack
            .system(&membership.0)
            .and_then(|s| s.links.first().cloned())
            .or_else(|| Some(config.pilot.home_system.clone()));
        runtime.standing_order = StandingOrder::Flee {
            target_system: safe_sys,
        };
    }

    // Auto-refuel / low-fuel emergency reflex if undocked and fuel < low_fuel_ratio:
    let fuel_ratio = if fuel.max > 0.0 {
        fuel.current / fuel.max
    } else {
        1.0
    };
    if docked.is_none()
        && pirate_threats.is_empty()
        && fuel_ratio < config.pilot.low_fuel_ratio
        && !has_docking_order
        && let Some(sys) = content.pack.system(&membership.0)
        && let Some(port) = sys
            .planets
            .first()
            .map(|p| p.id.clone())
            .or_else(|| sys.stations.first().map(|s| s.id.clone()))
    {
        runtime.standing_order = StandingOrder::FlyTo {
            system: membership.0.clone(),
            location: Some(port),
            dock_on_arrival: true,
        };
    }

    match runtime.standing_order.clone() {
        StandingOrder::Idle => {
            if docked.is_none() {
                commands.client_trigger(SetWarp(false));
                commands.client_trigger(SetThrottle(0.0));
                commands.client_trigger(SetTurn(0.0));
                commands.client_trigger(SetFiring(false));
            }
        }
        StandingOrder::Flee { target_system } => {
            if docked.is_some() {
                runtime.standing_order = StandingOrder::Idle;
                return;
            }
            if pirate_threats.is_empty()
                && target_system
                    .as_ref()
                    .is_some_and(|sys| sys == &membership.0)
            {
                // Reached safe system and clear of pirates; dock at nearest port or home:
                let port = content
                    .pack
                    .system(&membership.0)
                    .and_then(|s| {
                        s.planets
                            .first()
                            .map(|p| p.id.clone())
                            .or_else(|| s.stations.first().map(|st| st.id.clone()))
                    })
                    .unwrap_or_else(|| config.pilot.home_port.clone());
                runtime.standing_order = StandingOrder::FlyTo {
                    system: membership.0.clone(),
                    location: Some(port),
                    dock_on_arrival: true,
                };
                return;
            }
            let sys_pos = target_system
                .as_deref()
                .and_then(|s| content.pack.system(s))
                .map(|s| Vec2::new(s.position.0, s.position.1));
            let raw_goal = compute_flee_target(position.0, &pirate_threats, sys_pos);
            let waypoint = next_hazard_safe_waypoint(position.0, raw_goal, &content.hazards);
            let (turn, throttle) = steer(position.0, heading.0, waypoint);
            let use_warp = fuel.current > 10.0 && throttle > 0.5;
            commands.client_trigger(SetWarp(use_warp));
            commands.client_trigger(SetTurn(turn));
            commands.client_trigger(SetThrottle(throttle));
            commands.client_trigger(SetFiring(false));
        }
        StandingOrder::FlyTo {
            system,
            location,
            dock_on_arrival,
        } => {
            if let Some(d) = docked {
                if d.system == system && location.as_ref().is_none_or(|loc| loc == &d.location) {
                    // Already docked at destination:
                    return;
                }
                if runtime.dock_request_cooldown_ticks == 0 {
                    commands.client_trigger(RequestLiftOff);
                    runtime.dock_request_cooldown_ticks = 12;
                }
                return;
            }

            let Some((goal_pos, radius, kind)) = resolve_destination(
                &content.pack,
                &system,
                location.as_deref(),
                &planets,
                &dockables,
            ) else {
                return;
            };

            let dist_to_goal = position.0.distance(goal_pos);
            let dock_threshold = if dock_on_arrival {
                radius + 150.0
            } else {
                260.0
            };

            if dist_to_goal <= dock_threshold {
                commands.client_trigger(SetWarp(false));
                commands.client_trigger(SetThrottle(0.0));
                commands.client_trigger(SetTurn(0.0));
                commands.client_trigger(SetFiring(false));

                if dock_on_arrival {
                    if let Some(loc) = location
                        && runtime.dock_request_cooldown_ticks == 0
                    {
                        if kind == Some(DockableKind::Planet) && membership.0 == system {
                            commands.client_trigger(RequestLand { planet: loc });
                        } else {
                            commands.client_trigger(RequestDock {
                                system,
                                location: loc,
                            });
                        }
                        runtime.dock_request_cooldown_ticks = 15;
                    }
                } else {
                    runtime.standing_order = StandingOrder::Idle;
                }
                return;
            }

            let waypoint = next_hazard_safe_waypoint(position.0, goal_pos, &content.hazards);
            let slow_radius = if waypoint == goal_pos && dock_on_arrival {
                radius + 420.0
            } else {
                0.0
            };
            let (turn, throttle) = steer_approach(position.0, heading.0, waypoint, slow_radius);
            let desired = (waypoint - position.0).y.atan2((waypoint - position.0).x);
            let diff = (desired - heading.0 + std::f32::consts::PI)
                .rem_euclid(2.0 * std::f32::consts::PI)
                - std::f32::consts::PI;
            let use_warp = dist_to_goal > 550.0 && diff.abs() < 0.4 && fuel.current > 20.0;
            commands.client_trigger(SetWarp(use_warp));
            commands.client_trigger(SetTurn(turn));
            commands.client_trigger(SetThrottle(throttle));
            commands.client_trigger(SetFiring(false));
        }
    }
}

fn resolve_destination(
    pack: &ContentPack,
    system: &str,
    location: Option<&str>,
    planets: &Query<(&PlanetTag, &Position), With<Remote>>,
    dockables: &Query<(&DockableTag, &Position), With<Remote>>,
) -> Option<(Vec2, f32, Option<DockableKind>)> {
    if let Some(loc) = location {
        if let Some((tag, pos)) = dockables
            .iter()
            .find(|(tag, _)| tag.system == system && tag.id == loc)
        {
            return Some((pos.0, tag.radius, Some(tag.kind)));
        }
        if let Some((tag, pos)) = planets
            .iter()
            .find(|(tag, _)| tag.system == system && tag.id == loc)
        {
            return Some((pos.0, tag.radius, Some(DockableKind::Planet)));
        }
        let sys = pack.system(system)?;
        if let Some(p_pos) = sys.planet_position(loc) {
            return Some((
                Vec2::new(p_pos.0, p_pos.1),
                sv_data::PLANET_RADIUS,
                Some(DockableKind::Planet),
            ));
        }
        if let Some(station) = sys.stations.iter().find(|s| s.id == loc) {
            let pos = match &station.placement {
                sv_data::StationPlacement::Fixed { position } => {
                    Vec2::new(sys.position.0 + position.0, sys.position.1 + position.1)
                }
                sv_data::StationPlacement::Orbit { planet, radius, .. } => {
                    let center = sys.planet_position(planet)?;
                    Vec2::new(center.0 + *radius, center.1)
                }
            };
            return Some((pos, station.radius, Some(DockableKind::Station)));
        }
        None
    } else {
        let sys = pack.system(system)?;
        Some((Vec2::new(sys.position.0, sys.position.1), 0.0, None))
    }
}
