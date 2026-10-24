use super::progression::ready;
use super::*;
use crate::gameplay::contracts::{self, ContractBoard};
use evr_protocol::{ChiefEncounter, ChiefMood, RespondToEncounter};

fn synthetic_board(h: &mut Harness, giver: (&str, &str), destination: (&str, &str)) {
    h.server
        .world_mut()
        .resource_mut::<ContractBoard>()
        .contracts = vec![evr_data::QuestDef {
        id: "contract-0".into(),
        name: "Synthetic Parcel".into(),
        giver: (giver.0.into(), giver.1.into()),
        kind: evr_data::QuestKind::Delivery {
            commodity: "food".into(),
            qty: 3,
            destination: (destination.0.into(), destination.1.into()),
            deadline_secs: None,
        },
        reward_credits: 555,
        description: "Test fixture".into(),
    }];
}

fn dock_port(h: &mut Harness, system: &str, port: &str) {
    let position = {
        let mut query = h.server.world_mut().query::<(&DockableTag, &Position)>();
        query
            .iter(h.server.world())
            .find(|(tag, _)| tag.system == system && tag.id == port)
            .map(|(_, pos)| pos.0)
            .expect("port exists")
    };
    h.set_server_ship(1, |ship| {
        ship.position = position + Vec2::new(30.0, 0.0);
        ship.velocity = Vec2::ZERO;
    });
    for _ in 0..30 {
        h.step();
    }
    h.send(evr_protocol::RequestDock {
        system: system.into(),
        location: port.into(),
    });
}

fn active_quest(h: &mut Harness) -> Option<evr_protocol::ActiveQuest> {
    h.server
        .world_mut()
        .query::<(&Owner, &ActiveQuests)>()
        .iter(h.server.world())
        .find(|(owner, _)| owner.0 == 1)
        .and_then(|(_, quests)| quests.quests.first().cloned())
}

#[test]
fn contract_offers_accept_refresh_and_turn_in_without_completing_the_ledger() {
    let (mut h, entity) = ready();
    synthetic_board(&mut h, ("vega", "vega-station"), ("sol", "lin-relay"));
    h.server
        .world_mut()
        .insert_resource(EncounterRng::with_roll(10));

    dock_port(&mut h, "vega", "vega-station");
    h.step_until(200, |h| {
        h.server
            .world()
            .get::<ChiefEncounter>(entity)
            .is_some_and(|enc| enc.mood == ChiefMood::Quest)
    });

    let offered = h
        .server
        .world()
        .get::<ChiefEncounter>(entity)
        .unwrap()
        .clone();
    assert!(offered.text.contains("Synthetic Parcel"));

    h.send(RespondToEncounter {
        choice: "accept_quest".into(),
    });
    h.step_until(200, |h| active_quest(h).is_some());
    let quest = active_quest(&mut h).expect("contract accepted");
    assert_eq!(quest.id, "contract-0");
    assert_eq!(quest.reward_credits, 555);
    assert!(
        quest.procedural,
        "board contracts must be flagged procedural"
    );

    contracts::force_contract_refresh(h.server.world_mut(), 1);
    h.step_until(200, |h| {
        active_quest(h).is_some_and(|q| q.id == "contract-0")
    });
    assert!(
        h.server
            .world_mut()
            .resource::<ContractBoard>()
            .contracts
            .iter()
            .all(|c| c.name != "Synthetic Parcel"),
        "refreshing must replace the board but keep the accepted job"
    );

    h.send(evr_protocol::RequestLiftOff);
    for _ in 0..10 {
        h.step();
    }
    dock_port(&mut h, "sol", "lin-relay");
    h.step_until(200, |h| {
        h.server
            .world()
            .get::<ChiefEncounter>(entity)
            .is_some_and(|enc| enc.mood == ChiefMood::TurnIn)
    });
    h.send(RespondToEncounter {
        choice: "turn_in".into(),
    });
    h.step_until(200, |h| active_quest(h).is_none());

    let quests = h
        .server
        .world()
        .get::<ActiveQuests>(entity)
        .unwrap()
        .clone();
    assert!(
        quests.completed.is_empty(),
        "procedural contracts must never enter the one-shot completed ledger"
    );
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        500 + 555,
        "turn-in must pay the contract reward"
    );
}

#[test]
fn authored_quests_keep_priority_over_board_contracts() {
    let (mut h, entity) = ready();
    synthetic_board(&mut h, ("sol", "earth"), ("sol", "lin-relay"));
    h.server
        .world_mut()
        .insert_resource(EncounterRng::with_roll(10));

    h.set_server_ship(1, |ship| {
        ship.position = Vec2::new(480.0, 0.0);
        ship.velocity = Vec2::ZERO;
    });
    for _ in 0..10 {
        h.step();
    }
    h.send(evr_protocol::RequestLand {
        planet: "earth".into(),
    });
    h.step_until(200, |h| {
        h.server
            .world()
            .get::<ChiefEncounter>(entity)
            .is_some_and(|enc| enc.mood == ChiefMood::Quest)
    });

    let offered = h
        .server
        .world()
        .get::<ChiefEncounter>(entity)
        .unwrap()
        .clone();
    assert!(
        offered.text.contains("Newhaven"),
        "the authored quest must win the offer over the synthetic contract"
    );

    h.send(RespondToEncounter {
        choice: "accept_quest".into(),
    });
    h.step_until(200, |h| active_quest(h).is_some());
    let quest = active_quest(&mut h).expect("authored quest accepted");
    assert_eq!(quest.id, "newhaven-relief");
    assert!(!quest.procedural, "authored quests are not procedural");
}
