use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use sv_pilot::{
    MIN_DECISION_INTERVAL, MockBrain, OllamaBrain, PilotBrain, PilotConfig, build_pilot_app,
    build_udp_transport, load_persona_and_pack,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(addr_str) = flag_value(&args, "--connect") else {
        eprintln!(
            "usage: second-velocity-pilot --connect host:port --persona content/pilots/<id>.ron [--mock] [--ollama-url http://127.0.0.1:11434] [--model qwen2.5:3b] [--content dir] [--verbose]"
        );
        std::process::exit(2);
    };
    let Some(persona_str) = flag_value(&args, "--persona") else {
        eprintln!(
            "usage: second-velocity-pilot --connect host:port --persona content/pilots/<id>.ron [--mock] [--ollama-url http://127.0.0.1:11434] [--model qwen2.5:3b] [--content dir] [--verbose]"
        );
        std::process::exit(2);
    };

    let server_addr: SocketAddr = addr_str
        .parse()
        .unwrap_or_else(|e| panic!("--connect takes host:port, got {addr_str:?}: {e}"));
    let persona_path = PathBuf::from(persona_str);
    let content_dir = flag_value(&args, "--content")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("content"));
    let mock = args.iter().any(|arg| arg == "--mock");
    let verbose = args.iter().any(|arg| arg == "--verbose" || arg == "-v");
    let ollama_url =
        flag_value(&args, "--ollama-url").unwrap_or_else(|| "http://127.0.0.1:11434".to_owned());
    let model = flag_value(&args, "--model").unwrap_or_else(|| "qwen2.5:3b".to_owned());

    let (pilot, pack) = match load_persona_and_pack(&persona_path, &content_dir) {
        Ok(pair) => pair,
        Err(err) => {
            eprintln!("second-velocity-pilot: {err}");
            std::process::exit(1);
        }
    };

    let (client_id, transport) = build_udp_transport(server_addr);
    let brain: Box<dyn PilotBrain> = if mock {
        Box::new(MockBrain::new())
    } else {
        Box::new(OllamaBrain::new(ollama_url.clone(), model.clone()))
    };

    let config = PilotConfig {
        pilot: pilot.clone(),
        client_id,
        server_addr: Some(server_addr),
        min_decision_interval: MIN_DECISION_INTERVAL,
        verbose,
    };

    let mut app = build_pilot_app(config, pack, transport, None, brain);
    eprintln!(
        "second-velocity-pilot: '{}' ({}) connecting to {server_addr} (brain={})",
        pilot.name,
        pilot.id,
        if mock {
            "mock".to_owned()
        } else {
            format!("ollama:{model}@{ollama_url}")
        }
    );

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
