---
status: accepted
---

# Galaxy expansion: content, suns, black holes

Multi-phase feature roadmap, approved 2026-10-21. Each phase is independently
shippable; check items off as they land. Architecture decision for phases 2-3
is [ADR-0012](../decisions/0012-environmental-hazards-stars-black-holes.md).

Scope answers locked with the owner on 2026-10-21:

- Content scale: **moderate** (~8 ships, ~4 systems, ~6 quests, ~3 commodities)
- Suns: **featured systems only** (3-4), content-defined offsets keep spawn and
  planet lanes safe
- Sun damage: **ramp + lethal core** (0 at 1.8x radius, ~25 dps surface, lethal
  core; shields absorb first; NPCs burn too)
- Black holes: **gravity pull + 50/50 kill/teleport**, outcomes drawn from the
  seeded `EncounterRng` so scenarios are deterministic
- NPC fleets: **extend hardcoded rosters** in this pass; data-driven fleets are
  a separate future workstream

## Phase 0 — Foundation (done)

- [x] ADR-0012 written
- [x] This roadmap written
- [x] Director decision emitted

## Phase 1 — Content expansion (data-only, no mechanics)

Ships (add-ship skill; placeholder art entries in
`ops/generate_placeholder_art.py` with race tints; roles spread across hull
sizes; prices balanced against the existing 17):

- [x] terran-courier (Light, fast trader)
- [x] terran-frigate (Heavy, gunship)
- [x] vellari-sprinter (Light, scout/courier)
- [x] korath-cutter (Light, raider)
- [x] korath-reaver (Heavy, marauder)
- [x] sylph-skiff (Light, explorer)
- [x] sylph-dreamweaver (Medium, support)
- [x] auroran-titan (Capital, battleship)
- [x] manifest wired; `cargo test -p evr-data` green (pack test counts
      updated: 25 ships / 15 systems / 10 quests)

Commodities (~3 new ids woven into new + some existing price tables so trade
loops have fresh spreads):

- [x] ore (kestrel-drift 12, newhaven 15 -> sol 40, tarsis 38, hawking 30)
- [x] isotopes (deepwatch 55, hawking 45 -> wolf-359 90, vega 82, procyon 78)
- [x] textiles (greenreach 10 -> sirius 32, tarsis 28, polaris 30, sol 18)

Systems (~4 new; positions >800px from existing membership zones; reciprocal
links; planets with chiefs; placeholder planet art):

- [x] cygnus-x1 (2800,-3200) independent, frontier mining, links: tarsis + ross-154
- [x] lalande (-600,-800) confederation rim agrarian, links: sol + sirius
- [x] trappist (-3400,1000) independent, deep research, links: wolf-359 + procyon
- [x] hawking (3000,-900) independent, hazard staging, links: barnard + new-kashmir

Quests (~6 new via add-quest skill; deliveries to new systems, bounties in
pirate lanes, timed contracts):

- [x] ore run cygnus-x1 -> sol (cygnus-ore-shipment)
- [x] isotope delivery trappist -> cygnus-x1 (deepwatch-isotope-run)
- [x] textiles run lalande -> sirius (greenreach-textiles)
- [x] bounty from hawking on the alpha-centauri ore lane (hawking-lane-bounty;
      pirates only spawn in alpha-centauri, so bounty quests must target it)
- [x] timed medicine run lalande -> new-kashmir (greenreach-cold-medicine)
- [x] machinery delivery hawking -> tarsis (hawking-refinery-parts)

NPC rosters (`crates/server/src/gameplay.rs` merchant/defender/explorer
lists, several stationed in new systems):

- [x] merchants: +3 (terran-courier lalande->sol, vellari-sprinter
      cygnus-x1->tarsis, sylph-dreamweaver hawking->new-kashmir)
- [x] defenders: +2 (terran-frigate lalande, auroran-titan cygnus-x1)
- [x] explorers: +1 (sylph-skiff trappist->procyon)
- [x] `cargo test -p evr-server` green (embedded slice exercises pack)

Validation for phase 1:

- [x] `cargo test -p evr-data` and `cargo test -p evr-server` green
- [x] `cargo clippy --workspace -- -D warnings` and `cargo fmt --all --check`
- [x] `make bench` still green (all 5 scenarios PASS, 4 consecutive runs of
      the previously flaky one)
- [x] Robustness fix landed: `ProvokeNearestNpc` in `crates/mcp/src/benchmark.rs`
      now steers and chases its target each tick instead of firing blind from
      a fixed teleport spot (adding NPCs anywhere shifted the chaotic tick
      history and occasionally dodged every shot)
- [x] @review pass on the diff (REQUEST CHANGES resolved): delivery turn-in
      is chief-based, so sirius-prime, tarsis-port, kashmir-fields and
      vega-station got chiefs (the pre-existing `urgent-vega-vaccine` quest
      was uncompletable without one); lalande moved to (400, 1800) to clear
      all membership zones; quest field order aligned

Follow-up (groomed, small): the remaining 5 planets without chiefs
(procyon, wolf-359, polaris, barnard, ross-154 planets) — docking there
offers no chief encounter today.

## Phase 2 — Suns (featured systems)

