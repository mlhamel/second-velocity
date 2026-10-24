use super::*;
use evr_protocol::{BuyOutfit, BuyShip, SellOutfit};

fn player(h: &mut Harness, owner: u64) -> Entity {
    h.server
        .world_mut()
        .query::<(Entity, &Owner)>()
        .iter(h.server.world())
        .find(|(_, id)| id.0 == owner)
        .unwrap()
        .0
}

fn ready() -> (Harness, Entity) {
    let mut h = Harness::new();
    h.step_until(200, |h| h.my_ship().is_some());
    let entity = player(&mut h, 1);
    (h, entity)
}

fn dock_earth(h: &mut Harness) {
    h.set_server_ship(1, |s| {
        s.position = Vec2::new(480.0, 0.0);
        s.velocity = Vec2::ZERO;
    });
    h.send(RequestLand {
        planet: "earth".into(),
    });
    let entity = player(h, 1);
    h.step_until(100, |h| h.server.world().get::<DockedAt>(entity).is_some());
}

fn settle(h: &mut Harness) {
    for _ in 0..8 {
        h.step();
    }
}

#[test]
fn hull_purchase_preserves_entity_cargo_quests_and_transfers_equipment() {
    let (mut h, entity) = ready();
    set_scenario_credits(h.server.world_mut(), 1, 20_000);
    dock_earth(&mut h);
    h.server
        .world_mut()
        .get_mut::<Wallet>(entity)
        .unwrap()
        .0
        .add_cargo("food", 3, 20);
    h.server
        .world_mut()
        .get_mut::<ActiveQuests>(entity)
        .unwrap()
        .completed
        .push("old-contract".into());
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    settle(&mut h);
    let world = h.server.world();
    assert_eq!(
        world.get::<ShipInfo>(entity).unwrap().def_id,
        "terran-courier"
    );
    assert_eq!(world.get::<Wallet>(entity).unwrap().0.credits, 8000);
    assert_eq!(world.get::<Wallet>(entity).unwrap().0.cargo_qty("food"), 3);
    assert_eq!(
        world.get::<EquippedOutfits>(entity).unwrap().ids,
        ["light-blaster"]
    );
    assert_eq!(
        world.get::<AccountState>(entity).unwrap().cargo_capacity,
        26
    );
    assert_eq!(world.get::<DockedAt>(entity).unwrap().location, "earth");
    assert_eq!(
        world.get::<ActiveQuests>(entity).unwrap().completed,
        ["old-contract"]
    );
    let ship = &world.get::<Ship>(entity).unwrap().0;
    assert_eq!(
        (ship.max_shields, ship.max_armor, ship.max_fuel),
        (45.0, 30.0, 140.0)
    );
    assert_eq!(
        world.get::<ShipSpec>(entity).unwrap().flight.max_speed,
        250.0
    );
}

