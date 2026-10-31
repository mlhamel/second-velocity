---
status: accepted
---

# Planetary rings and orbiting moons

Enrich star system worlds with two celestial features attached directly to
`PlanetDef` in content:

1. **Planetary rings (`ring: Option<PlanetRingDef>`)**: purely visual 2.5D
   tilted ring systems that wrap behind and in front of a planet's disc with
   pixel-art concentric banding and a Cassini-style gap.
2. **Moons (`moons: Vec<MoonDef>`)**: natural satellites orbiting their parent
   planet using deterministic circular orbit math (`sv_core::orbit_position`).
   Moons can be either **ambient** (`landable: false`, scenic satellites on the
   starfield and minimap) or **landable** (`landable: true`, orbiting moonports
   where pilots can land with `L` to refuel, trade, visit a moonbase chief, or
   use a shipyard/outfitter).

## Locked design choices (owner, 2026-10-31)

- **Moons**: support both ambient (`landable: false`) and landable
  (`landable: true`) moons. Barren moons are scenic or refuel-only anchorages;
  inhabited moons can host a moonbase market, chief, quests, and shops.
- **Planetary rings**: purely visual 2.5D tilted rings (no collision or drag
  gameplay effect), rendered with a back arc behind the planet and a front arc
  in front of the planet.

## Content schema (`sv-data`)

- `PlanetRingDef` on `PlanetDef` (`#[serde(default)] pub ring: Option<PlanetRingDef>`):
  - `inner_radius: f32` (must exceed `PLANET_RADIUS`)
  - `outer_radius: f32` (must exceed `inner_radius`)
  - `tilt_deg: f32` (finite rotation angle in degrees)
  - `aspect: f32` (foreshortening ratio in `0.15..=0.75`, default `0.35`)
  - `color: (f32, f32, f32)` (RGB in `0.0..=1.0`)
- `MoonDef` on `PlanetDef` (`#[serde(default)] pub moons: Vec<MoonDef>`):
  - `id: String`, `name: String`, `description: String`
  - `radius: f32` (positive, smaller than `PLANET_RADIUS`)
  - `orbit_radius: f32`, `period_secs: f32`, `#[serde(default)] phase: f32`
  - `#[serde(default)] landable: bool`
  - Optional port fields (used when `landable: true`): `sprite`, `chief`,
    `commodities`, `shipyard`, `outfitter`
- Validation in `sv-data`:
  - All moon IDs are unique; landable moon IDs participate in global
    `"docking location"` ID uniqueness and `DockableDef::Moon`.
  - Non-landable moons cannot declare `chief`, `commodities`, `shipyard`, or
    `outfitter`.
  - Every moon orbit must clear its parent planet's surface (`PLANET_RADIUS`),
    any planetary ring (`ring.outer_radius`), other moons on the same planet,
    and any orbital station around that planet.

## Simulation, replication, and presentation

- `sv-protocol`:
  - `MoonTag { system, planet, id, name, radius, landable }` replicated for all
    moons.
  - `DockableKind::Moon` added for landable moons so `DockableTag` and
    `DockedAt` distinguish planets, stations, and moons.
- `sv-server`:
  - `spawn_world` spawns every moon with `MoonTag`, `Position`, and
    `StationOrbit` centered on its parent planet; `station_motion` advances all
    orbiting moons alongside orbital stations.
  - Landable moons also receive `DockableTag` (`DockableKind::Moon`) and
    `PortMarket`, participating in docking (`RequestDock` / `RequestLand`),
    orbital docking tracking (`sync_docked_ships`), refueling, encounters,
    contract boards, and market drift.
- `sv-client`:
  - `textures.rs` procedurally generates `moon` (cratered pixel-art sphere) plus
    `ring_back` and `ring_front` (concentric banded half-ellipses with a gap).
  - `visuals.rs` attaches back/front ring children to ringed planets and renders
    orbiting `MoonVisual` sprites updated each frame from replicated `Position`.
  - `minimap.rs` renders moons as distinct satellite blips.

## Checklist

- [x] Save idea spec in `docs/ideas/2026-10-31-planetary-rings-and-moons.md`
- [x] Record ADR-0022 in `docs/decisions/0022-planetary-rings-and-moons.md`
- [x] Add `PlanetRingDef`, `MoonDef`, `DockableDef::Moon`, and validation in `sv-data`
- [x] Add `MoonTag` and `DockableKind::Moon` in `sv-protocol`
- [x] Spawn and orbit moons (ambient + landable) in `sv-server`
- [x] Render 2.5D planetary rings and orbiting moons in `sv-client` (`textures.rs`, `visuals.rs`, `minimap.rs`)
- [x] Populate `sol-reach` content with moons (e.g. Luna & Phobos in Sol, Vela Relay Moon in Vega) and planetary rings (e.g. Newhaven in Alpha Centauri, Sirius Prime in Sirius, Deepwatch in Trappist)
- [x] Add benchmark scenario `content/scenarios/moon-landing-trade.ron` + tests