- [x] Schema: `SystemDef.star: Option<StarDef>` (position offset from system
      center, radius, optional sprite; serde-defaulted; sprite asset validated
      like planet sprites)
- [x] Protocol: replicated `StarTag { system, radius }` + `register()`
- [x] Server: spawn stars in `spawn_world`; new `star_heat` system — ramp 0 at
      1.8x radius -> ~25 dps at surface -> lethal core (500 dps inside 0.5x);
      applies to undocked players AND NPCs; heat-zone entry notice with
      cooldown; reuses `apply_damage`/death pipeline. Heat is spatial
      (corona proximity), not membership-tagged: star heat zones sit beyond
      their own system's 800px membership disc, so membership checks let
      stale-branded ships burn-proof themselves
- [x] Place stars in 4 featured systems (sol, sirius, cygnus-x1, hawking) with
      safe offsets — every heat zone clears all other systems' membership
      discs, spawn points, planet lanes, the full meteorite field disc
      (not just the spawn bands — drifting meteorites wrap the whole disc),
      and NPC travel corridors; enforced by the
      `star_placements_keep_heat_zones_clear` pack test
- [x] Client: star visual (placeholder corona texture `textures/stars/corona.png`
      via art script + procedural fallback; z=-2 behind planets),
      minimap orange dot
- [x] Scenario schema/runner: `FlyToStar { system, approach }` (runner pins the
      ship at the standoff; free-flight parking near hazards is dt-jitter
      fragile) + `AssertHull { min, max }` steps (loader validation +
      `crates/mcp/src/benchmark.rs` match arms)
- [x] Scenario: `sun-heat-damage.ron`; server unit tests (safe distance zero
      damage, NPC burn, lethal core kills, docked immunity, corona warning
      notice)
- [x] GDD: "Environmental hazards" section in `docs/gdd/combat.md`
- [x] Full validation battery + @review (REQUEST CHANGES resolved: heat zones
      now clear the full meteorite field discs — hawking/cygnus stars moved
      outward; sirius star moved due south after the new geometry test caught
      it inside tarsis's membership disc; star sprite scaled so the visible
      corona matches the 1.8r heat boundary; FlyToStar zero-vector guard.
      Two kept-by-design notes: star radius<=0 reuses the "star config"
      UnknownReference like the meteorite-field precedent, and
      HeatWarningCooldown rides on all ships since it is part of the
      star_heat query — player-only would exclude NPCs from burning.
      Re-review: APPROVE)

## Phase 3 — Black holes (1-2)

- [x] Schema: `SystemDef.black_hole: Option<BlackHoleDef>` (position, event
      horizon radius, influence radius, pull strength, optional sprite;
      serde-defaulted; finite/positive values and assets validated)
- [x] Protocol: replicated `BlackHoleTag { system, radius, influence }`
- [x] Server: gravity pull inside influence (accel toward hole scaled by
      proximity); crossing horizon draws `EncounterRng`: 50% lethal damage /
      50% teleport to seeded-random other system center + notice;
      `update_system_membership` picks up new location; docked ships immune
- [x] Content: one hole in Cygnus X-1, offset `(0, -1900)`, horizon 90,
      influence 500, pull strength 180; geometry tests protect all systems'
      spawn/membership discs, fields, planet lanes, stars, and linked routes
- [x] Client: dark disc + accretion ring placeholder texture; minimap violet dot
- [x] Scenarios: `black-hole-teleport.ron` (seeded -> `AssertSystem`),
      `black-hole-death.ron` (seeded -> `AssertAlive(false)`)
- [x] Server unit tests: pull vector, both outcomes, docked immunity,
      preserved teleport state, replication/notices, and existing respawn flow
- [x] Full validation battery + independent review: `cargo test --workspace`
      (74 tests), `cargo build --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo fmt --all --check`, and `make bench` (all 8 scenarios PASS).
      Review found endpoint-only horizon tunneling; fixed with swept
      segment/horizon intersection and a scheduled warp-movement regression.
      Added full-server NPC merchant-brain teleport coverage. Re-review: APPROVE.

Implementation notes:
- Gravity uses world-space distance, including ships with stale system tags.
- Membership reads authoritative `ShipState.position` for players and NPCs,
  so a teleport is recognized without waiting for replicated position sync.
- Horizon detection sweeps the actual integrated flight segment before any
  gravity changes velocity, so fast and grazing crossings cannot skip it.
- `FlyToBlackHole { system }` pins horizon entry as a deterministic scenario
  fixture; the server resolves the outcome. Seeds 0/1 select death/teleport;
  seed 1 currently lands in Tarsis. `observe()` exposes black-hole positions,
  horizon radii, and influence radii.

## Follow-up rules

- Any session picking this up: read this file + ADR-0012, check the Director
  log for the latest decisions, do not re-litigate locked scope answers.
- Data-driven NPC fleets (content RON instead of hardcoded rosters) is a
  separate idea if it comes up again.
- After any repo move/rename: `cargo clean -p evr-data -p evr-server -p
  evr-mcp` before trusting `cargo test --workspace` (stale baked
  `CARGO_MANIFEST_DIR` paths fail silently).