#[test]
fn purchases_reject_undocked_insufficient_funds_unknown_stock_and_wrong_ports() {
    let (mut h, entity) = ready();
    h.send(BuyOutfit {
        outfit: "shield-generator-1".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        500
    );
    dock_earth(&mut h);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    h.send(BuyShip {
        ship: "missing".into(),
    });
    h.send(BuyOutfit {
        outfit: "missing".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<ShipInfo>(entity).unwrap().def_id,
        PLAYER_SHIP
    );
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        500
    );
    h.send(RequestLiftOff);
    settle(&mut h);
    let port_pos = h
        .server
        .world_mut()
        .query::<(&DockableTag, &Position)>()
        .iter(h.server.world())
        .find(|(p, _)| p.id == "lin-relay")
        .unwrap()
        .1
        .0;
    h.set_server_ship(1, |s| {
        s.position = port_pos;
        s.velocity = Vec2::ZERO;
    });
    h.send(RequestDock {
        system: "sol".into(),
        location: "lin-relay".into(),
    });
    settle(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 100_000);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    h.send(SellOutfit {
        outfit: "light-blaster".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        100_000
    );
    assert_eq!(
        h.server.world().get::<EquippedOutfits>(entity).unwrap().ids,
        ["light-blaster"]
    );
}

#[test]
fn same_update_transactions_cannot_overspend_or_stack_passive_equipment() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 1000);
    h.send(BuyOutfit {
        outfit: "shield-generator-1".into(),
    });
    h.send(BuyOutfit {
        outfit: "aux-fuel-tank".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        250
    );
    assert_eq!(
        h.server.world().get::<Ship>(entity).unwrap().0.max_shields,
        60.0
    );
    assert_eq!(
        h.server.world().get::<Ship>(entity).unwrap().0.max_fuel,
        120.0
    );
    set_scenario_credits(h.server.world_mut(), 1, 10_000);
    h.send(BuyOutfit {
        outfit: "shield-generator-2".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        10_000
    );
    h.send(SellOutfit {
        outfit: "shield-generator-1".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        10_375
    );
    assert_eq!(
        h.server.world().get::<Ship>(entity).unwrap().0.max_shields,
        40.0
    );
}

#[test]
fn weapon_slots_prevent_extra_mounts_and_incompatible_hull_swaps() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 100_000);
    h.send(BuyOutfit {
        outfit: "pulse-blaster".into(),
    });
    settle(&mut h);
    let credits = h.server.world().get::<Wallet>(entity).unwrap().0.credits;
    h.send(BuyOutfit {
        outfit: "light-blaster".into(),
    });
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        credits
    );
    assert_eq!(
        h.server.world().get::<ShipInfo>(entity).unwrap().def_id,
        PLAYER_SHIP
    );
    assert_eq!(
        h.server
            .world()
            .get::<EquippedOutfits>(entity)
            .unwrap()
            .ids
            .len(),
        2
    );
    h.send(RequestLiftOff);
    settle(&mut h);
    h.send(SetFiring(true));
    settle(&mut h);
    let mut damages = h
        .server
        .world_mut()
        .query::<&Projectile>()
        .iter(h.server.world())
        .filter(|p| p.owner == 1)
        .map(|p| p.damage as u32)
        .collect::<Vec<_>>();
    damages.sort();
    damages.dedup();
    assert_eq!(
        damages,
        [6, 12],
        "both independent equipped weapon banks must fire"
    );
}

#[test]
fn smaller_hull_rejects_cargo_without_discarding_it() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 100_000);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    settle(&mut h);
    h.server
        .world_mut()
        .get_mut::<Wallet>(entity)
        .unwrap()
        .0
        .add_cargo("food", 25, 26);
    let credits = h.server.world().get::<Wallet>(entity).unwrap().0.credits;
    h.send(BuyShip {
        ship: "shuttle".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<ShipInfo>(entity).unwrap().def_id,
        "terran-courier"
    );
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        credits
    );
    assert_eq!(
        h.server
            .world()
            .get::<Wallet>(entity)
            .unwrap()
            .0
            .cargo_qty("food"),
        25
    );
}

#[test]
fn capacity_changes_do_not_refuel_and_resale_clamps_remaining_fuel() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 2000);
    h.send(BuyOutfit {
        outfit: "aux-fuel-tank".into(),
    });
    settle(&mut h);
    let state = &h.server.world().get::<Ship>(entity).unwrap().0;
    assert_eq!((state.fuel, state.max_fuel), (120.0, 170.0));
    h.set_server_ship(1, |s| s.fuel = 160.0);
    h.send(SellOutfit {
        outfit: "aux-fuel-tank".into(),
    });
    settle(&mut h);
    let state = &h.server.world().get::<Ship>(entity).unwrap().0;
    assert_eq!((state.fuel, state.max_fuel), (120.0, 120.0));
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        1550
    );
}

struct FailingSave;
impl crate::Storage for FailingSave {
    fn create_account(&self, _: &str, _: &str) -> Result<crate::PlayerState, crate::StorageError> {
        Err(crate::StorageError::NotFound)
    }
    fn verify_login(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Option<crate::PlayerState>, crate::StorageError> {
        Err(crate::StorageError::NotFound)
    }
    fn save_player(&self, _: &str, _: &crate::PlayerState) -> Result<(), crate::StorageError> {
        Err(crate::StorageError::NotFound)
    }
    fn close(&self) {}
}

#[test]
fn failed_purchase_persistence_changes_neither_ship_nor_credits() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 20_000);
    h.server.insert_resource(StorageResource {
        storage: Some(Box::new(FailingSave)),
    });
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    h.send(BuyOutfit {
        outfit: "shield-generator-1".into(),
    });
    settle(&mut h);
    let world = h.server.world();
    assert_eq!(world.get::<Wallet>(entity).unwrap().0.credits, 20_000);
    assert_eq!(world.get::<ShipInfo>(entity).unwrap().def_id, PLAYER_SHIP);
    assert_eq!(
        world.get::<EquippedOutfits>(entity).unwrap().ids,
        ["light-blaster"]
    );
    assert_eq!(world.get::<Ship>(entity).unwrap().0.max_shields, 40.0);
}

