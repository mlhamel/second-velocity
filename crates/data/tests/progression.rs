use std::fs;
use std::path::Path;

use sv_data::{ContentError, ContentPack, OutfitDef, OutfitKind};

#[test]
fn loadouts_recompute_from_base_and_enforce_category_and_weapon_limits() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let mut pack = ContentPack::load_dir(&root).unwrap();
    let ids = |values: &[&str]| values.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    let stats = pack
        .loadout_stats(
            "shuttle",
            &ids(&[
                "light-blaster",
                "pulse-blaster",
                "shield-generator-1",
                "aux-fuel-tank",
                "capacitor-bank",
            ]),
        )
        .unwrap();
    assert_eq!(
        (
            stats.max_shields,
            stats.max_armor,
            stats.max_fuel,
            stats.max_energy,
            stats.energy_regen,
            stats.weapon_slots_used
        ),
        (60.0, 25.0, 170.0, 100.0, 16.0, 2)
    );
    assert!(
        pack.loadout_stats("shuttle", &ids(&["capacitor-bank", "capacitor-bank"]))
            .is_err()
    );
    assert_eq!(
        pack.loadout_stats("shuttle", &[]).unwrap().max_shields,
        40.0
    );
    assert!(
        pack.loadout_stats(
            "shuttle",
            &ids(&["light-blaster", "pulse-blaster", "light-blaster"])
        )
        .is_err()
    );
    assert!(
        pack.loadout_stats("terran-courier", &ids(&["light-blaster", "pulse-blaster"]))
            .is_err()
    );
    assert!(
        pack.loadout_stats(
            "shuttle",
            &ids(&["shield-generator-1", "shield-generator-2"])
        )
        .is_err()
    );
    assert!(
        pack.loadout_stats("shuttle", &ids(&["aux-fuel-tank", "aux-fuel-tank"]))
            .is_err()
    );
    assert!(pack.loadout_stats("shuttle", &ids(&["unknown"])).is_err());
    pack.outfits.push(OutfitDef {
        id: "test-armor".into(),
        name: "Armor".into(),
        description: "".into(),
        price: 100,
        mass: 1.0,
        kind: OutfitKind::ArmorPlate { armor: 15.0 },
        icon: None,
    });
    assert_eq!(
        pack.loadout_stats("shuttle", &ids(&["test-armor"]))
            .unwrap()
            .max_armor,
        40.0
    );
    assert!(
        pack.loadout_stats("shuttle", &ids(&["test-armor", "test-armor"]))
            .is_err()
    );
}

fn fixture(
    name: &str,
    services: &str,
    kind: &str,
    price: u32,
    defaults: &str,
    step: &str,
) -> Result<ContentPack, ContentError> {
    let dir = std::env::temp_dir().join(format!("sv-progression-{name}-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("manifest.ron"), r#"(id:"t",name:"T",version:(0,1,0),ships:["ship.ron"],systems:["system.ron"],outfits:["outfit.ron"],scenarios:["scenario.ron"])"#).unwrap();
    fs::write(dir.join("ship.ron"), format!(r#"(id:"s",name:"S",description:"",price:1,mass:1.0,turn_rate:1.0,acceleration:1.0,max_speed:1.0,max_shields:1.0,max_armor:1.0,fuel:1,cargo:1,weapon_slots:1,default_outfits:[{defaults}])"#)).unwrap();
    fs::write(dir.join("system.ron"),format!(r#"(id:"sys",name:"Sys",description:"",government:"independent",position:(0.0,0.0),links:[],planets:[(id:"p",name:"P",description:"",commodities:[],{services})])"#)).unwrap();
    fs::write(
        dir.join("outfit.ron"),
        format!(r#"(id:"o",name:"O",description:"",price:{price},mass:1.0,kind:{kind})"#),
    )
    .unwrap();
    fs::write(
        dir.join("scenario.ron"),
        format!(r#"(id:"test",name:"Test",description:"",steps:[{step}])"#),
    )
    .unwrap();
    let result = ContentPack::load_dir(&dir);
    fs::remove_dir_all(dir).unwrap();
    result
}

#[test]
fn shops_are_optional_and_stock_references_are_checked() {
    assert!(fixture("old", "", "ShieldGenerator(max_shields:1.0)", 1, "", "").is_ok());
    let result = fixture(
        "yard",
        r#"shipyard:Some((ships:["missing"])),"#,
        "ShieldGenerator(max_shields:1.0)",
        1,
        "",
        "",
    );
    assert!(matches!(
        result,
        Err(ContentError::UnknownReference {
            kind: "shipyard ship",
            ..
        })
    ));
    let result = fixture(
        "outfitter",
        r#"outfitter:Some((outfits:["missing"])),"#,
        "ShieldGenerator(max_shields:1.0)",
        1,
        "",
        "",
    );
    assert!(matches!(
        result,
        Err(ContentError::UnknownReference {
            kind: "outfitter outfit",
            ..
        })
    ));
    let result = fixture(
        "duplicate",
        r#"outfitter:Some((outfits:["o","o"])),"#,
        "ShieldGenerator(max_shields:1.0)",
        1,
        "",
        "",
    );
    assert!(matches!(
        result,
        Err(ContentError::DuplicateId {
            kind: "outfitter stock",
            ..
        })
    ));
}

#[test]
fn invalid_outfit_effects_prices_defaults_and_scenario_ids_are_rejected() {
    for kind in [
        "ShieldGenerator(max_shields:NaN)",
        "FuelTank(fuel:0.0)",
        "ArmorPlate(armor:-1.0)",
        "Weapon((damage:1.0,cooldown:0.0,projectile_speed:1.0,projectile_ttl:1.0))",
    ] {
        assert!(matches!(
            fixture("invalid", "", kind, 1, "", ""),
            Err(ContentError::UnknownReference {
                kind: "outfit config",
                ..
            })
        ));
    }
    assert!(fixture("price", "", "ShieldGenerator(max_shields:1.0)", 0, "", "").is_err());
    assert!(matches!(
        fixture(
            "defaults",
            "",
            "ShieldGenerator(max_shields:1.0)",
            1,
            r#""o","o""#,
            ""
        ),
        Err(ContentError::UnknownReference {
            kind: "ship loadout",
            ..
        })
    ));
    assert!(matches!(
        fixture(
            "scenario",
            "",
            "ShieldGenerator(max_shields:1.0)",
            1,
            "",
            r#"BuyOutfit(outfit:"missing")"#
        ),
        Err(ContentError::UnknownReference {
            kind: "scenario outfit",
            ..
        })
    ));
}
