# ADR 0022 — Planetary rings and orbiting moons

- Status: accepted
- Date: 2026-10-31

## Context

Planets in Second Velocity were uniform 90px discs at fixed offsets, and the
only orbiting bodies were artificial stations (`StationPlacement::Orbit`,
ADR-0013). Adding planetary rings and natural moons (both scenic ambient moons
and landable moonports) gives systems visual variety and new orbital points of
interest while reusing existing orbit math and docking infrastructure.

## Decision

1. **Planetary rings (`PlanetRingDef` on `PlanetDef`)**:
   - Purely visual 2.5D tilted ring systems configured per planet in RON
     (`inner_radius`, `outer_radius`, `tilt_deg`, `aspect`, `color`).
   - `sv-data` validates that `inner_radius > PLANET_RADIUS`,
     `outer_radius > inner_radius`, `aspect` is in `0.15..=0.75`, and `color`
     channels are in `0.0..=1.0`.
   - `sv-client` generates procedural `ring_back` and `ring_front` half-ring
     textures with concentric pixel bands and a Cassini-style division, spawning
     two child sprites on ringed planets (`z = -0.1` behind the planet and
     `z = +0.1` in front of the planet, rotated by `tilt_deg`).
2. **Moons (`MoonDef` on `PlanetDef`)**:
   - Configured per planet with `id`, `name`, `description`, `radius`,
     `orbit_radius`, `period_secs`, `phase`, `landable: bool`, and optional port
     fields (`sprite`, `chief`, `commodities`, `shipyard`, `outfitter`).
   - `sv-data` validates that moon orbits clear the parent planet, any planetary
     ring (`outer_radius`), sibling moons, and sibling orbital stations, and
     rejects port services on non-landable moons (`landable: false`).
   - Landable moons (`landable: true`) participate in `DockableDef::Moon` and
     global docking location ID uniqueness.
3. **Replication and server authority (`sv-protocol`, `sv-server`)**:
   - `MoonTag` is replicated on all moon entities alongside `Position`, updated
     each server tick via `StationOrbit` and `sv_core::orbit_position`.
   - Landable moons also carry `DockableTag` with `DockableKind::Moon` and
     `PortMarket`, reusing `RequestDock` / `RequestLand`, `sync_docked_ships`,
     chief encounters, shops, and market evolution without a second code path.
   - Ambient moons (`landable: false`) omit `DockableTag`, remaining purely
     scenic in space and on the minimap.

## Consequences

- Ringed planets and ambient moons add depth to star systems with minimal
  bandwidth (zero wire cost for rings; one `Position` component per orbiting
  moon).
- Landable moons work seamlessly with existing player `L` landing prompts,
  quests, contracts, markets, and `sv-mcp` scenario steps (`Dock` /
  `FlyToStation`).
