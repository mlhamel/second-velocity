---
name: meteorite-fields
description: Build playbook for the meteorite fields feature - multi-size space meteorites with stationary/slow/fast drift, self-rotation, ship collision damage, weapon targeting, and recursive binary fragmentation (1 -> 2 -> 4 -> disappear). Use when implementing, reviewing, or extending meteorite fields.
---

# Meteorite fields — space hazards & binary fragmentation

Feature: star systems can host meteorite fields containing rocks of diverse
sizes (`Large`, `Medium`, `Small`) and speed classes (`Stationary`, `Slow`,
`Fast`). Moving meteorites rotate on themselves (`Heading` spin) and travel in
varied directions. Crashing into a meteorite damages the ship; firing weapons
at a meteorite damages it and recursively splits it into **2 parts**, then
**2 parts** again, until the smallest fragments disappear.

All design decisions below align with
[ADR-0010](../../../docs/decisions/0010-meteorite-fields-and-fragmentation.md).

## Locked decisions

- **Sizes & binary split chain**:
  - `Large` (radius `28.0`, HP `40.0`, impact damage `22.0`) → splits into **2 `Medium`**
  - `Medium` (radius `17.0`, HP `20.0`, impact damage `12.0`) → splits into **2 `Small`**
  - `Small` (radius `10.0`, HP `10.0`, impact damage `6.0`) → shatters and **disappears**
- **Movement & self-rotation**:
  - `Stationary`: `speed = 0.0`, `spin_rate = 0.0` (does not rotate on itself)
  - `Slow`: `speed = 18.0..42.0`, `spin_rate = ±(0.6..1.4) rad/s`
  - `Fast`: `speed = 75.0..135.0`, `spin_rate = ±(1.8..3.4) rad/s`
  - Moving meteorites travel along their velocity vector and wrap within the
    system field boundary (`field_radius`, default `1400.0`) so they stay in
    the playable space.
- **Ship collisions**:
  - Only undocked ships (`Without<DockedAt>`) collide with meteorites.
  - Collision damages the ship via `sv_core::combat::apply_damage`, emits
    `ShipDamaged` (and `ShipDestroyed` if lethal), and applies a rebound
    impulse + short per-meteorite impact cooldown (`0.6s`) to prevent
    multi-tick instant kills.
  - Crashing into a meteorite also deals impact damage to the meteorite itself
    (which can fracture it if its HP drops to `0`).
- **Weapon fire & fragmentation**:
  - Any ship (player or NPC) can shoot meteorites with standard projectiles.
  - When a projectile hits a meteorite, the projectile despawns, `ShipDamaged`
    (or hit flash) triggers on the meteorite, and if `hp <= 0.0`, the server
    spawns 2 child meteorites of the next smaller `MeteoriteSize` (or none for
    `Small`) with velocity vectors rotated by `+42°` and `-42°` from the parent
    / impact direction and opposite spin signs.

## Architecture

### 1. Core math (`crates/core`)

- Extend `crates/core/src/combat.rs` (or a new `meteorite.rs` module exported
  from `crates/core/src/lib.rs`) with pure, Bevy-free helpers:
  - `MeteoriteSize::{Large, Medium, Small}` with `radius()`, `max_hp()`,
    `collision_damage()`, and `child_size() -> Option<MeteoriteSize>`.
  - `split_velocities(parent_vel: Vec2, fallback_dir: Vec2, child_speed_boost: f32) -> (Vec2, Vec2)`
    computing the two diverging child velocity vectors (`±42°` rotation).
  - Unit tests verifying `Large -> Some(Medium) -> Some(Small) -> None`,
    radius/damage scaling, and diverging split vectors.

### 2. Content layer (`crates/data` & `content/systems/`)

- In `crates/data/src/system.rs`, add:
  ```rust
  #[derive(Debug, Clone, Deserialize)]
  pub struct MeteoriteFieldDef {
      pub count: u32,
      #[serde(default = "default_field_radius")]
      pub radius: f32,
      #[serde(default)]
      pub stationary_weight: u32,
      #[serde(default = "default_slow_weight")]
      pub slow_weight: u32,
      #[serde(default = "default_fast_weight")]
      pub fast_weight: u32,
  }
  ```
  and `#[serde(default)] pub meteorites: Option<MeteoriteFieldDef>` on
  `SystemDef`.
- Validate in `crates/data/src/loader.rs` that `radius > 0.0` and at least one
  weight is non-zero when `count > 0`.
- Configure meteorite fields in `content/systems/sol.ron` (moderate belt) and
  `content/systems/alpha-centauri.ron` (denser frontier belt).

### 3. Protocol layer (`crates/protocol/src/lib.rs`)

