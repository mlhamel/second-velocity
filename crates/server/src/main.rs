use std::net::SocketAddr;
use std::path::PathBuf;

use bevy::log::{Level, LogPlugin};
use clap::Parser;

#[derive(Debug, Parser, PartialEq, Eq)]
#[command(
    name = "second-velocity-server",
    version,
    about = "Standalone UDP shard server for Second Velocity: The Living Galaxy"
)]
struct ServerCli {
    /// UDP port to listen on
    #[arg(default_value_t = 3456)]
    port: u16,
    /// SQLite database path for persistent player state
    #[arg(long)]
    storage: Option<PathBuf>,
    /// Run as a scoped quadrant server (community shard)
    #[arg(long, value_parser = non_empty_quadrant)]
    quadrant: Option<String>,
    /// Verbose (debug) logging
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn non_empty_quadrant(id: &str) -> Result<String, String> {
    if id.trim().is_empty() {
        Err("quadrant id must not be empty".to_owned())
    } else {
        Ok(id.to_owned())
    }
}

fn main() {
    let cli = ServerCli::parse();

    let content_root = PathBuf::from("content");
    let mut app = sv_server::build_server_app_scoped(
        content_root,
        sv_server::TickMode::Fixed,
        cli.storage.as_deref(),
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
        cli.verbose, cli.quadrant, cli.storage
    );
    app.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_default_and_scoped_quadrant_flags() {
        let default_cli = ServerCli::try_parse_from(std::iter::empty::<String>()).unwrap();
        assert_eq!(
            default_cli,
            ServerCli {
                port: 3456,
                storage: None,
                quadrant: None,
                verbose: false,
            }
        );

        let scoped = ServerCli::try_parse_from([
            "second-velocity-server",
            "4000",
            "--quadrant",
            "sol-reach",
            "--storage",
            "shard.db",
            "--verbose",
        ])
        .unwrap();
        assert_eq!(
            scoped,
            ServerCli {
                port: 4000,
                storage: Some(PathBuf::from("shard.db")),
                quadrant: Some("sol-reach".to_owned()),
                verbose: true,
            }
        );

        assert!(ServerCli::try_parse_from(["second-velocity-server", "--quadrant"]).is_err());
        assert!(
            ServerCli::try_parse_from(["second-velocity-server", "--quadrant", "--verbose"])
                .is_err()
        );
        assert!(
            ServerCli::try_parse_from(["second-velocity-server", "--quadrant", ""]).is_err(),
            "empty quadrant ids must fail fast"
        );
        assert!(ServerCli::try_parse_from(["second-velocity-server", "not-a-port"]).is_err());
        assert!(ServerCli::try_parse_from(["second-velocity-server", "--nope"]).is_err());
    }
}
