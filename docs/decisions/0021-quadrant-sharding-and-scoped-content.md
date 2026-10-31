# ADR 0021 — Quadrant sharding and quadrant-scoped content (Phases 0–1)

- Status: accepted
- Date: 2026-10-31

## Context

Second Velocity targets a community-shard galaxy (ADR-0008) where multiple
operators can host pieces of one continuous 2D plane while offline single-player
continues to embed the full galaxy over an in-memory transport (ADR-0004).
Previously, all 15 systems, 12 quests, and 3 ghost-pilot personas lived in flat
top-level `content/{systems,quests,pilots}/` directories, `sv-server` always
loaded and spawned the entire galaxy, and pirate spawning / contract-board
bounties hardcoded `PIRATE_SYSTEM = "alpha-centauri"`.

## Decision

1. **Quadrant-scoped content folders (`content/quadrants/<id>/`)**:
   - A resource is quadrant-scoped if it references systems, planets, or ports,
     and global if other content references it.
   - `content/quadrants/<id>/manifest.ron` is the `QuadrantDef` (`id`, `name`,
     rectangle center `origin`, full `size`, and relative lists of `systems`,
     `quests`, and `pilots`).
   - Top-level `content/manifest.ron` lists `quadrants` plus global definitions
     (`ships`, `outfits`, `commodities`, `scenarios`, `factions`).
   - Today's 15 systems, 12 quests, and 3 pilots live in
     `content/quadrants/sol-reach/` with `origin: (0.0, 0.0)` and
     `size: (10000.0, 10000.0)`. The empty `content/missions/` directory is
     removed.
2. **Coordinate model and containment invariants in `sv-data`**:
   - Galaxy position = `quadrant.origin + system.position`. Because `sol-reach`
     is anchored at `(0.0, 0.0)`, all existing coordinates and saves remain
     numerically unchanged.
   - Quadrant rectangles must have finite dimensions exceeding
     `2 * SYSTEM_EDGE_CLEARANCE` (`1600.0`) and must not overlap one another.
   - Every system center must sit strictly more than
     `SYSTEM_EDGE_CLEARANCE = 800.0` (`SYSTEM_MEMBERSHIP_RADIUS`) inside all
     four edges of its quadrant rectangle so system membership discs never touch
     a quadrant boundary.
   - Planet and station IDs are validated for global uniqueness across all
     loaded quadrants.
   - Static references (`system.links`, quest givers/destinations/bounty
     systems, and pilot `home_system`/`home_port`) must stay strictly within
     their quadrant; cross-quadrant references are rejected by the loader.
3. **Two-level scoped loader (`ContentPack::load_dir_scoped`)**:
   - `ContentPack::load_dir(root)` loads all quadrants (`load_dir_scoped(root, None)`).
   - `ContentPack::load_dir_scoped(root, Some(quadrant_id))` loads global
     content plus only the selected quadrant, and filters global `scenarios` to
     those whose referenced systems, ports, and quests belong to the served
     quadrant.
4. **One server, one quadrant (`--quadrant <id>`)**:
   - `sv-server` accepts `--quadrant <id>` and records `ServedQuadrant(Option<String>)`.
     When omitted, the server loads all quadrants (offline embedded default).
   - Because `ContentPack` is scoped at load time, `spawn_world`, `MarketState`,
     `FactionState`, `ContractBoard`, NPC rosters, and system membership
     automatically scope to the served quadrant.
5. **Per-system `pirate` content flag**:
   - `PIRATE_SYSTEM` is replaced by `#[serde(default)] pub pirate: bool` on
     `SystemDef` (`pirate: true` on `alpha-centauri.ron`).
   - The flag feeds contract-board hazard math and bounty target selection in
     `contracts.rs`, initial pirate spawning in `spawn_world`, and pirate
     respawning in `respawn_pirates`. A served scope with no `pirate: true`
     systems spawns no pirates and generates no pirate bounty contracts.
6. **Galaxy map quadrant overlay (`sv-client`)**:
   - `GalaxyMapPlugin` projects quadrant bounding rectangles and renders each
     quadrant's boundary box and `[id]` label beneath system nodes.

## Consequences

- Offline single-player and existing saves are unaffected (`sol-reach` at
  `(0.0, 0.0)`).
- Fleet shards can now run a single quadrant cleanly via
  `sv-server --quadrant <id>`, establishing the foundation for Phase 2 boundary
  wrap-around and Phase 3 peer gossip + HMAC handoff tickets.
