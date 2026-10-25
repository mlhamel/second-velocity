use std::fs;
use std::path::Path;

use sv_data::{ContentError, ContentPack};

fn fixture(name: &str, hole: &str, steps: &str) -> Result<ContentPack, ContentError> {
    let root = std::env::temp_dir().join(format!("sv-black-hole-{name}-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("manifest.ron"),
        r#"(id:"test",name:"Test",version:(0,1,0),ships:[],systems:["a.ron","b.ron"],outfits:[],scenarios:["s.ron"])"#,
    ).unwrap();
    fs::write(
        root.join("a.ron"),
        format!(r#"(id:"a",name:"A",description:"",government:"independent",position:(0.0,0.0),links:[],planets:[],{hole})"#),
    ).unwrap();
    fs::write(
        root.join("b.ron"),
        r#"(id:"b",name:"B",description:"",government:"independent",position:(3000.0,0.0),links:[],planets:[])"#,
    ).unwrap();
    fs::write(
        root.join("s.ron"),
        format!(r#"(id:"s",name:"S",description:"",steps:[{steps}])"#),
    )
    .unwrap();
    let result = ContentPack::load_dir(&root);
    fs::remove_dir_all(root).unwrap();
    result
}

#[test]
fn black_hole_optional_fields_preserve_old_packs() {
    let pack = fixture("old", "", "").unwrap();
    assert!(pack.system("a").unwrap().black_hole.is_none());
    let pack = fixture(
        "no-sprite",
        "black_hole:Some((position:(0.0,-1900.0),radius:90.0,influence:500.0,pull_strength:180.0)),",
        r#"FlyToBlackHole(system:"a")"#,
    ).unwrap();
    assert!(
        pack.system("a")
            .unwrap()
            .black_hole
            .as_ref()
            .unwrap()
            .sprite
            .is_none()
    );
}

#[test]
fn black_hole_invalid_parameters_are_rejected() {
    for (position, radius, influence, strength) in [
        ("(0.0,0.0)", "0.0", "500.0", "180.0"),
        ("(0.0,0.0)", "-1.0", "500.0", "180.0"),
        ("(0.0,0.0)", "90.0", "90.0", "180.0"),
        ("(0.0,0.0)", "90.0", "50.0", "180.0"),
        ("(0.0,0.0)", "90.0", "500.0", "0.0"),
        ("(0.0,0.0)", "90.0", "500.0", "-1.0"),
        ("(NaN,0.0)", "90.0", "500.0", "180.0"),
        ("(0.0,0.0)", "NaN", "500.0", "180.0"),
        ("(0.0,0.0)", "90.0", "inf", "180.0"),
        ("(0.0,0.0)", "90.0", "500.0", "NaN"),
    ] {
        let result = fixture(
            "invalid",
            &format!(
                "black_hole:Some((position:{position},radius:{radius},influence:{influence},pull_strength:{strength})),"
            ),
            "",
        );
        assert!(
            matches!(
                result,
                Err(ContentError::UnknownReference {
                    kind: "black hole config",
                    ..
                })
            ),
            "expected config rejection, got {result:?}"
        );
    }
}

#[test]
fn black_hole_sprite_and_scenario_targets_are_validated() {
    let result = fixture(
        "sprite",
        r#"black_hole:Some((position:(0.0,-1900.0),radius:90.0,influence:500.0,pull_strength:180.0,sprite:Some("textures/black-holes/missing.png"))),"#,
        "",
    );
    assert!(matches!(
        result,
        Err(ContentError::UnknownReference { kind: "asset", .. })
    ));
    for (name, target, expected) in [
        ("no-hole", "a", "scenario black hole target"),
        ("no-system", "missing", "scenario system"),
    ] {
        let result = fixture(name, "", &format!(r#"FlyToBlackHole(system:"{target}")"#));
        assert!(
            matches!(&result, Err(ContentError::UnknownReference { kind, .. }) if *kind == expected),
            "expected {expected}, got {result:?}"
        );
    }
}

fn segment_distance(point: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let d = (b.0 - a.0, b.1 - a.1);
    let len_sq = d.0 * d.0 + d.1 * d.1;
    let t = if len_sq > 0.0 {
        (((point.0 - a.0) * d.0 + (point.1 - a.1) * d.1) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point.0 - a.0 - d.0 * t).hypot(point.1 - a.1 - d.1 * t)
}

#[test]
fn black_hole_influence_clears_routine_routes_and_other_hazards() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = ContentPack::load_dir(&root).unwrap();
    let mut count = 0;
    for system in &pack.systems {
        let Some(hole) = &system.black_hole else {
            continue;
        };
        count += 1;
        let pos = (
            system.position.0 + hole.position.0,
            system.position.1 + hole.position.1,
        );
        for other in &pack.systems {
            let distance = (pos.0 - other.position.0).hypot(pos.1 - other.position.1);
            assert!(
                distance > hole.influence + 800.0,
                "{} reaches {} spawn/membership disc",
                system.id,
                other.id
            );
            if let Some(field) = &other.meteorites {
                assert!(
                    distance > hole.influence + field.radius,
                    "{} reaches {} meteorite field",
                    system.id,
                    other.id
                );
            }
            if let Some(star) = &other.star {
                let star_pos = (
                    other.position.0 + star.position.0,
                    other.position.1 + star.position.1,
                );
                assert!(
                    (pos.0 - star_pos.0).hypot(pos.1 - star_pos.1)
                        > hole.influence + 1.8 * star.radius
                );
            }
            for (index, _) in other.planets.iter().enumerate() {
                let planet = (
                    other.position.0 + 500.0 + index as f32 * 600.0,
                    other.position.1,
                );
                assert!(segment_distance(pos, other.position, planet) > hole.influence + 220.0);
            }
            for link in &other.links {
                let destination = pack.system(link).unwrap();
                // Include the east-offset merchant waypoints and north/south
                // explorer lanes rather than only the center-to-center line.
                for offset in [(0.0, 0.0), (500.0, 220.0), (120.0, -220.0)] {
                    let a = (other.position.0 + offset.0, other.position.1 + offset.1);
                    let b = (
                        destination.position.0 + offset.0,
                        destination.position.1 + offset.1,
                    );
                    assert!(
                        segment_distance(pos, a, b) > hole.influence + 200.0,
                        "{} reaches {} -> {} corridor",
                        system.id,
                        other.id,
                        link
                    );
                }
            }
        }
    }
    assert!((1..=2).contains(&count));
}
