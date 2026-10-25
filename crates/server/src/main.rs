use std::net::SocketAddr;
use std::path::PathBuf;

use bevy::log::{Level, LogPlugin};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|arg| arg == "--verbose" || arg == "-v");

    let mut port: u16 = 3456;
    let mut storage_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--storage" => {
                if let Some(path) = args.get(i + 1) {
                    storage_path = Some(PathBuf::from(path));
                    i += 2;
                    continue;
                }
            }
            "--verbose" | "-v" => {}
            other => {
                if let Ok(parsed) = other.parse::<u16>() {
                    port = parsed;
                }
            }
        }
        i += 1;
    }

    let content_root = PathBuf::from("content");
    let mut app = sv_server::build_server_app(
        content_root,
        sv_server::TickMode::Fixed,
        storage_path.as_deref(),
    )
    .expect("content pack must load");

    app.add_plugins(LogPlugin {
        level: if verbose { Level::DEBUG } else { Level::INFO },
        filter: if verbose {
            "sv_server=trace,sv_protocol=debug,bevy_replicon=debug".to_owned()
        } else {
            "sv_server=info".to_owned()
        },
        ..Default::default()
    });

    let bound = sv_server::attach_udp(
        &mut app,
        SocketAddr::from(([0, 0, 0, 0], port)),
        sv_server::MAX_CLIENTS,
    )
    .expect("udp socket must bind");
    println!(
        "sv-server: listening on {bound} (verbose={verbose}, storage={:?})",
        storage_path
    );
    app.run();
}
