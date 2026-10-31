use std::fs;
use std::path::Path;

use sv_data::{ContentError, ContentPack};

const DUPLICATE_SHIP: &str = r#"( id: "dupe", name: "Dupe", description: "", price: 1, mass: 1.0, turn_rate: 1.0, acceleration: 1.0, max_speed: 1.0, max_shields: 1.0, max_armor: 1.0, fuel: 1, cargo: 1, weapon_slots: 1 )"#;

#[test]
fn base_pack_loads() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = ContentPack::load_dir(&root).expect("base pack must load");

    assert_eq!(pack.manifest.id, "sv-base");
    assert_eq!(pack.quadrants.len(), 1);
    assert_eq!(pack.quadrants[0].id, "sol-reach");
    assert_eq!(pack.ships.len(), 25);
    assert_eq!(pack.systems.len(), 15);
    assert!(
        pack.systems.iter().all(|sys| sys.quadrant == "sol-reach"),
        "every sol-reach system must be tagged with its quadrant"
    );
    assert_eq!(pack.outfits.len(), 8);
    assert_eq!(pack.commodities.len(), 8);
    assert_eq!(pack.quests.len(), 12);
    assert_eq!(pack.scenarios.len(), 18);
    assert_eq!(pack.factions.len(), 4);
    assert_eq!(pack.pilots.len(), 3);

    let trader = pack
        .pilot("cautious-trader")
        .expect("cautious-trader pilot exists");
    assert_eq!(trader.home_system, "sol");
    assert_eq!(trader.home_port, "earth");
    let courier = pack.pilot("courier").expect("courier pilot exists");
    assert_eq!(courier.home_port, "lin-relay");
    let hunter = pack
        .pilot("bounty-hunter")
        .expect("bounty-hunter pilot exists");
    assert_eq!(hunter.home_system, "vega");

    let confederation = pack
        .faction("confederation")
        .expect("confederation faction exists");
    assert_eq!(confederation.side, sv_data::FactionSide::Confederation);
    let rebellion = pack.faction("rebellion").expect("rebellion faction exists");
    assert_eq!(rebellion.side, sv_data::FactionSide::Rebellion);
    let independent = pack
        .faction("independent")
        .expect("independent faction exists");
    assert_eq!(independent.side, sv_data::FactionSide::Neutral);
    let pirate = pack.faction("pirate").expect("pirate faction exists");
    assert_eq!(pirate.side, sv_data::FactionSide::Neutral);

    let vega = pack.system("vega").expect("vega exists");
    assert_eq!(vega.government, "rebellion");

    let food = pack.commodity("food").expect("food commodity exists");
    assert_eq!(food.name, "Foodstuffs");
    assert_eq!(food.icon.as_deref(), Some("textures/commodities/food.png"));

    let shuttle = pack.ship("shuttle").expect("shuttle exists");
    assert_eq!(shuttle.name, "Shuttle");
    assert_eq!(shuttle.race, "terran");
    assert_eq!(shuttle.size, sv_data::ShipSize::Light);
    assert_eq!(shuttle.default_outfits, vec!["light-blaster".to_owned()]);

    let dreadnought = pack
        .ship("korath-dreadnought")
        .expect("korath-dreadnought exists");
    assert_eq!(dreadnought.race, "korath");
    assert_eq!(dreadnought.size, sv_data::ShipSize::Capital);

    let sol = pack.system("sol").expect("sol exists");
    assert_eq!(sol.government, "confederation");
    assert_eq!(sol.planets.len(), 1);
    assert_eq!(sol.planets[0].commodities.len(), 5);
    assert!(sol.planets[0].chief.is_some());
    assert!(sol.meteorites.is_some());
    assert!(sol.links.contains(&"alpha-centauri".to_owned()));
    assert!(sol.links.contains(&"vega".to_owned()));

    let sol_star = sol.star.as_ref().expect("sol has a star");
    assert!(
        sol.black_hole.is_none(),
        "old systems need no black-hole field"
    );
    let hole = pack
        .system("cygnus-x1")
        .and_then(|system| system.black_hole.as_ref())
        .expect("Cygnus X-1 has a black hole");
    assert_eq!(hole.radius, 90.0);
    assert_eq!(hole.influence, 500.0);
    assert_eq!(sol_star.radius, 240.0);
    assert_eq!(
        sol_star.sprite.as_deref(),
        Some("textures/stars/corona.png")
    );
    assert_eq!(
        pack.system("sirius")
            .and_then(|s| s.star.as_ref())
            .map(|s| s.radius),
        Some(220.0)
    );
    assert_eq!(
        pack.system("cygnus-x1")
            .and_then(|s| s.star.as_ref())
            .map(|s| s.radius),
        Some(220.0)
    );
    assert_eq!(
        pack.system("hawking")
            .and_then(|s| s.star.as_ref())
            .map(|s| s.radius),
        Some(220.0)
    );
    assert!(
        pack.system("alpha-centauri")
            .is_some_and(|s| s.star.is_none())
    );

    let alpha = pack
        .system("alpha-centauri")
        .expect("alpha centauri exists");
    assert!(alpha.pirate);
    assert!(alpha.links.contains(&"sol".to_owned()));
    assert!(alpha.planets[0].chief.is_some());
    assert!(!pack.system("sol").unwrap().pirate);

    let relief = pack
        .quest("newhaven-relief")
        .expect("newhaven-relief quest exists");
    assert_eq!(relief.giver, ("sol".to_owned(), "earth".to_owned()));
    assert_eq!(relief.reward_credits, 450);

    let trade_scenario = pack
        .scenario("sol-centauri-trade")
        .expect("sol-centauri-trade scenario exists");
    assert!(!trade_scenario.steps.is_empty());

    let blaster = pack.outfit("light-blaster").expect("light-blaster exists");
    let sv_data::OutfitKind::Weapon(weapon) = &blaster.kind else {
        panic!("light blaster must be a weapon")
    };
    assert!(weapon.damage > 0.0);
}