#[test]
fn purchases_persist_immediately_and_autosave_keeps_owned_identity() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    let db = crate::SqliteStorage::open_memory().unwrap();
    db.create_account("pilot", "fixture-only").unwrap();
    h.server.insert_resource(StorageResource {
        storage: Some(Box::new(db)),
    });
    set_scenario_credits(h.server.world_mut(), 1, 20_000);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    settle(&mut h);
    h.send(SellOutfit {
        outfit: "light-blaster".into(),
    });
    settle(&mut h);
    let read = |h: &Harness| {
        h.server
            .world()
            .resource::<StorageResource>()
            .storage
            .as_ref()
            .unwrap()
            .verify_login("pilot", "fixture-only")
            .unwrap()
            .unwrap()
    };
    let state = read(&h);
    assert_eq!(state.ship_id, "terran-courier");
    assert_eq!(state.equipped_outfits, Some(vec![]));
    assert_eq!(state.credits, 8600);
    h.server.world_mut().resource_mut::<SaveTimer>().0 = 11.0;
    h.step();
    assert_eq!(read(&h).ship_id, "terran-courier");
    assert!(
        h.server
            .world()
            .get::<WeaponBanks>(entity)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn login_restores_absolute_position_resources_and_explicit_loadout() {
    let (mut h, entity) = ready();
    let db = crate::SqliteStorage::open_memory().unwrap();
    db.create_account("returning-pilot", "fixture-only")
        .unwrap();
    let saved = crate::PlayerState {
        ship_id: "terran-courier".into(),
        system: "alpha-centauri".into(),
        position: (1700.0, 123.0),
        fuel: 77.0,
        shields: 22.0,
        armor: 23.0,
        credits: 999,
        equipped_outfits: Some(vec![
            "light-blaster".into(),
            "shield-generator-1".into(),
            "aux-fuel-tank".into(),
        ]),
        ..default()
    };
    db.save_player("returning-pilot", &saved).unwrap();
    h.server.world_mut().despawn(entity);
    h.server.insert_resource(StorageResource {
        storage: Some(Box::new(db)),
    });
    h.send(LoginRequest {
        username: "returning-pilot".into(),
        password: "fixture-only".into(),
    });
    h.step_until(100, |h| h.server_ship_position(1).is_some());
    let restored = player(&mut h, 1);
    let state = &h.server.world().get::<Ship>(restored).unwrap().0;
    assert_eq!(state.position, Vec2::new(1700.0, 123.0));
    assert_eq!(
        (state.fuel, state.armor, state.max_shields, state.max_fuel),
        (77.0, 23.0, 65.0, 190.0)
    );
    assert!((state.shields - 22.0).abs() < 1.0);
    assert_eq!(
        h.server.world().get::<ShipInfo>(restored).unwrap().def_id,
        "terran-courier"
    );
}

#[test]
fn simultaneous_deaths_keep_separate_owned_snapshots_and_empty_loadouts() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 20_000);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    h.send(SellOutfit {
        outfit: "light-blaster".into(),
    });
    settle(&mut h);
    h.server
        .world_mut()
        .resource_scope(|world, content: Mut<Content>| {
            let mut commands = world.commands();
            let other = spawn_ship(
                &mut commands,
                &content.0,
                "terran-hauler",
                ShipKind::Player,
                Some(2),
                "sol",
                Vec2::ZERO,
            );
            commands.entity(other).insert((
                Wallet(Account::new(123)),
                ActiveQuests::default(),
                PlayerName("second".into()),
            ));
        });
    h.server.world_mut().flush();
    let other = player(&mut h, 2);
    h.server
        .world_mut()
        .get_mut::<Ship>(entity)
        .unwrap()
        .0
        .armor = 0.0;
    h.server.world_mut().get_mut::<Ship>(other).unwrap().0.armor = 0.0;
    h.step();
    let pending = h.server.world().resource::<PendingRecoveries>();
    assert_eq!(pending.0.len(), 2);
    assert_eq!(pending.0[&1].state.ship_id, "terran-courier");
    assert_eq!(pending.0[&1].state.equipped_outfits, Some(vec![]));
    assert_eq!(pending.0[&2].state.ship_id, "terran-hauler");
    assert_eq!(pending.0[&2].state.credits, 123);
    h.server
        .world_mut()
        .resource_mut::<PendingRecoveries>()
        .0
        .get_mut(&1)
        .unwrap()
        .remaining = 0.0;
    h.step_until(100, |h| h.server_ship_position(1).is_some());
    let restored = player(&mut h, 1);
    assert_eq!(
        h.server.world().get::<ShipInfo>(restored).unwrap().def_id,
        "terran-courier"
    );
    assert!(
        h.server
            .world()
            .get::<EquippedOutfits>(restored)
            .unwrap()
            .ids
            .is_empty()
    );
    assert_eq!(
        h.server.world().get::<Ship>(restored).unwrap().0.armor,
        30.0
    );
    assert!(
        h.server
            .world()
            .resource::<PendingRecoveries>()
            .0
            .contains_key(&2)
    );
}

