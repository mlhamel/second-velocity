use glam::Vec2;

pub mod contract;
pub mod market;

/// Scripted circular orbit; using elapsed simulation time avoids incremental drift.
pub fn orbit_position(
    center: Vec2,
    radius: f32,
    period_secs: f32,
    phase: f32,
    elapsed: f64,
) -> Vec2 {
    let angle = (f64::from(phase)
        + elapsed.rem_euclid(f64::from(period_secs)) / f64::from(period_secs)
            * std::f64::consts::TAU)
        .rem_euclid(std::f64::consts::TAU);
    center + Vec2::new(angle.cos() as f32, angle.sin() as f32) * radius
}

#[derive(Debug, Clone)]
pub struct ShipState {
    pub position: Vec2,
    pub velocity: Vec2,
    pub rotation: f32,
    pub shields: f32,
    pub max_shields: f32,
    pub armor: f32,
    pub max_armor: f32,
    pub fuel: f32,
    pub max_fuel: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct FlightModel {
    pub turn_rate: f32,
    pub acceleration: f32,
    pub max_speed: f32,
    pub drag: f32,
}

impl ShipState {
    pub fn new(max_shields: f32, max_armor: f32, max_fuel: f32) -> Self {
        Self {
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            rotation: 0.0,
            shields: max_shields,
            max_shields,
            armor: max_armor,
            max_armor,
            fuel: max_fuel,
            max_fuel,
        }
    }

    pub fn heading(&self) -> Vec2 {
        Vec2::new(self.rotation.cos(), self.rotation.sin())
    }

    pub fn apply_control(&mut self, turn: f32, throttle: f32, model: &FlightModel, dt: f32) {
        self.rotation += turn * model.turn_rate * dt;
        self.velocity += self.heading() * throttle * model.acceleration * dt;
        self.velocity *= (-model.drag * dt).exp();
        let speed = self.velocity.length();
        if speed > model.max_speed {
            self.velocity *= model.max_speed / speed;
        }
        self.position += self.velocity * dt;
    }

    pub fn apply_damage(&mut self, amount: f32) -> bool {
        if amount <= 0.0 {
            return self.is_destroyed();
        }
        let absorbed = self.shields.min(amount);
        self.shields -= absorbed;
        let spill = amount - absorbed;
        if spill > 0.0 {
            self.armor = (self.armor - spill).max(0.0);
        }
        self.is_destroyed()
    }

    pub fn regen_shields(&mut self, rate: f32, dt: f32) {
        self.shields = (self.shields + rate * dt).min(self.max_shields);
    }

    pub fn is_destroyed(&self) -> bool {
        self.armor <= 0.0
    }
}

#[derive(Debug, Clone)]
pub struct WeaponState {
    pub cooldown_remaining: f32,
}

impl WeaponState {
    pub fn new() -> Self {
        Self {
            cooldown_remaining: 0.0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.cooldown_remaining = (self.cooldown_remaining - dt).max(0.0);
    }

    pub fn try_fire(&mut self, cooldown: f32) -> bool {
        if self.cooldown_remaining <= 0.0 {
            self.cooldown_remaining = cooldown;
            true
        } else {
            false
        }
    }
}

impl Default for WeaponState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub credits: u32,
    pub cargo: Vec<(String, u32)>,
}

impl Account {
    pub fn new(credits: u32) -> Self {
        Self {
            credits,
            cargo: Vec::new(),
        }
    }

    pub fn cargo_used(&self) -> u32 {
        self.cargo.iter().map(|(_, qty)| qty).sum()
    }

    pub fn cargo_qty(&self, commodity: &str) -> u32 {
        self.cargo
            .iter()
            .find(|(id, _)| id == commodity)
            .map(|(_, qty)| *qty)
            .unwrap_or(0)
    }

    pub fn add_cargo(&mut self, commodity: &str, qty: u32, capacity: u32) -> bool {
        if self.cargo_used() + qty > capacity {
            return false;
        }
        match self.cargo.iter_mut().find(|(id, _)| id == commodity) {
            Some((_, existing)) => *existing += qty,
            None => self.cargo.push((commodity.to_owned(), qty)),
        }
        true
    }

