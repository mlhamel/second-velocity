use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use bevy::prelude::*;
use bevy_replicon_renet2::renet2::RenetClient;
use sv_server::{
    EMBEDDED_CLIENT_ID, SqliteStorage, TickMode, attach_memory, build_server_app,
    gameplay::StorageResource,
    gameplay::market::{MarketKey, MarketState, force_market_ticks},
};

#[test]
fn spectator_streams_news_and_prices() {
    let content_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let storage_path = std::env::temp_dir().join(format!("sv-log-test-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&storage_path);

    let mut server =
        build_server_app(content_root, TickMode::PerUpdate, None).expect("server must build");
    let storage = SqliteStorage::open(&storage_path).expect("storage must open");
    server.insert_resource(StorageResource {
        storage: Some(Box::new(storage)),
    });
    let (transport, _socket) = attach_memory(&mut server);
    server.finish();

    let config = sv_log::LogConfig {
        server_addr: "127.0.0.1:3456".parse().unwrap(),
        watch: None,
        commodity: Some("food".to_owned()),
        json: false,
        quiet: false,
        content_dir: None,
        print: false,
    };
    let mut client = sv_log::build_app(config, transport);

    let step = |server: &mut App, client: &mut App| {
        client.update();
        server.update();
        sleep(Duration::from_millis(15));
    };

    let mut steps = 0;
    while !client.world().resource::<RenetClient>().is_connected() {
        step(&mut server, &mut client);
        steps += 1;
        assert!(steps < 400, "sv-log client must connect");
    }
    for _ in 0..10 {
        step(&mut server, &mut client);
    }

    let spectator = EMBEDDED_CLIENT_ID as u64;
    {
        let mut ships = server.world_mut().query::<&sv_protocol::ShipInfo>();
        let world = server.world();
        assert!(
            !ships.iter(world).any(|info| info.owner == Some(spectator)),
            "spectator must not spawn a ship"
        );
    }

    client
        .world_mut()
        .resource_mut::<sv_log::FeedQueue>()
        .0
        .clear();

    server
        .world_mut()
        .resource_mut::<MarketState>()
        .shift_supply(
            MarketKey {
                system: "sol".to_owned(),
                port: "earth".to_owned(),
                commodity: "food".to_owned(),
            },
            -120,
            10,
        );
    force_market_ticks(server.world_mut(), 1);

    for _ in 0..10 {
        step(&mut server, &mut client);
    }

    let feed = client.world().resource::<sv_log::FeedQueue>();
    assert!(
        feed.0.iter().any(|event| matches!(
            event,
            sv_log::FeedEvent::News { text, .. } if text.contains("runs scarce")
        )),
        "expected scarce-food news in the feed, got {feed:?}"
    );
    assert!(
        feed.0.iter().any(|event| matches!(
            event,
            sv_log::FeedEvent::Price { system, port, commodity, new, .. }
                if system == "sol" && port == "earth" && commodity == "food" && *new >= 15
        )),
        "expected sol/earth food price spike in the feed, got {feed:?}"
    );

    let board = client.world().resource::<sv_log::MarketBoard>();
    let earth = board
        .0
        .get(&("sol".to_owned(), "earth".to_owned()))
        .expect("earth must be on the board");
    assert_eq!(earth.get("food"), Some(&16));

    let _ = std::fs::remove_file(&storage_path);
}