#[test]
fn star_placements_keep_heat_zones_clear() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = ContentPack::load_dir(&root).expect("base pack must load");

    const MEMBERSHIP_RADIUS: f32 = 800.0;
    const PLANET_OFFSET_X: f32 = 500.0;
    const PLANET_SPACING: f32 = 600.0;
    const PLANET_RADIUS: f32 = 90.0;

    for system in &pack.systems {
        let Some(star) = &system.star else {
            continue;
        };
        let star_pos = (
            system.position.0 + star.position.0,
            system.position.1 + star.position.1,
        );
        let heat_radius = 1.8 * star.radius;
        let offset_len = (star.position.0.hypot(star.position.1)).max(0.0);

        for other in &pack.systems {
            if other.id == system.id {
                continue;
            }
            let dx = star_pos.0 - other.position.0;
            let dy = star_pos.1 - other.position.1;
            let distance = dx.hypot(dy);
            assert!(
                distance > MEMBERSHIP_RADIUS + heat_radius,
                "{} star heat zone reaches into {}'s membership disc (dist {distance:.0})",
                system.id,
                other.id
            );
        }

        if let Some(field) = &system.meteorites {
            assert!(
                offset_len > field.radius + heat_radius,
                "{} star heat zone penetrates its own meteorite field disc",
                system.id
            );
        }

        for (index, planet) in system.planets.iter().enumerate() {
            let planet_dx =
                star_pos.0 - (system.position.0 + PLANET_OFFSET_X + index as f32 * PLANET_SPACING);
            let planet_dy = star_pos.1 - system.position.1;
            let distance = planet_dx.hypot(planet_dy) - PLANET_RADIUS;
            assert!(
                distance > heat_radius,
                "{} star heat zone reaches planet {}'s approach lane",
                system.id,
                planet.id
            );
        }

        assert!(
            offset_len - heat_radius > 400.0,
            "{} star heat zone comes too close to its system's spawn point",
            system.id
        );
    }
}

