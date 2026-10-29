use std::f32::consts::PI;

use bevy::math::Vec2;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use bevy_replicon::shared::backend::connected_client::NetworkId;
use sv_core::{Account, FlightModel, ShipState, WeaponState, star_heat_dps};
use sv_data::{
    ContentPack, OutfitKind, QuestDef, QuestKind, StationPlacement, WeaponDef, WeaponMount,
};
use sv_protocol::{
    AccountState, ActiveQuest, ActiveQuests, BlackHoleTag, BuyCommodity, ChiefEncounter, ChiefMood,
    CycleTarget, DockableKind, DockableTag, DockedAt, EncounterOption, Energy, EquippedOutfits,
    Explosion, Fuel, HailShip, Heading, Hull, LoginRequest, LoginResponse, Meteorite,
    MeteoriteSize, Notice, PlanetTag, PlayerDied, PlayerReputation, PortMarket, Position,
    Projectile, ProjectileKind, QuestTarget, RequestDock, RequestLand, RequestLiftOff,
    RespondToEncounter, SellCommodity, SetAfterburner, SetBrake, SetFiring, SetSecondaryFiring,
    SetThrottle, SetTurn, SetWarp, ShipInfo, ShipKind, StarTag, SystemControl, SystemMembership,
    TargetId, TargetLock, Velocity,
};

use crate::Content;

pub mod contracts;
pub mod factions;
pub mod market;
mod progression;
use progression::*;

pub const PLAYER_SYSTEM: &str = "sol";
pub const PIRATE_SYSTEM: &str = "alpha-centauri";
pub const PLAYER_SHIP: &str = "shuttle";
pub const PIRATE_SHIP: &str = "turncoat";
pub const POLICE_SHIP: &str = "patrol";
pub const FINE_CREDITS: u32 = 200;
pub const STARTING_CREDITS: u32 = 500;
pub const JUMP_FUEL_COST: f32 = 25.0;
pub const DRAG: f32 = 0.4;
pub const SHIELD_REGEN: f32 = 4.0;
pub const SHIP_RADIUS: f32 = 14.0;
pub const PLANET_RADIUS: f32 = sv_data::PLANET_RADIUS;
pub const PLANET_OFFSET_X: f32 = sv_data::PLANET_OFFSET_X;
pub const LAND_RANGE: f32 = 220.0;
pub const HAIL_RANGE: f32 = 240.0;
pub const PIRATE_SIGHT_RANGE: f32 = 900.0;
pub const PIRATE_FIRE_RANGE: f32 = 300.0;
pub const PIRATE_RESPAWN_SECS: f32 = 15.0;
pub const PLAYER_RESPAWN_SECS: f32 = 3.0;
pub const SYSTEM_MEMBERSHIP_RADIUS: f32 = 800.0;
pub const FUEL_PRICE_PER_UNIT: f32 = 1.0;
pub const WARP_SPEED_MULTIPLIER: f32 = 3.0;
pub const WARP_FUEL_RATE: f32 = 8.0;
pub const AFTERBURNER_SPEED_MULTIPLIER: f32 = 1.6;
pub const AFTERBURNER_ENERGY_RATE: f32 = 18.0;
pub const TURRET_RANGE: f32 = 580.0;
pub const MISSILE_DETECT_RANGE: f32 = 900.0;
pub const DEFAULT_ENCOUNTER_SEED: u64 = 0;
pub const TRIBUTE_CREDITS: u32 = 50;
pub const OFFER_QTY: u32 = 3;
pub const METEORITE_HIT_COOLDOWN_SECS: f32 = 0.6;
pub const STAR_HEAT_NOTICE_COOLDOWN_SECS: f32 = 12.0;
pub const NPC_OWNER_BASE: u64 = 10_000;
pub const MERCHANT_LANE_Y: f32 = 220.0;

#[derive(Component, Debug, Clone)]
pub enum NpcBrain {
    Merchant {
        commodity: String,
        qty: u32,
        target_system: String,
        target_planet: String,
        target_pos: Vec2,
        dwell_timer: f32,
        route_step: u32,
    },
    Defender {
        patrol_center: Vec2,
        waypoint_angle: f32,
    },
    Explorer {
        current_system: String,
        target_system: String,
        target_pos: Vec2,
        step: u32,
        visited: Vec<String>,
    },
}

#[derive(Component, Debug, Clone)]
pub struct MeteoriteMotion {
    pub velocity: Vec2,
    pub center: Vec2,
    pub field_radius: f32,
    pub hit_cooldown: f32,
}

#[derive(Component, Debug, Clone)]
pub struct HeatWarningCooldown(f32);

#[derive(Component)]
struct BlackHoleGravity(f32);

#[derive(Component)]
struct StationOrbit {
    center: Vec2,
    radius: f32,
    period_secs: f32,
    phase: f32,
    started_at: f64,
}

#[derive(Component)]
struct DockingOffset(Vec2);

impl Default for HeatWarningCooldown {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Resource, Debug, Clone)]
pub struct EncounterRng(pub u64);

impl Default for EncounterRng {
    fn default() -> Self {
        Self(DEFAULT_ENCOUNTER_SEED)
    }
}

impl EncounterRng {
    pub fn with_roll(first_roll: u32) -> Self {
        Self(u64::from(first_roll) << 32)
    }

    pub fn next_u32(&mut self, bound: u32) -> u32 {
        let sample = self.0 >> 32;
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        if bound == 0 {
            0
        } else {
            (sample % u64::from(bound)) as u32
        }
    }
}

#[derive(Component, Debug, Clone)]
enum PendingEncounter {
    Offer {
        chief_name: String,
        commodity: String,
        qty: u32,
        cost: u32,
    },
    Tribute {
        chief_name: String,
        amount: u32,
    },
    Quest {
        chief_name: String,
        quest_id: String,
    },
    TurnIn {
        quest_id: String,
    },
    ShipHail {
        ship_name: String,
        fuel_gift: f32,
    },
}

#[derive(Component, Default)]
pub struct LastDamagedBy(pub Option<u64>);

pub fn parse_active_quests(json: &str) -> ActiveQuests {
    if let Ok(parsed) = serde_json::from_str::<ActiveQuests>(json) {
        parsed
    } else if let Ok(quests) = serde_json::from_str::<Vec<ActiveQuest>>(json) {
        ActiveQuests {
            quests,
            completed: Vec::new(),
        }
    } else {
        ActiveQuests::default()
    }
}

pub fn encode_active_quests(quests: &ActiveQuests) -> String {
    serde_json::to_string(quests).unwrap_or_else(|_| "[]".to_owned())
}

/// Benchmark harness fixture. Not a network intent or MCP tool.
pub fn set_scenario_credits(world: &mut World, owner: u64, credits: u32) {
    let mut query = world.query::<(&Owner, &mut Wallet, &mut AccountState)>();
    for (ship_owner, mut wallet, mut account) in query.iter_mut(world) {
        if ship_owner.0 == owner {
            wallet.0.credits = credits;
            account.credits = credits;
        }
    }
}

#[derive(Component)]
pub struct Ship(pub ShipState);

#[derive(Component)]
struct ShipSpec {
    flight: FlightModel,
    cargo_capacity: u32,
}

struct EquippedWeapon {
    def: WeaponDef,
    state: WeaponState,
}

#[derive(Component, Default)]
struct WeaponBanks(Vec<EquippedWeapon>);

#[derive(Component, Default)]
struct Control {
    throttle: f32,
    turn: f32,
    firing: bool,
    secondary_firing: bool,
    afterburner: bool,
    brake: bool,
    warp: bool,
}

#[derive(Component)]
pub struct Owner(pub u64);

#[derive(Component)]
pub struct Npc;

#[derive(Component)]
pub struct PlayerShip;

#[derive(Component)]
pub struct ClientRef(pub Entity);

#[derive(Component)]
struct Wallet(pub Account);

#[derive(Component)]
pub struct PlayerName(pub String);

#[derive(Component)]
struct ProjectileMotion {
    speed: f32,
    shooter_entity: Option<Entity>,
    shooter_kind: ShipKind,
    homing_target: Option<Entity>,
    turn_rate: f32,
}

#[derive(Component)]
pub struct Aggro(pub u64);

struct Recovery {
    state: crate::storage::PlayerState,
    name: String,
    remaining: f32,
    needs_save: bool,
}

#[derive(Resource, Default)]
struct PendingRecoveries(std::collections::HashMap<u64, Recovery>);

#[derive(Resource)]
struct SaveTimer(f32);

impl Default for SaveTimer {
    fn default() -> Self {
        Self(0.0)
    }
}

#[derive(Resource, Default)]
struct PirateRespawn(Option<f32>);

#[derive(Resource, Default)]
pub struct StorageResource {
    pub storage: Option<Box<dyn crate::storage::Storage>>,
}

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StorageResource>()
            .init_resource::<SaveTimer>()
            .init_resource::<PendingRecoveries>()
            .init_resource::<PirateRespawn>()
            .init_resource::<EncounterRng>()
            .init_resource::<market::MarketState>()
            .init_resource::<contracts::ContractBoard>()
            .add_observer(on_set_throttle)
            .add_observer(on_set_turn)
            .add_observer(on_set_firing)
            .add_observer(on_set_secondary_firing)
            .add_observer(on_set_afterburner)
            .add_observer(on_set_brake)
            .add_observer(on_cycle_target)
            .add_observer(on_request_land)
            .add_observer(on_request_dock)
            .add_observer(on_request_lift_off)
            .add_observer(on_buy_ship)
            .add_observer(on_buy_outfit)
            .add_observer(on_sell_outfit)
            .add_observer(on_buy_commodity)
            .add_observer(on_sell_commodity)
            .add_observer(on_set_warp)
            .add_observer(on_hail_ship)
            .add_observer(on_respond_to_encounter)
            .add_observer(on_login_request)
            .add_systems(OnEnter(ServerState::Running), spawn_world)
            .add_systems(
                Update,
                (
                    authorize_clients,
                    (
                        station_motion,
                        sync_docked_ships,
                        market::economy_tick,
                        market::flush_market_news,
                        factions::flush_faction_news,
                        contracts::contract_refresh,
                        factions::dispatch_tick,
                    )
                        .chain(),
                    apply_ship_control,
                    black_hole_gravity,
                    meteorite_movement,
                    update_system_membership,
                    factions::patrol_hostility,
                    npc_ai,
                    fire_weapons,
                    projectile_flight,
                    projectile_hits,
                    meteorite_ship_collisions,
                    star_heat,
                    shield_regen,
                    quest_progress,
                    (
                        factions::faction_kill_effects,
                        ship_deaths,
                        respawn_players,
                        respawn_pirates,
                        sync_ship_state,
                        sync_accounts,
                        factions::sync_system_control,
                        auto_save,
                    )
                        .chain(),
                )
                    .chain(),
            );
    }
}

