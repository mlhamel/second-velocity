use super::*;

impl Harness {
    fn station_player(&mut self) -> Entity {
        self.server
            .world_mut()
            .query_filtered::<(Entity, &Owner), With<PlayerShip>>()
            .iter(self.server.world())
            .find(|(_, owner)| owner.0 == 1)
            .unwrap()
            .0
    }

    fn station_port(&mut self, location: &str) -> (DockableTag, Vec2) {
        self.server
            .world_mut()
            .query::<(&DockableTag, &Position)>()
            .iter(self.server.world())
            .find(|(port, _)| port.system == "sol" && port.id == location)
            .map(|(port, pos)| (port.clone(), pos.0))
            .unwrap()
    }

    fn dock_at_station(&mut self, location: &str) -> Entity {
        self.step_until(200, |h| h.my_ship().is_some());
        let (_, position) = self.station_port(location);
        self.set_server_ship(1, |ship| {
            ship.position = position + Vec2::X * 100.0;
            ship.velocity = Vec2::ZERO;
        });
        self.send(RequestDock {
            system: "sol".into(),
            location: location.into(),
        });
        let client = self.my_ship().unwrap();
        self.step_until(100, |h| {
            h.client_component::<DockedAt>(client)
                .is_some_and(|d| d.location == location)
        });
        client
    }
}

#[test]
fn station_positions_replicate_and_orbit_without_moving_fixed_ports() {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    h.step_until(100, |h| {
        h.client
            .world_mut()
            .query::<&DockableTag>()
            .iter(h.client.world())
            .filter(|tag| tag.kind == DockableKind::Station)
            .count()
            == 2
    });
    let (_, start) = h.station_port("lin-relay");
    let (_, fixed) = h.station_port("wayfarer-exchange");
    for _ in 0..30 {
        h.step();
    }
    let (_, end) = h.station_port("lin-relay");
    assert!(start.distance(end) > 1.0);
    assert!(((end - Vec2::new(500.0, 0.0)).length() - 240.0).abs() < 0.01);
    assert_eq!(h.station_port("wayfarer-exchange").1, fixed);
}

#[test]
fn station_range_is_authoritative_and_other_players_cannot_be_docked() {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    let player = h.station_player();
    // The replicated position is deliberately forged near the port, but Ship
    // stays at the far-away spawn: only authoritative state can authorize docking.
    let (_, port_pos) = h.station_port("wayfarer-exchange");
    h.server.world_mut().get_mut::<Position>(player).unwrap().0 = port_pos;
    h.send(RequestDock {
        system: "sol".into(),
        location: "wayfarer-exchange".into(),
    });
    for _ in 0..6 {
        h.step();
    }
    assert!(h.server.world().get::<DockedAt>(player).is_none());
    let other = h
        .server
        .world_mut()
        .resource_scope(|world, content: Mut<Content>| {
            let mut commands = world.commands();
            let entity = spawn_ship(
                &mut commands,
                &content.0,
                PLAYER_SHIP,
                ShipKind::Player,
                Some(2),
                "sol",
                port_pos,
            );
            commands
                .entity(entity)
                .insert((Wallet(Account::new(500)), ActiveQuests::default()));
            entity
        });
    h.server.world_mut().flush();
    h.send(RequestDock {
        system: "sol".into(),
        location: "wayfarer-exchange".into(),
    });
    for _ in 0..6 {
        h.step();
    }
    assert!(h.server.world().get::<DockedAt>(player).is_none());
    assert!(h.server.world().get::<DockedAt>(other).is_none());
}

#[test]
fn station_docking_follows_orbit_and_undocks_at_current_port_safely() {
    let mut h = Harness::new();
    let client = h.dock_at_station("lin-relay");
    let player = h.station_player();
    let (tag, start) = h.station_port("lin-relay");
    let offset = h.server.world().get::<Ship>(player).unwrap().0.position - start;
    let fuel = h.server.world().get::<Ship>(player).unwrap().0.fuel;
    h.send(SetWarp(true));
    h.send(SetThrottle(1.0));
    for _ in 0..60 {
        h.step();
    }
    let (_, current) = h.station_port("lin-relay");
    assert!(current.distance(start) > 2.0);
    let ship = &h.server.world().get::<Ship>(player).unwrap().0;
    assert!((ship.position - current - offset).length() < 0.01);
    assert_eq!(ship.velocity, Vec2::ZERO);
    assert_eq!(ship.fuel, fuel);
    h.send(RequestLiftOff);
    h.step_until(100, |h| h.client_component::<DockedAt>(client).is_none());
    let (_, current) = h.station_port("lin-relay");
    let ship = &h.server.world().get::<Ship>(player).unwrap().0;
    assert!(ship.position.distance(current) > tag.radius + SHIP_RADIUS);
    assert!(ship.position.distance(current) < tag.radius + SHIP_RADIUS + 50.0);
    assert_eq!(ship.velocity, Vec2::ZERO);
    assert!(h.server.world().get::<DockingOffset>(player).is_none());
    assert!(h.server.world().get::<ChiefEncounter>(player).is_none());
}

