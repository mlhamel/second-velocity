# ADR 0019 — Tactical combat: energy capacitor, flight controls, target lock, and weapon archetypes

- Status: accepted
- Date: 2026-10-28

## Context

Prior to this decision, every combat encounter in Second Velocity played out
with a single verb ("hold Spacebar and turn"):
- Weapons had no energy cost and ships had no energy capacitor pool, leaving
  the *Escape Velocity* energy juggle (`docs/gdd/combat.md`, `core-loops.md`)
  unimplemented.
- Flight lacked a retrograde brake (`S` / `ArrowDown` to auto-align opposite
  velocity) and an afterburner (`Shift` to trade capacitor energy for burst
  speed and acceleration).
- Players fought blind without a target lock (`T` / `R`), enemy shield/armor
  telemetry, or a velocity lead pip for fixed weapons.
- All weapons in `content/outfits/` were fixed-forward dumbfire guns spawning
  on the same pixel, leaving slow haulers unable to track agile fighters and
  providing no secondary homing ordnance.

## Decision

1. **Energy Capacitor Pool (`sv-core`, `sv-data`, `sv-protocol`, `sv-server`)**:
   - `ShipState` tracks `energy`, `max_energy`, and `energy_regen`, recharging
     continuously in flight (`regen_energy(dt)`).
   - Base `max_energy` and `energy_regen` derive from `ShipSize` (`Light`:
     60 / 10/s, `Medium`: 85 / 13/s, `Heavy`: 120 / 16/s, `Capital`: 160 /
     20/s), with optional per-hull overrides (`max_energy`, `energy_regen`) on
     `ShipDef`.
   - `OutfitKind::Reactor { max_energy, energy_regen }` allows one installed
     capacitor/reactor upgrade per hull in `loadout_stats`.
   - Server replicates `Energy { current, max }` on ships; client HUD renders
     an `ENERGY` bar below `FUEL`.
2. **Built-in Afterburner (`SetAfterburner`) and Retrograde Brake (`SetBrake`)**:
   - Holding `Shift` sends `SetAfterburner(true)`. While undocked and
     `energy > 0`, the ship drains capacitor energy (`AFTERBURNER_ENERGY_RATE`)
     to multiply acceleration and `max_speed` (`AFTERBURNER_SPEED_MULTIPLIER =
     1.6`).
   - Holding `S` / `ArrowDown` sends `SetBrake(true)`. When manual `turn == 0`,
     the server steers the ship toward `-velocity` (`retrograde_turn`) and
     zeroes turn once aligned or stationary.
3. **Target Lock (`CycleTarget`, `TargetLock`)**:
   - Clients send `CycleTarget { hostile_only: bool }` (`T` cycles in-system
     ships, `R` selects nearest hostile).
   - Server stores and replicates `TargetLock(pub Option<u64>)` (targeting a
     replicated `TargetId(pub u64)` assigned to every ship entity), clearing it
     automatically if the target leaves the system, docks, or is destroyed.
   - Client HUD displays locked target name, role, and live shield/armor
     percentages; `visuals.rs` renders a target bracket and a velocity lead
     indicator pip ahead of the target.
4. **Unified Weapon Slots & Archetypes (`WeaponMount`, `SetSecondaryFiring`)**:
   - `WeaponDef` adds `#[serde(default)] pub mount: WeaponMount` (`Fixed`,
     `Turret`, `Missile { turn_rate: f32 }`) and `#[serde(default)] pub
     energy_cost: f32` (defaulting to a modest per-shot drain when omitted).
   - All weapons install into `weapon_slots`:
     - `Fixed` weapons fire on `SetFiring` (`Space`) along ship heading when no
       target is locked, or aim toward the locked target (`TargetLock`) using
       velocity lead (`sv_core::lead_target_position`) when a target lock is
       active in the same system, with lateral wing offsets when multiple fixed
       mounts are equipped.
     - `Turret` weapons fire on `SetFiring` (`Space`), auto-aiming at the
       locked target (or nearest hostile in range even without a lock) using
       velocity lead.
     - `Missile` weapons fire on `SetSecondaryFiring` (`F`), spawning
       `ProjectileKind::Missile` entities that steer toward their target at
       `turn_rate` rad/s during `projectile_flight`.

## Consequences

- Existing ship RON files and scenarios remain valid without boilerplate thanks
  to `ShipSize`-derived capacitor defaults and serde defaults on `WeaponDef`.
- A single `light-blaster` draws less energy per second than base `Light`
  regeneration, keeping asteroid-fragmentation and basic combat tests
  continuous, while heavier weapons, dual mounts, turrets, missiles, and
  afterburners create a genuine capacitor management loop.
- Protocol additions (`Energy`, `TargetId`, `TargetLock`, `ProjectileKind`,
  `SetBrake`, `SetAfterburner`, `SetSecondaryFiring`, `CycleTarget`) move
  client and server together as a single workspace change.
