use std::path::PathBuf;
use sv_protocol::{
    ActiveQuest, ChiefEncounter, ChiefMood, EncounterOption, PlayerReputation, QuestTarget,
    RespondToEncounter, ShipInfo, ShipKind, SystemMembership,
};

use super::progression::{dock_earth, ready};
use super::*;
use crate::gameplay::factions;

fn base_pack() -> sv_data::ContentPack {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    sv_data::ContentPack::load_dir(&root).expect("base pack must load")
}

#[test]
fn faction_math_follows_content() {
    let pack = base_pack();
    assert_eq!(factions::side_sign(&pack, "confederation"), 1);
    assert_eq!(factions::side_sign(&pack, "rebellion"), -1);
    assert_eq!(factions::side_sign(&pack, "independent"), 0);
    assert_eq!(factions::controller_for(&pack, "sol", 0), "confederation");
    assert_eq!(factions::controller_for(&pack, "vega", -100), "rebellion");
    assert_eq!(
        factions::controller_for(&pack, "alpha-centauri", 49),
        "independent"
    );
    assert_eq!(
        factions::controller_for(&pack, "alpha-centauri", 50),
        "confederation"
    );
    assert_eq!(
        factions::controller_for(&pack, "alpha-centauri", -50),
        "rebellion"
    );
}

#[test]
fn apply_rep_clamps_and_creates_entries() {
    let mut standings = Vec::new();
    factions::apply_rep(&mut standings, "rebellion", 30);
    factions::apply_rep(&mut standings, "rebellion", 30);
    factions::apply_rep(&mut standings, "rebellion", 100);
    factions::apply_rep(&mut standings, "pirate", -200);
    assert_eq!(
        standings,
        vec![("rebellion".to_owned(), 100), ("pirate".to_owned(), -100),]
    );
}

#[test]
fn init_state_matches_content_and_dispatch_reports() {
    let pack = base_pack();
    let mut state = factions::init_faction_state(&StorageResource::default(), &pack);
    assert_eq!(state.meter("sol"), 60);
    assert_eq!(state.meter("vega"), -60);
    assert_eq!(state.meter("alpha-centauri"), 0);
    assert_eq!(state.controller("sol"), "confederation");
    assert_eq!(state.controller("vega"), "rebellion");
    assert_eq!(state.controller("alpha-centauri"), "independent");

    state.shift_meter(&pack, "alpha-centauri", 45);
    assert_eq!(state.controller("alpha-centauri"), "independent");
    state.shift_meter(&pack, "alpha-centauri", 10);
    assert_eq!(state.controller("alpha-centauri"), "confederation");
    assert_eq!(
        state.news.last().map(String::as_str),
        Some("Confederation now holds Alpha Centauri.")
    );

    let dispatch = factions::compose_dispatch(&pack, &state);
    assert!(dispatch.contains("Confederation holds 5"), "{dispatch}");
    assert!(dispatch.contains("Alpha Centauri at +55"), "{dispatch}");
    assert!(!dispatch.contains("front lines hold"), "{dispatch}");
}

#[test]
fn control_persists_and_restores() {
    let pack = base_pack();
    let db = crate::SqliteStorage::open_memory().expect("storage opens");
    db.save_control(&[
        crate::ControlRow {
            system: "vega".to_owned(),
            meter: -70,
        },
        crate::ControlRow {
            system: "sol".to_owned(),
            meter: 40,
        },
    ])
    .expect("save");
    db.save_control(&[crate::ControlRow {
        system: "vega".to_owned(),
        meter: -75,
    }])
    .expect("upsert");
    let rows = db.load_control().expect("load");
    assert_eq!(rows.len(), 2);
    assert!(rows.contains(&crate::ControlRow {
        system: "vega".to_owned(),
        meter: -75,
    }));

    let storage = StorageResource {
        storage: Some(Box::new(db)),
    };
    let state = factions::init_faction_state(&storage, &pack);
    assert_eq!(state.meter("vega"), -75);
    assert_eq!(state.controller("vega"), "rebellion");
    assert_eq!(state.meter("sol"), 40);
}

#[test]
fn quest_turn_in_moves_reputation_and_control() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);

    h.server
        .world_mut()
        .get_mut::<ActiveQuests>(entity)
        .unwrap()
        .quests
        .push(ActiveQuest {
            id: "faction-test-bounty".to_owned(),
            name: "Sweep the Lanes".to_owned(),
            giver_system: "vega".to_owned(),
            giver_planet: "vega-station".to_owned(),
            target: QuestTarget::Bounty {
                system: "vega".to_owned(),
                target: "turncoat".to_owned(),
                killed: true,
            },
            reward_credits: 100,
            remaining_secs: None,
            procedural: true,
        });
    h.server.world_mut().entity_mut(entity).insert((
        ChiefEncounter {
            chief_name: "Dockmaster Rosh".to_owned(),
            portrait: None,
            mood: ChiefMood::TurnIn,
            text: "The rebellion thanks you.".to_owned(),
            options: vec![EncounterOption {
                id: "turn_in".to_owned(),
                label: "Collect payment".to_owned(),
            }],
        },
        PendingEncounter::TurnIn {
            quest_id: "faction-test-bounty".to_owned(),
        },
    ));
    h.send(RespondToEncounter {
        choice: "turn_in".to_owned(),
    });
    for _ in 0..4 {
        h.step();
    }

    let reputation = h
        .server
        .world()
        .get::<PlayerReputation>(entity)
        .expect("players carry reputation")
        .clone();
    assert!(
        reputation.standings.contains(&("rebellion".to_owned(), 5)),
        "turn-in must credit the giver faction: {:?}",
        reputation.standings
    );
    let state = h.server.world().resource::<factions::FactionState>();
    assert_eq!(state.meter("vega"), -70, "control shifts toward the giver");

    let mut controls = h
        .server
        .world_mut()
        .query_filtered::<&sv_protocol::SystemControl, With<bevy_replicon::prelude::Replicated>>();
    let vega = controls
        .iter(h.server.world())
        .find(|control| control.system == "vega")
        .expect("vega control entity replicates");
    assert_eq!(vega.meter, -70);
    assert_eq!(vega.controller, "rebellion");
}

#[test]
fn pirate_kills_move_reputation_and_control() {
    let (mut h, entity) = ready();

    let mut ship_state = sv_core::ShipState::new(10.0, 10.0, 50.0);
    ship_state.shields = 0.0;
    ship_state.armor = 0.0;
    h.server.world_mut().spawn((
        Ship(ship_state),
        ShipInfo {
            def_id: "turncoat".to_owned(),
            kind: ShipKind::Pirate,
            owner: None,
            name: "Vex Marauder".to_owned(),
        },
        SystemMembership("vega".to_owned()),
        Npc,
        LastDamagedBy(Some(1)),
    ));
    for _ in 0..4 {
        h.step();
    }

    let reputation = h
        .server
        .world()
        .get::<PlayerReputation>(entity)
        .expect("players carry reputation")
        .clone();
    assert!(
        reputation.standings.contains(&("rebellion".to_owned(), 2)),
        "pirate kills must credit the system owner exactly once: {:?}",
        reputation.standings
    );
    let state = h.server.world().resource::<factions::FactionState>();
    assert_eq!(
        state.meter("vega"),
        -62,
        "pirate kills strengthen the local controller"
    );
}
