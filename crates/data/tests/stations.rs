use std::fs;
use std::path::Path;

use sv_data::{ContentError, ContentPack, DockableDef, StationPlacement};

fn fixture(
    name: &str,
    stations: &str,
    quest: Option<&str>,
    steps: &str,
) -> Result<ContentPack, ContentError> {
    fixture_at(name, stations, quest, steps, "(0.0,0.0)")
}

fn fixture_at(
    name: &str,
    stations: &str,
    quest: Option<&str>,
    steps: &str,
    origin: &str,
) -> Result<ContentPack, ContentError> {
    let root = std::env::temp_dir().join(format!("sv-stations-{name}-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let quests = if quest.is_some() {
        r#"["q.ron"]"#
    } else {
        "[]"
    };
    fs::write(root.join("manifest.ron"), format!(
        r#"(id:"t",name:"T",version:(0,1,0),ships:[],systems:["s.ron"],outfits:[],quests:{quests},scenarios:["test.ron"])"#
    )).unwrap();
    fs::write(root.join("s.ron"), format!(
        r#"(id:"s",name:"S",description:"",government:"independent",position:{origin},links:[],planets:[(id:"p",name:"P",description:"",commodities:[])],{stations})"#
    )).unwrap();
    fs::write(
        root.join("test.ron"),
        format!(r#"(id:"test",name:"Test",description:"",steps:[{steps}])"#),
    )
    .unwrap();
    if let Some(quest) = quest {
        fs::write(root.join("q.ron"), quest).unwrap();
    }
    let result = ContentPack::load_dir(&root);
    fs::remove_dir_all(root).unwrap();
    result
}

#[test]
fn old_packs_and_unstaffed_stations_load() {
    assert!(
        fixture("old", "", None, "").unwrap().systems[0]
            .stations
            .is_empty()
    );
    let pack = fixture("fixed", r#"stations:[(id:"freeport",name:"Freeport",description:"",radius:40.0,placement:Fixed(position:(-1000.0,900.0)))],"#, None,
        r#"FlyToStation(system:"s",station:"freeport",dock:true),Dock(system:"s",location:"freeport"),AssertDocked(Some("freeport"))"#).unwrap();
    let port = pack.dockable("s", "freeport").unwrap();
    assert!(matches!(port, DockableDef::Station(_)));
    assert!(port.host().is_none());
    assert!(port.commodities().is_empty());
    assert_eq!(port.name(), "Freeport");
}

#[test]
fn ambiguous_ids_and_invalid_orbits_are_rejected() {
    let duplicate = fixture(
        "duplicate",
        r#"stations:[(id:"p",name:"Port",description:"",radius:40.0,placement:Fixed(position:(0.0,800.0)))],"#,
        None,
        "",
    );
    assert!(matches!(
        duplicate,
        Err(ContentError::DuplicateId {
            kind: "docking location",
            ..
        })
    ));
    for placement in [
        r#"Fixed(position:(NaN,0.0))"#,
        r#"Fixed(position:(0.0,inf))"#,
        r#"Orbit(planet:"missing",radius:240.0,period_secs:180.0)"#,
        r#"Orbit(planet:"p",radius:100.0,period_secs:180.0)"#,
        r#"Orbit(planet:"p",radius:240.0,period_secs:0.0)"#,
        r#"Orbit(planet:"p",radius:240.0,period_secs:NaN)"#,
        r#"Orbit(planet:"p",radius:inf,period_secs:180.0)"#,
        r#"Orbit(planet:"p",radius:240.0,period_secs:180.0,phase:NaN)"#,
    ] {
        let result = fixture(
            "orbit",
            &format!(
                r#"stations:[(id:"port",name:"Port",description:"",radius:40.0,placement:{placement})],"#
            ),
            None,
            "",
        );
        assert!(
            matches!(
                result,
                Err(ContentError::UnknownReference {
                    kind: "station config",
                    ..
                })
            ),
            "{placement}: {result:?}"
        );
    }
    for radius in ["0.0", "-1.0", "NaN"] {
        let result = fixture(
            "radius",
            &format!(
                r#"stations:[(id:"port",name:"Port",description:"",radius:{radius},placement:Fixed(position:(0.0,800.0)))],"#
            ),
            None,
            "",
        );
        assert!(matches!(
            result,
            Err(ContentError::UnknownReference {
                kind: "station config",
                ..
            })
        ));
    }
}

#[test]
fn orbital_world_origins_and_sweeps_must_remain_finite() {
    let station = r#"stations:[(id:"port",name:"Port",description:"",radius:40.0,placement:Orbit(planet:"p",radius:240.0,period_secs:180.0))],"#;
    for origin in ["(NaN,0.0)", "(0.0,inf)", "(-inf,0.0)"] {
        assert!(matches!(
            fixture_at("origin", station, None, "", origin),
            Err(ContentError::UnknownReference {
                kind: "station config",
                ..
            })
        ));
    }
    let huge_orbit = station.replace("radius:240.0", "radius:3.0e38");
    assert!(matches!(
        fixture_at("overflow", &huge_orbit, None, "", "(2.0e38,0.0)"),
        Err(ContentError::UnknownReference {
            kind: "station config",
            ..
        })
    ));
}

#[test]
fn station_assets_quests_and_scenario_targets_are_validated() {
    let station = r#"stations:[(id:"port",name:"Port",description:"",radius:40.0,placement:Fixed(position:(0.0,800.0)))],"#;
    let quest = r#"(id:"q",name:"Q",description:"",giver:("s","port"),kind:Delivery(commodity:"food",qty:1,destination:("s","p")),reward_credits:10)"#;
    assert!(matches!(
        fixture("host", station, Some(quest), ""),
        Err(ContentError::UnknownReference {
            kind: "quest station master",
            ..
        })
    ));
    assert!(matches!(
        fixture(
            "target",
            station,
            None,
            r#"Dock(system:"s",location:"missing")"#
        ),
        Err(ContentError::UnknownReference {
            kind: "scenario dock target",
            ..
        })
    ));
    assert!(matches!(
        fixture(
            "planet-not-station",
            station,
            None,
            r#"FlyToStation(system:"s",station:"p")"#
        ),
        Err(ContentError::UnknownReference {
            kind: "scenario station",
            ..
        })
    ));
    let missing_sprite = station.replace(
        "placement:",
        r#"sprite:Some("textures/stations/missing.png"),placement:"#,
    );
    assert!(matches!(
        fixture("sprite", &missing_sprite, None, ""),
        Err(ContentError::UnknownReference { kind: "asset", .. })
    ));
}

#[test]
fn shipped_stations_and_complete_orbits_clear_planets_and_lethal_hazards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = ContentPack::load_dir(&root).unwrap();
    let mut count = 0;
    for system in &pack.systems {
        for station in &system.stations {
            count += 1;
            let (center, orbit) = match &station.placement {
                StationPlacement::Fixed { position } => (
                    (
                        system.position.0 + position.0,
                        system.position.1 + position.1,
                    ),
                    0.0,
                ),
                StationPlacement::Orbit { planet, radius, .. } => {
                    (system.planet_position(planet).unwrap(), *radius)
                }
            };
            let distance = |point: (f32, f32)| (center.0 - point.0).hypot(center.1 - point.1);
            for other in &pack.systems {
                assert!((distance(other.position) - orbit).abs() > station.radius + 80.0);
                for planet in &other.planets {
                    assert!(
                        (distance(other.planet_position(&planet.id).unwrap()) - orbit).abs()
                            > station.radius + sv_data::PLANET_RADIUS + 20.0
                    );
                }
                if let Some(star) = &other.star {
                    let pos = (
                        other.position.0 + star.position.0,
                        other.position.1 + star.position.1,
                    );
                    assert!(distance(pos) - orbit > station.radius + 1.8 * star.radius + 40.0);
                }
                if let Some(hole) = &other.black_hole {
                    let pos = (
                        other.position.0 + hole.position.0,
                        other.position.1 + hole.position.1,
                    );
                    assert!(distance(pos) - orbit > station.radius + hole.influence + 40.0);
                }
            }
        }
    }
    assert_eq!(count, 2);
}