    pub fn remove_cargo(&mut self, commodity: &str, qty: u32) -> bool {
        let Some((_, existing)) = self.cargo.iter_mut().find(|(id, _)| id == commodity) else {
            return false;
        };
        if *existing < qty {
            return false;
        }
        *existing -= qty;
        if *existing == 0 {
            self.cargo.retain(|(id, _)| id != commodity);
        }
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteoriteTier {
    Large,
    Medium,
    Small,
}

impl MeteoriteTier {
    pub const fn radius(self) -> f32 {
        match self {
            Self::Large => 28.0,
            Self::Medium => 17.0,
            Self::Small => 10.0,
        }
    }

    pub const fn max_hp(self) -> f32 {
        match self {
            Self::Large => 27.0,
            Self::Medium => 18.0,
            Self::Small => 9.0,
        }
    }

    pub const fn collision_damage(self) -> f32 {
        match self {
            Self::Large => 18.0,
            Self::Medium => 10.0,
            Self::Small => 5.0,
        }
    }

    pub const fn child_tier(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Medium),
            Self::Medium => Some(Self::Small),
            Self::Small => None,
        }
    }
}

pub const STAR_HEAT_BEGIN: f32 = 1.8;
pub const STAR_SURFACE_DPS: f32 = 25.0;
pub const STAR_CORE_FRACTION: f32 = 0.5;
pub const STAR_CORE_DPS: f32 = 500.0;

/// Whether the segment between two positions relative to a hole touches its horizon.
pub fn black_hole_horizon_crossed(start: Vec2, end: Vec2, radius: f32) -> bool {
    if !start.is_finite() || !end.is_finite() || !radius.is_finite() || radius <= 0.0 {
        return false;
    }
    let segment = end - start;
    let length_squared = segment.length_squared();
    let t = if length_squared > 0.0 {
        (-start.dot(segment) / length_squared).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (start + segment * t).length() <= radius
}
/// Bounded inward acceleration, increasing linearly from zero at the influence
/// boundary to `pull_strength` at the event horizon.
pub fn black_hole_acceleration(
    toward_hole: Vec2,
    radius: f32,
    influence: f32,
    pull_strength: f32,
) -> Vec2 {
    if !toward_hole.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !influence.is_finite()
        || influence <= radius
        || !pull_strength.is_finite()
        || pull_strength <= 0.0
    {
        return Vec2::ZERO;
    }
    let distance = toward_hole.length();
    let proximity = ((influence - distance) / (influence - radius)).clamp(0.0, 1.0);
    toward_hole.normalize_or_zero() * (pull_strength * proximity)
}

pub fn star_heat_dps(distance: f32, radius: f32) -> f32 {
    if radius <= 0.0 {
        return 0.0;
    }
    if distance >= radius * STAR_HEAT_BEGIN {
        return 0.0;
    }
    if distance <= radius * STAR_CORE_FRACTION {
        return STAR_CORE_DPS;
    }
    if distance >= radius {
        let ramp = (radius * STAR_HEAT_BEGIN - distance) / (radius * (STAR_HEAT_BEGIN - 1.0));
        return STAR_SURFACE_DPS * ramp;
    }
    let depth = (radius - distance) / (radius * (1.0 - STAR_CORE_FRACTION));
    STAR_SURFACE_DPS + (STAR_CORE_DPS - STAR_SURFACE_DPS) * depth
}

pub fn split_meteorite_velocities(
    parent_velocity: Vec2,
    fallback_dir: Vec2,
    speed_boost: f32,
) -> (Vec2, Vec2) {
    let base_dir = parent_velocity
        .try_normalize()
        .or_else(|| fallback_dir.try_normalize())
        .unwrap_or(Vec2::X);
    let speed = (parent_velocity.length() + speed_boost).max(speed_boost.max(25.0));
    let angle = 0.733_038_3_f32; // ~42 degrees
    let rot_pos = Vec2::from_angle(angle);
    let rot_neg = Vec2::from_angle(-angle);
    (
        base_dir.rotate(rot_pos) * speed,
        base_dir.rotate(rot_neg) * speed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_orbit_preserves_radius_phase_and_period() {
        let center = Vec2::new(500.0, 20.0);
        assert_eq!(
            orbit_position(center, 200.0, 100.0, 0.0, 0.0),
            center + Vec2::X * 200.0
        );
        assert!(
            (orbit_position(center, 200.0, 100.0, 0.0, 25.0) - center - Vec2::Y * 200.0).length()
                < 0.001
        );
        for time in [0.0, 0.1, 3.0, 25.0, 100.0, 1e8] {
            let position = orbit_position(center, 200.0, 100.0, 0.3, time);
            assert!(((position - center).length() - 200.0).abs() < 0.001);
            assert!(
                (position - orbit_position(center, 200.0, 100.0, 0.3, time + 100.0)).length()
                    < 0.001
            );
        }
    }

    #[test]
    fn black_hole_horizon_detects_swept_and_grazing_crossings() {
        assert!(black_hole_horizon_crossed(
            Vec2::new(-150.0, 0.0),
            Vec2::new(150.0, 0.0),
            90.0
        ));
        assert!(black_hole_horizon_crossed(
            Vec2::new(-10.0, 89.9),
            Vec2::new(10.0, 89.9),
            90.0
        ));
        assert!(!black_hole_horizon_crossed(
            Vec2::new(-10.0, 90.1),
            Vec2::new(10.0, 90.1),
            90.0
        ));
        assert!(black_hole_horizon_crossed(Vec2::ZERO, Vec2::ZERO, 90.0));
        assert!(!black_hole_horizon_crossed(
            Vec2::X * 100.0,
            Vec2::X * 100.0,
            90.0
        ));
        assert!(!black_hole_horizon_crossed(
            Vec2::ZERO,
            Vec2::ZERO,
            f32::NAN
        ));
    }
    #[test]
    fn black_hole_gravity_points_inward_and_ramps_to_horizon() {
        let far = black_hole_acceleration(Vec2::new(0.0, -300.0), 100.0, 500.0, 200.0);
        assert_eq!(far, Vec2::new(0.0, -100.0));
        let near = black_hole_acceleration(Vec2::new(0.0, -100.0), 100.0, 500.0, 200.0);
        assert_eq!(near, Vec2::new(0.0, -200.0));
        for distance in [500.0, 600.0] {
            assert_eq!(
                black_hole_acceleration(Vec2::X * distance, 100.0, 500.0, 200.0),
                Vec2::ZERO
            );
        }
    }

    #[test]
    fn black_hole_gravity_handles_center_and_invalid_parameters() {
        for (delta, radius, influence, strength) in [
            (Vec2::ZERO, 100.0, 500.0, 200.0),
            (Vec2::X, 0.0, 500.0, 200.0),
            (Vec2::X, 100.0, 100.0, 200.0),
            (Vec2::X, 100.0, 500.0, -1.0),
            (Vec2::X, f32::NAN, 500.0, 200.0),
            (Vec2::X, 100.0, f32::INFINITY, 200.0),
            (Vec2::splat(f32::NAN), 100.0, 500.0, 200.0),
        ] {
            assert_eq!(
                black_hole_acceleration(delta, radius, influence, strength),
                Vec2::ZERO
            );
        }
    }

    fn flight(turn_rate: f32, acceleration: f32, max_speed: f32, drag: f32) -> FlightModel {
        FlightModel {
            turn_rate,
            acceleration,
            max_speed,
            drag,
        }
    }

    #[test]
    fn damage_is_absorbed_by_shields_first() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        let destroyed = ship.apply_damage(10.0);
        assert!(!destroyed);
        assert_eq!(ship.shields, 30.0);
        assert_eq!(ship.armor, 25.0);
    }

    #[test]
    fn damage_spills_to_armor() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        ship.apply_damage(50.0);
        assert_eq!(ship.shields, 0.0);
        assert_eq!(ship.armor, 15.0);
    }

    #[test]
    fn destruction_when_armor_drains() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        assert!(ship.apply_damage(65.0));
        assert!(ship.is_destroyed());
    }

