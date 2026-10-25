use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use sv_log::{LogConfig, build_app, build_transport};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(addr) = flag_value(&args, "--connect") else {
        eprintln!(
            "usage: second-velocity-log --connect host:port [--watch system/port] [--commodity id] [--json] [--quiet (market feed only)] [--content dir]"
        );
        std::process::exit(2);
    };
    let server_addr: SocketAddr = addr
        .parse()
        .unwrap_or_else(|e| panic!("--connect takes host:port, got {addr:?}: {e}"));
    let watch = flag_value(&args, "--watch").map(|value| {
        let Some((system, port)) = value.split_once('/') else {
            panic!("--watch takes system/port, got {value:?}");
        };
        (system.to_owned(), port.to_owned())
    });
    let commodity = flag_value(&args, "--commodity");
    let json = args.iter().any(|arg| arg == "--json");
    let quiet = args.iter().any(|arg| arg == "--quiet");
    let content_dir = flag_value(&args, "--content")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("content"));

    let config = LogConfig {
        server_addr,
        watch,
        commodity,
        json,
        quiet,
        content_dir: Some(content_dir),
        print: true,
    };
    let transport = build_transport(server_addr);
    let mut app = build_app(config, transport);
    eprintln!("second-velocity-log: streaming {server_addr} (spectator; never logs in)");
    loop {
        app.update();
        std::thread::sleep(Duration::from_millis(15));
    }
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .filter(|value| !value.starts_with("--"))
        .cloned()
}
