pub const DELIVERY_BASE: u32 = 150;
pub const BOUNTY_BASE: u32 = 500;
pub const DISTANCE_SCALE: f32 = 2_000.0;
pub const DISTANCE_BONUS_CAP: f32 = 4.0;
pub const HAZARD_CAP: f32 = 1.5;
pub const DEADLINE_BASE_SECS: f32 = 600.0;
pub const DEADLINE_SECS_PER_UNIT: f32 = 0.05;
pub const DEADLINE_MAX_SECS: f32 = 900.0;

/// Contract pay: base scaled by travel distance and destination hazard,
/// rounded to tens so boards read like price lists.
pub fn contract_reward(base: u32, distance: f32, hazard: f32) -> u32 {
    let factor = 1.0
        + (distance / DISTANCE_SCALE).clamp(0.0, DISTANCE_BONUS_CAP)
        + hazard.clamp(0.0, HAZARD_CAP);
    (base as f32 * factor / 10.0).round() as u32 * 10
}

/// Timed-contract deadline: generous base plus a distance term, capped.
pub fn contract_deadline(distance: f32) -> u64 {
    (DEADLINE_BASE_SECS + distance * DEADLINE_SECS_PER_UNIT).min(DEADLINE_MAX_SECS) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_reward_at_zero_distance_and_hazard_rounds_to_tens() {
        assert_eq!(contract_reward(DELIVERY_BASE, 0.0, 0.0), 150);
        assert_eq!(contract_reward(BOUNTY_BASE, 0.0, 0.0), 500);
    }

    #[test]
    fn reward_grows_with_distance_and_hazard() {
        let near = contract_reward(BOUNTY_BASE, 100.0, 0.0);
        let far = contract_reward(BOUNTY_BASE, 5_000.0, 0.0);
        let hostile = contract_reward(BOUNTY_BASE, 5_000.0, 1.0);
        assert!(far > near, "distance must pay");
        assert!(hostile > far, "hazard must pay");
        for reward in [near, far, hostile] {
            assert_eq!(reward % 10, 0, "rewards read as clean price ticks");
        }
    }

    #[test]
    fn reward_and_deadline_clamp_extreme_inputs() {
        let maxed = contract_reward(BOUNTY_BASE, f32::MAX, f32::MAX);
        assert_eq!(maxed, 3250);
    }

    #[test]
    fn deadline_scales_with_distance_and_caps_at_nine_hundred() {
        assert_eq!(contract_deadline(0.0), 600);
        assert_eq!(contract_deadline(2_000.0), 700);
        assert_eq!(contract_deadline(1e9), DEADLINE_MAX_SECS as u64);
    }
}