#[test]
fn duplicate_ids_are_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["a.ron", "b.ron"], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(dir.join("a.ron"), DUPLICATE_SHIP).expect("ship must be writable");
    fs::write(dir.join("b.ron"), DUPLICATE_SHIP).expect("ship must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(result, Err(ContentError::DuplicateId { .. })),
        "expected DuplicateId, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn unknown_outfit_reference_is_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-ref-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["a.ron"], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
        r#"( id: "s", name: "S", description: "", price: 1, mass: 1.0, turn_rate: 1.0, acceleration: 1.0, max_speed: 1.0, max_shields: 1.0, max_armor: 1.0, fuel: 1, cargo: 1, weapon_slots: 1, default_outfits: ["missing-gun"] )"#,
    )
    .expect("ship must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(result, Err(ContentError::UnknownReference { .. })),
        "expected UnknownReference, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn missing_sprite_asset_is_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-sprite-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["a.ron"], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
        r#"( id: "s", name: "S", description: "", price: 1, mass: 1.0, turn_rate: 1.0, acceleration: 1.0, max_speed: 1.0, max_shields: 1.0, max_armor: 1.0, fuel: 1, cargo: 1, weapon_slots: 1, sprite: Some("textures/ships/ghost.png") )"#,
    )
    .expect("ship must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(
            result,
            Err(ContentError::UnknownReference { kind: "asset", .. })
        ),
        "expected missing-asset rejection, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn missing_outfit_icon_is_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-icon-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], outfits: ["a.ron"] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
        r#"( id: "o", name: "O", description: "", price: 1, mass: 1.0, kind: Weapon((damage: 1.0, cooldown: 1.0, projectile_speed: 1.0, projectile_ttl: 1.0)), icon: Some("textures/outfits/ghost.png") )"#,
    )
    .expect("outfit must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(
            result,
            Err(ContentError::UnknownReference { kind: "asset", .. })
        ),
        "expected missing-asset rejection, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn invalid_asset_paths_are_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-asset-test-{}", std::process::id()));
    let assets = dir.join("assets/textures");
    let content = dir.join("content");
    fs::create_dir_all(&assets).expect("dirs must be creatable");
    fs::create_dir_all(&content).expect("dirs must be creatable");

    for (sprite, kind) in [
        ("textures/../../secret.png", "asset (invalid path)"),
        ("textures/ship.txt", "asset (not an image)"),
        ("textures/missing.png", "asset"),
    ] {
        fs::write(
            content.join("ship.ron"),
            format!(
                r#"( id: "s", name: "S", description: "", price: 1, mass: 1.0, turn_rate: 1.0, acceleration: 1.0, max_speed: 1.0, max_shields: 1.0, max_armor: 1.0, fuel: 1, cargo: 1, weapon_slots: 1, sprite: Some("{sprite}") )"#
            ),
        )
        .expect("ship must be writable");
        fs::write(
            content.join("manifest.ron"),
            r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["ship.ron"], outfits: [] )"#,
        )
        .expect("manifest must be writable");

        let result = ContentPack::load_dir(&content);
        assert!(
            matches!(
                &result,
                Err(ContentError::UnknownReference {
                    kind: found_kind,
                    ..
                }) if *found_kind == kind
            ),
            "sprite {sprite}: expected {kind}, got {result:?}"
        );
    }

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn non_reciprocal_links_are_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-link-test-{}", std::process::id()));
    let q_dir = dir.join("quadrants/q");
    fs::create_dir_all(&q_dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), quadrants: ["quadrants/q/manifest.ron"], ships: [], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        q_dir.join("manifest.ron"),
        r#"( id: "q", name: "Q", origin: (0.0, 0.0), size: (10000.0, 10000.0), systems: ["a.ron", "b.ron"] )"#,
    )
    .expect("quadrant manifest must be writable");
    fs::write(
        q_dir.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: ["b"], planets: [] )"#,
    )
    .expect("system must be writable");
    fs::write(
        q_dir.join("b.ron"),
        r#"( id: "b", name: "B", government: "independent", description: "", position: (100.0, 0.0), links: [], planets: [] )"#,
    )
    .expect("system must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(
            result,
            Err(ContentError::UnknownReference {
                kind: "reciprocal link",
                ..
            })
        ),
        "expected non-reciprocal link rejection, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn unknown_quest_planet_reference_is_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-quest-test-{}", std::process::id()));
    let q_dir = dir.join("quadrants/q");
    fs::create_dir_all(&q_dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), quadrants: ["quadrants/q/manifest.ron"], ships: [], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        q_dir.join("manifest.ron"),
        r#"( id: "q", name: "Q", origin: (0.0, 0.0), size: (10000.0, 10000.0), systems: ["a.ron"], quests: ["q.ron"] )"#,
    )
    .expect("quadrant manifest must be writable");
    fs::write(
        q_dir.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "p", name: "P", description: "", commodities: [] )] )"#,
    )
    .expect("system must be writable");
    fs::write(
        q_dir.join("q.ron"),
        r#"( id: "q1", name: "Q", giver: ("a", "p"), kind: Delivery(commodity: "food", qty: 1, destination: ("a", "nowhere")), reward_credits: 100, description: "" )"#,
    )
    .expect("quest must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(
            result,
            Err(ContentError::UnknownReference {
                kind: "quest destination planet",
                ..
            })
        ),
        "expected quest destination planet rejection, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn missing_chief_portrait_is_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-chief-test-{}", std::process::id()));
    let q_dir = dir.join("quadrants/q");
    fs::create_dir_all(&q_dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), quadrants: ["quadrants/q/manifest.ron"], ships: [], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        q_dir.join("manifest.ron"),
        r#"( id: "q", name: "Q", origin: (0.0, 0.0), size: (10000.0, 10000.0), systems: ["a.ron"] )"#,
    )
    .expect("quadrant manifest must be writable");
    fs::write(
        q_dir.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "p", name: "P", description: "", chief: Some(( name: "Chief", portrait: Some("textures/chiefs/ghost.png"), greeting: "", offer_text: "", tribute_text: "", quest_text: "" )), commodities: [] )] )"#,
    )
    .expect("system must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(
            result,
            Err(ContentError::UnknownReference { kind: "asset", .. })
        ),
        "expected missing chief portrait rejection, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn unknown_scenario_reference_is_rejected() {
    let dir = std::env::temp_dir().join(format!("sv-data-scenario-test-{}", std::process::id()));
    let q_dir = dir.join("quadrants/q");
    fs::create_dir_all(&q_dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), quadrants: ["quadrants/q/manifest.ron"], ships: [], outfits: [], scenarios: ["s.ron"] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        q_dir.join("manifest.ron"),
        r#"( id: "q", name: "Q", origin: (0.0, 0.0), size: (10000.0, 10000.0), systems: ["a.ron"] )"#,
    )
    .expect("quadrant manifest must be writable");
    fs::write(
        q_dir.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "p", name: "P", description: "", commodities: [] )] )"#,
    )
    .expect("system must be writable");
    fs::write(
        dir.join("s.ron"),
        r#"( id: "s1", name: "S", description: "", steps: [FlyToPlanet(system: "a", planet: "ghost", land: true)] )"#,
    )
    .expect("scenario must be writable");

    let result = ContentPack::load_dir(&dir);
    assert!(
        matches!(
            result,
            Err(ContentError::UnknownReference {
                kind: "scenario target planet",
                ..
            })
        ),
        "expected scenario target planet rejection, got {result:?}"
    );

    fs::remove_dir_all(&dir).expect("temp dir must be removable");
}

