use std::net::SocketAddr;
use std::path::PathBuf;

use bevy::log::{Level, LogPlugin};

#[derive(Debug, PartialEq, Eq)]
struct ServerCli {
    port: u16,
    storage_path: Option<PathBuf>,
    quadrant: Option<String>,
    verbose: bool,
}

fn parse_cli(args: &[String]) -> Result<ServerCli, String> {
    let mut port: u16 = 3456;
    let mut storage_path: Option<PathBuf> = None;
    let mut quadrant: Option<String> = None;
    let mut verbose = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--storage" => {
                let path = args.get(i + 1).ok_or_else(|| {
                    "missing value for --storage (usage: second-velocity-server [port] [--storage <path.db>] [--quadrant <id>] [--verbose])".to_owned()
                })?;
                storage_path = Some(PathBuf::from(path));
                i += 2;
                continue;
            }
            "--quadrant" => {
                let id = args.get(i + 1).ok_or_else(|| {
                    "missing value for --quadrant (usage: second-velocity-server [port] [--storage <path.db>] [--quadrant <id>] [--verbose])".to_owned()
                })?;
                if id.trim().is_empty() || id.starts_with('-') {
                    return Err("invalid value for --quadrant".to_owned());
                }
                quadrant = Some(id.clone());
                i += 2;
                continue;
            }
            "--verbose" | "-v" => {
                verbose = true;
            }
            other => {
                if let Ok(parsed) = other.parse::<u16>() {
                    port = parsed;
                }
            }
        }
        i += 1;
    }
    Ok(ServerCli {
        port,
        storage_path,
        quadrant,
        verbose,
    })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = parse_cli(&args).unwrap_or_else(|err| {
        eprintln!("sv-server: {err}");
        std::process::exit(2);
    });

    let content_root = PathBuf::from("content");
    let mut app = sv_server::build_server_app_scoped(
        content_root,
        sv_server::TickMode::Fixed,
        cli.storage_path.as_deref(),
        cli.quadrant.as_deref(),
    )
    .expect("content pack must load");

    app.add_plugins(LogPlugin {
        level: if cli.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        },
        filter: if cli.verbose {
            "sv_server=trace,sv_protocol=debug,bevy_replicon=debug".to_owned()
        } else {
            "sv_server=info".to_owned()
        },
        ..Default::default()
    });

    let bound = sv_server::attach_udp(
        &mut app,
        SocketAddr::from(([0, 0, 0, 0], cli.port)),
        sv_server::MAX_CLIENTS,
    )
    .expect("udp socket must bind");
    println!(
        "sv-server: listening on {bound} (verbose={}, quadrant={:?}, storage={:?})",
        cli.verbose, cli.quadrant, cli.storage_path
    );
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_default_and_scoped_quadrant_flags() {
        let default_cli = parse_cli(&[]).unwrap();
        assert_eq!(
            default_cli,
            ServerCli {
                port: 3456,
                storage_path: None,
                quadrant: None,
                verbose: false,
            }
        );

        let scoped = parse_cli(&[
            "4000".to_owned(),
            "--quadrant".to_owned(),
            "sol-reach".to_owned(),
            "--storage".to_owned(),
            "shard.db".to_owned(),
            "--verbose".to_owned(),
        ])
        .unwrap();
        assert_eq!(
            scoped,
            ServerCli {
                port: 4000,
                storage_path: Some(PathBuf::from("shard.db")),
                quadrant: Some("sol-reach".to_owned()),
                verbose: true,
            }
        );

        assert!(parse_cli(&["--quadrant".to_owned()]).is_err());
        assert!(parse_cli(&["--quadrant".to_owned(), "--verbose".to_owned()]).is_err());
    }
}
