use std::path::PathBuf;
use std::thread::sleep;
use std::time::Duration;

use bevy::prelude::*;
use bevy_replicon::prelude::Remote;
use bevy_replicon_renet2::renet2::RenetClient;
use sv_pilot::{MockBrain, PilotConfig, PilotRuntime, build_pilot_app, load_persona_and_pack};
use sv_protocol::{AccountState, ActiveQuests, DockedAt, ShipInfo};
use sv_server::{
    EMBEDDED_CLIENT_ID, SqliteStorage, Storage, TickMode, attach_memory, build_server_app,
    gameplay::{EncounterRng, StorageResource},
};

#[test]
fn stub_brain_pilot_completes_trade_and_delivery_quest_loop() {
    let content_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let persona_path = content_root.join("pilots/cautious-trader.ron");
    let storage_path =
        std::env::temp_dir().join(format!("sv-pilot-test-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&storage_path);

    let (pilot, pack) =
        load_persona_and_pack(&persona_path, &content_root).expect("persona and pack must load");

    let mut server =
        build_server_app(&content_root, TickMode::PerUpdate, None).expect("server app must build");
    let storage = SqliteStorage::open(&storage_path).expect("storage must open");
    server.insert_resource(StorageResource {
        storage: Some(Box::new(storage)),
    });
    // Default EncounterRng(0) rolls Quest (`newhaven-relief`) when docking at Earth:
    server.insert_resource(EncounterRng::default());

    let (transport, socket) = attach_memory(&mut server);
    server.finish();

    let client_id = EMBEDDED_CLIENT_ID as u64;
    let config = PilotConfig {
        pilot: pilot.clone(),
        client_id,
        server_addr: None,
        min_decision_interval: Duration::ZERO,
        verbose: false,
    };
    let mut client = build_pilot_app(
        config,
        pack,
        transport,
        Some(socket),
        Box::new(MockBrain::new()),
    );

    let step = |server: &mut App, client: &mut App| {
        client.update();
        server.update();
        sleep(Duration::from_millis(4));
    };

    // 1. Connect and auto-register/login via LoginRequest:
    let mut ticks = 0;
    while !client.world().resource::<RenetClient>().is_connected()
        || !client.world().resource::<PilotRuntime>().logged_in
    {
        step(&mut server, &mut client);
        ticks += 1;
        assert!(ticks < 400, "pilot must connect and log in");
    }

    // 2. Reflex flies to Earth, docks, MockBrain accepts `newhaven-relief`,
    // buys food on Earth, lifts off, flies to Alpha Centauri / Newhaven, docks,
    // turns in `newhaven-relief`, and sells the food cargo:
    let mut completed_quest = false;
    let mut traded_cargo = false;
    let mut docked_earth = false;
    let mut docked_newhaven = false;

    for _ in 0..2200 {
        step(&mut server, &mut client);

        let mut query = client.world_mut().query_filtered::<(
            &ShipInfo,
            Option<&DockedAt>,
            Option<&AccountState>,
            Option<&ActiveQuests>,
        ), With<Remote>>();
        let world = client.world();
        if let Some((_, docked, account, quests)) = query
            .iter(world)
            .find(|(info, ..)| info.owner == Some(client_id))
        {
            if let Some(d) = docked {
                if d.location == "earth" {
                    docked_earth = true;
                }
                if d.location == "newhaven" {
                    docked_newhaven = true;
                }
            }
            if let Some(acc) = account
                && acc.cargo.iter().any(|(c, q)| c == "food" && *q > 0)
            {
                traded_cargo = true;
            }
            if let Some(q) = quests
                && q.completed.iter().any(|id| id == "newhaven-relief")
            {
                completed_quest = true;
                break;
            }
        }
    }

    assert!(docked_earth, "pilot must fly to and dock at Earth");
    assert!(traded_cargo, "pilot must buy trade cargo at Earth");
    assert!(
        docked_newhaven,
        "pilot must fly to and dock at Newhaven in Alpha Centauri"
    );
    assert!(
        completed_quest,
        "pilot must deliver and turn in newhaven-relief at Newhaven"
    );

    // Check final credits and persistence:
    let final_credits = {
        let mut query = client
            .world_mut()
            .query_filtered::<(&ShipInfo, &AccountState), With<Remote>>();
        let world = client.world();
        query
            .iter(world)
            .find(|(info, _)| info.owner == Some(client_id))
            .map(|(_, acc)| acc.credits)
            .expect("pilot account must exist")
    };
    assert!(
        final_credits > 800,
        "expected credits > 800 after quest reward + trade, got {final_credits}"
    );

    let db = SqliteStorage::open(&storage_path).expect("storage must reopen");
    let saved = db
        .verify_login(&pilot.username, &pilot.password)
        .expect("verify")
        .expect("pilot account must be persisted in SQLite");
    assert!(!saved.ship_id.is_empty());

    let _ = std::fs::remove_file(&storage_path);
}