#[test]
fn quadrant_edge_clearance_overlap_and_containment_are_enforced() {
    let dir = std::env::temp_dir().join(format!("sv-data-quadrant-test-{}", std::process::id()));
    let q1 = dir.join("quadrants/alpha");
    let q2 = dir.join("quadrants/beta");
    fs::create_dir_all(&q1).unwrap();
    fs::create_dir_all(&q2).unwrap();

    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), quadrants: ["quadrants/alpha/manifest.ron", "quadrants/beta/manifest.ron"], ships: [], outfits: [] )"#,
    )
    .unwrap();

    // 1. Overlapping quadrant rectangles are rejected.
    fs::write(
        q1.join("manifest.ron"),
        r#"( id: "alpha", name: "Alpha", origin: (0.0, 0.0), size: (4000.0, 4000.0), systems: [] )"#,
    )
    .unwrap();
    fs::write(
        q2.join("manifest.ron"),
        r#"( id: "beta", name: "Beta", origin: (3000.0, 0.0), size: (4000.0, 4000.0), systems: [] )"#,
    )
    .unwrap();
    assert!(matches!(
        ContentPack::load_dir(&dir),
        Err(ContentError::UnknownReference {
            kind: "quadrant overlap",
            ..
        })
    ));

    // Fix q2 to sit adjacent (non-overlapping): [2000..6000] vs [-2000..2000].
    fs::write(
        q2.join("manifest.ron"),
        r#"( id: "beta", name: "Beta", origin: (4000.0, 0.0), size: (4000.0, 4000.0), systems: ["b.ron"] )"#,
    )
    .unwrap();
    fs::write(
        q1.join("manifest.ron"),
        r#"( id: "alpha", name: "Alpha", origin: (0.0, 0.0), size: (4000.0, 4000.0), systems: ["a.ron"] )"#,
    )
    .unwrap();

    // 2. System closer than 800px to quadrant edge is rejected (max_x = 2000, pos = 1200 -> clearance = 800, not strictly > 800).
    fs::write(
        q1.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (1200.0, 0.0), links: [], planets: [( id: "pa", name: "PA", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    fs::write(
        q2.join("b.ron"),
        r#"( id: "b", name: "B", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "pb", name: "PB", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    assert!(matches!(
        ContentPack::load_dir(&dir),
        Err(ContentError::UnknownReference {
            kind: "quadrant edge clearance",
            ..
        })
    ));

    // 3. Duplicate planet/station ID across different quadrants is rejected.
    fs::write(
        q1.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "shared-port", name: "PA", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    fs::write(
        q2.join("b.ron"),
        r#"( id: "b", name: "B", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "shared-port", name: "PB", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    assert!(matches!(
        ContentPack::load_dir(&dir),
        Err(ContentError::DuplicateId {
            kind: "docking location",
            ..
        })
    ));

    // 4. Cross-quadrant reciprocal hyperlinks are rejected.
    fs::write(
        q1.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: ["b"], planets: [( id: "pa", name: "PA", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    fs::write(
        q2.join("b.ron"),
        r#"( id: "b", name: "B", government: "independent", description: "", position: (0.0, 0.0), links: ["a"], planets: [( id: "pb", name: "PB", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    assert!(matches!(
        ContentPack::load_dir(&dir),
        Err(ContentError::UnknownReference {
            kind: "cross-quadrant link",
            ..
        })
    ));

    // 5. Cross-quadrant quest destination is rejected.
    fs::write(
        q1.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "pa", name: "PA", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    fs::write(
        q2.join("b.ron"),
        r#"( id: "b", name: "B", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "pb", name: "PB", description: "", commodities: [] )] )"#,
    )
    .unwrap();
    fs::write(
        q1.join("manifest.ron"),
        r#"( id: "alpha", name: "Alpha", origin: (0.0, 0.0), size: (4000.0, 4000.0), systems: ["a.ron"], quests: ["q.ron"] )"#,
    )
    .unwrap();
    fs::write(
        q1.join("q.ron"),
        r#"( id: "q1", name: "Cross", giver: ("a", "pa"), kind: Delivery(commodity: "food", qty: 1, destination: ("b", "pb")), reward_credits: 100, description: "" )"#,
    )
    .unwrap();
    assert!(matches!(
        ContentPack::load_dir(&dir),
        Err(ContentError::UnknownReference {
            kind: "cross-quadrant quest",
            ..
        })
    ));

    // 6. Valid two-quadrant pack loads all quadrants when unflagged, or a single quadrant with coordinate offset when scoped.
    fs::write(
        q1.join("q.ron"),
        r#"( id: "q1", name: "Local", giver: ("a", "pa"), kind: Delivery(commodity: "food", qty: 1, destination: ("a", "pa")), reward_credits: 100, description: "" )"#,
    )
    .unwrap();
    let full = ContentPack::load_dir(&dir).expect("valid two-quadrant pack loads");
    assert_eq!(full.quadrants.len(), 2);
    assert_eq!(full.systems.len(), 2);
    assert_eq!(full.system("a").unwrap().position, (0.0, 0.0));
    assert_eq!(full.system("b").unwrap().position, (4000.0, 0.0));

    let beta_only = ContentPack::load_dir_scoped(&dir, Some("beta"))
        .expect("scoped load for beta must succeed");
    assert_eq!(beta_only.quadrants.len(), 1);
    assert_eq!(beta_only.quadrants[0].id, "beta");
    assert_eq!(beta_only.systems.len(), 1);
    assert_eq!(beta_only.systems[0].id, "b");
    assert_eq!(beta_only.systems[0].position, (4000.0, 0.0));
    assert!(beta_only.quests.is_empty());

    fs::remove_dir_all(&dir).unwrap();
}
