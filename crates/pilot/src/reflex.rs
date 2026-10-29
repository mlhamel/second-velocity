use std::f32::consts::PI;

use bevy::math::Vec2;
use sv_core::STAR_HEAT_BEGIN;
use sv_data::ContentPack;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HazardDisc {
    pub center: Vec2,
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StandingOrder {
    Idle,
    FlyTo {
        system: String,
        location: Option<String>,
        dock_on_arrival: bool,
    },
    Flee {
        target_system: Option<String>,
    },
}

/// Steering helper (follows `tour.rs` `steer()` pattern with deceleration-aware
/// approach inside `slow_radius`).
pub fn steer(current: Vec2, heading: f32, target: Vec2) -> (f32, f32) {
    steer_approach(current, heading, target, 0.0)
}

pub fn steer_approach(current: Vec2, heading: f32, target: Vec2, slow_radius: f32) -> (f32, f32) {
    let delta = target - current;
    let distance = delta.length();
    if distance < 1e-3 {
        return (0.0, 0.0);
    }
    let desired = delta.y.atan2(delta.x);
    let diff = (desired - heading + PI).rem_euclid(2.0 * PI) - PI;
    let turn = if diff.abs() < 0.03 {
        0.0
    } else {
        diff.signum()
    };
    let aligned_throttle = if slow_radius > 0.0 && distance < slow_radius {
        (distance / slow_radius).clamp(0.18, 1.0)
    } else {
        1.0
    };
    let throttle = if diff.abs() < 0.6 {
        aligned_throttle
    } else {
        0.2
    };
    (turn, throttle)
}

/// Collects hazard discs from content:
/// - Star heat discs (`1.8 * star.radius` + safety margin)
/// - Black-hole influence discs (`hole.influence` + safety margin)
/// - Meteorite field discs (`field.radius` around system center, if clear of start/goal)
pub fn collect_hazards(pack: &ContentPack, safety_margin: f32) -> Vec<HazardDisc> {
    let mut discs = Vec::new();
    for sys in &pack.systems {
        let sys_center = Vec2::new(sys.position.0, sys.position.1);
        if let Some(star) = &sys.star {
            let center = sys_center + Vec2::new(star.position.0, star.position.1);
            let radius = star.radius * STAR_HEAT_BEGIN + safety_margin;
            discs.push(HazardDisc { center, radius });
        }
        if let Some(hole) = &sys.black_hole {
            let center = sys_center + Vec2::new(hole.position.0, hole.position.1);
            let radius = hole.influence + safety_margin;
            discs.push(HazardDisc { center, radius });
        }
    }
    discs
}

/// Checks whether the segment `start -> goal` intersects any hazard disc and,
/// if so, returns a tangent bypass waypoint around the first blocking disc.
pub fn next_hazard_safe_waypoint(start: Vec2, goal: Vec2, hazards: &[HazardDisc]) -> Vec2 {
    let seg = goal - start;
    let seg_len = seg.length();
    if seg_len < 1.0 {
        return goal;
    }
    let dir = seg / seg_len;

    let mut nearest_blocker: Option<(f32, &HazardDisc)> = None;
    for disc in hazards {
        if start.distance(disc.center) <= disc.radius || goal.distance(disc.center) <= disc.radius {
            // If start is inside a disc, steer radially out toward goal side;
            // if goal itself is inside, cannot bypass.
            continue;
        }
        let proj = (disc.center - start).dot(dir);
        if proj <= 0.0 || proj >= seg_len {
            continue;
        }
        let closest = start + dir * proj;
        let dist_to_center = closest.distance(disc.center);
        if dist_to_center < disc.radius
            && nearest_blocker.is_none_or(|(best_proj, _)| proj < best_proj)
        {
            nearest_blocker = Some((proj, disc));
        }
    }

    let Some((proj, blocker)) = nearest_blocker else {
        return goal;
    };

    let closest = start + dir * proj;
    let offset = closest - blocker.center;
    let normal = if offset.length_squared() > 1e-4 {
        offset.normalize()
    } else {
        Vec2::new(-dir.y, dir.x)
    };
    blocker.center + normal * (blocker.radius + 80.0)
}

/// Computes a flee target vector away from threats or toward a safe system.
pub fn compute_flee_target(
    current_pos: Vec2,
    threat_positions: &[Vec2],
    fallback_system_pos: Option<Vec2>,
) -> Vec2 {
    if let Some(sys_pos) = fallback_system_pos
        && current_pos.distance(sys_pos) > 300.0
    {
        return sys_pos;
    }
    if threat_positions.is_empty() {
        return current_pos + Vec2::new(-1200.0, 0.0);
    }
    let mut away = Vec2::ZERO;
    for t in threat_positions {
        let delta = current_pos - *t;
        let d = delta.length().max(1.0);
        away += delta / (d * d);
    }
    let dir = away.try_normalize().unwrap_or(Vec2::X);
    current_pos + dir * 1400.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn tangent_waypoint_routes_around_sun_heat_disc() {
        let hazard = HazardDisc {
            center: Vec2::new(500.0, 0.0),
            radius: 432.0, // 240 * 1.8
        };
        let start = Vec2::new(0.0, 20.0);
        let goal = Vec2::new(1200.0, 20.0);
        let waypoint = next_hazard_safe_waypoint(start, goal, &[hazard]);
        assert_ne!(waypoint, goal, "direct line crosses the disc");
        assert!(
            waypoint.distance(hazard.center) > hazard.radius,
            "waypoint must lie outside the hazard disc"
        );
    }

    #[test]
    fn clear_segment_returns_goal_directly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let pack = ContentPack::load_dir(&root).expect("pack loads");
        let hazards = collect_hazards(&pack, 40.0);
        // Sol (0, 0) to Earth (500, 0) is clear of Sol's star at (300, -1600):
        let wp = next_hazard_safe_waypoint(Vec2::ZERO, Vec2::new(500.0, 0.0), &hazards);
        assert_eq!(wp, Vec2::new(500.0, 0.0));
    }

    #[test]
    fn deceleration_reduces_throttle_near_dock() {
        let (_, far_throttle) = steer_approach(Vec2::ZERO, 0.0, Vec2::new(800.0, 0.0), 360.0);
        let (_, near_throttle) = steer_approach(Vec2::ZERO, 0.0, Vec2::new(100.0, 0.0), 360.0);
        assert_eq!(far_throttle, 1.0);
        assert!((0.18..0.4).contains(&near_throttle));
    }
}