static NEXT_TARGET_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn alloc_target_id() -> u64 {
    NEXT_TARGET_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub fn spawn_ship(
    commands: &mut Commands,
    pack: &ContentPack,
    ship_id: &str,
    kind: ShipKind,
    owner: Option<u64>,
    system: &str,
    offset: Vec2,
) -> Entity {
    let def = pack.ship(ship_id).expect("ship must exist in pack");
    let sys = pack.system(system).expect("system must exist in pack");
    let base = Vec2::new(sys.position.0, sys.position.1);
    let stats = pack
        .loadout_stats(ship_id, &def.default_outfits)
        .expect("validated default loadout");
    let mut ship_state = ShipState::new(stats.max_shields, stats.max_armor, stats.max_fuel)
        .with_energy(stats.max_energy, stats.energy_regen);
    ship_state.position = base + offset;

    let mut entity = commands.spawn((
        (
            Replicated,
            Ship(ship_state),
            EquippedOutfits {
                ids: def.default_outfits.clone(),
            },
            weapon_banks(pack, &def.default_outfits),
            ShipSpec {
                flight: FlightModel {
                    turn_rate: def.turn_rate,
                    acceleration: def.acceleration,
                    max_speed: def.max_speed,
                    drag: DRAG,
                },
                cargo_capacity: def.cargo,
            },
            Control::default(),
            LastDamagedBy::default(),
            HeatWarningCooldown::default(),
            Owner(owner.unwrap_or(0)),
            Position(base + offset),
            Velocity(Vec2::ZERO),
            Heading(0.0),
        ),
        (
            TargetId(alloc_target_id()),
            TargetLock::default(),
            Hull {
                shields: stats.max_shields,
                max_shields: stats.max_shields,
                armor: stats.max_armor,
                max_armor: stats.max_armor,
            },
            Fuel {
                current: stats.max_fuel,
                max: stats.max_fuel,
            },
            Energy {
                current: stats.max_energy,
                max: stats.max_energy,
            },
            ShipInfo {
                def_id: def.id.clone(),
                kind,
                owner: if kind == ShipKind::Player {
                    owner
                } else {
                    None
                },
                name: def.name.clone(),
            },
            SystemMembership(system.to_owned()),
        ),
    ));

    if kind == ShipKind::Player {
        entity.insert(PlayerShip);
    } else {
        entity.insert(Npc);
    }
    entity.id()
}

pub fn planet_trade_waypoint(pack: &ContentPack, system_id: &str) -> (String, Vec2) {
    let Some(sys) = pack.system(system_id) else {
        return ("unknown".to_owned(), Vec2::ZERO);
    };
    let planet_id = sys
        .planets
        .first()
        .map_or_else(|| "station".to_owned(), |p| p.id.clone());
    let pos = Vec2::new(
        sys.position.0 + PLANET_OFFSET_X,
        sys.position.1 + MERCHANT_LANE_Y,
    );
    (planet_id, pos)
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_npc_role(
    commands: &mut Commands,
    pack: &ContentPack,
    ship_id: &str,
    kind: ShipKind,
    npc_owner: u64,
    system: &str,
    offset: Vec2,
    brain: NpcBrain,
) -> Entity {
    let entity = spawn_ship(
        commands,
        pack,
        ship_id,
        kind,
        Some(npc_owner),
        system,
        offset,
    );
    commands.entity(entity).insert(brain);
    entity
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_meteorite(
    commands: &mut Commands,
    system: &str,
    center: Vec2,
    field_radius: f32,
    position: Vec2,
    velocity: Vec2,
    size: MeteoriteSize,
    spin_rate: f32,
    variant: u8,
) -> Entity {
    let effective_spin = if velocity.length_squared() > 0.0 {
        if spin_rate == 0.0 { 1.2 } else { spin_rate }
    } else {
        0.0
    };
    let max_hp = size.max_hp();
    commands
        .spawn((
            Replicated,
            Meteorite {
                size,
                hp: max_hp,
                max_hp,
                spin_rate: effective_spin,
                variant,
            },
            MeteoriteMotion {
                velocity,
                center,
                field_radius,
                hit_cooldown: 0.0,
            },
            Position(position),
            Heading(0.0),
            SystemMembership(system.to_owned()),
        ))
        .id()
}

fn spawn_world(
    mut commands: Commands,
    content: Res<Content>,
    time: Res<Time>,
    storage: Res<StorageResource>,
) {
    let pack = &content.0;
    let market_state = market::init_market_state(&storage, pack);
    let faction_state = factions::init_faction_state(&storage, pack);
    for system in &pack.systems {
        commands.spawn((
            Replicated,
            SystemControl {
                system: system.id.clone(),
                controller: faction_state
                    .controllers
                    .get(&system.id)
                    .cloned()
                    .unwrap_or_else(|| system.government.clone()),
                meter: faction_state.meters.get(&system.id).copied().unwrap_or(0),
            },
        ));
        let sys_center = Vec2::new(system.position.0, system.position.1);
        for (index, planet) in system.planets.iter().enumerate() {
            commands.spawn((
                Replicated,
                PlanetTag {
                    system: system.id.clone(),
                    id: planet.id.clone(),
                    name: planet.name.clone(),
                    radius: PLANET_RADIUS,
                },
                DockableTag {
                    system: system.id.clone(),
                    id: planet.id.clone(),
                    name: planet.name.clone(),
                    radius: PLANET_RADIUS,
                    kind: DockableKind::Planet,
                },
                PortMarket {
                    prices: market::port_prices(&market_state, pack, &system.id, &planet.id),
                },
                Position(Vec2::new(
                    system.position.0 + PLANET_OFFSET_X + index as f32 * sv_data::PLANET_SPACING,
                    system.position.1,
                )),
            ));
        }

        for station in &system.stations {
            let (position, orbit) = match &station.placement {
                StationPlacement::Fixed { position } => (sys_center + Vec2::from(*position), None),
                StationPlacement::Orbit {
                    planet,
                    radius,
                    period_secs,
                    phase,
                } => {
                    let center = Vec2::from(
                        system
                            .planet_position(planet)
                            .expect("validated orbital parent"),
                    );
                    (
                        sv_core::orbit_position(center, *radius, *period_secs, *phase, 0.0),
                        Some(StationOrbit {
                            center,
                            radius: *radius,
                            period_secs: *period_secs,
                            phase: *phase,
                            started_at: time.elapsed_secs_f64(),
                        }),
                    )
                }
            };
            let mut entity = commands.spawn((
                Replicated,
                DockableTag {
                    system: system.id.clone(),
                    id: station.id.clone(),
                    name: station.name.clone(),
                    radius: station.radius,
                    kind: DockableKind::Station,
                },
                PortMarket {
                    prices: market::port_prices(&market_state, pack, &system.id, &station.id),
                },
                Position(position),
            ));
            if let Some(orbit) = orbit {
                entity.insert(orbit);
            }
        }

        if let Some(star) = &system.star {
            commands.spawn((
                Replicated,
                StarTag {
                    system: system.id.clone(),
                    radius: star.radius,
                },
                Position(sys_center + Vec2::new(star.position.0, star.position.1)),
            ));
        }
        if let Some(hole) = &system.black_hole {
            commands.spawn((
                Replicated,
                BlackHoleTag {
                    system: system.id.clone(),
                    radius: hole.radius,
                    influence: hole.influence,
                },
                BlackHoleGravity(hole.pull_strength),
                Position(sys_center + Vec2::from(hole.position)),
            ));
        }

        if let Some(field) = &system.meteorites {
            let total_weight =
                (field.stationary_weight + field.slow_weight + field.fast_weight).max(1);
            for i in 0..field.count {
                let size = match i % 3 {
                    0 => MeteoriteSize::Large,
                    1 => MeteoriteSize::Medium,
                    _ => MeteoriteSize::Small,
                };
                // Place meteorites in upper and lower belts (|y| >= 240) so the equatorial
                // planet/jump approach lane starts clear of immediate spawn-point collisions.
                let band_sign = if i % 2 == 0 { 1.0_f32 } else { -1.0_f32 };
                let frac = (i as f32 + 0.5) / (field.count.max(1) as f32);
                let offset_x = (frac - 0.5) * field.radius * 1.3;
                let offset_y = band_sign * (260.0 + ((i * 137) % 380) as f32);
                let pos = sys_center + Vec2::new(offset_x, offset_y);

                let bucket = (i * 7 + 3) % total_weight;
                let dir_angle = ((i as f32) * 1.15) - 0.5;
                let dir = Vec2::new(dir_angle.cos(), dir_angle.sin() * 0.35).normalize_or(Vec2::X);
                let spin_sign = if i % 2 == 0 { 1.0 } else { -1.0 };

                let (velocity, spin_rate) = if bucket < field.stationary_weight {
                    (Vec2::ZERO, 0.0)
                } else if bucket < field.stationary_weight + field.slow_weight {
                    let speed = 22.0 + ((i * 11) % 18) as f32;
                    (dir * speed, spin_sign * (0.8 + (i % 3) as f32 * 0.25))
                } else {
                    let speed = 82.0 + ((i * 17) % 40) as f32;
                    (dir * speed, spin_sign * (2.0 + (i % 3) as f32 * 0.45))
                };

                spawn_meteorite(
                    &mut commands,
                    &system.id,
                    sys_center,
                    field.radius,
                    pos,
                    velocity,
                    size,
                    spin_rate,
                    (i % 4) as u8,
                );
            }
        }
    }
    spawn_ship(
        &mut commands,
        pack,
        PIRATE_SHIP,
        ShipKind::Pirate,
        None,
        PIRATE_SYSTEM,
        Vec2::new(0.0, -400.0),
    );
    spawn_living_galaxy_npcs(&mut commands, pack);
    commands.insert_resource(market_state);
    commands.insert_resource(faction_state);
}

fn station_motion(time: Res<Time>, mut stations: Query<(&StationOrbit, &mut Position)>) {
    for (orbit, mut position) in &mut stations {
        position.0 = sv_core::orbit_position(
            orbit.center,
            orbit.radius,
            orbit.period_secs,
            orbit.phase,
            time.elapsed_secs_f64() - orbit.started_at,
        );
    }
}

fn sync_docked_ships(
    mut ships: Query<(
        &mut Ship,
        &DockedAt,
        Option<&DockingOffset>,
        &mut SystemMembership,
    )>,
    ports: Query<(&DockableTag, &Position)>,
) {
    for (mut ship, docked, offset, mut membership) in &mut ships {
        membership.0.clone_from(&docked.system);
        ship.0.velocity = Vec2::ZERO;
        if let Some(offset) = offset
            && let Some((_, position)) = ports
                .iter()
                .find(|(port, _)| port.system == docked.system && port.id == docked.location)
        {
            ship.0.position = position.0 + offset.0;
        }
    }
}

fn spawn_living_galaxy_npcs(commands: &mut Commands, pack: &ContentPack) {
    let mut next_owner = NPC_OWNER_BASE;
    let mut alloc_owner = || {
        let id = next_owner;
        next_owner += 1;
        id
    };

    // 1. Merchants hauling stock between star systems across 5 races & size classes:
    let merchant_roster = [
        (
            "vellari-clipper",
            "sol",
            Vec2::new(140.0, MERCHANT_LANE_Y),
            "alpha-centauri",
            "food",
            8,
        ),
        (
            "terran-hauler",
            "alpha-centauri",
            Vec2::new(180.0, MERCHANT_LANE_Y),
            "sol",
            "machinery",
            12,
        ),
        (
            "terran-bulk-freighter",
            "vega",
            Vec2::new(220.0, MERCHANT_LANE_Y),
            "sirius",
            "medicine",
            24,
        ),
        (
            "vellari-ark",
            "sirius",
            Vec2::new(160.0, MERCHANT_LANE_Y),
            "procyon",
            "food",
            18,
        ),
        (
            "korath-barge",
            "barnard",
            Vec2::new(200.0, MERCHANT_LANE_Y),
            "wolf-359",
            "machinery",
            15,
        ),
        (
            "sylph-cantor",
            "new-kashmir",
            Vec2::new(180.0, MERCHANT_LANE_Y),
            "polaris",
            "medicine",
            14,
        ),
        (
            "terran-courier",
            "lalande",
            Vec2::new(150.0, MERCHANT_LANE_Y),
            "sol",
            "food",
            10,
        ),
        (
            "vellari-sprinter",
            "cygnus-x1",
            Vec2::new(130.0, MERCHANT_LANE_Y),
            "tarsis",
            "ore",
            6,
        ),
        (
            "sylph-dreamweaver",
            "hawking",
            Vec2::new(170.0, MERCHANT_LANE_Y),
            "new-kashmir",
            "isotopes",
            8,
        ),
    ];

    for (idx, (ship_id, start_sys, offset, dest_sys, commodity, qty)) in
        merchant_roster.into_iter().enumerate()
    {
        if pack.ship(ship_id).is_none()
            || pack.system(start_sys).is_none()
            || pack.system(dest_sys).is_none()
        {
            continue;
        }
        let (target_planet, target_pos) = planet_trade_waypoint(pack, dest_sys);
        spawn_npc_role(
            commands,
            pack,
            ship_id,
            ShipKind::Merchant,
            alloc_owner(),
            start_sys,
            offset,
            NpcBrain::Merchant {
                commodity: commodity.to_owned(),
                qty,
                target_system: dest_sys.to_owned(),
                target_planet,
                target_pos,
                dwell_timer: 1.5,
                route_step: idx as u32,
            },
        );
    }

    // 2. Defenders patrolling systems and hunting pirates:
    let defender_roster = [
        ("terran-gunboat", "sol", Vec2::new(-160.0, 230.0), 0.0_f32),
        ("korath-ironclad", "vega", Vec2::new(-180.0, 240.0), 1.2_f32),
        (
            "korath-dreadnought",
            "wolf-359",
            Vec2::new(-200.0, 250.0),
            2.4_f32,
        ),
        (
            "auroran-corsair",
            "tarsis",
            Vec2::new(-150.0, 230.0),
            0.8_f32,
        ),
        (
            "auroran-galleon",
            "polaris",
            Vec2::new(-210.0, 260.0),
            1.9_f32,
        ),
        (
            "terran-frigate",
            "lalande",
            Vec2::new(-170.0, 240.0),
            0.5_f32,
        ),
        (
            "auroran-titan",
            "cygnus-x1",
            Vec2::new(-200.0, 260.0),
            2.1_f32,
        ),
    ];

    for (ship_id, sys_id, offset, angle) in defender_roster {
        let Some(sys) = pack.system(sys_id) else {
            continue;
        };
        if pack.ship(ship_id).is_none() {
            continue;
        }
        let patrol_center = Vec2::new(sys.position.0, sys.position.1 + 230.0);
        spawn_npc_role(
            commands,
            pack,
            ship_id,
            ShipKind::Defender,
            alloc_owner(),
            sys_id,
            offset,
            NpcBrain::Defender {
                patrol_center,
                waypoint_angle: angle,
            },
        );
    }

    // 3. Explorers traveling from system to system discovering the galaxy:
    let explorer_roster = [
        ("sylph-wanderer", "sol", Vec2::new(90.0, -220.0), "vega"),
        (
            "terran-surveyor",
            "procyon",
            Vec2::new(110.0, -220.0),
            "sirius",
        ),
        (
            "vellari-seeker",
            "ross-154",
            Vec2::new(100.0, -220.0),
            "barnard",
        ),
        (
            "sylph-skiff",
            "trappist",
            Vec2::new(105.0, -215.0),
            "procyon",
        ),
    ];

    for (idx, (ship_id, start_sys, offset, target_sys)) in explorer_roster.into_iter().enumerate() {
        let Some(dest) = pack.system(target_sys) else {
            continue;
        };
        if pack.ship(ship_id).is_none() || pack.system(start_sys).is_none() {
            continue;
        }
        let target_pos = Vec2::new(dest.position.0 + 120.0, dest.position.1 - 220.0);
        spawn_npc_role(
            commands,
            pack,
            ship_id,
            ShipKind::Explorer,
            alloc_owner(),
            start_sys,
            offset,
            NpcBrain::Explorer {
                current_system: start_sys.to_owned(),
                target_system: target_sys.to_owned(),
                target_pos,
                step: idx as u32,
                visited: vec![start_sys.to_owned()],
            },
        );
    }
}

fn raw_id(client_id: ClientId, clients: &Query<&NetworkId>) -> u64 {
    match client_id.entity() {
        Some(entity) => clients.get(entity).map(|id| id.get()).unwrap_or(0),
        None => 0,
    }
}

fn notice(commands: &mut Commands, client: ClientId, text: impl Into<String>) {
    let text = text.into();
    info!("server: notice to {client:?}: {text}");
    commands.server_trigger(ToClients {
        targets: SendTargets::Single(client),
        message: Notice { text },
    });
}

#[allow(clippy::too_many_arguments)]
fn on_login_request(
    trigger: On<FromClient<LoginRequest>>,
    mut commands: Commands,
    content: Res<Content>,
    clients: Query<&NetworkId>,
    storage: Res<StorageResource>,
    logged_in: Query<&Owner, With<PlayerShip>>,
    active_names: Query<&PlayerName>,
    mut pending: ResMut<PendingRecoveries>,
) {
    let username = trigger.event().username.clone();
    let owner = raw_id(trigger.client_id, &clients);
    let client = trigger.client_id;
    if logged_in.iter().any(|o| o.0 == owner) {
        return;
    }
    let result = (|| -> Result<crate::PlayerState, String> {
        if active_names.iter().any(|name| name.0 == username) {
            return Err("Already logged in.".into());
        }
        let db = storage
            .storage
            .as_ref()
            .ok_or("Persistent accounts are unavailable.")?;
        let mut saved = match db.verify_login(&username, &trigger.event().password) {
            Ok(Some(state)) => state,
            Ok(None) => db
                .create_account(&username, &trigger.event().password)
                .map_err(|_| "Account creation failed.")?,
            Err(_) => return Err("Login failed.".into()),
        };
        if let Some(recovery) = pending.0.values().find(|r| r.name == username) {
            saved = recovery.state.clone();
            db.save_player(&username, &saved)
                .map_err(|_| "Recovery could not be saved; retry login.")?;
        }
        restore_ship(&content.0, &saved)?;
        Ok(saved)
    })();
    match result {
        Err(message) => commands.server_trigger(ToClients {
            targets: SendTargets::Single(client),
            message: LoginResponse {
                success: false,
                message,
                credits: 0,
                cargo: vec![],
                system: String::new(),
            },
        }),
        Ok(saved) => {
            let Some(entity) = client.entity() else {
                return;
            };
            if let Err(error) = spawn_saved_player(
                &mut commands,
                &content.0,
                owner,
                entity,
                username.clone(),
                &saved,
            ) {
                notice(&mut commands, client, error);
                return;
            }
            pending.0.retain(|_, recovery| recovery.name != username);
            commands.server_trigger(ToClients {
                targets: SendTargets::Single(client),
                message: LoginResponse {
                    success: true,
                    message: "Welcome aboard.".into(),
                    credits: saved.credits,
                    cargo: saved.cargo,
                    system: saved.system,
                },
            });
        }
    }
}

fn authorize_clients(
    mut commands: Commands,
    content: Res<Content>,
    newly_authorized: Query<(Entity, &NetworkId), Added<AuthorizedClient>>,
    players: Query<(Entity, &Owner), With<PlayerShip>>,
    pending: Res<PendingRecoveries>,
    storage: Res<StorageResource>,
) {
    for (client_entity, network_id) in &newly_authorized {
        if storage.storage.is_some() {
            continue;
        }
        let owner = network_id.get();
        if let Some((player_entity, _)) = players.iter().find(|(_, existing)| existing.0 == owner) {
            info!(
                "server: re-bound authorized client owner={owner} to existing ship {player_entity:?}"
            );
            commands
                .entity(player_entity)
                .insert(ClientRef(client_entity));
        } else if !pending.0.contains_key(&owner) {
            info!("server: spawning default player ship for authorized client owner={owner}");
            spawn_default_player(&mut commands, &content, owner, client_entity);
        }
    }
}

fn spawn_default_player(
    commands: &mut Commands,
    content: &Res<Content>,
    owner: u64,
    client_entity: Entity,
) {
    let capacity = content
        .0
        .ship(PLAYER_SHIP)
        .expect("player ship must exist in pack")
        .cargo;
    let account = Account::new(STARTING_CREDITS);
    let ship = spawn_ship(
        commands,
        &content.0,
        PLAYER_SHIP,
        ShipKind::Player,
        Some(owner),
        PLAYER_SYSTEM,
        Vec2::ZERO,
    );
    commands.entity(ship).insert((
        Wallet(account.clone()),
        AccountState {
            credits: account.credits,
            cargo: account.cargo.clone(),
            cargo_capacity: capacity,
        },
        ActiveQuests::default(),
        PlayerReputation::default(),
        ClientRef(client_entity),
        PlayerName("pilot".to_owned()),
    ));
}

fn sanitize_input(value: f32, min: f32, max: f32) -> Option<f32> {
    if value.is_finite() {
        Some(value.clamp(min, max))
    } else {
        None
    }
}

fn on_set_throttle(
    trigger: On<FromClient<SetThrottle>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let Some(throttle) = sanitize_input(trigger.event().0, 0.0, 1.0) else {
        return;
    };
    debug!("server: owner={owner} set throttle={throttle:.2}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.throttle = throttle;
        }
    }
}

fn on_set_turn(
    trigger: On<FromClient<SetTurn>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let Some(turn) = sanitize_input(trigger.event().0, -1.0, 1.0) else {
        return;
    };
    debug!("server: owner={owner} set turn={turn:.2}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.turn = turn;
        }
    }
}

fn on_set_firing(
    trigger: On<FromClient<SetFiring>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let firing = trigger.event().0;
    debug!("server: owner={owner} set firing={firing}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.firing = firing;
        }
    }
}

fn on_set_secondary_firing(
    trigger: On<FromClient<SetSecondaryFiring>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let firing = trigger.event().0;
    debug!("server: owner={owner} set secondary_firing={firing}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.secondary_firing = firing;
        }
    }
}

fn on_set_afterburner(
    trigger: On<FromClient<SetAfterburner>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let afterburner = trigger.event().0;
    debug!("server: owner={owner} set afterburner={afterburner}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.afterburner = afterburner;
        }
    }
}

fn on_set_brake(
    trigger: On<FromClient<SetBrake>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let brake = trigger.event().0;
    debug!("server: owner={owner} set brake={brake}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.brake = brake;
        }
    }
}

#[allow(clippy::type_complexity)]
fn on_cycle_target(
    trigger: On<FromClient<CycleTarget>>,
    clients: Query<&NetworkId>,
    mut players: Query<
        (
            Entity,
            &Owner,
            &Position,
            &SystemMembership,
            &mut TargetLock,
        ),
        With<PlayerShip>,
    >,
    candidates: Query<
        (
            Entity,
            &TargetId,
            &Position,
            &ShipInfo,
            &SystemMembership,
            Option<&Aggro>,
        ),
        (With<Ship>, Without<DockedAt>),
    >,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let hostile_only = trigger.event().hostile_only;
    let Some((my_entity, _, my_pos, my_sys, mut lock)) = players
        .iter_mut()
        .find(|(_, ship_owner, ..)| ship_owner.0 == owner)
    else {
        return;
    };
    let mut matching: Vec<(u32, u64)> = candidates
        .iter()
        .filter(|(entity, _, _, info, sys, aggro)| {
            if *entity == my_entity || sys.0 != my_sys.0 {
                return false;
            }
            if hostile_only {
                info.kind == ShipKind::Pirate || aggro.is_some_and(|a| a.0 == owner)
            } else {
                true
            }
        })
        .map(|(_, tid, pos, ..)| (my_pos.0.distance(pos.0).to_bits(), tid.0))
        .collect();
    matching.sort_unstable();
    if matching.is_empty() {
        lock.0 = None;
        return;
    }
    let next = match lock.0 {
        Some(current) => matching
            .iter()
            .position(|(_, tid)| *tid == current)
            .map_or(matching[0].1, |idx| matching[(idx + 1) % matching.len()].1),
        None => matching[0].1,
    };
    lock.0 = Some(next);
}

fn apply_ship_control(
    time: Res<Time>,
    mut ships: Query<(&Control, &ShipSpec, &mut Ship), Without<DockedAt>>,
) {
    let dt = time.delta_secs();
    for (control, spec, mut ship) in &mut ships {
        let mut model = spec.flight;
        let warping = control.warp && ship.0.fuel > 0.0;
        let mut afterburning = false;
        if warping {
            model.max_speed *= WARP_SPEED_MULTIPLIER;
            model.acceleration *= WARP_SPEED_MULTIPLIER;
            ship.0.fuel = (ship.0.fuel - WARP_FUEL_RATE * dt).max(0.0);
        } else if control.afterburner && ship.0.drain_energy(AFTERBURNER_ENERGY_RATE, dt) {
            model.max_speed *= AFTERBURNER_SPEED_MULTIPLIER;
            model.acceleration *= AFTERBURNER_SPEED_MULTIPLIER;
            afterburning = true;
        }
        let turn = if control.turn == 0.0 && control.brake && !warping {
            sv_core::retrograde_turn(ship.0.rotation, ship.0.velocity)
        } else {
            control.turn
        };
        let throttle = if warping || (afterburning && control.throttle == 0.0) {
            1.0
        } else {
            control.throttle
        };
        ship.0.apply_control(turn, throttle, &model, dt);
    }
}

fn steer_toward(position: Vec2, heading: f32, target: Vec2) -> (f32, f32, f32) {
    let delta = target - position;
    let distance = delta.length();
    let desired = delta.y.atan2(delta.x);
    let diff = (desired - heading + PI).rem_euclid(2.0 * PI) - PI;
    let turn = if diff.abs() < 0.04 {
        0.0
    } else {
        diff.signum()
    };
    (distance, diff, turn)
}

type NpcShips<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static Heading,
        &'static ShipInfo,
        &'static mut SystemMembership,
        &'static mut Control,
        Option<&'static Aggro>,
        Option<&'static mut NpcBrain>,
    ),
    With<Npc>,
>;