#[test]
fn station_docking_ignores_stale_membership_and_uses_its_own_market() {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    let player = h.station_player();
    h.server
        .world_mut()
        .get_mut::<SystemMembership>(player)
        .unwrap()
        .0 = "wolf-359".into();
    let client = h.dock_at_station("wayfarer-exchange");
    assert_eq!(
        h.client_component::<DockedAt>(client).unwrap().kind,
        DockableKind::Station
    );
    assert_eq!(
        h.server.world().get::<SystemMembership>(player).unwrap().0,
        "sol"
    );
    let credits = h.server.world().get::<Wallet>(player).unwrap().0.credits;
    h.send(BuyCommodity {
        commodity: "food".into(),
        qty: 2,
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<Wallet>(player)
            .unwrap()
            .0
            .cargo_qty("food")
            == 2
    });
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        credits - 36
    );
    h.send(SellCommodity {
        commodity: "food".into(),
        qty: 1,
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<Wallet>(player)
            .unwrap()
            .0
            .cargo_qty("food")
            == 1
    });
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        credits - 18
    );
    h.send(BuyCommodity {
        commodity: "isotopes".into(),
        qty: 1,
    });
    h.send(BuyCommodity {
        commodity: "food".into(),
        qty: u32::MAX,
    });
    for _ in 0..6 {
        h.step();
    }
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        credits - 18
    );
}

#[test]
fn station_without_master_or_market_still_docks_refuels_and_clears_encounters() {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    let station = h
        .server
        .world_mut()
        .resource_mut::<Content>()
        .into_inner()
        .0
        .systems
        .iter_mut()
        .find(|s| s.id == "sol")
        .unwrap()
        .stations
        .iter_mut()
        .find(|s| s.id == "wayfarer-exchange")
        .unwrap();
    station.master = None;
    station.commodities.clear();
    h.set_server_ship(1, |ship| ship.fuel = 100.0);
    let player = h.station_player();
    h.server.world_mut().entity_mut(player).insert((
        ChiefEncounter {
            chief_name: "Old hail".into(),
            portrait: None,
            mood: ChiefMood::Offer,
            text: "".into(),
            options: vec![],
        },
        PendingEncounter::ShipHail {
            ship_name: "Old hail".into(),
            fuel_gift: 25.0,
        },
    ));
    h.dock_at_station("wayfarer-exchange");
    assert_eq!(h.server.world().get::<Ship>(player).unwrap().0.fuel, 120.0);
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        480
    );
    assert!(h.server.world().get::<ChiefEncounter>(player).is_none());
    assert!(h.server.world().get::<PendingEncounter>(player).is_none());
    h.send(BuyCommodity {
        commodity: "food".into(),
        qty: 1,
    });
    for _ in 0..6 {
        h.step();
    }
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        480
    );
}

#[test]
fn station_docked_ships_keep_all_environmental_hazard_immunity() {
    let mut h = Harness::new();
    h.dock_at_station("lin-relay");
    let player = h.station_player();
    let position = h.server.world().get::<Ship>(player).unwrap().0.position;
    h.server.world_mut().spawn((
        StarTag {
            system: "sol".into(),
            radius: 1000.0,
        },
        Position(position),
    ));
    h.server.world_mut().spawn((
        BlackHoleTag {
            system: "sol".into(),
            radius: 1000.0,
            influence: 1500.0,
        },
        BlackHoleGravity(200.0),
        Position(position),
    ));
    let mut commands = h.server.world_mut().commands();
    spawn_meteorite(
        &mut commands,
        "sol",
        position,
        100.0,
        position,
        Vec2::ZERO,
        MeteoriteSize::Large,
        0.0,
        0,
    );
    h.server.world_mut().flush();
    for _ in 0..12 {
        h.step();
    }
    let ship = &h.server.world().get::<Ship>(player).unwrap().0;
    assert_eq!((ship.shields, ship.armor), (40.0, 25.0));
    assert!(h.server.world().get::<DockedAt>(player).is_some());
}