#[test]
fn legacy_null_loadout_uses_defaults_but_explicit_empty_stays_empty() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = ContentPack::load_dir(&root).unwrap();
    let mut state = crate::PlayerState::default();
    assert_eq!(restore_ship(&pack, &state).unwrap().1, ["light-blaster"]);
    state.equipped_outfits = Some(vec![]);
    assert!(restore_ship(&pack, &state).unwrap().1.is_empty());
    state.equipped_outfits = Some(vec!["missing-outfit".into()]);
    assert!(restore_ship(&pack, &state).is_err());
}

#[test]
fn large_credit_balance_allows_a_valid_net_debit_trade_in() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, u32::MAX);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    settle(&mut h);
    assert_eq!(
        h.server.world().get::<ShipInfo>(entity).unwrap().def_id,
        "terran-courier"
    );
    assert_eq!(
        h.server.world().get::<Wallet>(entity).unwrap().0.credits,
        u32::MAX - 12_000
    );
}

#[test]
fn death_with_retired_outfit_drops_it_and_keeps_the_hull() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);
    set_scenario_credits(h.server.world_mut(), 1, 20_000);
    h.send(BuyShip {
        ship: "terran-courier".into(),
    });
    h.send(BuyOutfit {
        outfit: "shield-generator-1".into(),
    });
    settle(&mut h);
    h.server
        .world_mut()
        .resource_scope(|_, mut content: Mut<Content>| {
            content
                .0
                .outfits
                .retain(|outfit| outfit.id != "shield-generator-1");
        });
    h.server
        .world_mut()
        .get_mut::<Ship>(entity)
        .unwrap()
        .0
        .armor = 0.0;
    h.step();
    let pending = h.server.world().resource::<PendingRecoveries>();
    assert_eq!(pending.0[&1].state.ship_id, "terran-courier");
    assert_eq!(
        pending.0[&1].state.equipped_outfits,
        Some(vec!["light-blaster".into()])
    );
    h.server
        .world_mut()
        .resource_mut::<PendingRecoveries>()
        .0
        .get_mut(&1)
        .unwrap()
        .remaining = 0.0;
    h.step_until(100, |h| h.server_ship_position(1).is_some());
    let restored = player(&mut h, 1);
    assert_eq!(
        h.server.world().get::<ShipInfo>(restored).unwrap().def_id,
        "terran-courier"
    );
    assert_eq!(
        h.server
            .world()
            .get::<Ship>(restored)
            .unwrap()
            .0
            .max_shields,
        45.0
    );
}

#[test]
fn death_with_retired_hull_recovers_in_a_factory_shuttle() {
    let (mut h, entity) = ready();
    h.server
        .world_mut()
        .get_mut::<ShipInfo>(entity)
        .unwrap()
        .def_id = "retired-hull".into();
    h.server
        .world_mut()
        .get_mut::<Ship>(entity)
        .unwrap()
        .0
        .armor = 0.0;
    h.step();
    let pending = h.server.world().resource::<PendingRecoveries>();
    assert_eq!(pending.0[&1].state.ship_id, PLAYER_SHIP);
    assert_eq!(pending.0[&1].state.credits, STARTING_CREDITS);
    assert_eq!(
        pending.0[&1].state.equipped_outfits,
        Some(vec!["light-blaster".into()])
    );
    assert!(
        h.drain_notices()
            .iter()
            .any(|notice| notice.contains("factory shuttle"))
    );
    h.server
        .world_mut()
        .resource_mut::<PendingRecoveries>()
        .0
        .get_mut(&1)
        .unwrap()
        .remaining = 0.0;
    h.step_until(100, |h| h.server_ship_position(1).is_some());
    let restored = player(&mut h, 1);
    assert_eq!(
        h.server.world().get::<ShipInfo>(restored).unwrap().def_id,
        PLAYER_SHIP
    );
}