fn npc_ai(
    mut commands: Commands,
    time: Res<Time>,
    content: Res<Content>,
    mut market: ResMut<market::MarketState>,
    mut npcs: NpcShips,
    players: Query<(Entity, &Position, &Owner, Option<&DockedAt>), With<PlayerShip>>,
) {
    let dt = time.delta_secs();
    let pack = &content.0;

    let mut pirate_positions: Vec<(Entity, Vec2, String)> = Vec::new();
    let mut defender_positions: Vec<(Entity, Vec2, String)> = Vec::new();
    for (entity, pos, _, info, membership, _, _, _) in &npcs {
        if info.kind == ShipKind::Pirate {
            pirate_positions.push((entity, pos.0, membership.0.clone()));
        } else if info.kind == ShipKind::Defender {
            defender_positions.push((entity, pos.0, membership.0.clone()));
        }
    }

    for (entity, position, heading, info, mut membership, mut control, aggro, brain) in &mut npcs {
        control.throttle = 0.0;
        control.turn = 0.0;
        control.firing = false;
        control.warp = false;

        // 1. Retaliation / Aggro against an attacker (universal across all NPC roles):
        if let Some(Aggro(wanted_owner)) = aggro {
            let Some((_, target_pos, _, docked)) = players
                .iter()
                .find(|(_, _, target_owner, _)| target_owner.0 == *wanted_owner)
            else {
                if info.kind == ShipKind::Police {
                    commands.entity(entity).despawn();
                    continue;
                }
                commands.entity(entity).remove::<Aggro>();
                continue;
            };
            if docked.is_some() {
                if matches!(
                    info.kind,
                    ShipKind::Merchant | ShipKind::Defender | ShipKind::Explorer
                ) {
                    commands.entity(entity).remove::<Aggro>();
                } else {
                    continue;
                }
            } else {
                let (distance, diff, turn) = steer_toward(position.0, heading.0, target_pos.0);
                if distance <= PIRATE_SIGHT_RANGE {
                    control.turn = turn;
                    control.throttle = if distance > 200.0 { 1.0 } else { 0.3 };
                    control.firing = distance < PIRATE_FIRE_RANGE && diff.abs() < 0.35;
                    continue;
                } else if info.kind != ShipKind::Police && info.kind != ShipKind::Pirate {
                    commands.entity(entity).remove::<Aggro>();
                } else {
                    continue;
                }
            }
        }

        // 2. Pirate behavior: hunt undocked players or return fire at nearby defenders:
        if info.kind == ShipKind::Pirate {
            let nearest_player = players
                .iter()
                .filter(|(_, _, _, docked)| docked.is_none())
                .map(|(_, pos, _, _)| pos.0)
                .min_by_key(|pos| position.0.distance(*pos).to_bits());
            let nearest_defender = defender_positions
                .iter()
                .filter(|(_, pos, sys)| {
                    *sys == membership.0 || position.0.distance(*pos) <= PIRATE_SIGHT_RANGE
                })
                .map(|(_, pos, _)| *pos)
                .min_by_key(|pos| position.0.distance(*pos).to_bits());

            let target = match (nearest_player, nearest_defender) {
                (Some(p), Some(d)) => {
                    if position.0.distance(p) <= position.0.distance(d) {
                        Some(p)
                    } else {
                        Some(d)
                    }
                }
                (Some(p), None) => Some(p),
                (None, Some(d)) => Some(d),
                (None, None) => None,
            };
            let Some(target_pos) = target else {
                continue;
            };
            let (distance, diff, turn) = steer_toward(position.0, heading.0, target_pos);
            if distance > PIRATE_SIGHT_RANGE {
                continue;
            }
            control.turn = turn;
            control.throttle = if distance > 200.0 { 1.0 } else { 0.3 };
            control.firing = distance < PIRATE_FIRE_RANGE && diff.abs() < 0.35;
            continue;
        }

        // 3. Autonomous role brains (Merchant, Defender, Explorer):
        let Some(mut brain) = brain else {
            continue;
        };
        match &mut *brain {
            NpcBrain::Defender {
                patrol_center,
                waypoint_angle,
            } => {
                let target_pirate = pirate_positions
                    .iter()
                    .filter(|(p_ent, p_pos, p_sys)| {
                        *p_ent != entity
                            && (*p_sys == membership.0
                                || position.0.distance(*p_pos) <= PIRATE_SIGHT_RANGE)
                    })
                    .min_by_key(|(_, p_pos, _)| position.0.distance(*p_pos).to_bits());

                if let Some((_, pirate_pos, _)) = target_pirate {
                    let (distance, diff, turn) = steer_toward(position.0, heading.0, *pirate_pos);
                    if distance <= PIRATE_SIGHT_RANGE {
                        control.turn = turn;
                        control.throttle = if distance > 180.0 { 1.0 } else { 0.35 };
                        control.firing = distance < PIRATE_FIRE_RANGE && diff.abs() < 0.35;
                        continue;
                    }
                }

                let waypoint = *patrol_center
                    + Vec2::new(waypoint_angle.cos() * 200.0, waypoint_angle.sin() * 80.0);
                let (distance, diff, turn) = steer_toward(position.0, heading.0, waypoint);
                if distance < 65.0 {
                    *waypoint_angle = (*waypoint_angle + 1.25).rem_euclid(2.0 * PI);
                }
                control.turn = turn;
                control.throttle = if diff.abs() < 0.7 { 0.55 } else { 0.2 };
            }
            NpcBrain::Merchant {
                commodity,
                qty,
                target_system,
                target_planet,
                target_pos,
                dwell_timer,
                route_step,
            } => {
                if let Some(sys) = pack.system(target_system) {
                    let sys_center = Vec2::new(sys.position.0, sys.position.1);
                    if position.0.distance(sys_center) < SYSTEM_MEMBERSHIP_RADIUS
                        && membership.0 != *target_system
                    {
                        membership.0 = target_system.clone();
                    }
                }

                let (distance, diff, turn) = steer_toward(position.0, heading.0, *target_pos);
                if distance <= 95.0 {
                    control.throttle = 0.0;
                    control.turn = 0.0;
                    *dwell_timer -= dt;
                    if *dwell_timer <= 0.0 {
                        *dwell_timer = 1.5;
                        *route_step = route_step.wrapping_add(1);
                        let arrived_system = target_system.clone();
                        membership.0 = arrived_system.clone();
                        market.shift_supply(
                            market::MarketKey {
                                system: arrived_system.clone(),
                                port: target_planet.clone(),
                                commodity: commodity.clone(),
                            },
                            *qty as i32,
                            *qty,
                        );

                        const COMMODITIES: [&str; 3] = ["food", "machinery", "medicine"];
                        let next_comm = pack
                            .system(&arrived_system)
                            .and_then(|s| s.planets.first())
                            .and_then(|p| {
                                if p.commodities.is_empty() {
                                    None
                                } else {
                                    Some(
                                        p.commodities[(*route_step as usize) % p.commodities.len()]
                                            .id
                                            .clone(),
                                    )
                                }
                            })
                            .unwrap_or_else(|| {
                                COMMODITIES[(*route_step as usize) % COMMODITIES.len()].to_owned()
                            });
                        *commodity = next_comm;
                        *qty = 6 + (*route_step % 4) * 3;

                        let next_sys = pack
                            .system(&arrived_system)
                            .and_then(|s| {
                                if s.links.is_empty() {
                                    None
                                } else {
                                    Some(s.links[(*route_step as usize) % s.links.len()].clone())
                                }
                            })
                            .unwrap_or_else(|| PLAYER_SYSTEM.to_owned());
                        let (next_planet, next_pos) = planet_trade_waypoint(pack, &next_sys);
                        *target_system = next_sys;
                        *target_planet = next_planet;
                        *target_pos = next_pos;
                    }
                } else {
                    control.turn = turn;
                    control.throttle = if diff.abs() < 0.65 { 0.85 } else { 0.25 };
                }
            }
            NpcBrain::Explorer {
                current_system,
                target_system,
                target_pos,
                step,
                visited,
            } => {
                if let Some(sys) = pack.system(target_system) {
                    let sys_center = Vec2::new(sys.position.0, sys.position.1);
                    if position.0.distance(sys_center) < SYSTEM_MEMBERSHIP_RADIUS
                        && membership.0 != *target_system
                    {
                        membership.0 = target_system.clone();
                    }
                }

                let (distance, diff, turn) = steer_toward(position.0, heading.0, *target_pos);
                if distance <= 110.0 {
                    let arrived = target_system.clone();
                    *current_system = arrived.clone();
                    membership.0 = arrived.clone();
                    if !visited.contains(&arrived) {
                        visited.push(arrived.clone());
                    }
                    *step = step.wrapping_add(1);
                    let next_sys = pack
                        .system(&arrived)
                        .and_then(|s| {
                            if s.links.is_empty() {
                                None
                            } else {
                                s.links
                                    .iter()
                                    .find(|link| !visited.contains(link))
                                    .cloned()
                                    .or_else(|| {
                                        Some(s.links[(*step as usize) % s.links.len()].clone())
                                    })
                            }
                        })
                        .unwrap_or_else(|| PLAYER_SYSTEM.to_owned());
                    if let Some(dest) = pack.system(&next_sys) {
                        *target_pos = Vec2::new(dest.position.0 + 120.0, dest.position.1 - 220.0);
                    }
                    *target_system = next_sys;
                } else {
                    control.turn = turn;
                    control.throttle = if diff.abs() < 0.65 { 0.9 } else { 0.3 };
                }
            }
        }
    }
}

type FiringShips<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static TargetId,
        &'static Control,
        &'static Owner,
        &'static ShipInfo,
        &'static SystemMembership,
        &'static TargetLock,
        Option<&'static Aggro>,
        &'static mut Ship,
        &'static mut WeaponBanks,
    ),
    Without<DockedAt>,
>;

struct TargetSnapshot {
    entity: Entity,
    target_id: u64,
    kind: ShipKind,
    system: String,
    position: Vec2,
    velocity: Vec2,
    aggro_owner: Option<u64>,
}

#[allow(clippy::too_many_arguments)]
fn select_weapon_target<'a>(
    shooter: Entity,
    shooter_owner: u64,
    shooter_kind: ShipKind,
    shooter_sys: &str,
    shooter_pos: Vec2,
    locked_id: Option<u64>,
    max_range: f32,
    targets: &'a [TargetSnapshot],
) -> Option<&'a TargetSnapshot> {
    if let Some(tid) = locked_id
        && let Some(found) = targets.iter().find(|t| {
            t.entity != shooter
                && t.target_id == tid
                && t.system == shooter_sys
                && shooter_pos.distance(t.position) <= max_range
        })
    {
        return Some(found);
    }
    targets
        .iter()
        .filter(|t| {
            if t.entity == shooter || t.system != shooter_sys {
                return false;
            }
            let dist = shooter_pos.distance(t.position);
            if dist > max_range {
                return false;
            }
            match shooter_kind {
                ShipKind::Player => {
                    t.kind == ShipKind::Pirate || t.aggro_owner == Some(shooter_owner)
                }
                ShipKind::Pirate => matches!(t.kind, ShipKind::Player | ShipKind::Defender),
                ShipKind::Defender => t.kind == ShipKind::Pirate,
                _ => t.aggro_owner.is_some(),
            }
        })
        .min_by_key(|t| shooter_pos.distance(t.position).to_bits())
}

fn fire_weapons(mut commands: Commands, time: Res<Time>, mut ships: FiringShips) {
    let dt = time.delta_secs();
    let targets: Vec<TargetSnapshot> = ships
        .iter()
        .map(
            |(entity, tid, _, _, info, sys, _, aggro, ship, _)| TargetSnapshot {
                entity,
                target_id: tid.0,
                kind: info.kind,
                system: sys.0.clone(),
                position: ship.0.position,
                velocity: ship.0.velocity,
                aggro_owner: aggro.map(|a| a.0),
            },
        )
        .collect();

    for (entity, _, control, owner, info, membership, lock, _, mut ship, mut banks) in &mut ships {
        let fixed_total = banks
            .0
            .iter()
            .filter(|w| matches!(w.def.mount, WeaponMount::Fixed))
            .count();
        let mut fixed_index = 0usize;

        for blaster in &mut banks.0 {
            blaster.state.tick(dt);
            let is_fixed = matches!(blaster.def.mount, WeaponMount::Fixed);
            let slot_idx = if is_fixed {
                let current = fixed_index;
                fixed_index += 1;
                current
            } else {
                0
            };

            let triggered = match blaster.def.mount {
                WeaponMount::Fixed | WeaponMount::Turret => control.firing,
                WeaponMount::Missile { .. } => {
                    control.secondary_firing || control.firing && info.kind != ShipKind::Player
                }
            };
            if !triggered || blaster.state.cooldown_remaining > 0.0 {
                continue;
            }
            let energy_cost = blaster.def.effective_energy_cost();
            if !ship.0.try_spend_energy(energy_cost) {
                continue;
            }
            let _ = blaster.state.try_fire(blaster.def.cooldown);

            let (angle, spawn_pos, kind, homing_target, turn_rate) = match blaster.def.mount {
                WeaponMount::Fixed => {
                    let aimed_angle = lock
                        .0
                        .and_then(|tid| {
                            targets.iter().find(|t| {
                                t.entity != entity && t.target_id == tid && t.system == membership.0
                            })
                        })
                        .map_or(ship.0.rotation, |target| {
                            let lead = sv_core::lead_target_position(
                                ship.0.position,
                                target.position,
                                target.velocity,
                                blaster.def.projectile_speed,
                            );
                            let delta = lead - ship.0.position;
                            delta.y.atan2(delta.x)
                        });
                    let dir = Vec2::new(aimed_angle.cos(), aimed_angle.sin());
                    let right = Vec2::new(-dir.y, dir.x);
                    let lateral = if fixed_total > 1 {
                        (slot_idx as f32 - (fixed_total as f32 - 1.0) * 0.5) * 5.0
                    } else {
                        0.0
                    };
                    (
                        aimed_angle,
                        ship.0.position + dir * SHIP_RADIUS + right * lateral,
                        ProjectileKind::Bolt,
                        None,
                        0.0,
                    )
                }
                WeaponMount::Turret => {
                    let aimed_angle = select_weapon_target(
                        entity,
                        owner.0,
                        info.kind,
                        &membership.0,
                        ship.0.position,
                        lock.0,
                        TURRET_RANGE,
                        &targets,
                    )
                    .map_or(ship.0.rotation, |target| {
                        let lead = sv_core::lead_target_position(
                            ship.0.position,
                            target.position,
                            target.velocity,
                            blaster.def.projectile_speed,
                        );
                        let delta = lead - ship.0.position;
                        delta.y.atan2(delta.x)
                    });
                    let dir = Vec2::new(aimed_angle.cos(), aimed_angle.sin());
                    (
                        aimed_angle,
                        ship.0.position + dir * SHIP_RADIUS,
                        ProjectileKind::TurretBolt,
                        None,
                        0.0,
                    )
                }
                WeaponMount::Missile { turn_rate } => {
                    let chosen = select_weapon_target(
                        entity,
                        owner.0,
                        info.kind,
                        &membership.0,
                        ship.0.position,
                        lock.0,
                        MISSILE_DETECT_RANGE,
                        &targets,
                    );
                    let aimed_angle = chosen.map_or(ship.0.rotation, |target| {
                        let delta = target.position - ship.0.position;
                        delta.y.atan2(delta.x)
                    });
                    let dir = Vec2::new(aimed_angle.cos(), aimed_angle.sin());
                    (
                        aimed_angle,
                        ship.0.position + dir * SHIP_RADIUS,
                        ProjectileKind::Missile,
                        chosen.map(|t| t.entity),
                        turn_rate,
                    )
                }
            };

            commands.spawn((
                Replicated,
                Projectile {
                    damage: blaster.def.damage,
                    ttl: blaster.def.projectile_ttl,
                    owner: owner.0,
                    kind,
                },
                ProjectileMotion {
                    speed: blaster.def.projectile_speed,
                    shooter_entity: Some(entity),
                    shooter_kind: info.kind,
                    homing_target,
                    turn_rate,
                },
                Position(spawn_pos),
                Heading(angle),
            ));
        }
    }
}

type ProjectileTargetShips<'w, 's> =
    Query<'w, 's, (Entity, &'static Position, &'static ShipInfo), (With<Ship>, Without<DockedAt>)>;