    #[test]
    fn drained_shields_with_armor_left_survives() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        assert!(!ship.apply_damage(50.0));
        assert_eq!(ship.shields, 0.0);
        assert_eq!(ship.armor, 15.0);
        assert!(!ship.is_destroyed());
    }

    #[test]
    fn armor_zero_with_shields_up_is_destroyed() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        ship.armor = 0.0;
        assert!(ship.is_destroyed());
    }

    #[test]
    fn overkill_damage_clamps_at_zero() {
        let mut ship = ShipState::new(10.0, 10.0, 100.0);
        ship.apply_damage(1000.0);
        assert_eq!(ship.shields, 0.0);
        assert_eq!(ship.armor, 0.0);
    }

    #[test]
    fn zero_and_negative_damage_is_ignored() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        ship.apply_damage(0.0);
        ship.apply_damage(-10.0);
        assert_eq!(ship.shields, 40.0);
        assert_eq!(ship.armor, 25.0);
        assert!(!ship.is_destroyed());
    }

    #[test]
    fn shield_regen_caps_at_max() {
        let mut ship = ShipState::new(40.0, 25.0, 100.0);
        ship.shields = 38.0;
        ship.regen_shields(5.0, 1.0);
        assert_eq!(ship.shields, 40.0);
    }

    #[test]
    fn thrust_moves_ship_along_heading() {
        let mut ship = ShipState::new(10.0, 10.0, 100.0);
        ship.rotation = 0.0;
        ship.apply_control(0.0, 1.0, &flight(3.0, 100.0, 200.0, 0.0), 1.0);
        assert!(ship.position.x > 90.0);
        assert_eq!(ship.position.y, 0.0);
    }

    #[test]
    fn turning_rotates_heading() {
        let mut ship = ShipState::new(10.0, 10.0, 100.0);
        ship.apply_control(
            1.0,
            0.0,
            &flight(std::f32::consts::FRAC_PI_2, 100.0, 200.0, 0.0),
            1.0,
        );
        let heading = ship.heading();
        assert!((heading.y - 1.0).abs() < 1e-5);
        assert!(heading.x.abs() < 1e-5);
    }

    #[test]
    fn drag_decays_velocity() {
        let mut ship = ShipState::new(10.0, 10.0, 100.0);
        ship.velocity = Vec2::new(100.0, 0.0);
        ship.apply_control(0.0, 0.0, &flight(1.0, 100.0, 200.0, 0.5), 1.0);
        let decayed = 100.0 * (-0.5_f32).exp();
        assert!((ship.velocity.x - decayed).abs() < 1e-3);
    }

    #[test]
    fn speed_is_clamped_to_max() {
        let mut ship = ShipState::new(10.0, 10.0, 100.0);
        ship.apply_control(0.0, 1.0, &flight(0.0, 1000.0, 50.0, 0.0), 1.0);
        assert!(ship.velocity.length() <= 50.0);
    }

    #[test]
    fn weapon_respects_cooldown() {
        let mut weapon = WeaponState::new();
        assert!(weapon.try_fire(0.25));
        assert!(!weapon.try_fire(0.25));
        weapon.tick(0.25);
        assert!(weapon.try_fire(0.25));
    }

    #[test]
    fn cargo_respects_capacity() {
        let mut account = Account::new(1000);
        assert!(account.add_cargo("food", 15, 20));
        assert!(!account.add_cargo("food", 6, 20));
        assert_eq!(account.cargo_qty("food"), 15);
        assert!(account.remove_cargo("food", 15));
        assert_eq!(account.cargo_qty("food"), 0);
        assert!(account.cargo.is_empty());
        assert!(!account.remove_cargo("food", 1));
    }

    #[test]
    fn meteorite_tier_binary_split_chain_terminates() {
        assert_eq!(
            MeteoriteTier::Large.child_tier(),
            Some(MeteoriteTier::Medium)
        );
        assert_eq!(
            MeteoriteTier::Medium.child_tier(),
            Some(MeteoriteTier::Small)
        );
        assert_eq!(MeteoriteTier::Small.child_tier(), None);
        assert!(MeteoriteTier::Large.radius() > MeteoriteTier::Medium.radius());
        assert!(MeteoriteTier::Medium.radius() > MeteoriteTier::Small.radius());
    }

    #[test]
    fn split_meteorite_velocities_diverge() {
        let (v1, v2) = split_meteorite_velocities(Vec2::new(40.0, 0.0), Vec2::X, 15.0);
        assert!(v1.x > 0.0 && v2.x > 0.0);
        assert!(v1.y > 0.0 && v2.y < 0.0);
        assert!((v1.length() - 55.0).abs() < 1e-3);
        assert!((v2.length() - 55.0).abs() < 1e-3);
    }

    #[test]
    fn star_heat_ramp_is_zero_beyond_heat_zone() {
        let radius = 220.0;
        assert_eq!(star_heat_dps(radius * STAR_HEAT_BEGIN, radius), 0.0);
        assert_eq!(star_heat_dps(radius * STAR_HEAT_BEGIN * 2.0, radius), 0.0);
        assert_eq!(star_heat_dps(radius, 0.0), 0.0);
    }

    #[test]
    fn star_heat_ramp_reaches_surface_dps_and_lethal_core() {
        let radius = 220.0;
        assert!((star_heat_dps(radius, radius) - STAR_SURFACE_DPS).abs() < 1e-3);
        assert!((star_heat_dps(1.4 * radius, radius) - 12.5).abs() < 1e-3);
        assert!(
            star_heat_dps(0.75 * radius, radius) > 2.0 * STAR_SURFACE_DPS,
            "inside the star the ramp must steepen past surface heat"
        );
        assert_eq!(
            star_heat_dps(radius * STAR_CORE_FRACTION, radius),
            STAR_CORE_DPS
        );
        assert_eq!(star_heat_dps(0.0, radius), STAR_CORE_DPS);
        assert!(
            star_heat_dps(radius * STAR_CORE_FRACTION * 0.9, radius) >= STAR_CORE_DPS,
            "the core fraction must be lethal regardless of hull"
        );
    }
}