- Add replicated component and size enum:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum MeteoriteSize {
      Large,
      Medium,
      Small,
  }

  #[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct Meteorite {
      pub size: MeteoriteSize,
      pub hp: f32,
      pub max_hp: f32,
      pub spin_rate: f32,
      pub variant: u8,
  }
  ```
- Register `Meteorite` in `protocol::register(app)` (`app.replicate::<Meteorite>()`).
- Add roundtrip serialization unit tests in `crates/protocol/src/lib.rs`.

### 4. Server simulation (`crates/server/src/gameplay.rs`)

- **Spawning (`spawn_world`)**:
  - For each system with `meteorites: Some(field)`, deterministically spawn
    `field.count` initial meteorites using a seeded RNG (`MeteoriteRng` resource)
    across `Large`, `Medium`, and `Small` sizes and `Stationary`, `Slow`, and
    `Fast` speed profiles, keeping a safe clearance zone around planets and the
    `(0, 0)` player spawn point.
- **Movement & self-rotation (`meteorite_movement`)**:
  - Advance `Position` by `Velocity * dt` for moving meteorites.
  - Advance `Heading` by `meteorite.spin_rate * dt` (wrapping to `[-PI, PI]`)
    whenever `Velocity.0.length_squared() > 0.0`.
  - Wrap positions that exceed `field_radius` back across the belt.
- **Projectile hits (`projectile_hits`)**:
  - Check projectiles against `Meteorite` entities in the same `SystemMembership`.
  - On hit: despawn projectile, subtract `proj.damage` from `meteorite.hp`,
    emit `ShipDamaged` for hit-flash feedback.
  - When `meteorite.hp <= 0.0`, despawn parent, emit `ShipDestroyed` for debris
    burst, and if `size.child_size()` is `Some(next_size)`, spawn **2 child
    meteorites** at `pos ± perp * offset` with diverging velocities and
    opposite `spin_rate`.
- **Ship-meteorite collisions (`meteorite_ship_collisions`)**:
  - Query undocked ships (`Without<DockedAt>`) and meteorites in the same
    system.
  - When `pos.distance(m_pos) <= SHIP_HIT_RADIUS + m.size.radius()`:
    - Apply collision damage to ship `Hull`, emit `ShipDamaged`, push ship and
      meteorite apart, and apply impact damage to the meteorite (triggering
      binary fragmentation if `meteorite.hp <= 0.0`).

### 5. Client presentation & art (`crates/client` & `assets/textures/`)

- Add `assets/textures/meteorites/{large,medium,small}.png` (128×128 RGBA PNG
  cratered rock sprites) and fallback procedural irregular polygon / cratered
  rock textures in `crates/client/src/textures.rs` +
  `ops/generate_placeholder_art.py`.
- In `crates/client/src/visuals.rs`:
  - Spawn sprite visuals on `Added<Meteorite>` sized to `size.radius() * 2.2`
    in the active system (`ViewSystem`).
  - Sync `Position` and `Heading` in `sync_transforms` so moving meteorites
     visibly spin on themselves as they drift across space.
  - Reuse `FlashState` and `spawn_debris` when a meteorite takes damage or
    fragments.
- Show meteorites as subtle grey dots on the minimap (`crates/client/src/hud.rs`).

### 6. MCP & Benchmark Scenario (`crates/mcp` & `content/scenarios/`)

- Expose `"meteorites"` in `Harness::observe()` JSON (`id`, `size`, `position`,
  `heading`, `hp`, `max_hp`, `spinning`) and track `peak_meteorites` in
  benchmark entity stats if helpful.
- Add `content/scenarios/meteorite-fragmentation.ron` (and/or extend scenario
  steps in `crates/data/src/scenario.rs` with `ShootMeteorite` /
  `AssertMeteoriteSplit`) to benchmark and integration-test shooting a `Large`
  meteorite into `2 Medium` → `4 Small` → `0` and verifying ship-collision
  hull damage.

## Phases (verify after each; keep `cargo test` green)

1. **Docs & ADR**: `docs/decisions/0010-meteorite-fields-and-fragmentation.md`
   and `.opencode/skills/meteorite-fields/SKILL.md`.
2. **Core + Data + Protocol**: `MeteoriteSize` math in `sv-core`,
   `MeteoriteFieldDef` in `sv-data` + `content/systems/*.ron`, replicated
   `Meteorite` component in `sv-protocol`. Verify: `cargo test -p sv-core -p sv-data -p sv-protocol`.
3. **Server simulation**: spawning, drift + self-spin, ship collision damage,
   projectile hits, and recursive binary splitting (`1 -> 2 -> 4 -> 0`) with
   server unit tests. Verify: `cargo test -p sv-server`.
4. **Client visuals & art**: meteorite sprites, `visuals.rs` spawning/spinning,
   minimap blips. Verify: `cargo build -p sv-client` + `cargo clippy --workspace -- -D warnings`.
5. **MCP & Benchmark scenario**: `observe()` meteorite array + benchmark
   scenario in `content/scenarios/`. Verify: `make bench` and `make check`.

## Status

- 2026-10-17: ADR-0010 and `meteorite-fields` skill created (`0bb17fc`).
- 2026-10-17: Phases 2–5 implemented and verified:
  - **Core (`crates/core`)**: Added `MeteoriteTier::{Large, Medium, Small}` and `split_meteorite_velocities` with unit tests.
  - **Content (`crates/data`, `content/systems/`)**: Added `MeteoriteFieldDef` on `SystemDef`, loader validation, and meteorite belt definitions in `sol.ron` and `alpha-centauri.ron`.
  - **Protocol (`crates/protocol`)**: Added `MeteoriteSize` and replicated `Meteorite` component (`size`, `hp`, `max_hp`, `spin_rate`, `variant`).
  - **Server (`crates/server`)**: Added `MeteoriteMotion`, `spawn_meteorite`, belt spawning in `spawn_world`, `meteorite_movement` (drift + self-spin when moving), `projectile_hits` recursive binary fragmentation (`1 -> 2 -> 4 -> 0`), and `meteorite_ship_collisions` (undocked ship hull damage + rebound).
  - **Client (`crates/client`)**: Added 3 cratered rock texture variants in `textures.rs` and `MeteoriteVisual` attachment, hit-flash feedback, and rotation sync in `visuals.rs`.
  - **MCP & Benchmark (`crates/mcp`, `content/scenarios/`)**: Added `"meteorites"` array to `Harness::observe()`, added `ClearMeteorites`, `SpawnMeteorite`, `DestroyNearestMeteorite`, and `AssertMeteorites` scenario steps, and added `content/scenarios/meteorite-fragmentation.ron`.
