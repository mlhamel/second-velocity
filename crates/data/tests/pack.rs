use std::fs;
use std::path::Path;

use evr_data::{ContentError, ContentPack};

const DUPLICATE_SHIP: &str = r#"( id: "dupe", name: "Dupe", description: "", price: 1, mass: 1.0, turn_rate: 1.0, acceleration: 1.0, max_speed: 1.0, max_shields: 1.0, max_armor: 1.0, fuel: 1, cargo: 1, weapon_slots: 1 )"#;

#[test]
fn base_pack_loads() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = ContentPack::load_dir(&root).expect("base pack must load");

    assert_eq!(pack.manifest.id, "evr-base");
    assert_eq!(pack.ships.len(), 25);
    assert_eq!(pack.systems.len(), 15);
    assert_eq!(pack.outfits.len(), 5);
    assert_eq!(pack.quests.len(), 12);
    assert_eq!(pack.scenarios.len(), 12);

    let shuttle = pack.ship("shuttle").expect("shuttle exists");
    assert_eq!(shuttle.name, "Shuttle");
    assert_eq!(shuttle.race, "terran");
    assert_eq!(shuttle.size, evr_data::ShipSize::Light);
    assert_eq!(shuttle.default_outfits, vec!["light-blaster".to_owned()]);

    let dreadnought = pack
        .ship("korath-dreadnought")
        .expect("korath-dreadnought exists");
    assert_eq!(dreadnought.race, "korath");
    assert_eq!(dreadnought.size, evr_data::ShipSize::Capital);

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
    assert!(alpha.links.contains(&"sol".to_owned()));
    assert!(alpha.planets[0].chief.is_some());

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
    let evr_data::OutfitKind::Weapon(weapon) = &blaster.kind else {
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
    let dir = std::env::temp_dir().join(format!("evr-data-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["a.ron", "b.ron"], systems: [], outfits: [] )"#,
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
    let dir = std::env::temp_dir().join(format!("evr-data-ref-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["a.ron"], systems: [], outfits: [] )"#,
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
    let dir = std::env::temp_dir().join(format!("evr-data-sprite-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["a.ron"], systems: [], outfits: [] )"#,
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
    let dir = std::env::temp_dir().join(format!("evr-data-icon-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], systems: [], outfits: ["a.ron"] )"#,
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
    let dir = std::env::temp_dir().join(format!("evr-data-asset-test-{}", std::process::id()));
    let assets = dir.join("assets/textures");
    let content = dir.join("content");
    fs::create_dir_all(&assets).expect("dirs must be creatable");
    fs::create_dir_all(&content).expect("dirs must be creatable");
    fs::write(
        content.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], systems: ["a.ron"], outfits: [] )"#,
    )
    .expect("manifest must be writable");

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
            r#"( id: "t", name: "t", version: (0, 1, 0), ships: ["ship.ron"], systems: [], outfits: [] )"#,
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
    let dir = std::env::temp_dir().join(format!("evr-data-link-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], systems: ["a.ron", "b.ron"], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: ["b"], planets: [] )"#,
    )
    .expect("system must be writable");
    fs::write(
        dir.join("b.ron"),
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
    let dir = std::env::temp_dir().join(format!("evr-data-quest-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], systems: ["a.ron"], outfits: [], quests: ["q.ron"] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
        r#"( id: "a", name: "A", government: "independent", description: "", position: (0.0, 0.0), links: [], planets: [( id: "p", name: "P", description: "", commodities: [] )] )"#,
    )
    .expect("system must be writable");
    fs::write(
        dir.join("q.ron"),
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
    let dir = std::env::temp_dir().join(format!("evr-data-chief-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], systems: ["a.ron"], outfits: [] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
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
    let dir = std::env::temp_dir().join(format!("evr-data-scenario-test-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir must be creatable");
    fs::write(
        dir.join("manifest.ron"),
        r#"( id: "t", name: "t", version: (0, 1, 0), ships: [], systems: ["a.ron"], outfits: [], scenarios: ["s.ron"] )"#,
    )
    .expect("manifest must be writable");
    fs::write(
        dir.join("a.ron"),
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
