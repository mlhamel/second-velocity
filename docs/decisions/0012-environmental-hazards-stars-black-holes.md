# ADR 0012 — Environmental hazards: stars and black holes

- Status: accepted
- Date: 2026-10-21

## Context

The galaxy currently contains ships, projectiles, planets, and meteorite
fields, but systems themselves are inert: nothing marks where a system's star
sits, and deep space carries no risk beyond pirates. We want two new
environmental hazard types — both pure content-addressable data, consistent
with ADR-0002 (content is RON data in `content/`, never code):

1. **Suns**: an actual star entity in featured systems. Flying too close
   ramps up heat damage; the core is lethal. Shields absorb heat first, like
   any damage.
2. **Black holes**: gravity pulls nearby ships toward the hole; crossing the
   event horizon resolves randomly — the ship is destroyed or teleported to
   a random other system.

Key constraints discovered in the codebase that shape this design:

- All systems share one continuous world coordinate space; "entering" a system
  is proximity-based (`SYSTEM_MEMBERSHIP_RADIUS = 800` around each system's
  `position`). There is no hyperjump mechanic to hook into.
- Players spawn exactly at system centers; planets sit +500..+X east of
  center. Sun and black-hole positions must be content-defined offsets that
  keep spawn and planet approach lanes safe.
- The damage pipeline (`Ship::apply_damage` -> `is_destroyed` -> `ship_deaths`)
  already handles explosion, `PlayerDied`, and respawn; meteorites
  (ADR-0010) proved the "replicated tag + server-side system applies damage"
  pattern.
- The server owns a seeded `EncounterRng` resource and scenarios can pin it via
  `encounter_seed`, so random outcomes are benchmark-reproducible.

## Decision

1. **Content schema (crates/data)**: `SystemDef` grows two serde-defaulted
   optional fields — `star: Option<StarDef>` and
   `black_hole: Option<BlackHoleDef>`. Both are positioned relative to the
   system center. Old packs load unchanged; sprite assets are validated like
   planet sprites. Fields:
   - `StarDef { position: (f32, f32), radius: f32, sprite: Option<String> }`
   - `BlackHoleDef { position: (f32, f32), radius: f32, influence: f32,
     pull_strength: f32, sprite: Option<String> }`
2. **Protocol (crates/protocol)**: two new replicated marker components —
   `StarTag { system, radius }` and `BlackHoleTag { system, radius,
   influence }` — registered in `register()` alongside `PlanetTag`/
   `Meteorite`. The protocol hash changes; client and server move together in
   this repo, so no migration is needed.
3. **Suns (crates/server)**: a `star_heat` system applies proximity-ramped
   heat damage to every undocked ship in the same system: zero damage beyond
   1.8x radius, ramping to ~25 dps at the surface, lethal inside a core
   fraction of the radius. NPCs burn too (universe consistency). A notice
   warns on heat-zone entry, cooldown-gated. Deaths flow through the
   existing pipeline.
4. **Black holes (crates/server)**: a `black_hole_gravity` system accelerates
   undocked ships toward the hole inside `influence`, scaled by proximity.
   Crossing `radius` consumes the ship: `EncounterRng` decides 50% lethal
   damage (existing death pipeline) / 50% teleport (position re-assigned to
   a seeded-random other system's center, velocity zeroed, notice sent;
   `update_system_membership` picks up the new system on its own). Docked
   ships are immune to both pull and consumption.
5. **Benchmarks (content/scenarios)**: the scenario schema gains
   `FlyToStar { system }` / `FlyToBlackHole { system }` targeting steps and
   an `AssertHull` shield/armor assert, implemented in the evr-mcp runner
   and validated in the loader. Each hazard ships with seeded scenarios:
   `sun-heat-damage.ron`, `black-hole-teleport.ron`,
   `black-hole-death.ron`, plus server unit tests in the meteorite-test
   style (see ADR-0010).
6. **Presentation (crates/client)**: stars render as a large warm corona
   sprite (new placeholder generator entries); black holes as a dark disc
   with an accretion ring; both get distinct minimap dot colors. Hazard math
   stays server-side — clients never simulate damage (ADR-0004).

## Consequences

- Featured systems (3-4 suns, 1-2 black holes) gain navigational texture:
  trade routes can shave past a sun's heat zone; a black hole makes its
  system's deep space genuinely dangerous.
- Randomness is seeded server-side; a player sees a fair 50/50, but
  benchmarks pin `encounter_seed` and stay deterministic.
- The schema is additive; third-party packs referencing this schema version
  load old content unchanged.
- Data-driven NPC fleets and fuel costs for teleports were considered and
  deferred (teleport keeps fuel; fleets stay hardcoded rosters this pass).
- The roadmap for content/system placement lives in
  `docs/ideas/2026-10-21-galaxy-expansion.md`.