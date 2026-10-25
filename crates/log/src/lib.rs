use std::collections::{BTreeMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bevy::app::App;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport};
use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetClient};
use bevy_replicon_renet2::{RenetChannelsExt, RepliconRenetPlugins};
use serde::Serialize;
use sv_data::ContentPack;
use sv_protocol::{
    DockableTag, DockedAt, Explosion, Notice, PROTOCOL_ID, PlayerDied, PortMarket, ShipInfo,
};

pub const RECONNECT_COOLDOWN: Duration = Duration::from_secs(5);
const QUEUE_CAP: usize = 4096;
const DOCK_GRACE_FRAMES: u32 = 10;

#[derive(Clone, Resource)]
pub struct LogConfig {
    pub server_addr: SocketAddr,
    pub watch: Option<(String, String)>,
    pub commodity: Option<String>,
    pub json: bool,
    pub quiet: bool,
    pub content_dir: Option<PathBuf>,
    pub print: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FeedEvent {
    News {
        ts: String,
        ts_ms: u64,
        text: String,
    },
    Board {
        ts: String,
        ts_ms: u64,
        system: String,
        port: String,
        port_name: String,
        commodity: String,
        commodity_name: String,
        price: u32,
    },
    Price {
        ts: String,
        ts_ms: u64,
        system: String,
        port: String,
        port_name: String,
        commodity: String,
        commodity_name: String,
        old: u32,
        new: u32,
    },
    Dock {
        ts: String,
        ts_ms: u64,
        ship: String,
        system: String,
        port: String,
    },
    Explosion {
        ts: String,
        ts_ms: u64,
        system: String,
        position: [f32; 2],
    },
    Death {
        ts: String,
        ts_ms: u64,
    },
}

#[derive(Resource, Default, Debug)]
pub struct NoticeQueue(pub VecDeque<String>);

#[derive(Resource, Default, Debug)]
pub struct FeedQueue(pub VecDeque<FeedEvent>);

#[derive(Resource, Default, Debug)]
pub struct MarketBoard(pub BTreeMap<(String, String), BTreeMap<String, u32>>);

#[derive(Resource, Default)]
pub struct Names(pub Option<ContentPack>);

#[derive(Resource)]
pub struct ReconnectState {
    pub last_attempt: Instant,
    pub dock_grace: u32,
}

pub fn build_transport(server_addr: SocketAddr) -> NetcodeClientTransport {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after the epoch");
    let client_id = current_time.as_nanos() as u64;
    let socket = UdpSocket::bind("0.0.0.0:0").expect("client socket must bind");
    NetcodeClientTransport::new(
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
    .expect("client transport must be valid")
}

pub fn build_app(config: LogConfig, transport: NetcodeClientTransport) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(RepliconPlugins);
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
    match config
        .content_dir
        .as_ref()
        .map(|dir| ContentPack::load_dir(dir))
    {
        Some(Ok(pack)) => {
            app.insert_resource(Names(Some(pack)));
        }
        Some(Err(_)) => {
            eprintln!("second-velocity-log: content pack unavailable; commodity ids shown as-is");
            app.init_resource::<Names>();
        }
        None => {
            app.init_resource::<Names>();
        }
    }
    app.insert_resource(config);
    app.init_resource::<NoticeQueue>();
    app.init_resource::<FeedQueue>();
    app.init_resource::<MarketBoard>();
    app.insert_resource(ReconnectState {
        last_attempt: Instant::now(),
        dock_grace: 0,
    });
    app.add_observer(record_notice);
    app.add_observer(record_death);
    app.add_observer(record_explosion);
    app.add_systems(
        Update,
        (
            flush_notices,
            track_market,
            track_dockings,
            reconnect,
            print_feed,
        )
            .chain(),
    );
    app.finish();
    app
}

fn record_notice(trigger: On<Notice>, mut queue: ResMut<NoticeQueue>) {
    push_capped(&mut queue.0, trigger.event().text.clone());
}

fn record_death(_death: On<PlayerDied>, config: Res<LogConfig>, mut feed: ResMut<FeedQueue>) {
    if config.quiet {
        return;
    }
    let (ts, ts_ms) = now_stamp();
    push_capped(&mut feed.0, FeedEvent::Death { ts, ts_ms });
}

fn record_explosion(trigger: On<Explosion>, config: Res<LogConfig>, mut feed: ResMut<FeedQueue>) {
    if config.quiet {
        return;
    }
    let explosion = trigger.event();
    let (ts, ts_ms) = now_stamp();
    push_capped(
        &mut feed.0,
        FeedEvent::Explosion {
            ts,
            ts_ms,
            system: explosion.system.clone(),
            position: [explosion.position.0, explosion.position.1],
        },
    );
}

fn flush_notices(mut notices: ResMut<NoticeQueue>, mut feed: ResMut<FeedQueue>) {
    for text in notices.0.drain(..) {
        let (ts, ts_ms) = now_stamp();
        push_capped(&mut feed.0, FeedEvent::News { ts, ts_ms, text });
    }
}

fn track_market(
    mut board: ResMut<MarketBoard>,
    names: Res<Names>,
    config: Res<LogConfig>,
    mut feed: ResMut<FeedQueue>,
    ports: Query<(&DockableTag, &PortMarket), With<Remote>>,
) {
    for (tag, market) in &ports {
        if let Some((system, port)) = &config.watch
            && (&tag.system, &tag.id) != (system, port)
        {
            continue;
        }
        let entry = board
            .0
            .entry((tag.system.clone(), tag.id.clone()))
            .or_default();
        entry.retain(|commodity, _| market.prices.iter().any(|row| &row.commodity == commodity));
        for row in &market.prices {
            if let Some(commodity) = &config.commodity
                && &row.commodity != commodity
            {
                continue;
            }
            match entry.get(&row.commodity) {
                Some(old) if *old != row.price => {
                    let (ts, ts_ms) = now_stamp();
                    push_capped(
                        &mut feed.0,
                        FeedEvent::Price {
                            ts,
                            ts_ms,
                            system: tag.system.clone(),
                            port: tag.id.clone(),
                            port_name: tag.name.clone(),
                            commodity: row.commodity.clone(),
                            commodity_name: commodity_name(&names, &row.commodity).to_owned(),
                            old: *old,
                            new: row.price,
                        },
                    );
                    entry.insert(row.commodity.clone(), row.price);
                }
                None => {
                    let (ts, ts_ms) = now_stamp();
                    push_capped(
                        &mut feed.0,
                        FeedEvent::Board {
                            ts,
                            ts_ms,
                            system: tag.system.clone(),
                            port: tag.id.clone(),
                            port_name: tag.name.clone(),
                            commodity: row.commodity.clone(),
                            commodity_name: commodity_name(&names, &row.commodity).to_owned(),
                            price: row.price,
                        },
                    );
                    entry.insert(row.commodity.clone(), row.price);
                }
                _ => {}
            }
        }
    }
}

#[allow(clippy::type_complexity)]
fn track_dockings(
    config: Res<LogConfig>,
    mut reconnect: ResMut<ReconnectState>,
    mut feed: ResMut<FeedQueue>,
    docks: Query<(&ShipInfo, &DockedAt), (With<Remote>, Added<DockedAt>)>,
) {
    if reconnect.dock_grace > 0 {
        reconnect.dock_grace -= 1;
        return;
    }
    if config.quiet {
        return;
    }
    for (ship, docked) in &docks {
        let (ts, ts_ms) = now_stamp();
        push_capped(
            &mut feed.0,
            FeedEvent::Dock {
                ts,
                ts_ms,
                ship: ship.name.clone(),
                system: docked.system.clone(),
                port: docked.location.clone(),
            },
        );
    }
}

fn reconnect(world: &mut World) {
    let connected = world
        .get_resource::<NetcodeClientTransport>()
        .is_some_and(|transport| transport.is_connected());
    let since_attempt = world.resource::<ReconnectState>().last_attempt.elapsed();
    if connected || since_attempt < RECONNECT_COOLDOWN {
        return;
    }
    let config = world.resource::<LogConfig>().clone();
    world.resource_mut::<ReconnectState>().last_attempt = Instant::now();
    world.resource_mut::<ReconnectState>().dock_grace = DOCK_GRACE_FRAMES;
    eprintln!(
        "second-velocity-log: reconnecting to {}",
        config.server_addr
    );
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
    world.insert_resource(build_transport(config.server_addr));
    world.resource_mut::<MarketBoard>().0.clear();
    let mut remotes = world.query_filtered::<Entity, With<Remote>>();
    let entities: Vec<Entity> = remotes.iter(world).collect();
    for entity in entities {
        world.entity_mut(entity).despawn();
    }
}

fn print_feed(config: Res<LogConfig>, mut feed: ResMut<FeedQueue>) {
    if !config.print {
        return;
    }
    for event in feed.0.drain(..) {
        if config.json {
            if let Ok(line) = serde_json::to_string(&event) {
                println!("{line}");
            }
        } else {
            println!("{}", human(&event));
        }
    }
}

fn human(event: &FeedEvent) -> String {
    match event {
        FeedEvent::News { ts, text, .. } => format!("[{ts}] NEWS  {text}"),
        FeedEvent::Board {
            ts,
            system,
            port,
            port_name,
            commodity_name,
            price,
            ..
        } => format!("[{ts}] BOARD {system}/{port}  {port_name}: {commodity_name} at {price} cr"),
        FeedEvent::Price {
            ts,
            system,
            port,
            commodity_name,
            old,
            new,
            ..
        } => format!("[{ts}] PRICE {system}/{port}  {commodity_name} {old} -> {new} cr"),
        FeedEvent::Dock {
            ts,
            ship,
            system,
            port,
            ..
        } => format!("[{ts}] DOCK  {ship} docked at {port} ({system})"),
        FeedEvent::Explosion {
            ts,
            system,
            position,
            ..
        } => format!(
            "[{ts}] BOOM  explosion in {system} at ({:.0}, {:.0})",
            position[0], position[1]
        ),
        FeedEvent::Death { ts, .. } => format!("[{ts}] DEATH a ship was destroyed"),
    }
}

fn commodity_name<'a>(names: &'a Names, id: &'a str) -> &'a str {
    names
        .0
        .as_ref()
        .and_then(|pack| pack.commodity(id))
        .map(|commodity| commodity.name.as_str())
        .unwrap_or(id)
}

fn push_capped<T>(queue: &mut VecDeque<T>, event: T) {
    if queue.len() >= QUEUE_CAP {
        queue.pop_front();
    }
    queue.push_back(event);
}

fn now_stamp() -> (String, u64) {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = since.as_secs();
    (
        format!(
            "{:02}:{:02}:{:02}",
            (secs / 3600) % 24,
            (secs / 60) % 60,
            secs % 60
        ),
        since.as_millis() as u64,
    )
}
