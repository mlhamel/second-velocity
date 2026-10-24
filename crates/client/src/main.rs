mod chief;
mod hud;
mod input;
mod minimap;
mod screenshots;
mod shops;
mod sky;
mod sky_gen;
mod textures;
mod tour;
mod visuals;

use std::net::SocketAddr;
use std::time::SystemTime;

use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::netcode::{ClientAuthentication, NativeSocket, NetcodeClientTransport};
use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetClient};
use bevy_replicon_renet2::{RenetChannelsExt, RepliconRenetPlugins};

#[derive(Resource)]
pub struct MyNetworkId(pub u64);

struct EmbeddedServer(App);

#[derive(Resource)]
struct LoginCredentials {
    username: String,
    password: String,
}

fn send_login(
    credentials: Res<LoginCredentials>,
    client: Res<RenetClient>,
    mut sent: Local<bool>,
    mut commands: Commands,
) {
    if *sent || !client.is_connected() {
        return;
    }
    *sent = true;
    commands.client_trigger(evr_protocol::LoginRequest {
        username: credentials.username.clone(),
        password: credentials.password.clone(),
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let verbose = args.iter().any(|arg| arg == "--verbose" || arg == "-v");
    let capture_tour = args.iter().any(|arg| arg == "--capture-tour");
    let connect_addr = args.iter().position(|arg| arg == "--connect").map(|index| {
        let addr = args
            .get(index + 1)
            .unwrap_or_else(|| panic!("--connect takes host:port"));
        addr.parse::<SocketAddr>()
            .unwrap_or_else(|_| panic!("--connect takes host:port, got {addr:?}"))
    });
    assert!(
        !(capture_tour && connect_addr.is_some()),
        "--capture-tour requires embedded mode"
    );

    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(bevy::log::LogPlugin {
                level: if verbose {
                    bevy::log::Level::DEBUG
                } else {
                    bevy::log::Level::INFO
                },
                filter: if verbose {
                    "evr_client=trace,evr_server=trace,evr_protocol=debug,bevy_replicon=debug,wgpu=error,naga=warn,bevy_render=info".to_owned()
                } else {
                    bevy::log::DEFAULT_FILTER.to_owned()
                },
                ..default()
            })
            .set(AssetPlugin {
                file_path: "../../assets".to_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Second Velocity: The Living Galaxy".to_owned(),
                    resolution: bevy::window::WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
    );
    app.add_plugins(RepliconPlugins);
    app.add_plugins(RepliconRenetPlugins);
    evr_protocol::register(&mut app);

    app.add_plugins((
        visuals::VisualsPlugin,
        input::InputPlugin,
        hud::HudPlugin,
        chief::ChiefPlugin,
        shops::ShopsPlugin,
        screenshots::ScreenshotPlugin,
        minimap::MinimapPlugin,
        sky::SkyPlugin,
    ));
    app.add_systems(
        Startup,
        (
            spawn_camera,
            textures::create_textures,
            visuals::spawn_starfield,
        )
            .chain(),
    );
    if capture_tour {
        let output_dir = args
            .iter()
            .position(|arg| arg == "--capture-dir")
            .map(|index| {
                std::path::PathBuf::from(
                    args.get(index + 1)
                        .expect("--capture-dir takes a directory"),
                )
            })
            .unwrap_or_else(|| "docs/screenshots".into());
        app.add_plugins(tour::CaptureTourPlugin { output_dir });
    }

    let (server_channels, client_channels) = {
        let channels = app.world().resource::<RepliconChannels>();
        (channels.server_configs(), channels.client_configs())
    };
    let client = RenetClient::new(
        ConnectionConfig::from_channels(server_channels, client_channels),
        false,
    );
    app.insert_resource(client);

    let my_id = match connect_addr {
        Some(server_addr) => {
            let current_time = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("system time must be after the epoch");
            let client_id = current_time.as_nanos() as u64;
            let socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("client socket must bind");
            let transport = NetcodeClientTransport::new(
                current_time,
                ClientAuthentication::Unsecure {
                    client_id,
                    protocol_id: evr_server::PROTOCOL_ID,
                    socket_id: 0,
                    server_addr,
                    user_data: None,
                },
                NativeSocket::new(socket).expect("client socket must be valid"),
            )
            .expect("client transport must be valid");
            app.insert_resource(transport);
            println!("evr-client: connecting to {server_addr} as {client_id}");
            client_id
        }
        None => {
            app.add_systems(First, step_embedded_server);
            let mut server_app =
                evr_server::build_server_app("content", evr_server::TickMode::Fixed, None)
                    .expect("content pack must load");
            let (transport, _socket) = evr_server::attach_memory(&mut server_app);
            server_app.finish();
            app.insert_resource(transport);
            app.insert_non_send(EmbeddedServer(server_app));
            evr_server::EMBEDDED_CLIENT_ID as u64
        }
    };
    let content_pack = evr_data::ContentPack::load_dir(std::path::Path::new("content"))
        .expect("content pack must load");
    app.insert_resource(input::ClientContent(content_pack));
    app.insert_resource(MyNetworkId(my_id));

    if connect_addr.is_some() {
        let username = std::env::args()
            .position(|arg| arg == "--username")
            .and_then(|index| std::env::args().nth(index + 1))
            .unwrap_or_else(|| "pilot".to_owned());
        let password = std::env::args()
            .position(|arg| arg == "--password")
            .and_then(|index| std::env::args().nth(index + 1))
            .unwrap_or_else(|| "pilot".to_owned());
        app.add_systems(Update, send_login.run_if(resource_exists::<RenetClient>));
        app.insert_resource(LoginCredentials { username, password });
    }

    app.run();
}

fn step_embedded_server(mut server: NonSendMut<EmbeddedServer>) {
    server.0.update();
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