fn projectile_flight(
    mut commands: Commands,
    time: Res<Time>,
    ships: ProjectileTargetShips,
    mut projectiles: Query<
        (
            Entity,
            &mut Heading,
            &mut Position,
            &mut Projectile,
            &ProjectileMotion,
        ),
        Without<Ship>,
    >,
) {
    let dt = time.delta_secs();
    for (entity, mut heading, mut position, mut projectile, motion) in &mut projectiles {
        if motion.turn_rate > 0.0 {
            let target_pos = motion
                .homing_target
                .and_then(|target_ent| ships.get(target_ent).ok().map(|(_, pos, _)| pos.0))
                .or_else(|| {
                    ships
                        .iter()
                        .filter(|(ent, pos, info)| {
                            Some(*ent) != motion.shooter_entity
                                && info.owner != Some(projectile.owner)
                                && info.kind == ShipKind::Pirate
                                && position.0.distance(pos.0) <= MISSILE_DETECT_RANGE
                        })
                        .min_by_key(|(_, pos, _)| position.0.distance(pos.0).to_bits())
                        .map(|(_, pos, _)| pos.0)
                });
            if let Some(target_pos) = target_pos {
                heading.0 = sv_core::steer_angle_toward(
                    heading.0,
                    position.0,
                    target_pos,
                    motion.turn_rate,
                    dt,
                );
            }
        }
        let direction = Vec2::new(heading.0.cos(), heading.0.sin());
        position.0 += direction * motion.speed * dt;
        projectile.ttl -= dt;
        if projectile.ttl <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn projectile_hits(
    mut commands: Commands,
    content: Res<Content>,
    faction_state: Res<factions::FactionState>,
    projectiles: Query<(Entity, &Position, &Heading, &Projectile, &ProjectileMotion)>,
    aggros: Query<&Aggro>,
    ships: Query<(Entity, &ShipInfo, Option<&ClientRef>), With<Ship>>,
    memberships: Query<&SystemMembership, With<Ship>>,
    mut hittable: Query<
        (
            &mut Ship,
            &mut LastDamagedBy,
            Option<&mut Wallet>,
            Option<&mut PlayerReputation>,
        ),
        Without<DockedAt>,
    >,
    mut meteorites: Query<(
        Entity,
        &Position,
        &mut Meteorite,
        &MeteoriteMotion,
        &SystemMembership,
    )>,
    clients: Query<Entity, With<ConnectedClient>>,
) {
    let mut ship_hits = Vec::new();
    let mut meteorite_hits = Vec::new();
    for (projectile_entity, position, heading, projectile, motion) in &projectiles {
        let mut hit_ship = false;
        for (ship_entity, info, _) in &ships {
            if motion.shooter_entity == Some(ship_entity) {
                continue;
            }
            if info.owner == Some(projectile.owner) {
                continue;
            }
            // Prevent NPC friendly fire except between Pirates and Defenders:
            if motion.shooter_kind != ShipKind::Player && info.kind != ShipKind::Player {
                let valid_npc_vs_npc = (motion.shooter_kind == ShipKind::Defender
                    && info.kind == ShipKind::Pirate)
                    || (motion.shooter_kind == ShipKind::Pirate && info.kind == ShipKind::Defender);
                if !valid_npc_vs_npc {
                    continue;
                }
            }
            if let Ok((ship, _, _, _)) = hittable.get(ship_entity)
                && position.0.distance(ship.0.position) <= SHIP_RADIUS
            {
                ship_hits.push((
                    projectile_entity,
                    ship_entity,
                    projectile.damage,
                    projectile.owner,
                    motion.shooter_kind,
                ));
                hit_ship = true;
                break;
            }
        }
        if hit_ship {
            continue;
        }
        for (m_entity, m_pos, meteorite, _, _) in &meteorites {
            if position.0.distance(m_pos.0) <= meteorite.size.radius() {
                let proj_dir = Vec2::new(heading.0.cos(), heading.0.sin());
                meteorite_hits.push((projectile_entity, m_entity, projectile.damage, proj_dir));
                break;
            }
        }
    }

    for (projectile_entity, m_entity, damage, proj_dir) in meteorite_hits {
        commands.entity(projectile_entity).despawn();
        let Ok((_, m_pos, mut meteorite, motion, m_sys)) = meteorites.get_mut(m_entity) else {
            continue;
        };
        if meteorite.hp <= 0.0 {
            continue;
        }
        meteorite.hp = (meteorite.hp - damage).max(0.0);
        debug!(
            "server: projectile hit {:?} meteorite in '{}' for {damage:.1} dmg (hp={:.1}/{:.1})",
            meteorite.size, m_sys.0, meteorite.hp, meteorite.max_hp
        );
        if meteorite.hp <= 0.0 {
            fragment_meteorite(
                &mut commands,
                m_entity,
                m_pos.0,
                &meteorite,
                motion,
                &m_sys.0,
                proj_dir,
                &clients,
            );
        }
    }

    let mut fined_now: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for (projectile_entity, victim_entity, damage, attacker_owner, shooter_kind) in ship_hits {
        commands.entity(projectile_entity).despawn();
        let Ok((mut victim_ship, mut last_damaged, _, _)) = hittable.get_mut(victim_entity) else {
            continue;
        };
        victim_ship.0.apply_damage(damage);
        if attacker_owner != 0 && shooter_kind == ShipKind::Player {
            last_damaged.0 = Some(attacker_owner);
        }

        let Ok((_, victim_info, _)) = ships.get(victim_entity) else {
            continue;
        };
        debug!(
            "server: projectile from owner={attacker_owner} hit '{}' ({:?}) for {damage:.1} dmg (shields={:.1}, armor={:.1})",
            victim_info.name, victim_info.kind, victim_ship.0.shields, victim_ship.0.armor
        );

        // Universal NPC retaliation when fired upon by a player:
        if shooter_kind == ShipKind::Player
            && attacker_owner != 0
            && victim_info.kind != ShipKind::Player
        {
            let already_aggro = aggros
                .get(victim_entity)
                .is_ok_and(|existing| existing.0 == attacker_owner);
            commands.entity(victim_entity).insert(Aggro(attacker_owner));
            if !already_aggro
                && matches!(
                    victim_info.kind,
                    ShipKind::Merchant | ShipKind::Defender | ShipKind::Explorer
                )
                && let Some((attacker_entity, _, Some(client_ref))) = ships
                    .iter()
                    .find(|(_, info, _)| info.owner == Some(attacker_owner))
            {
                commands
                    .entity(attacker_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
                notice(
                    &mut commands,
                    ClientId::Client(client_ref.0),
                    format!(
                        "{} ({}) powers weapons and returns fire!",
                        victim_info.name,
                        victim_info.kind.as_str()
                    ),
                );
            }
        }

        let is_pvp = victim_info.kind == ShipKind::Player
            && shooter_kind == ShipKind::Player
            && attacker_owner != 0
            && victim_info.owner != Some(attacker_owner);
        if !is_pvp {
            continue;
        }
        let Some((attacker_entity, _, attacker_client_ref)) = ships
            .iter()
            .find(|(_, info, _)| info.owner == Some(attacker_owner))
        else {
            continue;
        };
        let Ok(attacker_membership) = memberships.get(attacker_entity) else {
            continue;
        };
        let attacker_system = attacker_membership.0.clone();
        let controller = faction_state.controller(&attacker_system).to_owned();
        if factions::side_sign(&content.0, &controller) == 0 {
            continue;
        }
        let police_exists = aggros.iter().any(|aggro| aggro.0 == attacker_owner)
            || fined_now.contains(&attacker_owner);
        if police_exists {
            continue;
        }
        let attacker_pos = hittable
            .get(attacker_entity)
            .map(|(ship, _, _, _)| ship.0.position)
            .unwrap_or_default();
        let sys_center = content
            .0
            .system(&attacker_system)
            .map(|system| Vec2::new(system.position.0, system.position.1))
            .unwrap_or_default();
        let offset = (attacker_pos - sys_center) + Vec2::new(280.0, -160.0);
        factions::spawn_patrol(
            &mut commands,
            &content.0,
            &attacker_system,
            offset,
            attacker_owner,
        );
        fined_now.insert(attacker_owner);
        if let Ok((_, _, Some(mut wallet), Some(mut reputation))) =
            hittable.get_mut(attacker_entity)
        {
            let fine = wallet.0.credits.min(FINE_CREDITS);
            wallet.0.credits -= fine;
            factions::apply_rep(&mut reputation.standings, &controller, factions::REP_FINE);
            if let Some(client_ref) = attacker_client_ref {
                notice(
                    &mut commands,
                    ClientId::Client(client_ref.0),
                    format!(
                        "Customs drones: {fine} cr fine for unlawful weapons discharge in {}.",
                        factions::system_name(&content.0, &attacker_system)
                    ),
                );
            }
        }
        if let Some(client_ref) = attacker_client_ref {
            notice(
                &mut commands,
                ClientId::Client(client_ref.0),
                format!(
                    "A {} patrol locks onto you.",
                    factions::faction_name(&content.0, &controller)
                ),
            );
        }
    }
}

fn meteorite_movement(
    time: Res<Time>,
    mut meteorites: Query<(
        &mut Position,
        &mut Heading,
        &Meteorite,
        &mut MeteoriteMotion,
    )>,
) {
    let dt = time.delta_secs();
    for (mut pos, mut heading, meteorite, mut motion) in &mut meteorites {
        motion.hit_cooldown = (motion.hit_cooldown - dt).max(0.0);
        if motion.velocity.length_squared() > 0.0 {
            pos.0 += motion.velocity * dt;
            heading.0 = (heading.0 + meteorite.spin_rate * dt + PI).rem_euclid(2.0 * PI) - PI;
            let offset = pos.0 - motion.center;
            if motion.field_radius > 0.0 && offset.length() > motion.field_radius {
                let wrapped =
                    motion.center - offset.normalize_or(Vec2::X) * (motion.field_radius - 12.0);
                pos.0 = wrapped;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn fragment_meteorite(
    commands: &mut Commands,
    entity: Entity,
    position: Vec2,
    meteorite: &Meteorite,
    motion: &MeteoriteMotion,
    system: &str,
    impact_dir: Vec2,
    clients: &Query<Entity, With<ConnectedClient>>,
) {
    let explosion = Explosion {
        position: (position.x, position.y),
        system: system.to_owned(),
    };
    for client_entity in clients {
        commands.server_trigger(ToClients {
            targets: SendTargets::Single(ClientId::Client(client_entity)),
            message: explosion.clone(),
        });
    }
    commands.entity(entity).despawn();

    if let Some(child_size) = meteorite.size.child_size() {
        let (v1, v2) = sv_core::split_meteorite_velocities(motion.velocity, impact_dir, 22.0);
        let lateral = (v1 - v2).try_normalize().unwrap_or(Vec2::Y) * (child_size.radius() * 0.9);
        let base_spin = meteorite.spin_rate.abs().max(1.2) * 1.35;
        info!(
            "server: {:?} meteorite in '{}' split into 2 {:?} fragments at ({:.0}, {:.0})",
            meteorite.size, system, child_size, position.x, position.y
        );
        spawn_meteorite(
            commands,
            system,
            motion.center,
            motion.field_radius,
            position + lateral,
            v1,
            child_size,
            base_spin,
            meteorite.variant.wrapping_add(1),
        );
        spawn_meteorite(
            commands,
            system,
            motion.center,
            motion.field_radius,
            position - lateral,
            v2,
            child_size,
            -base_spin,
            meteorite.variant.wrapping_add(2),
        );
    } else {
        info!(
            "server: Small meteorite in '{}' shattered and disappeared at ({:.0}, {:.0})",
            system, position.x, position.y
        );
    }
}

#[allow(clippy::type_complexity)]
fn meteorite_ship_collisions(
    mut commands: Commands,
    mut ships: Query<(&mut Ship, &ShipInfo), (With<PlayerShip>, Without<DockedAt>)>,
    mut meteorites: Query<(
        Entity,
        &Position,
        &mut Meteorite,
        &mut MeteoriteMotion,
        &SystemMembership,
    )>,
    clients: Query<Entity, With<ConnectedClient>>,
) {
    for (m_entity, m_pos, mut meteorite, mut motion, m_sys) in &mut meteorites {
        if motion.hit_cooldown > 0.0 || meteorite.hp <= 0.0 {
            continue;
        }
        let hit_radius = SHIP_RADIUS + meteorite.size.radius();
        for (mut ship, info) in &mut ships {
            let delta = ship.0.position - m_pos.0;
            let dist = delta.length();
            if dist <= hit_radius {
                let damage = meteorite.size.collision_damage();
                ship.0.apply_damage(damage);
                let normal = delta.try_normalize().unwrap_or(Vec2::X);
                ship.0.position = m_pos.0 + normal * (hit_radius + 4.0);
                ship.0.velocity += normal * 75.0;
                motion.hit_cooldown = METEORITE_HIT_COOLDOWN_SECS;

                info!(
                    "server: '{}' ({:?}) crashed into {:?} meteorite for {damage:.1} dmg (shields={:.1}, armor={:.1})",
                    info.name, info.kind, meteorite.size, ship.0.shields, ship.0.armor
                );

                meteorite.hp = (meteorite.hp - 12.0).max(0.0);
                if meteorite.hp <= 0.0 {
                    fragment_meteorite(
                        &mut commands,
                        m_entity,
                        m_pos.0,
                        &meteorite,
                        &motion,
                        &m_sys.0,
                        -normal,
                        &clients,
                    );
                }
                break;
            }
        }
    }
}

fn black_hole_gravity(
    mut commands: Commands,
    time: Res<Time>,
    content: Res<Content>,
    mut rng: ResMut<EncounterRng>,
    mut ships: Query<(&mut Ship, Option<&ClientRef>), Without<DockedAt>>,
    holes: Query<(&BlackHoleTag, &BlackHoleGravity, &Position)>,
) {
    for (mut ship, client_ref) in &mut ships {
        if ship.0.is_destroyed() {
            continue;
        }
        // This system immediately follows apply_ship_control, which integrates
        // position += velocity * dt. Recover its exact segment start before
        // any gravity modifies velocity, so grazing/warp crossings cannot tunnel.
        let flight_start = ship.0.position - ship.0.velocity * time.delta_secs();
        for (hole, gravity, position) in &holes {
            let delta = position.0 - ship.0.position;
            if sv_core::black_hole_horizon_crossed(
                flight_start - position.0,
                ship.0.position - position.0,
                hole.radius,
            ) {
                if rng.next_u32(2) == 0 {
                    ship.0.apply_damage(f32::MAX);
                    if let Some(client_ref) = client_ref {
                        notice(
                            &mut commands,
                            ClientId::Client(client_ref.0),
                            "Event horizon crossed. Your insurer declines to comment.",
                        );
                    }
                } else {
                    let destinations: Vec<_> = content
                        .0
                        .systems
                        .iter()
                        .filter(|system| system.id != hole.system)
                        .collect();
                    // Validation requires another system for the teleport branch.
                    if let Some(destination) =
                        destinations.get(rng.next_u32(destinations.len() as u32) as usize)
                    {
                        ship.0.position = Vec2::from(destination.position);
                        ship.0.velocity = Vec2::ZERO;
                        if let Some(client_ref) = client_ref {
                            notice(
                                &mut commands,
                                ClientId::Client(client_ref.0),
                                format!(
                                    "The black hole spits you out in {}. Check the cargo straps.",
                                    destination.name
                                ),
                            );
                        }
                    }
                }
                // One outcome per ship per tick, even if hazard discs overlap.
                break;
            }
            ship.0.velocity +=
                sv_core::black_hole_acceleration(delta, hole.radius, hole.influence, gravity.0)
                    * time.delta_secs();
        }
    }
}

fn star_heat(
    mut commands: Commands,
    time: Res<Time>,
    mut ships: Query<(&mut Ship, Option<&ClientRef>, &mut HeatWarningCooldown), Without<DockedAt>>,
    stars: Query<(&StarTag, &Position)>,
) {
    let dt = time.delta_secs();
    for (mut ship, client_ref, mut warn_cooldown) in &mut ships {
        warn_cooldown.0 = (warn_cooldown.0 - dt).max(0.0);
        let mut total_dps = 0.0;
        for (star, star_pos) in &stars {
            let dist = ship.0.position.distance(star_pos.0);
            total_dps += star_heat_dps(dist, star.radius);
        }
        if total_dps <= 0.0 {
            continue;
        }
        ship.0.apply_damage(total_dps * dt);
        if let Some(client_ref) = client_ref
            && warn_cooldown.0 <= 0.0
        {
            notice(
                &mut commands,
                ClientId::Client(client_ref.0),
                "Stellar corona: hull heat climbing. Back off or bake.",
            );
            warn_cooldown.0 = STAR_HEAT_NOTICE_COOLDOWN_SECS;
        }
    }
}

fn shield_regen(time: Res<Time>, mut ships: Query<(&Control, &mut Ship)>) {
    let dt = time.delta_secs();
    for (control, mut ship) in &mut ships {
        ship.0.regen_shields(SHIELD_REGEN, dt);
        if !control.afterburner {
            ship.0.regen_energy(dt);
        }
    }
}

fn quest_progress(
    mut commands: Commands,
    time: Res<Time>,
    dying_npcs: Query<(&Ship, &ShipInfo, &SystemMembership, &LastDamagedBy), With<Npc>>,
    mut players: Query<
        (&Owner, &mut ActiveQuests, &mut Wallet, Option<&ClientRef>),
        With<PlayerShip>,
    >,
) {
    let dt = time.delta_secs();

    for (ship, info, membership, last_damaged) in &dying_npcs {
        if !ship.0.is_destroyed() || info.kind != ShipKind::Pirate {
            continue;
        }
        let Some(killer_owner) = last_damaged.0 else {
            continue;
        };
        for (owner, mut quests, _, client_ref) in &mut players {
            if owner.0 != killer_owner {
                continue;
            }
            for quest in &mut quests.quests {
                if let QuestTarget::Bounty {
                    system,
                    target,
                    killed,
                } = &mut quest.target
                    && !*killed
                    && *system == membership.0
                    && (*target == info.def_id || target == "pirate")
                {
                    *killed = true;
                    if let Some(client_ref) = client_ref {
                        notice(
                            &mut commands,
                            ClientId::Client(client_ref.0),
                            format!(
                                "Bounty target eliminated in {}. Return to {} to collect.",
                                membership.0, quest.giver_planet
                            ),
                        );
                    }
                }
            }
        }
    }

    for (_, mut quests, mut wallet, client_ref) in &mut players {
        let mut expired = Vec::new();
        for (idx, quest) in quests.quests.iter_mut().enumerate() {
            if let Some(rem) = &mut quest.remaining_secs {
                *rem -= dt;
                if *rem <= 0.0 {
                    expired.push(idx);
                }
            }
        }
        for idx in expired.into_iter().rev() {
            let failed = quests.quests.remove(idx);
            if let QuestTarget::Delivery { commodity, qty, .. } = &failed.target {
                let _ = wallet.0.remove_cargo(commodity, *qty);
            }
            if let Some(client_ref) = client_ref {
                notice(
                    &mut commands,
                    ClientId::Client(client_ref.0),
                    format!("Contract '{}' expired.", failed.name),
                );
            }
        }
    }
}

type DyingShips<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Ship,
        &'static ShipInfo,
        &'static SystemMembership,
        &'static Owner,
        Option<&'static ClientRef>,
        Option<&'static Wallet>,
        Option<&'static ActiveQuests>,
        &'static EquippedOutfits,
        Option<&'static PlayerReputation>,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn ship_deaths(
    mut commands: Commands,
    mut pending: ResMut<PendingRecoveries>,
    content: Res<Content>,
    mut pirate_respawn: ResMut<PirateRespawn>,
    ships: DyingShips,
    names: Query<&PlayerName>,
    storage: Res<StorageResource>,
    clients: Query<Entity, With<ConnectedClient>>,
) {
    for (entity, ship, info, membership, owner, client_ref, wallet, quests, equipped, reputation) in
        &ships
    {
        if !ship.0.is_destroyed() {
            continue;
        }
        let explosion = Explosion {
            position: (ship.0.position.x, ship.0.position.y),
            system: membership.0.clone(),
        };
        for client in &clients {
            commands.server_trigger(ToClients {
                targets: SendTargets::Single(ClientId::Client(client)),
                message: explosion.clone(),
            });
        }
        if info.kind == ShipKind::Player {
            let equipped_ids: Vec<String> = equipped
                .ids
                .iter()
                .filter(|id| content.0.outfit(id).is_some())
                .cloned()
                .collect();
            let (hull_id, ids, stats) = match content.0.loadout_stats(&info.def_id, &equipped_ids) {
                Ok(stats) => (info.def_id.clone(), equipped_ids, stats),
                Err(error) => {
                    warn!(
                        "owner={}: recovery downgrades to the starter hull: {error}",
                        owner.0
                    );
                    if let Some(client) = client_ref {
                        notice(
                            &mut commands,
                            ClientId::Client(client.0),
                            "The registry no longer lists your hull. Recovery issues a factory shuttle.",
                        );
                    }
                    let def = content
                        .0
                        .ship(PLAYER_SHIP)
                        .expect("player ship must exist in pack");
                    let factory = def.default_outfits.clone();
                    let stats = content
                        .0
                        .loadout_stats(PLAYER_SHIP, &factory)
                        .expect("validated default loadout");
                    (PLAYER_SHIP.to_owned(), factory, stats)
                }
            };
            let completed = quests.map(|q| q.completed.clone()).unwrap_or_default();
            let saved = crate::PlayerState {
                credits: wallet.map_or(STARTING_CREDITS, |w| w.0.credits),
                cargo: vec![],
                ship_id: hull_id,
                system: PLAYER_SYSTEM.into(),
                position: (0.0, 0.0),
                fuel: stats.max_fuel,
                shields: stats.max_shields,
                armor: stats.max_armor,
                active_quests_json: encode_active_quests(&ActiveQuests {
                    quests: vec![],
                    completed,
                }),
                equipped_outfits: Some(ids),
                reputation_json: reputation
                    .map(factions::encode_reputation)
                    .unwrap_or_else(|| "[]".to_owned()),
            };
            let name = names
                .get(entity)
                .map(|n| n.0.clone())
                .unwrap_or_else(|_| "pilot".into());
            let needs_save = storage
                .storage
                .as_ref()
                .is_some_and(|db| db.save_player(&name, &saved).is_err());
            if needs_save {
                warn!("recovery for owner={} will retry persistence", owner.0);
            }
            pending.0.insert(
                owner.0,
                Recovery {
                    state: saved,
                    name,
                    remaining: PLAYER_RESPAWN_SECS,
                    needs_save,
                },
            );
            if let Some(client) = client_ref {
                commands.server_trigger(ToClients {
                    targets: SendTargets::Single(ClientId::Client(client.0)),
                    message: PlayerDied,
                });
            }
        } else if info.kind == ShipKind::Pirate {
            pirate_respawn.0 = Some(PIRATE_RESPAWN_SECS);
        }
        commands.entity(entity).despawn();
    }
}

fn respawn_players(
    mut commands: Commands,
    time: Res<Time>,
    content: Res<Content>,
    mut pending: ResMut<PendingRecoveries>,
    players: Query<&Owner, With<PlayerShip>>,
    clients: Query<(Entity, &NetworkId), With<ConnectedClient>>,
    storage: Res<StorageResource>,
) {
    let mut finished = Vec::new();
    for (&owner, recovery) in &mut pending.0 {
        recovery.remaining -= time.delta_secs();
        if recovery.remaining > 0.0 {
            continue;
        }
        if players.iter().any(|o| o.0 == owner) {
            finished.push(owner);
            continue;
        }
        if recovery.needs_save {
            let saved = storage
                .storage
                .as_ref()
                .is_none_or(|db| db.save_player(&recovery.name, &recovery.state).is_ok());
            if !saved {
                recovery.remaining = 0.5;
                continue;
            }
            recovery.needs_save = false;
        }
        let Some((client, _)) = clients.iter().find(|(_, id)| id.get() == owner) else {
            continue;
        };
        match spawn_saved_player(
            &mut commands,
            &content.0,
            owner,
            client,
            recovery.name.clone(),
            &recovery.state,
        ) {
            Ok(_) => finished.push(owner),
            Err(error) => {
                warn!("recovery failed for owner={owner}: {error}");
                recovery.remaining = 0.5;
            }
        }
    }
    for owner in finished {
        pending.0.remove(&owner);
    }
}

fn respawn_pirates(
    mut commands: Commands,
    time: Res<Time>,
    content: Res<Content>,
    mut respawn: ResMut<PirateRespawn>,
) {
    let Some(timer) = respawn.0.as_mut() else {
        return;
    };
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    respawn.0 = None;
    spawn_ship(
        &mut commands,
        &content.0,
        PIRATE_SHIP,
        ShipKind::Pirate,
        None,
        PIRATE_SYSTEM,
        Vec2::new(0.0, -400.0),
    );
}

fn update_system_membership(
    mut commands: Commands,
    content: Res<Content>,
    mut ships: Query<(&Ship, &mut SystemMembership, Option<&ClientRef>), Without<DockedAt>>,
) {
    for (ship, mut membership, client_ref) in &mut ships {
        if ship.0.is_destroyed() {
            continue;
        }
        let mut best: Option<(f32, &str)> = None;
        for system in &content.0.systems {
            let center = Vec2::new(system.position.0, system.position.1);
            let dist = ship.0.position.distance(center);
            if dist < SYSTEM_MEMBERSHIP_RADIUS && best.is_none_or(|(d, _)| dist < d) {
                best = Some((dist, &system.id));
            }
        }
        if let Some((_, id)) = best
            && membership.0 != id
        {
            membership.0 = id.to_owned();
            if let Some(system) = content.0.system(id)
                && let Some(client_ref) = client_ref
            {
                notice(
                    &mut commands,
                    ClientId::Client(client_ref.0),
                    format!("Entering {}.", system.name),
                );
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn sync_ship_state(
    valid_targets: Query<(&TargetId, &SystemMembership), (With<Ship>, Without<DockedAt>)>,
    mut ships: Query<(
        &Ship,
        &SystemMembership,
        &mut Position,
        &mut Velocity,
        &mut Heading,
        &mut Hull,
        &mut Fuel,
        &mut Energy,
        &mut TargetLock,
    )>,
) {
    for (
        ship,
        membership,
        mut position,
        mut velocity,
        mut heading,
        mut hull,
        mut fuel,
        mut energy,
        mut lock,
    ) in &mut ships
    {
        position.0 = ship.0.position;
        velocity.0 = ship.0.velocity;
        heading.0 = ship.0.rotation;
        hull.shields = ship.0.shields;
        hull.armor = ship.0.armor;
        fuel.current = ship.0.fuel;
        energy.current = ship.0.energy;
        energy.max = ship.0.max_energy;
        if let Some(tid) = lock.0
            && !valid_targets
                .iter()
                .any(|(other_id, other_sys)| other_id.0 == tid && other_sys.0 == membership.0)
        {
            lock.0 = None;
        }
    }
}

fn sync_accounts(mut players: Query<(&Wallet, &mut AccountState)>) {
    for (wallet, mut state) in &mut players {
        if state.credits != wallet.0.credits || state.cargo != wallet.0.cargo {
            state.credits = wallet.0.credits;
            state.cargo = wallet.0.cargo.clone();
        }
    }
}

type AutoSavePlayers<'w, 's> = Query<
    'w,
    's,
    (
        &'static PlayerName,
        &'static Wallet,
        &'static Ship,
        &'static SystemMembership,
        &'static ShipInfo,
        &'static EquippedOutfits,
        Option<&'static ActiveQuests>,
        Option<&'static PlayerReputation>,
    ),
    With<PlayerShip>,
>;

fn auto_save(
    time: Res<Time>,
    mut timer: ResMut<SaveTimer>,
    storage: Res<StorageResource>,
    players: AutoSavePlayers,
) {
    timer.0 += time.delta_secs();
    if timer.0 < 10.0 {
        return;
    }
    timer.0 = 0.0;
    let Some(db) = storage.storage.as_ref() else {
        return;
    };
    for (name, wallet, ship, membership, info, equipped, quests, reputation) in &players {
        let state = crate::PlayerState {
            credits: wallet.0.credits,
            cargo: wallet.0.cargo.clone(),
            ship_id: info.def_id.clone(),
            system: membership.0.clone(),
            position: (ship.0.position.x, ship.0.position.y),
            fuel: ship.0.fuel,
            shields: ship.0.shields,
            armor: ship.0.armor,
            active_quests_json: quests.map_or_else(|| "[]".into(), encode_active_quests),
            equipped_outfits: Some(equipped.ids.clone()),
            reputation_json: reputation
                .map(factions::encode_reputation)
                .unwrap_or_else(|| "[]".to_owned()),
        };
        if let Err(error) = db.save_player(&name.0, &state) {
            warn!("autosave failed: {error}");
        }
    }
}

type DockingShips<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Owner,
        &'static mut SystemMembership,
        Option<&'static DockedAt>,
        Option<&'static ActiveQuests>,
        &'static mut Ship,
        &'static mut Wallet,
        &'static mut Control,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn on_request_land(
    trigger: On<FromClient<RequestLand>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    board: Res<contracts::ContractBoard>,
    mut rng: ResMut<EncounterRng>,
    mut ships: DockingShips,
    ports: Query<(&DockableTag, &Position)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let system = ships
        .iter_mut()
        .find(|(_, ship_owner, ..)| ship_owner.0 == owner)
        .map(|(_, _, membership, ..)| membership.0.clone());
    let target = ports.iter().find(|(port, _)| {
        port.kind == DockableKind::Planet
            && Some(&port.system) == system.as_ref()
            && port.id == trigger.event().planet
    });
    if let Some((port, position)) = target {
        try_dock(
            owner,
            trigger.client_id,
            port,
            position.0,
            &mut commands,
            &content.0,
            &mut rng,
            &mut ships,
            &board,
        );
    } else {
        notice(
            &mut commands,
            trigger.client_id,
            "No planet answers by that name.",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn on_request_dock(
    trigger: On<FromClient<RequestDock>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    board: Res<contracts::ContractBoard>,
    mut rng: ResMut<EncounterRng>,
    mut ships: DockingShips,
    ports: Query<(&DockableTag, &Position)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let event = trigger.event();
    if let Some((port, position)) = ports
        .iter()
        .find(|(port, _)| port.system == event.system && port.id == event.location)
    {
        try_dock(
            owner,
            trigger.client_id,
            port,
            position.0,
            &mut commands,
            &content.0,
            &mut rng,
            &mut ships,
            &board,
        );
    } else {
        notice(
            &mut commands,
            trigger.client_id,
            "No docking port answers by that name.",
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn try_dock(
    owner: u64,
    client: ClientId,
    port: &DockableTag,
    position: Vec2,
    commands: &mut Commands,
    pack: &ContentPack,
    rng: &mut EncounterRng,
    ships: &mut DockingShips,
    board: &contracts::ContractBoard,
) {
    let Some((
        ship_entity,
        _,
        mut membership,
        docked,
        active_quests,
        mut ship,
        mut wallet,
        mut control,
    )) = ships
        .iter_mut()
        .find(|(_, ship_owner, ..)| ship_owner.0 == owner)
    else {
        return;
    };
    if docked.is_some() {
        notice(commands, client, "You are already docked.");
        return;
    }
    if ship.0.is_destroyed() {
        return;
    }
    let distance = ship.0.position.distance(position);
    if !distance.is_finite() || distance > LAND_RANGE + port.radius {
        notice(commands, client, "You are not close enough to dock.");
        return;
    }
    membership.0.clone_from(&port.system);
    ship.0.velocity = Vec2::ZERO;
    ship.0.energy = ship.0.max_energy;
    *control = Control::default();
    if port.kind == DockableKind::Station {
        let normal = (ship.0.position - position).normalize_or(Vec2::X);
        let offset = normal * (port.radius + SHIP_RADIUS + 12.0);
        ship.0.position = position + offset;
        commands.entity(ship_entity).insert(DockingOffset(offset));
    }
    commands.entity(ship_entity).insert(DockedAt {
        system: port.system.clone(),
        location: port.id.clone(),
        kind: port.kind,
    });
    commands
        .entity(ship_entity)
        .remove::<(ChiefEncounter, PendingEncounter)>();
    let missing = ship.0.max_fuel - ship.0.fuel;
    let cost = (missing * FUEL_PRICE_PER_UNIT).ceil() as u32;
    if cost == 0 {
        notice(
            commands,
            client,
            format!("Docked at {}. Tanks already full.", port.name),
        );
    } else if wallet.0.credits >= cost {
        wallet.0.credits -= cost;
        ship.0.fuel = ship.0.max_fuel;
        notice(
            commands,
            client,
            format!("Docked at {}. Refuel: {cost} cr.", port.name),
        );
    } else {
        notice(
            commands,
            client,
            format!(
                "Docked at {}. The fuel clerk wants cash you don't have.",
                port.name
            ),
        );
    }

    let empty_quests = ActiveQuests::default();
    let quests = active_quests.unwrap_or(&empty_quests);
    if let Some((encounter, pending)) =
        roll_chief_encounter(pack, rng, &membership.0, &port.id, &wallet.0, quests, board)
    {
        commands.entity(ship_entity).insert((encounter, pending));
    }
}

fn roll_chief_encounter(
    pack: &ContentPack,
    rng: &mut EncounterRng,
    system_id: &str,
    planet_id: &str,
    account: &Account,
    active_quests: &ActiveQuests,
    board: &contracts::ContractBoard,
) -> Option<(ChiefEncounter, PendingEncounter)> {
    let port_def = pack.dockable(system_id, planet_id)?;
    let chief = port_def.host()?;

    if let Some(quest) = active_quests.quests.iter().find(|q| match &q.target {
        QuestTarget::Delivery {
            commodity,
            qty,
            destination_system,
            destination_planet,
        } => {
            let held = account
                .cargo
                .iter()
                .find(|(c, _)| c == commodity)
                .map_or(0, |(_, q)| *q);
            destination_system == system_id && destination_planet == planet_id && held >= *qty
        }
        QuestTarget::Bounty { killed, .. } => {
            *killed && q.giver_system == system_id && q.giver_planet == planet_id
        }
    }) {
        let encounter = ChiefEncounter {
            chief_name: chief.name.clone(),
            portrait: chief.portrait.clone(),
            mood: ChiefMood::TurnIn,
            text: format!(
                "{}\n\n\"You're back with '{}' complete. Here is the {} cr voucher.\"",
                chief.greeting, quest.name, quest.reward_credits
            ),
            options: vec![
                EncounterOption {
                    id: "turn_in".to_owned(),
                    label: format!("Complete '{}' (+{} cr)", quest.name, quest.reward_credits),
                },
                EncounterOption {
                    id: "decline".to_owned(),
                    label: "Hold on".to_owned(),
                },
            ],
        };
        let pending = PendingEncounter::TurnIn {
            quest_id: quest.id.clone(),
        };
        return Some((encounter, pending));
    }

    let authored_quests: Vec<&QuestDef> = pack
        .quests
        .iter()
        .filter(|q| {
            q.giver.0 == system_id
                && q.giver.1 == planet_id
                && !active_quests.quests.iter().any(|aq| aq.id == q.id)
                && !active_quests.completed.contains(&q.id)
        })
        .collect();
    let available_quests: Vec<&QuestDef> = if authored_quests.is_empty() {
        board
            .contracts
            .iter()
            .filter(|q| {
                q.giver.0 == system_id
                    && q.giver.1 == planet_id
                    && !active_quests.quests.iter().any(|aq| aq.id == q.id)
            })
            .collect()
    } else {
        authored_quests
    };

    let roll = rng.next_u32(100);
    if !available_quests.is_empty() && roll < 50 {
        let idx = rng.next_u32(available_quests.len() as u32) as usize;
        let quest = available_quests[idx];
        let encounter = ChiefEncounter {
            chief_name: chief.name.clone(),
            portrait: chief.portrait.clone(),
            mood: ChiefMood::Quest,
            text: format!(
                "{}\n{}\n\n{} — {} (Reward: {} cr)",
                chief.greeting,
                chief.quest_text,
                quest.name,
                quest.description,
                quest.reward_credits
            ),
            options: vec![
                EncounterOption {
                    id: "accept_quest".to_owned(),
                    label: format!("Accept '{}' ({} cr)", quest.name, quest.reward_credits),
                },
                EncounterOption {
                    id: "decline".to_owned(),
                    label: "Decline contract".to_owned(),
                },
            ],
        };
        let pending = PendingEncounter::Quest {
            chief_name: chief.name.clone(),
            quest_id: quest.id.clone(),
        };
        return Some((encounter, pending));
    }

    let offer_threshold = if available_quests.is_empty() { 60 } else { 80 };
    if !port_def.commodities().is_empty() && roll < offer_threshold {
        let idx = rng.next_u32(port_def.commodities().len() as u32) as usize;
        let comm = &port_def.commodities()[idx];
        let qty = OFFER_QTY;
        let cost = (comm.price * qty) / 2;
        let encounter = ChiefEncounter {
            chief_name: chief.name.clone(),
            portrait: chief.portrait.clone(),
            mood: ChiefMood::Offer,
            text: format!(
                "{}\n{} ({} {} for {} cr)",
                chief.greeting, chief.offer_text, qty, comm.id, cost
            ),
            options: vec![
                EncounterOption {
                    id: "accept_offer".to_owned(),
                    label: format!("Buy {} {} ({} cr)", qty, comm.id, cost),
                },
                EncounterOption {
                    id: "decline".to_owned(),
                    label: "Pass on the offer".to_owned(),
                },
            ],
        };
        let pending = PendingEncounter::Offer {
            chief_name: chief.name.clone(),
            commodity: comm.id.clone(),
            qty,
            cost,
        };
        return Some((encounter, pending));
    }

    let amount = TRIBUTE_CREDITS;
    let encounter = ChiefEncounter {
        chief_name: chief.name.clone(),
        portrait: chief.portrait.clone(),
        mood: ChiefMood::Tribute,
        text: format!(
            "{}\n{} (Demands {} cr)",
            chief.greeting, chief.tribute_text, amount
        ),
        options: vec![
            EncounterOption {
                id: "pay_tribute".to_owned(),
                label: format!("Pay {} cr tribute", amount),
            },
            EncounterOption {
                id: "refuse_tribute".to_owned(),
                label: "Refuse to pay".to_owned(),
            },
        ],
    };
    let pending = PendingEncounter::Tribute {
        chief_name: chief.name.clone(),
        amount,
    };
    Some((encounter, pending))
}

type UndockingShips<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Owner,
        &'static DockedAt,
        &'static mut Ship,
        &'static mut Control,
        Option<&'static DockingOffset>,
    ),
>;

fn on_request_lift_off(
    trigger: On<FromClient<RequestLiftOff>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    mut ships: UndockingShips,
    ports: Query<(&DockableTag, &Position)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let Some((ship_entity, _, docked, mut ship, mut control, offset)) = ships
        .iter_mut()
        .find(|(_, ship_owner, ..)| ship_owner.0 == owner)
    else {
        return;
    };
    info!(
        "server: owner={owner} lifted off from {}/{}",
        docked.system, docked.location
    );
    if let Some(offset) = offset
        && let Some((port, position)) = ports
            .iter()
            .find(|(port, _)| port.system == docked.system && port.id == docked.location)
    {
        ship.0.position =
            position.0 + offset.0.normalize_or(Vec2::X) * (port.radius + SHIP_RADIUS + 16.0);
    }
    ship.0.velocity = Vec2::ZERO;
    *control = Control::default();
    commands
        .entity(ship_entity)
        .remove::<(DockedAt, DockingOffset, ChiefEncounter, PendingEncounter)>();
}

fn race_greeting(race: &str) -> &'static str {
    match race {
        "vellari" => "Solar winds favor your sails, traveler.",
        "korath" => "State your business quickly—our foundry reactors run hot.",
        "sylph" => "The harmonic currents carry your signal clearly across the void.",
        "auroran" => "Keep your shields steady and your transponder bright, outlander.",
        _ => "Reading you five-by-five on open civilian comms.",
    }
}

type HailingPlayers<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Owner,
        &'static Position,
        Option<&'static DockedAt>,
    ),
    With<PlayerShip>,
>;

type HailableNpcs<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static ShipInfo,
        Option<&'static Aggro>,
        Option<&'static NpcBrain>,
    ),
    With<Npc>,
>;

fn on_hail_ship(
    trigger: On<FromClient<HailShip>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    players: HailingPlayers,
    npcs: HailableNpcs,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let Some((player_entity, _, player_pos, docked)) = players
        .iter()
        .find(|(_, ship_owner, _, _)| ship_owner.0 == owner)
    else {
        return;
    };
    if docked.is_some() {
        notice(
            &mut commands,
            trigger.client_id,
            "Lift off before hailing passing vessels.",
        );
        return;
    }

    let friendly = npcs
        .iter()
        .filter(|(_, pos, info, aggro, _)| {
            aggro.is_none()
                && info.kind != ShipKind::Pirate
                && player_pos.0.distance(pos.0) <= HAIL_RANGE
        })
        .min_by_key(|(_, pos, _, _, _)| player_pos.0.distance(pos.0).to_bits());

    let Some((_, _, npc_info, _, npc_brain)) = friendly else {
        let hostile_near = npcs.iter().any(|(_, pos, info, aggro, _)| {
            (aggro.is_some() || info.kind == ShipKind::Pirate)
                && player_pos.0.distance(pos.0) <= HAIL_RANGE
        });
        if hostile_near {
            notice(
                &mut commands,
                trigger.client_id,
                "Static and weapons lock—they aren't interested in talking.",
            );
        } else {
            notice(
                &mut commands,
                trigger.client_id,
                "No friendly vessel within hailing range.",
            );
        }
        return;
    };

    let ship_def = content.0.ship(&npc_info.def_id);
    let race = ship_def.map_or("terran", |d| d.race.as_str());
    let portrait = ship_def.and_then(|d| d.sprite.clone());
    let greeting = race_greeting(race);
    let speaker = format!("{} ({})", npc_info.name, race);

    let (encounter, pending) = match npc_brain {
        Some(NpcBrain::Merchant {
            commodity,
            qty,
            target_system,
            ..
        }) => {
            let offer_qty = (*qty).clamp(1, OFFER_QTY);
            let cost = offer_qty * 12;
            (
                ChiefEncounter {
                    chief_name: speaker.clone(),
                    portrait,
                    mood: ChiefMood::Offer,
                    text: format!(
                        "{greeting}\n\n\"We're hauling {qty} tons of {commodity} toward {target_system}. We can transfer {offer_qty} {commodity} for {cost} cr if your hold has room.\""
                    ),
                    options: vec![
                        EncounterOption {
                            id: "accept_offer".to_owned(),
                            label: format!("Buy {offer_qty} {commodity} ({cost} cr)"),
                        },
                        EncounterOption {
                            id: "decline".to_owned(),
                            label: "Wish safe passage".to_owned(),
                        },
                    ],
                },
                PendingEncounter::Offer {
                    chief_name: speaker.clone(),
                    commodity: commodity.clone(),
                    qty: offer_qty,
                    cost,
                },
            )
        }
        Some(NpcBrain::Explorer {
            target_system,
            visited,
            ..
        }) => {
            let charted = visited.len();
            (
                ChiefEncounter {
                    chief_name: speaker.clone(),
                    portrait,
                    mood: ChiefMood::Offer,
                    text: format!(
                        "{greeting}\n\n\"Our survey array has charted {charted} systems so far; next vector is {target_system}. Take a spare fuel cell for the hyperlanes.\""
                    ),
                    options: vec![
                        EncounterOption {
                            id: "accept_hail_gift".to_owned(),
                            label: "Receive telemetry & fuel cell (+25 fuel)".to_owned(),
                        },
                        EncounterOption {
                            id: "decline".to_owned(),
                            label: "Clear comms".to_owned(),
                        },
                    ],
                },
                PendingEncounter::ShipHail {
                    ship_name: speaker.clone(),
                    fuel_gift: 25.0,
                },
            )
        }
        _ => (
            ChiefEncounter {
                chief_name: speaker.clone(),
                portrait,
                mood: ChiefMood::Offer,
                text: format!(
                    "{greeting}\n\n\"Patrol sweeps are active across this sector. Keep your weapons holstered near civilian hulls—here's a reserve fuel transfer.\""
                ),
                options: vec![
                    EncounterOption {
                        id: "accept_hail_gift".to_owned(),
                        label: "Sync patrol telemetry (+15 fuel)".to_owned(),
                    },
                    EncounterOption {
                        id: "decline".to_owned(),
                        label: "Close channel".to_owned(),
                    },
                ],
            },
            PendingEncounter::ShipHail {
                ship_name: speaker.clone(),
                fuel_gift: 15.0,
            },
        ),
    };

    info!("server: owner={owner} hailed '{speaker}'");
    notice(
        &mut commands,
        trigger.client_id,
        format!("Opened comms channel with {speaker}."),
    );
    commands.entity(player_entity).insert((encounter, pending));
}

type EncounterShips<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Owner,
        Option<&'static DockedAt>,
        &'static ShipSpec,
        &'static ChiefEncounter,
        &'static PendingEncounter,
        &'static mut Ship,
        &'static mut Wallet,
        &'static mut ActiveQuests,
        &'static mut PlayerReputation,
    ),
>;

fn on_respond_to_encounter(
    trigger: On<FromClient<RespondToEncounter>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    board: Res<contracts::ContractBoard>,
    mut faction_state: ResMut<factions::FactionState>,
    mut ships: EncounterShips,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let raw_choice = trigger.event().choice.trim();
    let Some((
        ship_entity,
        _,
        docked,
        spec,
        encounter,
        pending,
        mut ship,
        mut wallet,
        mut active_quests,
        mut reputation,
    )) = ships
        .iter_mut()
        .find(|(_, ship_owner, _, _, _, _, _, _, _, _)| ship_owner.0 == owner)
    else {
        return;
    };

    let resolved_choice = if let Ok(index) = raw_choice.parse::<usize>()
        && index >= 1
        && index <= encounter.options.len()
    {
        encounter.options[index - 1].id.as_str()
    } else {
        raw_choice
    };
    let location = docked.map_or_else(
        || "open-space".to_owned(),
        |d| format!("{}/{}", d.system, d.location),
    );
    info!(
        "server: owner={owner} responded to encounter at {location} with choice='{resolved_choice}' (raw='{raw_choice}')"
    );

    match pending.clone() {
        PendingEncounter::Offer {
            chief_name,
            commodity,
            qty,
            cost,
        } => match resolved_choice {
            "accept_offer" => {
                if wallet.0.credits < cost {
                    notice(&mut commands, trigger.client_id, "You can't afford that.");
                    return;
                }
                if !wallet.0.add_cargo(&commodity, qty, spec.cargo_capacity) {
                    notice(&mut commands, trigger.client_id, "Your cargo hold is full.");
                    return;
                }
                wallet.0.credits -= cost;
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("Bought {qty} {commodity} from {chief_name} for {cost} cr."),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            "decline" => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("You pass on {chief_name}'s offer."),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            _ => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    "That choice isn't on the table.",
                );
            }
        },
        PendingEncounter::ShipHail {
            ship_name,
            fuel_gift,
        } => match resolved_choice {
            "accept_hail_gift" => {
                ship.0.fuel = (ship.0.fuel + fuel_gift).min(ship.0.max_fuel);
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("Received telemetry and +{fuel_gift:.0} fuel from {ship_name}."),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            "decline" => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("Closed comms channel with {ship_name}."),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            _ => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    "That choice isn't on the table.",
                );
            }
        },
        PendingEncounter::Tribute { chief_name, amount } => match resolved_choice {
            "pay_tribute" => {
                if wallet.0.credits < amount {
                    notice(
                        &mut commands,
                        trigger.client_id,
                        "You don't have the credits to cover the tribute.",
                    );
                    return;
                }
                wallet.0.credits -= amount;
                if let Some(docked) = docked.as_ref() {
                    let faction = content
                        .0
                        .system(&docked.system)
                        .map(|system| system.government.clone())
                        .unwrap_or_default();
                    factions::apply_rep(
                        &mut reputation.standings,
                        &faction,
                        factions::REP_TRIBUTE_PAID,
                    );
                }
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("Paid {amount} cr tribute to {chief_name}."),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            "refuse_tribute" => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!(
                        "{chief_name} marks your hull registry. No fine today, but the port remembers."
                    ),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            _ => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    "That choice isn't on the table.",
                );
            }
        },
        PendingEncounter::Quest {
            chief_name,
            quest_id,
        } => match resolved_choice {
            "accept_quest" => {
                let authored = content.0.quest(&quest_id);
                let procedural = authored.is_none();
                let Some(quest_def) = authored.or_else(|| {
                    board
                        .contracts
                        .iter()
                        .find(|contract| contract.id == *quest_id)
                }) else {
                    return;
                };
                let target = match &quest_def.kind {
                    QuestKind::Delivery {
                        commodity,
                        qty,
                        destination,
                        ..
                    } => {
                        if !wallet.0.add_cargo(commodity, *qty, spec.cargo_capacity) {
                            notice(
                                &mut commands,
                                trigger.client_id,
                                "Your cargo hold is too full for the contract parcel.",
                            );
                            return;
                        }
                        QuestTarget::Delivery {
                            commodity: commodity.clone(),
                            qty: *qty,
                            destination_system: destination.0.clone(),
                            destination_planet: destination.1.clone(),
                        }
                    }
                    QuestKind::Bounty { system, target, .. } => QuestTarget::Bounty {
                        system: system.clone(),
                        target: target.clone(),
                        killed: false,
                    },
                };
                active_quests.quests.push(ActiveQuest {
                    id: quest_def.id.clone(),
                    name: quest_def.name.clone(),
                    giver_system: quest_def.giver.0.clone(),
                    giver_planet: quest_def.giver.1.clone(),
                    target,
                    reward_credits: quest_def.reward_credits,
                    remaining_secs: quest_def.kind.deadline_secs().map(|s| s as f32),
                    procedural,
                });
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("Accepted contract: {}.", quest_def.name),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            "decline" => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!("You decline {chief_name}'s contract."),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            _ => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    "That choice isn't on the table.",
                );
            }
        },
        PendingEncounter::TurnIn { quest_id } => match resolved_choice {
            "turn_in" => {
                let Some(pos) = active_quests.quests.iter().position(|q| q.id == quest_id) else {
                    return;
                };
                let quest = active_quests.quests[pos].clone();
                if let QuestTarget::Delivery { commodity, qty, .. } = &quest.target
                    && !wallet.0.remove_cargo(commodity, *qty)
                {
                    notice(
                        &mut commands,
                        trigger.client_id,
                        "You are missing the contract cargo.",
                    );
                    return;
                }
                active_quests.quests.remove(pos);
                if !quest.procedural && !active_quests.completed.contains(&quest.id) {
                    active_quests.completed.push(quest.id.clone());
                }
                wallet.0.credits += quest.reward_credits;
                let giver_faction = content
                    .0
                    .system(&quest.giver_system)
                    .map(|system| system.government.clone())
                    .unwrap_or_default();
                factions::apply_rep(
                    &mut reputation.standings,
                    &giver_faction,
                    factions::REP_QUEST_COMPLETE,
                );
                let control_delta = factions::CONTROL_QUEST_COMPLETE
                    * factions::side_sign(&content.0, &giver_faction);
                if control_delta != 0 {
                    faction_state.shift_meter(&content.0, &quest.giver_system, control_delta);
                }
                notice(
                    &mut commands,
                    trigger.client_id,
                    format!(
                        "Completed '{}': received {} cr.",
                        quest.name, quest.reward_credits
                    ),
                );
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            "decline" => {
                commands
                    .entity(ship_entity)
                    .remove::<(ChiefEncounter, PendingEncounter)>();
            }
            _ => {
                notice(
                    &mut commands,
                    trigger.client_id,
                    "That choice isn't on the table.",
                );
            }
        },
    }
}

fn listing(pack: &ContentPack, system: &str, location: &str, commodity: &str) -> bool {
    pack.dockable(system, location)
        .is_some_and(|port| port.commodities().iter().any(|entry| entry.id == commodity))
}

fn on_buy_commodity(
    trigger: On<FromClient<BuyCommodity>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    mut market: ResMut<market::MarketState>,
    mut ships: Query<(&Owner, &DockedAt, &ShipSpec, &mut Wallet, &mut Ship)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let event = trigger.event();
    let Some((_, docked, spec, mut wallet, _)) = ships
        .iter_mut()
        .find(|(ship_owner, _, _, _, _)| ship_owner.0 == owner)
    else {
        return;
    };
    let Some(price) = market
        .price(
            &content.0,
            &docked.system,
            &docked.location,
            &event.commodity,
        )
        .filter(|_| {
            listing(
                &content.0,
                &docked.system,
                &docked.location,
                &event.commodity,
            )
        })
    else {
        notice(
            &mut commands,
            trigger.client_id,
            format!("The market master has never heard of {}.", event.commodity),
        );
        return;
    };
    if event.qty == 0 {
        notice(
            &mut commands,
            trigger.client_id,
            "The broker stares at your empty order.",
        );
        return;
    };
    let Some(total) = price.checked_mul(event.qty) else {
        notice(
            &mut commands,
            trigger.client_id,
            "Nobody has that many credits.",
        );
        return;
    };
    if wallet.0.credits < total {
        notice(&mut commands, trigger.client_id, "You can't afford that.");
        return;
    }
    if !wallet
        .0
        .add_cargo(&event.commodity, event.qty, spec.cargo_capacity)
    {
        notice(&mut commands, trigger.client_id, "Your cargo hold is full.");
        return;
    }
    wallet.0.credits -= total;
    market.shift_supply(
        market::MarketKey {
            system: docked.system.clone(),
            port: docked.location.clone(),
            commodity: event.commodity.clone(),
        },
        -(event.qty as i32),
        event.qty,
    );
    notice(
        &mut commands,
        trigger.client_id,
        format!("Bought {} {} for {total} cr.", event.qty, event.commodity),
    );
}

fn on_sell_commodity(
    trigger: On<FromClient<SellCommodity>>,
    mut commands: Commands,
    clients: Query<&NetworkId>,
    content: Res<Content>,
    mut market: ResMut<market::MarketState>,
    mut ships: Query<(&Owner, &DockedAt, &mut Wallet)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let event = trigger.event();
    let Some((_, docked, mut wallet)) = ships
        .iter_mut()
        .find(|(ship_owner, _, _)| ship_owner.0 == owner)
    else {
        return;
    };
    let Some(price) = market
        .price(
            &content.0,
            &docked.system,
            &docked.location,
            &event.commodity,
        )
        .filter(|_| {
            listing(
                &content.0,
                &docked.system,
                &docked.location,
                &event.commodity,
            )
        })
    else {
        notice(
            &mut commands,
            trigger.client_id,
            format!("The market master has never heard of {}.", event.commodity),
        );
        return;
    };
    if event.qty == 0 {
        notice(
            &mut commands,
            trigger.client_id,
            "The broker stares at your empty order.",
        );
        return;
    }
    if !wallet.0.remove_cargo(&event.commodity, event.qty) {
        notice(
            &mut commands,
            trigger.client_id,
            format!("You don't have {} {}.", event.qty, event.commodity),
        );
        return;
    }
    let Some(total) = price.checked_mul(event.qty) else {
        notice(
            &mut commands,
            trigger.client_id,
            "Nobody has that many credits.",
        );
        return;
    };
    wallet.0.credits += total;
    market.shift_supply(
        market::MarketKey {
            system: docked.system.clone(),
            port: docked.location.clone(),
            commodity: event.commodity.clone(),
        },
        event.qty as i32,
        event.qty,
    );
    notice(
        &mut commands,
        trigger.client_id,
        format!("Sold {} {} for {total} cr.", event.qty, event.commodity),
    );
}

fn on_set_warp(
    trigger: On<FromClient<SetWarp>>,
    clients: Query<&NetworkId>,
    mut ships: Query<(&Owner, &mut Control)>,
) {
    let owner = raw_id(trigger.client_id, &clients);
    let warp = trigger.event().0;
    debug!("server: owner={owner} set warp={warp}");
    for (ship_owner, mut control) in &mut ships {
        if ship_owner.0 == owner {
            control.warp = warp;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use bevy::app::PostUpdate;
    use bevy::math::Vec2;
    use bevy::state::app::StatesPlugin;
    use bevy_replicon_renet2::netcode::{
        ClientAuthentication, NativeSocket, NetcodeClientTransport,
    };
    use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetClient};
    use bevy_replicon_renet2::{RenetChannelsExt, RepliconRenetPlugins};
    use sv_protocol::AccountState;

    use super::*;
    use crate::storage::Storage;
    use crate::{
        EMBEDDED_CLIENT_ID, PROTOCOL_ID, TickMode, attach_memory, attach_udp, build_server_app,
    };

    mod contracts;
    mod factions;
    mod market;
    mod progression;
    mod stations;

    struct Harness {
        server: App,
        client: App,
        steps: usize,
    }

    impl Harness {
        fn new() -> Self {
            let content_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
            let mut server = build_server_app(content_root, TickMode::PerUpdate, None)
                .expect("server app builds");
            let (transport, _socket) = attach_memory(&mut server);
            server.finish();

            let mut client = App::new();
            client.add_plugins(MinimalPlugins);
            client.add_plugins(StatesPlugin);
            client.add_plugins(RepliconPlugins.set(ServerPlugin::new(PostUpdate)));
            client.add_plugins(RepliconRenetPlugins);
            sv_protocol::register(&mut client);
            client.init_resource::<TestNoticeLog>();
            client.add_observer(record_test_notice);

            let (server_channels, client_channels) = {
                let channels = client.world().resource::<RepliconChannels>();
                (channels.server_configs(), channels.client_configs())
            };
            let renet_client = RenetClient::new(
                ConnectionConfig::from_channels(server_channels, client_channels),
                false,
            );
            client.insert_resource(renet_client);
            client.insert_resource(transport);
            client.finish();

            let mut harness = Self {
                server,
                client,
                steps: 0,
            };
            while !harness
                .client
                .world()
                .resource::<RenetClient>()
                .is_connected()
            {
                harness.step();
                assert!(harness.steps < 300, "client must connect");
            }
            harness
        }

        fn step(&mut self) {
            self.server.update();
            self.client.update();
            std::thread::sleep(Duration::from_millis(15));
            self.steps += 1;
        }

        fn step_until(&mut self, max_steps: usize, mut cond: impl FnMut(&mut Self) -> bool) {
            let start = self.steps;
            while !cond(self) {
                assert!(
                    self.steps - start < max_steps,
                    "condition not met within {max_steps} steps"
                );
                self.step();
            }
        }

        fn my_ship(&mut self) -> Option<Entity> {
            let mut state = self
                .client
                .world_mut()
                .query_filtered::<(Entity, &ShipInfo), With<Remote>>();
            let world = self.client.world();
            state
                .iter(world)
                .find(|(_, info)| info.owner == Some(EMBEDDED_CLIENT_ID as u64))
                .map(|(entity, _)| entity)
        }

        fn pirate_ship(&mut self) -> Option<Entity> {
            let mut state = self
                .client
                .world_mut()
                .query_filtered::<(Entity, &ShipInfo), With<Remote>>();
            let world = self.client.world();
            state
                .iter(world)
                .find(|(_, info)| info.kind == ShipKind::Pirate)
                .map(|(entity, _)| entity)
        }

        fn client_projectile_count(&mut self) -> usize {
            let mut state = self
                .client
                .world_mut()
                .query_filtered::<(), (With<Remote>, With<Projectile>)>();
            let world = self.client.world();
            state.iter(world).count()
        }

        fn client_component<T: Component + Clone>(&mut self, entity: Entity) -> Option<T> {
            self.client.world().get::<T>(entity).cloned()
        }

        fn server_ship_position(&mut self, owner: u64) -> Option<Vec2> {
            let mut state = self.server.world_mut().query::<(&Owner, &Ship)>();
            let world = self.server.world();
            state
                .iter(world)
                .find(|(o, _)| o.0 == owner)
                .map(|(_, ship)| ship.0.position)
        }

        fn set_server_ship(&mut self, owner: u64, mutate: impl Fn(&mut ShipState)) {
            let mut state = self.server.world_mut().query::<(&Owner, &mut Ship)>();
            let world = self.server.world_mut();
            for (o, mut ship) in state.iter_mut(world) {
                if o.0 == owner {
                    mutate(&mut ship.0);
                }
            }
        }

        fn send<E: Event>(&mut self, event: E) {
            self.client.world_mut().client_trigger(event);
        }
    }

    #[test]
    fn continuous_world_flight_and_combat() {
        let mut h = Harness::new();

        h.step_until(400, |h| {
            let mut planets = h
                .client
                .world_mut()
                .query_filtered::<(), (With<Remote>, With<PlanetTag>)>();
            let world = h.client.world();
            planets.iter(world).count() >= 1 && h.my_ship().is_some()
        });

        let my_ship = h.my_ship().expect("my ship must replicate");

        let start = h.server_ship_position(1).expect("player exists");
        h.send(SetThrottle(1.0));
        h.step_until(200, |h| {
            h.server_ship_position(1)
                .is_some_and(|p| p.x > start.x + 40.0)
        });
        h.send(SetThrottle(0.0));

        h.set_server_ship(1, |ship| {
            ship.rotation = 0.0;
            ship.position = Vec2::new(480.0, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        for _ in 0..5 {
            h.step();
        }
        h.send(RequestLand {
            planet: "earth".into(),
        });
        h.step_until(100, |h| h.client_component::<DockedAt>(my_ship).is_some());

        h.send(BuyCommodity {
            commodity: "food".into(),
            qty: 5,
        });
        h.step_until(100, |h| {
            h.client_component::<AccountState>(my_ship)
                .is_some_and(|a| a.credits == STARTING_CREDITS - 40)
        });

        h.send(RequestLiftOff);
        h.step_until(100, |h| h.client_component::<DockedAt>(my_ship).is_none());

        h.set_server_ship(1, |ship| {
            ship.rotation = 0.0;
            ship.position = Vec2::new(200.0, 300.0);
            ship.velocity = Vec2::ZERO;
        });
        h.set_server_ship(0, |ship| {
            ship.shields = 6.0;
            ship.armor = 4.0;
            ship.position = Vec2::new(260.0, 300.0);
            ship.velocity = Vec2::ZERO;
        });
        h.send(SetFiring(true));
        let mut saw_projectiles = false;
        h.step_until(300, |h| {
            if h.client_projectile_count() > 0 {
                saw_projectiles = true;
            }
            h.pirate_ship().is_none()
        });
        h.send(SetFiring(false));
        assert!(saw_projectiles, "client must see projectiles in flight");

        h.set_server_ship(1, |ship| {
            ship.shields = 0.0;
            ship.armor = 0.0;
        });
        h.step_until(100, |h| h.my_ship().is_none());
        h.step_until(500, |h| {
            h.my_ship()
                .is_some_and(|ship| h.client_component::<AccountState>(ship).is_some())
        });
        let respawned = h.my_ship().unwrap();
        let account = h.client_component::<AccountState>(respawned).unwrap();
        assert_eq!(account.credits, STARTING_CREDITS - 40);
        assert!(
            account.cargo.is_empty(),
            "cargo is lost on death, credits persist"
        );
    }

    #[test]
    fn login_and_persistence() {
        let storage_path =
            std::env::temp_dir().join(format!("sv-persist-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&storage_path);
        let content_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");

        let mut server = build_server_app(content_root.clone(), TickMode::PerUpdate, None)
            .expect("server with storage must build");
        let storage =
            crate::storage::SqliteStorage::open(&storage_path).expect("storage must open");
        server.insert_resource(StorageResource {
            storage: Some(Box::new(storage)),
        });
        let (transport, _socket) = attach_memory(&mut server);
        server.finish();

        let mut client = App::new();
        client.add_plugins(MinimalPlugins);
        client.add_plugins(StatesPlugin);
        client.add_plugins(RepliconPlugins.set(ServerPlugin::new(PostUpdate)));
        client.add_plugins(RepliconRenetPlugins);
        sv_protocol::register(&mut client);
        let (server_channels, client_channels) = {
            let channels = client.world().resource::<RepliconChannels>();
            (channels.server_configs(), channels.client_configs())
        };
        client.insert_resource(RenetClient::new(
            ConnectionConfig::from_channels(server_channels, client_channels),
            false,
        ));
        client.insert_resource(transport);
        client.finish();

        let step = |server: &mut App, client: &mut App| {
            client.update();
            server.update();
            std::thread::sleep(Duration::from_millis(15));
        };

        let mut connect_steps = 0;
        while !client.world().resource::<RenetClient>().is_connected() {
            step(&mut server, &mut client);
            connect_steps += 1;
            assert!(connect_steps < 400, "client must connect");
        }

        for _ in 0..10 {
            step(&mut server, &mut client);
        }

        client
            .world_mut()
            .client_trigger(sv_protocol::LoginRequest {
                username: "alice".to_owned(),
                password: "pass123".to_owned(),
            });

        let mut login_steps = 0;
        loop {
            step(&mut server, &mut client);
            login_steps += 1;
            assert!(login_steps < 400, "login must complete");

            let mut ships = server
                .world_mut()
                .query_filtered::<&Owner, With<PlayerShip>>();
            let world = server.world();
            if ships
                .iter(world)
                .any(|owner| owner.0 == EMBEDDED_CLIENT_ID as u64)
            {
                break;
            }
        }

        let mut accounts = server
            .world_mut()
            .query_filtered::<&Wallet, With<PlayerShip>>();
        let world = server.world();
        let account = accounts
            .iter(world)
            .next()
            .expect("player wallet must exist after login");
        assert_eq!(account.0.credits, STARTING_CREDITS);

        let mut ships = server.world_mut().query::<(&Owner, &mut Ship)>();
        let world = server.world_mut();
        for (owner, mut ship) in ships.iter_mut(world) {
            if owner.0 == EMBEDDED_CLIENT_ID as u64 {
                ship.0.position = Vec2::new(480.0, 0.0);
                ship.0.velocity = Vec2::ZERO;
            }
        }
        for _ in 0..5 {
            step(&mut server, &mut client);
        }

        client.world_mut().client_trigger(sv_protocol::RequestLand {
            planet: "earth".to_owned(),
        });
        let mut dock_steps = 0;
        loop {
            step(&mut server, &mut client);
            dock_steps += 1;
            assert!(dock_steps < 200, "must dock");

            let mut docked = client
                .world_mut()
                .query_filtered::<&DockedAt, With<Remote>>();
            let world = client.world();
            if docked.iter(world).count() > 0 {
                break;
            }
        }

        client
            .world_mut()
            .client_trigger(sv_protocol::BuyCommodity {
                commodity: "food".to_owned(),
                qty: 2,
            });
        for _ in 0..10 {
            step(&mut server, &mut client);
        }
        let mut accounts = client
            .world_mut()
            .query_filtered::<&AccountState, With<Remote>>();
        let world = client.world();
        let account = accounts
            .iter(world)
            .find(|_| true)
            .expect("account must exist");
        assert_eq!(account.credits, STARTING_CREDITS - 16);

        let save_timer = server.world_mut();
        save_timer.resource_mut::<SaveTimer>().0 = 999.0;
        step(&mut server, &mut client);

        let mut server2 = build_server_app(
            content_root,
            TickMode::PerUpdate,
            Some(storage_path.as_path()),
        )
        .expect("server2 must build");
        let (transport2, _socket2) = attach_memory(&mut server2);
        server2.finish();
        let _ = transport2;

        let storage_check =
            crate::storage::SqliteStorage::open(&storage_path).expect("storage must reopen");
        let loaded = storage_check
            .verify_login("alice", "pass123")
            .expect("verify")
            .expect("alice must exist after save");
        assert_eq!(loaded.credits, STARTING_CREDITS - 16);
        assert_eq!(loaded.cargo.len(), 1);
        assert_eq!(loaded.cargo[0].0, "food");
        assert_eq!(loaded.cargo[0].1, 2);

        let _ = std::fs::remove_file(&storage_path);
    }

    fn udp_client_app(client_id: u64, server_addr: std::net::SocketAddr) -> App {
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

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time must be after the epoch");
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("test socket must bind");
        let transport = NetcodeClientTransport::new(
            current_time,
            ClientAuthentication::Unsecure {
                client_id,
                protocol_id: PROTOCOL_ID,
                socket_id: 0,
                server_addr,
                user_data: None,
            },
            NativeSocket::new(socket).expect("test socket must be valid"),
        )
        .expect("test transport must be valid");
        app.insert_resource(transport);
        app.finish();
        app
    }

    fn remote_ship_ids(client: &mut App) -> Vec<u64> {
        let mut state = client
            .world_mut()
            .query_filtered::<&ShipInfo, With<Remote>>();
        let world = client.world();
        let mut owners: Vec<u64> = state.iter(world).filter_map(|info| info.owner).collect();
        owners.sort_unstable();
        owners
    }

    fn server_police_target(server: &mut App) -> Option<u64> {
        let mut state = server.world_mut().query::<&Aggro>();
        let world = server.world();
        state.iter(world).next().map(|aggro| aggro.0)
    }

    fn server_credits(server: &mut App, owner: u64) -> Option<u32> {
        let mut state = server.world_mut().query::<(&Owner, &Wallet)>();
        let world = server.world();
        state
            .iter(world)
            .find(|(wallet_owner, _)| wallet_owner.0 == owner)
            .map(|(_, wallet)| wallet.0.credits)
    }

    #[test]
    fn udp_two_players_pvp_summons_the_patrol() {
        let content_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let mut server = build_server_app(content_root, TickMode::PerUpdate, None)
            .expect("server app must build");
        let bound = attach_udp(
            &mut server,
            std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
            8,
        )
        .expect("udp socket must bind");
        server.finish();

        let mut alice = udp_client_app(101, bound);
        let mut bob = udp_client_app(202, bound);

        let step_all = |server: &mut App, alice: &mut App, bob: &mut App| {
            alice.update();
            bob.update();
            server.update();
            std::thread::sleep(Duration::from_millis(15));
        };

        let mut steps = 0;
        while !alice.world().resource::<RenetClient>().is_connected()
            || !bob.world().resource::<RenetClient>().is_connected()
        {
            step_all(&mut server, &mut alice, &mut bob);
            steps += 1;
            assert!(steps < 400, "both clients must connect");
        }

        while remote_ship_ids(&mut alice) != vec![101, 202]
            || remote_ship_ids(&mut bob) != vec![101, 202]
        {
            step_all(&mut server, &mut alice, &mut bob);
            steps += 1;
            assert!(steps < 800, "both players must replicate");
        }

        {
            let mut pirates = alice
                .world_mut()
                .query_filtered::<&ShipInfo, With<Remote>>();
            let world = alice.world();
            assert!(
                pirates
                    .iter(world)
                    .any(|info| info.kind == ShipKind::Pirate),
                "in the continuous world the alpha-centauri pirate is visible from sol"
            );
        }

        let set_ship = |server: &mut App, owner: u64, position: Vec2, rotation: f32| {
            let mut state = server.world_mut().query::<(&Owner, &mut Ship)>();
            let world = server.world_mut();
            for (ship_owner, mut ship) in state.iter_mut(world) {
                if ship_owner.0 == owner {
                    ship.0.position = position;
                    ship.0.velocity = Vec2::ZERO;
                    ship.0.rotation = rotation;
                }
            }
        };
        set_ship(&mut server, 101, Vec2::ZERO, 0.0);
        set_ship(&mut server, 202, Vec2::new(200.0, 0.0), 0.0);
        for _ in 0..5 {
            step_all(&mut server, &mut alice, &mut bob);
        }

        alice.world_mut().client_trigger(SetFiring(true));
        let mut fired = 0;
        while server_police_target(&mut server).is_none() {
            step_all(&mut server, &mut alice, &mut bob);
            fired += 1;
            assert!(fired < 400, "pvp offense must trigger the patrol");
        }
        alice.world_mut().client_trigger(SetFiring(false));

        assert_eq!(server_police_target(&mut server), Some(101));
        assert_eq!(
            server_credits(&mut server, 101),
            Some(STARTING_CREDITS - FINE_CREDITS),
            "the attacker is fined once per offense"
        );
        assert_eq!(
            server_credits(&mut server, 202),
            Some(STARTING_CREDITS),
            "the victim keeps their credits"
        );

        let mut police_seen = false;
        for _ in 0..100 {
            step_all(&mut server, &mut alice, &mut bob);
            let mut ships = alice
                .world_mut()
                .query_filtered::<&ShipInfo, With<Remote>>();
            let world = alice.world();
            police_seen =
                police_seen || ships.iter(world).any(|info| info.kind == ShipKind::Police);
        }
        assert!(police_seen, "the patrol must replicate to both players");
    }

    #[test]
    fn chief_encounter_and_delivery_quest_loop() {
        let mut h = Harness::new();

        h.step_until(400, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().expect("my ship must replicate");

        // Move near Earth and dock; default EncounterRng(4) rolls Quest (newhaven-relief).
        h.set_server_ship(1, |ship| {
            ship.position = Vec2::new(480.0, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        for _ in 0..5 {
            h.step();
        }
        h.send(RequestLand {
            planet: "earth".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_some()
        });
        let encounter = h.client_component::<ChiefEncounter>(my_ship).unwrap();
        assert_eq!(encounter.chief_name, "Port Director Lin");
        assert_eq!(encounter.mood, ChiefMood::Quest);

        // Accept the delivery quest; 4 medicine courier cargo is loaded automatically.
        h.send(RespondToEncounter {
            choice: "accept_quest".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_none()
                && h.client_component::<ActiveQuests>(my_ship)
                    .is_some_and(|q| q.quests.len() == 1)
        });
        let quests = h.client_component::<ActiveQuests>(my_ship).unwrap();
        assert_eq!(quests.quests[0].id, "newhaven-relief");
        let account = h.client_component::<AccountState>(my_ship).unwrap();
        assert_eq!(account.cargo, vec![("medicine".to_owned(), 4)]);

        // Lift off and fly to Alpha Centauri / Newhaven (planet at 2500.0, 0.0).
        h.send(RequestLiftOff);
        h.step_until(100, |h| h.client_component::<DockedAt>(my_ship).is_none());

        h.set_server_ship(1, |ship| {
            ship.position = Vec2::new(2480.0, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        for _ in 0..5 {
            h.step();
        }
        h.send(RequestLand {
            planet: "newhaven".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_some()
        });
        let turn_in = h.client_component::<ChiefEncounter>(my_ship).unwrap();
        assert_eq!(turn_in.chief_name, "Magistrate Kael");
        assert_eq!(turn_in.mood, ChiefMood::TurnIn);

        // Turn in the quest for 450 cr.
        h.send(RespondToEncounter {
            choice: "turn_in".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_none()
                && h.client_component::<ActiveQuests>(my_ship)
                    .is_some_and(|q| {
                        q.quests.is_empty() && q.completed.contains(&"newhaven-relief".to_owned())
                    })
        });
        let final_account = h.client_component::<AccountState>(my_ship).unwrap();
        assert_eq!(final_account.credits, STARTING_CREDITS + 450);
        assert!(final_account.cargo.is_empty());
    }

    #[test]
    fn chief_offer_and_tribute_moods() {
        let mut h = Harness::new();
        h.step_until(400, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().unwrap();

        // Roll 60 selects Offer on Earth.
        *h.server.world_mut().resource_mut::<EncounterRng>() = EncounterRng::with_roll(60);
        h.set_server_ship(1, |ship| {
            ship.position = Vec2::new(480.0, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        for _ in 0..5 {
            h.step();
        }
        h.send(RequestLand {
            planet: "earth".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_some()
        });
        let offer = h.client_component::<ChiefEncounter>(my_ship).unwrap();
        assert_eq!(offer.mood, ChiefMood::Offer);

        h.send(RespondToEncounter {
            choice: "accept_offer".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_none()
        });
        let after_offer = h.client_component::<AccountState>(my_ship).unwrap();
        assert!(!after_offer.cargo.is_empty());
        let credits_after_offer = after_offer.credits;

        h.send(RequestLiftOff);
        h.step_until(100, |h| h.client_component::<DockedAt>(my_ship).is_none());

        // Roll 90 selects Tribute on Earth.
        *h.server.world_mut().resource_mut::<EncounterRng>() = EncounterRng::with_roll(90);
        h.send(RequestLand {
            planet: "earth".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_some()
        });
        let tribute = h.client_component::<ChiefEncounter>(my_ship).unwrap();
        assert_eq!(tribute.mood, ChiefMood::Tribute);

        h.send(RespondToEncounter {
            choice: "pay_tribute".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_none()
        });
        let after_tribute = h.client_component::<AccountState>(my_ship).unwrap();
        assert_eq!(after_tribute.credits, credits_after_offer - TRIBUTE_CREDITS);
    }

    #[test]
    fn meteorite_stationary_vs_moving_spin_and_drift() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());

        // Clear default meteorites and spawn 1 stationary and 1 moving meteorite.
        let existing: Vec<Entity> = h
            .server
            .world_mut()
            .query_filtered::<Entity, With<Meteorite>>()
            .iter(h.server.world())
            .collect();
        for e in existing {
            h.server.world_mut().despawn(e);
        }

        let mut cmds = h.server.world_mut().commands();
        let stat_e = spawn_meteorite(
            &mut cmds,
            "sol",
            Vec2::ZERO,
            1200.0,
            Vec2::new(0.0, 400.0),
            Vec2::ZERO,
            MeteoriteSize::Large,
            2.0, // should be forced to 0.0 because velocity is zero
            0,
        );
        let mov_e = spawn_meteorite(
            &mut cmds,
            "sol",
            Vec2::ZERO,
            1200.0,
            Vec2::new(0.0, -400.0),
            Vec2::new(50.0, 0.0),
            MeteoriteSize::Medium,
            1.8,
            1,
        );
        h.server.world_mut().flush();

        for _ in 0..30 {
            h.step();
        }

        let stat_pos = h.server.world().get::<Position>(stat_e).unwrap().0;
        let stat_head = h.server.world().get::<Heading>(stat_e).unwrap().0;
        let stat_m = h.server.world().get::<Meteorite>(stat_e).unwrap();
        assert_eq!(stat_m.spin_rate, 0.0);
        assert_eq!(stat_pos, Vec2::new(0.0, 400.0));
        assert_eq!(stat_head, 0.0);

        let mov_pos = h.server.world().get::<Position>(mov_e).unwrap().0;
        let mov_head = h.server.world().get::<Heading>(mov_e).unwrap().0;
        assert!(
            mov_pos.x > 10.0,
            "moving meteorite must drift along velocity"
        );
        assert!(
            mov_head.abs() > 0.1,
            "moving meteorite must rotate on itself"
        );
    }

    #[test]
    fn meteorite_collision_damages_undocked_ship() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().unwrap();

        let initial_shields = h.client_component::<Hull>(my_ship).unwrap().shields;

        // Spawn a Large meteorite right on top of the player ship at (0, 0).
        let mut cmds = h.server.world_mut().commands();
        spawn_meteorite(
            &mut cmds,
            "sol",
            Vec2::ZERO,
            1200.0,
            Vec2::new(10.0, 0.0),
            Vec2::new(-30.0, 0.0),
            MeteoriteSize::Large,
            1.2,
            0,
        );
        h.server.world_mut().flush();

        h.step_until(60, |h| {
            h.client_component::<Hull>(my_ship)
                .is_some_and(|hull| hull.shields < initial_shields)
        });
        let hull_after = h.client_component::<Hull>(my_ship).unwrap();
        assert!(
            hull_after.shields < initial_shields,
            "crashing into a meteorite must damage the ship"
        );
    }

    #[test]
    fn meteorite_recursive_binary_fragmentation_until_disappear() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());

        // Clear all default meteorites so we can track the exact 1 -> 2 -> 4 -> 0 chain.
        let existing: Vec<Entity> = h
            .server
            .world_mut()
            .query_filtered::<Entity, With<Meteorite>>()
            .iter(h.server.world())
            .collect();
        for e in existing {
            h.server.world_mut().despawn(e);
        }

        let mut cmds = h.server.world_mut().commands();
        let root = spawn_meteorite(
            &mut cmds,
            "sol",
            Vec2::ZERO,
            1200.0,
            Vec2::new(90.0, 0.0),
            Vec2::ZERO,
            MeteoriteSize::Large,
            0.0,
            0,
        );
        h.server.world_mut().flush();

        // Aim player ship at the Large meteorite and fire until it splits into 2 Medium parts.
        h.send(SetFiring(true));
        h.step_until(200, |h| {
            let mut q = h.server.world_mut().query::<&Meteorite>();
            let world = h.server.world();
            q.iter(world).count() == 2 && q.iter(world).all(|m| m.size == MeteoriteSize::Medium)
        });
        assert!(
            h.server.world().get_entity(root).is_err(),
            "parent Large meteorite must be despawned"
        );

        // Move each Medium meteorite in front of the ship in turn and shoot until we have 4 Small parts.
        for _ in 0..2 {
            let med_entity = h
                .server
                .world_mut()
                .query::<(Entity, &Meteorite)>()
                .iter(h.server.world())
                .find(|(_, m)| m.size == MeteoriteSize::Medium)
                .map(|(e, _)| e)
                .expect("expected a Medium meteorite");
            h.server
                .world_mut()
                .get_mut::<Position>(med_entity)
                .unwrap()
                .0 = Vec2::new(85.0, 0.0);
            h.server
                .world_mut()
                .get_mut::<MeteoriteMotion>(med_entity)
                .unwrap()
                .velocity = Vec2::ZERO;

            h.step_until(200, |h| h.server.world().get_entity(med_entity).is_err());
        }

        let small_count = h
            .server
            .world_mut()
            .query::<&Meteorite>()
            .iter(h.server.world())
            .filter(|m| m.size == MeteoriteSize::Small)
            .count();
        assert_eq!(
            small_count, 4,
            "2 Medium meteorites must split into 4 Small parts"
        );

        // Shoot all 4 Small meteorites and verify they disappear completely (0 remaining).
        while let Some(small_entity) = h
            .server
            .world_mut()
            .query::<(Entity, &Meteorite)>()
            .iter(h.server.world())
            .find(|(_, m)| m.size == MeteoriteSize::Small)
            .map(|(e, _)| e)
        {
            h.server
                .world_mut()
                .get_mut::<Position>(small_entity)
                .unwrap()
                .0 = Vec2::new(80.0, 0.0);
            h.server
                .world_mut()
                .get_mut::<MeteoriteMotion>(small_entity)
                .unwrap()
                .velocity = Vec2::ZERO;
            h.step_until(200, |h| h.server.world().get_entity(small_entity).is_err());
        }

        let remaining = h
            .server
            .world_mut()
            .query::<&Meteorite>()
            .iter(h.server.world())
            .count();
        assert_eq!(
            remaining, 0,
            "Small meteorites must disappear when destroyed"
        );
    }

    #[derive(Resource, Default)]
    struct TestNoticeLog(Vec<String>);

    fn record_test_notice(trigger: On<Notice>, mut log: ResMut<TestNoticeLog>) {
        log.0.push(trigger.event().text.clone());
    }

    impl Harness {
        fn star_in_system(&mut self, system: &str) -> Option<(Vec2, f32)> {
            let mut q = self.server.world_mut().query::<(&StarTag, &Position)>();
            let world = self.server.world();
            q.iter(world)
                .find(|(star, _)| star.system == system)
                .map(|(star, pos)| (pos.0, star.radius))
        }

        fn drain_notices(&mut self) -> Vec<String> {
            std::mem::take(&mut self.client.world_mut().resource_mut::<TestNoticeLog>().0)
        }

        fn server_hull_by_owner(&mut self, owner: u64) -> Option<(f32, f32)> {
            let mut q = self.server.world_mut().query::<(&Owner, &Hull)>();
            let world = self.server.world();
            q.iter(world)
                .find(|(o, _)| o.0 == owner)
                .map(|(_, hull)| (hull.shields, hull.armor))
        }
    }

    #[test]
    fn star_replicates_to_client() {
        let mut h = Harness::new();
        h.step_until(300, |h| {
            let mut stars = h
                .client
                .world_mut()
                .query_filtered::<(&StarTag,), With<Remote>>();
            let world = h.client.world();
            stars.iter(world).count() >= 4
        });
        let sol_star = h
            .client
            .world_mut()
            .query_filtered::<(&StarTag,), With<Remote>>()
            .iter(h.client.world())
            .find(|(star,)| star.system == "sol")
            .map(|(star,)| star.clone());
        let sol_star = sol_star.expect("sol star must replicate");
        assert_eq!(sol_star.radius, 240.0);
    }

    fn black_hole_test_app(seed: u32) -> (App, Entity, Vec2) {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let pack = ContentPack::load_dir(&root).unwrap();
        let system = pack.system("cygnus-x1").unwrap();
        let hole = system.black_hole.as_ref().unwrap();
        let center = Vec2::from(system.position) + Vec2::from(hole.position);
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_millis(100));
        app.insert_resource(time)
            .insert_resource(EncounterRng::with_roll(seed))
            .add_systems(
                Update,
                (black_hole_gravity, update_system_membership).chain(),
            );
        app.world_mut().spawn((
            BlackHoleTag {
                system: system.id.clone(),
                radius: hole.radius,
                influence: hole.influence,
            },
            BlackHoleGravity(hole.pull_strength),
            Position(center),
        ));
        let mut state = ShipState::new(40.0, 25.0, 73.0);
        state.position = center;
        let ship = app
            .world_mut()
            .spawn((
                Ship(state),
                SystemMembership("sol".into()),
                // Deliberately stale replicated position: membership must use Ship.
                Position(Vec2::ZERO),
            ))
            .id();
        app.insert_resource(Content(pack));
        (app, ship, center)
    }

    #[test]
    fn black_hole_pulls_player_and_npc_spatially_without_consuming_rng() {
        let (mut app, ship, center) = black_hole_test_app(1);
        let initial_rng = app.world().resource::<EncounterRng>().0;
        app.world_mut().get_mut::<Ship>(ship).unwrap().0.position = center + Vec2::X * 300.0;
        let mut npc_state = ShipState::new(40.0, 25.0, 50.0);
        npc_state.position = center + Vec2::Y * 300.0;
        let npc = app.world_mut().spawn((Ship(npc_state), Npc)).id();
        app.update();
        let player_velocity = app.world().get::<Ship>(ship).unwrap().0.velocity;
        let npc_velocity = app.world().get::<Ship>(npc).unwrap().0.velocity;
        assert!(player_velocity.x < 0.0);
        assert_eq!(player_velocity.y, 0.0);
        assert!(npc_velocity.y < 0.0);
        assert_eq!(npc_velocity.x, 0.0);
        assert_eq!(app.world().resource::<EncounterRng>().0, initial_rng);
    }

    #[test]
    fn black_hole_outside_influence_is_safe() {
        let (mut app, ship, center) = black_hole_test_app(0);
        app.world_mut().get_mut::<Ship>(ship).unwrap().0.position = center + Vec2::X * 501.0;
        let initial_rng = app.world().resource::<EncounterRng>().0;
        app.update();
        let state = &app.world().get::<Ship>(ship).unwrap().0;
        assert_eq!(state.velocity, Vec2::ZERO);
        assert_eq!((state.shields, state.armor), (40.0, 25.0));
        assert_eq!(app.world().resource::<EncounterRng>().0, initial_rng);
    }

    #[test]
    fn black_hole_consumes_a_grazing_segment_with_both_endpoints_outside() {
        let (mut app, ship, center) = black_hole_test_app(0);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(250));
        app.add_systems(Update, apply_ship_control.before(black_hole_gravity));
        app.world_mut().entity_mut(ship).insert((
            Control {
                warp: true,
                ..default()
            },
            ShipSpec {
                flight: FlightModel {
                    turn_rate: 2.6,
                    acceleration: 95.0,
                    max_speed: 180.0,
                    drag: DRAG,
                },
                cargo_capacity: 20,
            },
        ));
        // Actual warp integration runs from (-55, 80) to (80, 80).
        // Both endpoints are outside radius 90, but the segment crosses it.
        {
            let mut state = app.world_mut().get_mut::<Ship>(ship).unwrap();
            state.0.position = center + Vec2::new(-55.0, 80.0);
            state.0.velocity = Vec2::new(540.0, 0.0);
        }
        app.update();
        let state = &app.world().get::<Ship>(ship).unwrap().0;
        assert!((state.position - center - Vec2::new(80.0, 80.0)).length() < 0.01);
        assert!(state.is_destroyed());
    }

    #[test]
    fn black_hole_seeded_teleport_preserves_state_and_updates_membership() {
        let (mut app, ship, _) = black_hole_test_app(1);
        let mut account = Account::new(987);
        account.add_cargo("ore", 3, 20);
        app.world_mut().entity_mut(ship).insert((
            Wallet(account),
            ActiveQuests {
                quests: vec![],
                completed: vec!["newhaven-relief".into()],
            },
        ));
        {
            let mut state = app.world_mut().get_mut::<Ship>(ship).unwrap();
            state.0.velocity = Vec2::new(90.0, -50.0);
            state.0.shields = 31.0;
            state.0.armor = 19.0;
        }
        let destination = app
            .world()
            .resource::<Content>()
            .0
            .system("tarsis")
            .unwrap()
            .position;
        let mut expected_rng = EncounterRng::with_roll(1);
        assert_eq!(expected_rng.next_u32(2), 1);
        expected_rng.next_u32(14);
        app.update();
        let state = &app.world().get::<Ship>(ship).unwrap().0;
        assert_eq!(state.position, Vec2::from(destination));
        assert_eq!(state.velocity, Vec2::ZERO);
        assert_eq!(state.fuel, 73.0);
        assert_eq!((state.shields, state.armor), (31.0, 19.0));
        assert_eq!(
            app.world().get::<SystemMembership>(ship).unwrap().0,
            "tarsis"
        );
        let wallet = &app.world().get::<Wallet>(ship).unwrap().0;
        assert_eq!(wallet.credits, 987);
        assert_eq!(wallet.cargo_qty("ore"), 3);
        assert_eq!(
            app.world().get::<ActiveQuests>(ship).unwrap().completed,
            ["newhaven-relief"]
        );
        assert_eq!(app.world().resource::<EncounterRng>().0, expected_rng.0);
        app.update();
        assert_eq!(app.world().resource::<EncounterRng>().0, expected_rng.0);
    }

    #[test]
    fn black_hole_lethal_branch_does_not_reroll_for_dead_ships() {
        let (mut app, ship, _) = black_hole_test_app(0);
        app.world_mut().entity_mut(ship).insert(Npc);
        let mut expected_rng = EncounterRng::with_roll(0);
        assert_eq!(expected_rng.next_u32(2), 0);
        app.update();
        assert!(app.world().get::<Ship>(ship).unwrap().0.is_destroyed());
        assert_eq!(app.world().resource::<EncounterRng>().0, expected_rng.0);
        app.update();
        assert_eq!(app.world().resource::<EncounterRng>().0, expected_rng.0);
    }

    #[test]
    fn black_hole_docked_ships_ignore_pull_and_horizon_for_both_seeds() {
        for seed in [0, 1] {
            let (mut app, ship, center) = black_hole_test_app(seed);
            app.world_mut().entity_mut(ship).insert(DockedAt {
                system: "sol".into(),
                location: "earth".into(),
                kind: DockableKind::Planet,
            });
            let initial_rng = app.world().resource::<EncounterRng>().0;
            for position in [center + Vec2::X * 300.0, center] {
                app.world_mut().get_mut::<Ship>(ship).unwrap().0.position = position;
                app.update();
                let state = &app.world().get::<Ship>(ship).unwrap().0;
                assert_eq!(state.position, position);
                assert_eq!(state.velocity, Vec2::ZERO);
                assert_eq!((state.shields, state.armor, state.fuel), (40.0, 25.0, 73.0));
                assert_eq!(app.world().resource::<EncounterRng>().0, initial_rng);
            }
        }
    }

    #[test]
    fn black_hole_replicates_and_teleport_notifies_client() {
        let mut h = Harness::new();
        h.step_until(300, |h| {
            h.my_ship().is_some()
                && h.client
                    .world_mut()
                    .query_filtered::<&BlackHoleTag, With<Remote>>()
                    .iter(h.client.world())
                    .count()
                    == 1
        });
        let hole = h
            .client
            .world_mut()
            .query::<&BlackHoleTag>()
            .iter(h.client.world())
            .next()
            .unwrap()
            .clone();
        assert_eq!((hole.radius, hole.influence), (90.0, 500.0));
        let center = h
            .server
            .world_mut()
            .query::<(&BlackHoleTag, &Position)>()
            .iter(h.server.world())
            .next()
            .unwrap()
            .1
            .0;
        h.server.insert_resource(EncounterRng::with_roll(1));
        h.set_server_ship(1, |ship| {
            ship.position = center;
            ship.velocity = Vec2::ZERO;
        });
        let client_ship = h.my_ship().unwrap();
        h.step_until(100, |h| {
            h.client_component::<SystemMembership>(client_ship)
                .is_some_and(|m| m.0 == "tarsis")
        });
        assert!(
            h.drain_notices()
                .iter()
                .any(|notice| notice.contains("black hole") && notice.contains("Tarsis"))
        );
    }

    #[test]
    fn black_hole_death_uses_existing_player_respawn_pipeline() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let center = h
            .server
            .world_mut()
            .query::<(&BlackHoleTag, &Position)>()
            .iter(h.server.world())
            .next()
            .unwrap()
            .1
            .0;
        h.server.insert_resource(EncounterRng::with_roll(0));
        h.set_server_ship(1, |ship| {
            ship.position = center;
            ship.velocity = Vec2::ZERO;
        });
        h.step_until(100, |h| h.my_ship().is_none());
        h.step_until(300, |h| h.my_ship().is_some());
        assert!(h.server_ship_position(1).unwrap().length() < 1.0);
    }

    #[test]
    fn black_hole_teleports_an_npc_with_a_brain_and_updates_membership() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let center = h
            .server
            .world_mut()
            .query::<(&BlackHoleTag, &Position)>()
            .iter(h.server.world())
            .next()
            .unwrap()
            .1
            .0;
        let npc = h
            .server
            .world_mut()
            .resource_scope(|world, content: Mut<Content>| {
                spawn_npc_role(
                    &mut world.commands(),
                    &content.0,
                    "terran-courier",
                    ShipKind::Merchant,
                    99_002,
                    "cygnus-x1",
                    Vec2::new(0.0, -1900.0),
                    NpcBrain::Merchant {
                        commodity: "ore".into(),
                        qty: 3,
                        target_system: "cygnus-x1".into(),
                        target_planet: "kestrel-drift".into(),
                        target_pos: center + Vec2::new(500.0, 1900.0),
                        dwell_timer: 1.5,
                        route_step: 0,
                    },
                )
            });
        h.server.world_mut().flush();
        h.server.insert_resource(EncounterRng::with_roll(1));
        h.step();
        assert_eq!(
            h.server.world().get::<SystemMembership>(npc).unwrap().0,
            "tarsis"
        );
        let destination = h
            .server
            .world()
            .resource::<Content>()
            .0
            .system("tarsis")
            .unwrap()
            .position;
        let state = &h.server.world().get::<Ship>(npc).unwrap().0;
        assert_eq!(state.position, Vec2::from(destination));
        assert_eq!(state.velocity, Vec2::ZERO);
        assert!(h.server.world().get::<NpcBrain>(npc).is_some());
    }

    #[test]
    fn star_heat_is_zero_at_safe_distance() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().unwrap();
        let initial = h.client_component::<Hull>(my_ship).unwrap();

        for _ in 0..120 {
            h.step();
        }

        let after = h.client_component::<Hull>(my_ship).unwrap();
        assert_eq!(
            (after.shields, after.armor),
            (initial.shields, initial.armor),
            "spawn point and planet lanes must sit outside every star's heat zone"
        );
    }

    #[test]
    fn star_heat_ramps_and_warns_near_corona() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().unwrap();
        let initial = h.client_component::<Hull>(my_ship).unwrap();
        let (star_pos, star_radius) = h.star_in_system("sol").expect("sol has a star");

        h.set_server_ship(EMBEDDED_CLIENT_ID as u64, |ship| {
            ship.position = star_pos + Vec2::new(1.3 * star_radius, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        h.step_until(60, |h| {
            h.client_component::<Hull>(my_ship)
                .is_some_and(|hull| hull.shields < initial.shields)
        });

        let after = h.client_component::<Hull>(my_ship).unwrap();
        assert!(
            after.shields < initial.shields,
            "the corona ramp must chew shields before armor"
        );
        let notices = h.drain_notices();
        assert!(
            notices.iter().any(|n| n.contains("corona")),
            "entering the heat zone must warn the player, got {notices:?}"
        );
    }

    #[test]
    fn star_heat_lethal_core_destroys_player() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let (star_pos, star_radius) = h.star_in_system("sol").expect("sol has a star");

        h.set_server_ship(EMBEDDED_CLIENT_ID as u64, |ship| {
            ship.position = star_pos + Vec2::new(0.3 * star_radius, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        h.step_until(60, |h| h.my_ship().is_none());
    }

    #[test]
    fn star_heat_burns_npc_ships() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let (star_pos, star_radius) = h.star_in_system("sol").expect("sol has a star");

        let npc_owner = 99_001u64;
        h.server
            .world_mut()
            .resource_scope(|world, content: Mut<Content>| {
                let mut cmds = world.commands();
                spawn_ship(
                    &mut cmds,
                    &content.0,
                    POLICE_SHIP,
                    ShipKind::Police,
                    Some(npc_owner),
                    "sol",
                    Vec2::ZERO,
                );
            });
        h.server.world_mut().flush();
        let initial = h.server_hull_by_owner(npc_owner).expect("npc hull");

        h.set_server_ship(npc_owner, |ship| {
            ship.position = star_pos + Vec2::new(1.1 * star_radius, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        h.step_until(60, |h| {
            h.server_hull_by_owner(npc_owner)
                .is_some_and(|(shields, armor)| shields < initial.0 || armor < initial.1)
        });

        let after = h
            .server_hull_by_owner(npc_owner)
            .expect("npc survives the test stay");
        assert!(
            after.0 < initial.0 || after.1 < initial.1,
            "NPCs must burn in the corona like anyone else"
        );
    }

    #[test]
    fn star_heat_spares_docked_ships() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().unwrap();

        h.set_server_ship(EMBEDDED_CLIENT_ID as u64, |ship| {
            ship.position = Vec2::new(480.0, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        for _ in 0..5 {
            h.step();
        }
        h.send(RequestLand {
            planet: "earth".into(),
        });
        h.step_until(100, |h| h.client_component::<DockedAt>(my_ship).is_some());
        let docked = h.client_component::<DockedAt>(my_ship).unwrap();
        assert_eq!(docked.location, "earth");

        let (star_pos, star_radius) = h.star_in_system("sol").expect("sol has a star");
        let initial = h.client_component::<Hull>(my_ship).unwrap();
        h.set_server_ship(EMBEDDED_CLIENT_ID as u64, |ship| {
            ship.position = star_pos + Vec2::new(0.3 * star_radius, 0.0);
            ship.velocity = Vec2::ZERO;
        });
        for _ in 0..60 {
            h.step();
        }

        let after = h.client_component::<Hull>(my_ship).unwrap();
        assert_eq!(
            (after.shields, after.armor),
            (initial.shields, initial.armor),
            "docked ships must be immune to stellar heat"
        );
    }

    #[test]
    fn merchant_npc_travels_and_trades_stock_between_systems() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());

        let merchant_entity = h
            .server
            .world_mut()
            .query::<(Entity, &ShipInfo, &SystemMembership)>()
            .iter(h.server.world())
            .find(|(_, info, sys)| info.kind == ShipKind::Merchant && sys.0 == "sol")
            .map(|(e, _, _)| e)
            .expect("Sol merchant must exist");

        let (initial_target_sys, target_pos, initial_step) = match h
            .server
            .world()
            .get::<NpcBrain>(merchant_entity)
            .expect("merchant brain")
        {
            NpcBrain::Merchant {
                target_system,
                target_pos,
                route_step,
                ..
            } => (target_system.clone(), *target_pos, *route_step),
            other => panic!("expected Merchant brain, got {other:?}"),
        };
        assert_eq!(initial_target_sys, "alpha-centauri");

        // Place merchant just outside its target trade waypoint so it arrives and cycles stock.
        h.server
            .world_mut()
            .get_mut::<Ship>(merchant_entity)
            .unwrap()
            .0
            .position = target_pos - Vec2::new(40.0, 0.0);

        h.step_until(250, |h| {
            matches!(
                h.server.world().get::<NpcBrain>(merchant_entity),
                Some(NpcBrain::Merchant { route_step, .. }) if *route_step > initial_step
            )
        });

        let (new_target_sys, new_comm) = match h.server.world().get::<NpcBrain>(merchant_entity) {
            Some(NpcBrain::Merchant {
                target_system,
                commodity,
                ..
            }) => (target_system.clone(), commodity.clone()),
            other => panic!("expected Merchant brain, got {other:?}"),
        };
        assert_ne!(
            new_target_sys, initial_target_sys,
            "merchant must pick a new destination system after trading stock"
        );
        assert!(!new_comm.is_empty());
    }

    #[test]
    fn defender_npc_fights_and_destroys_pirate() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());

        let defender_entity = h
            .server
            .world_mut()
            .query::<(Entity, &ShipInfo, &SystemMembership)>()
            .iter(h.server.world())
            .find(|(_, info, sys)| info.kind == ShipKind::Defender && sys.0 == "sol")
            .map(|(e, _, _)| e)
            .expect("Sol defender must exist");

        let pirate_entity = h
            .server
            .world_mut()
            .query::<(Entity, &ShipInfo)>()
            .iter(h.server.world())
            .find(|(_, info)| info.kind == ShipKind::Pirate)
            .map(|(e, _)| e)
            .expect("pirate must exist");

        // Move player far away so the duel is strictly Defender vs Pirate.
        h.set_server_ship(1, |s| {
            s.position = Vec2::new(-5000.0, -5000.0);
        });
        {
            let mut def_ship = h
                .server
                .world_mut()
                .get_mut::<Ship>(defender_entity)
                .unwrap();
            def_ship.0.position = Vec2::new(-160.0, 230.0);
            def_ship.0.rotation = 0.0;
            def_ship.0.velocity = Vec2::ZERO;
        }
        {
            let mut pir_ship = h.server.world_mut().get_mut::<Ship>(pirate_entity).unwrap();
            pir_ship.0.position = Vec2::new(-40.0, 230.0);
            pir_ship.0.shields = 8.0;
            pir_ship.0.armor = 6.0;
            pir_ship.0.velocity = Vec2::ZERO;
        }
        h.server
            .world_mut()
            .get_mut::<SystemMembership>(pirate_entity)
            .unwrap()
            .0 = "sol".to_owned();

        h.step_until(250, |h| h.server.world().get_entity(pirate_entity).is_err());
        assert!(
            h.server.world().get_entity(defender_entity).is_ok(),
            "defender must survive and destroy the pirate"
        );
    }

    #[test]
    fn explorer_npc_discovers_linked_systems() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());

        let explorer_entity = h
            .server
            .world_mut()
            .query::<(Entity, &ShipInfo, &SystemMembership)>()
            .iter(h.server.world())
            .find(|(_, info, sys)| info.kind == ShipKind::Explorer && sys.0 == "sol")
            .map(|(e, _, _)| e)
            .expect("Sol explorer must exist");

        let target_pos = match h.server.world().get::<NpcBrain>(explorer_entity) {
            Some(NpcBrain::Explorer { target_pos, .. }) => *target_pos,
            other => panic!("expected Explorer brain, got {other:?}"),
        };

        h.server
            .world_mut()
            .get_mut::<Ship>(explorer_entity)
            .unwrap()
            .0
            .position = target_pos - Vec2::new(30.0, 0.0);

        h.step_until(150, |h| {
            matches!(
                h.server.world().get::<NpcBrain>(explorer_entity),
                Some(NpcBrain::Explorer { visited, .. }) if visited.len() >= 2
            )
        });
    }

    #[test]
    fn hail_friendly_npc_ship_and_firing_triggers_retaliation() {
        let mut h = Harness::new();
        h.step_until(200, |h| h.my_ship().is_some());
        let my_ship = h.my_ship().unwrap();

        let (merchant_entity, merchant_pos) = h
            .server
            .world_mut()
            .query::<(Entity, &ShipInfo, &Position, &SystemMembership)>()
            .iter(h.server.world())
            .find(|(_, info, _, sys)| info.kind == ShipKind::Merchant && sys.0 == "sol")
            .map(|(e, _, p, _)| (e, p.0))
            .expect("Sol merchant must exist");

        // Fly close to the merchant and hail it.
        h.set_server_ship(1, |s| {
            s.position = merchant_pos - Vec2::new(80.0, 0.0);
            s.rotation = 0.0;
            s.velocity = Vec2::ZERO;
        });
        for _ in 0..5 {
            h.step();
        }
        h.send(HailShip);
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_some()
        });
        let encounter = h.client_component::<ChiefEncounter>(my_ship).unwrap();
        assert!(
            encounter.chief_name.contains("vellari"),
            "expected Vellari merchant hail, got {}",
            encounter.chief_name
        );

        // Buy cargo directly from the merchant in space.
        let credits_before = h.client_component::<AccountState>(my_ship).unwrap().credits;
        h.send(RespondToEncounter {
            choice: "accept_offer".into(),
        });
        h.step_until(100, |h| {
            h.client_component::<ChiefEncounter>(my_ship).is_none()
                && h.client_component::<AccountState>(my_ship)
                    .is_some_and(|a| a.credits < credits_before)
        });

        // Now fire at the merchant and verify it immediately retaliates!
        let current_merchant_pos = h.server.world().get::<Position>(merchant_entity).unwrap().0;
        h.set_server_ship(1, |s| {
            s.position = current_merchant_pos - Vec2::new(70.0, 0.0);
            s.rotation = 0.0;
            s.velocity = Vec2::ZERO;
        });
        let player_shields_before = h.client_component::<Hull>(my_ship).unwrap().shields;
        h.send(SetFiring(true));
        h.step_until(200, |h| {
            h.server
                .world()
                .get::<Aggro>(merchant_entity)
                .is_some_and(|a| a.0 == 1)
                && h.client_component::<Hull>(my_ship)
                    .is_some_and(|hull| hull.shields < player_shields_before)
        });
        h.send(SetFiring(false));
    }
}