#[test]
fn station_quest_delivery_roundtrip_and_planet_turn_in_work() {
    let mut h = Harness::new();
    h.server.insert_resource(EncounterRng::with_roll(0));
    let client = h.dock_at_station("lin-relay");
    let player = h.station_player();
    h.send(RespondToEncounter {
        choice: "accept_quest".into(),
    });
    h.step_until(100, |h| {
        !h.server
            .world()
            .get::<ActiveQuests>(player)
            .unwrap()
            .quests
            .is_empty()
    });
    let encoded = encode_active_quests(h.server.world().get::<ActiveQuests>(player).unwrap());
    assert!(encoded.contains("\"giver_planet\":\"lin-relay\""));
    assert!(encoded.contains("\"destination_planet\":\"wayfarer-exchange\""));
    assert_eq!(
        parse_active_quests(&encoded).quests[0].id,
        "relay-calibration-seals"
    );
    h.send(RequestLiftOff);
    h.step_until(100, |h| h.client_component::<DockedAt>(client).is_none());
    h.dock_at_station("wayfarer-exchange");
    h.send(RespondToEncounter {
        choice: "turn_in".into(),
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<ActiveQuests>(player)
            .unwrap()
            .completed
            .contains(&"relay-calibration-seals".into())
    });
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        850
    );
    h.send(RequestLiftOff);
    h.step_until(100, |h| h.client_component::<DockedAt>(client).is_none());
    h.server.insert_resource(EncounterRng::with_roll(0));
    h.dock_at_station("wayfarer-exchange");
    h.send(RespondToEncounter {
        choice: "accept_quest".into(),
    });
    h.step_until(100, |h| {
        !h.server
            .world()
            .get::<ActiveQuests>(player)
            .unwrap()
            .quests
            .is_empty()
    });
    h.send(RequestLiftOff);
    h.step_until(100, |h| h.client_component::<DockedAt>(client).is_none());
    h.set_server_ship(1, |ship| {
        ship.position = Vec2::new(480.0, 0.0);
        ship.velocity = Vec2::ZERO;
    });
    h.send(RequestLand {
        planet: "earth".into(),
    });
    h.step_until(100, |h| h.client_component::<DockedAt>(client).is_some());
    h.send(RespondToEncounter {
        choice: "turn_in".into(),
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<ActiveQuests>(player)
            .unwrap()
            .completed
            .contains(&"wayfarer-clinic-crates".into())
    });
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        1130
    );
}

#[test]
fn station_quest_json_survives_sqlite_and_legacy_quests_still_parse() {
    let legacy = r#"[{"id":"old","name":"Old","giver_system":"sol","giver_planet":"earth","target":{"Delivery":{"commodity":"medicine","qty":1,"destination_system":"sol","destination_planet":"lin-relay"}},"reward_credits":5,"remaining_secs":null}]"#;
    let quests = parse_active_quests(legacy);
    assert_eq!(quests.quests.len(), 1);
    let storage = crate::SqliteStorage::open_memory().unwrap();
    let mut state = storage
        .create_account("station-save", "test-only-password")
        .unwrap();
    state.active_quests_json = encode_active_quests(&quests);
    storage.save_player("station-save", &state).unwrap();
    let restored = storage
        .verify_login("station-save", "test-only-password")
        .unwrap()
        .unwrap();
    assert_eq!(parse_active_quests(&restored.active_quests_json), quests);
}

#[test]
fn station_master_turns_in_completed_bounties() {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    let player = h.station_player();
    h.server
        .world_mut()
        .entity_mut(player)
        .insert(ActiveQuests {
            quests: vec![ActiveQuest {
                id: "test-station-bounty".into(),
                name: "Station bounty".into(),
                giver_system: "sol".into(),
                giver_planet: "wayfarer-exchange".into(),
                target: QuestTarget::Bounty {
                    system: "alpha-centauri".into(),
                    target: "turncoat".into(),
                    killed: true,
                },
                reward_credits: 600,
                remaining_secs: None,
                procedural: false,
            }],
            completed: vec![],
        });
    h.dock_at_station("wayfarer-exchange");
    assert_eq!(
        h.server.world().get::<ChiefEncounter>(player).unwrap().mood,
        ChiefMood::TurnIn
    );
    h.send(RespondToEncounter {
        choice: "turn_in".into(),
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<ActiveQuests>(player)
            .unwrap()
            .quests
            .is_empty()
    });
    assert_eq!(
        h.server.world().get::<Wallet>(player).unwrap().0.credits,
        1100
    );
}

#[test]
fn moons_replicate_orbit_and_landable_moons_support_landing_while_ambient_moons_do_not() {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    h.step_until(100, |h| {
        h.client
            .world_mut()
            .query::<&MoonTag>()
            .iter(h.client.world())
            .any(|moon| moon.id == "luna" && moon.landable)
            && h.client
                .world_mut()
                .query::<&MoonTag>()
                .iter(h.client.world())
                .any(|moon| moon.id == "cinder" && !moon.landable)
    });

    // Ambient moon 'cinder' has no DockableTag.
    assert!(
        !h.server
            .world_mut()
            .query::<&DockableTag>()
            .iter(h.server.world())
            .any(|tag| tag.id == "cinder")
    );

    // Landable moon 'luna' has a DockableTag with DockableKind::Moon and accepts RequestLand.
    let (luna_tag, luna_pos) = h.station_port("luna");
    assert_eq!(luna_tag.kind, DockableKind::Moon);
    h.set_server_ship(1, |ship| {
        ship.position = luna_pos + Vec2::X * 80.0;
        ship.velocity = Vec2::ZERO;
    });
    h.send(RequestLand {
        planet: "luna".into(),
    });
    let client = h.my_ship().unwrap();
    h.step_until(100, |h| {
        h.client_component::<DockedAt>(client)
            .is_some_and(|d| d.location == "luna" && d.kind == DockableKind::Moon)
    });
}
