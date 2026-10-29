---
status: accepted
---

# Quadrant sharding: one galaxy, many servers

Divide the galaxy into N **quadrants** — generic sector rectangles
partitioning the current continuous 2D plane — and let each server serve
exactly one of them (one server, one quadrant; offline play keeps the
whole galaxy local). Servers discover each other via peer gossip (no
coordinator), hand pilots off across boundaries with short-lived HMAC
tickets, and wrap unserved edges so every shard is playable standalone.
Approved with owner 2026-10-29.

This is the scaling step for the community-shard goal (ADR-0008): one
galaxy, many operators, each hosting a piece — while offline single-player
runs ALL quadrants locally and is unaffected.

## Locked scope answers (owner, 2026-10-29)

- **Shape**: N quadrant regions, generic sector rectangles partitioning
  the continuous 2D plane. Not fixed at four; content-defined.
- **Compatibility**: today's 15 systems become quadrant **sol-reach**
  anchored at the origin (0,0). Existing saves and coordinates are
  untouched.
- **Server scoping**: **one server, one quadrant** (owner correction,
  2026-10-29 — the earlier configurable `--quadrants` set is rejected as
  needless flexibility). Fleet servers pass `--quadrant <id>` and serve
  exactly one; omitting the flag serves all quadrants, which is the
  offline embedded default — single-player is unaffected.
- **Discovery**: **peer gossip** — owner explicitly rejected a
  coordinator. Seed peers + keepalive + full-state quadrant map exchange;
  servers join at any time.
- **Identity**: per-server accounts + short-lived HMAC **handoff tickets**
  minted with a fleet-shared key. `IncomingPilot` carries the argon2 hash
  so the target server adopts the account. Name conflicts across servers
  are rejected loudly — accepted friends-scale limitation.
- **Server-down semantics** (owner, 2026-10-29): a player's anchor is
  their home quadrant. When a server goes down, visiting players it
  hosted automatically return to their home quadrant; players owned by
  the departed quadrant simply disappear — no other server hosts them —
  until it returns, and its edges revert to wrap-around.
- **Unserved boundary = wrap-around**: a quadrant behaves like a closed
  world; fly off one edge, reappear on the other (toroidal teleport
  reusing the black-hole teleport path). Edges open to handoffs only when
  the neighbor quadrant is actually served by another server.
- **Cross-quadrant travel**: spatial boundary crossing + auto-opening
  **wormhole black holes** between served quadrant pairs (configurable,
  Phase 4).
- **Ghost pilots**: constrained to their home quadrant (Phase 5).

## Why this architecture

- Every server stays fully authoritative for what it serves (ADR-0004);
  there is no shared write path and no global mutable state.
- Rectangles partition the existing continuous plane — no re-projection of
  content, no save migration; sol-reach simply is the galaxy we have today.
- Gossip over a coordinator removes the single point of failure and the
  ops ceremony: a new operator joins the fleet by pointing at one seed
  peer.
- One server, one quadrant: every shard has a single quadrant identity —
  gossip entry, boundary set, handoff targets are all 1:1; no server ever
  straddles a seam, and fleet capacity grows by adding one server per
  quadrant.
- Wrap-around means a shard whose neighbors are down is still a complete,
  playable space — the boundary degrades to a seam, never a wall.
- Offline play embeds all quadrants, so there is no second code path
  (ADR-0004's rule holds: one server app, memory or UDP transport).
- Handoff tickets keep accounts server-local: the sending server vouches
  for the pilot with a short-lived HMAC ticket; the receiver adopts the
  account by importing the argon2 hash from `IncomingPilot`. No central
  account store to operate or compromise.
- Server-down stays simple: no failover hosting, no split-brain — a
  departed server's players are down with it, and everyone else's
  visitors fall back home.

## Phase plan

First milestone = **Phase 0 + Phase 1**. Every phase ships with a
benchmark scenario + tests per house rules (AGENTS.md). Phases 2-5 are
queued: groom each at its phase-start session, not in advance.

### Phase 0 — Content schema + validation

- [ ] `content/quadrants.ron`: `QuadrantDef` (id, name, rectangle
      origin + size) + loader validation in sv-data
- [ ] `SystemDef.quadrant` assignment; every system belongs to exactly one
      quadrant; sol-reach covers today's 15 systems
- [ ] Coordinate model: galaxy position = quadrant origin + local offset
- [ ] Planet/station ids globally unique (enforced by the loader)
- [ ] Hyperlinks stay in-quadrant; cross-quadrant travel is spatial
      (wrap, handoff, wormholes), never hyperlink jumps
- [ ] 800px edge clearance: systems keep clear of their quadrant's edges
      so membership discs never straddle a boundary
- [ ] Galaxy map quadrant overlay in the client
- [ ] ADR-0021 written in this session

### Phase 1 — One server, one quadrant

- [ ] `--quadrant <id>` CLI flag (sv-server): fleet servers serve exactly
      one quadrant; omitted flag serves all (offline embedded default)
- [ ] `spawn_world`, membership candidates, `ContractBoard`,
      `FactionState`, and NPC rosters scoped to the served quadrant (all
      when no flag is passed)
- [ ] `PIRATE_SYSTEM` hardcode (`crates/server/src/gameplay.rs:31`) must
      become a per-system content flag — no system or quadrant is
      required to be a pirate system; flagging is per system. The flag
      feeds every consumer: contract-board hazard math
      (`crates/server/src/gameplay/contracts.rs`), the initial pirate
      spawn, and the pirate respawn timer. A served scope with no
      flagged pirate systems generates no pirate-hazard jobs and no
      pirates
- [ ] Scenario + tests for scoping (a two-quadrant pack serves one and
      ignores the other)

### Phase 2 — Boundaries + wrap

- Boundary detection; wrap-around teleport reusing the black-hole
  teleport path; edges open to handoff only when the neighbor is served.

### Phase 3 — Gossip mesh + cross-server handoff

- Seed peers, keepalive, full-state quadrant map exchange, any-time joins;
  with one server per quadrant the map is a flat quadrant → endpoint
  table.
- Handoff carries a `PlayerState` blob with `PendingRecoveries`-style
  failure safety (ADR-0014 pattern) so a dropped handoff never strands a
  pilot; the home server keeps fallback state for its players who are
  out visiting.
- Server-down flow (owner, 2026-10-29): keepalive loss marks the quadrant
  unserved — edges close and wrap resumes; hosted visitors automatically
  return home and are restored there; the departed server's own players
  go dark until it returns, including ones away visiting elsewhere (home
  is always the anchor).
- Prerequisites, in this order:
  - **Disconnect cleanup**: stale ships currently freeze accounts
    ("Already logged in.") — must be fixed before any handoff work.
  - **`PROTOCOL_ID` bump**: protocol hash changes; five binaries ship in
    lockstep — client, server, sv-pilot, sv-log, sv-mcp (PROTOCOL_ID
    lives in sv-protocol; sv-server re-exports it).

### Phase 4 — Wormholes

- Auto-opening wormhole black holes between served quadrant pairs,
  configurable; reuses black-hole visuals/teleport machinery (ADR-0012).

### Phase 5 — AI scoping + ops

- Ghost pilots (ADR-0018) constrained to their home quadrant.
- Fleet ops: seed-peer configuration, shard status surfacing.

## Open questions (groomed at phase start, not pre-locked)

- Gossip transport details: UDP port choice, seed-peer config format,
  keepalive cadence, quadrant-map exchange size as the fleet grows.
- Ticket hygiene: handoff-ticket TTL and rotation story for the
  fleet-shared HMAC key.
- **Authentication beyond friends-scale**: per-server accounts + HMAC
  handoff tickets is the accepted friends-scale answer; a real auth story
  (global identity, central or federated accounts) is a future workstream
  the owner flagged 2026-10-29. Revisit before any open/public fleet.
- Visitor-state freshness: when a visit ends by server death, the home
  server restores the returning visitor from fallback state — is that the
  handoff-moment snapshot, or do visits checkpoint home periodically?
  Determines how much in-visit progress a returned visitor loses.
- Faction metagame (ADR-0017): `FactionState` is per-shard today; does
  control of a system in an unattended quadrant drift for nobody, and
  does anything merge when a neighbor comes online?
- Economy isolation: each server owns its quadrants' living market
  (ADR-0015) outright — cross-server arbitrage does not exist. Accept as
  friends-scale, or gossip price headlines?
- Galaxy map on a fleet-connected client: show all known quadrants or
  only the served set?

## Accepted limitations (owner, 2026-10-29)

- Per-server accounts with no global namespace: a name taken on another
  server is rejected loudly at handoff. Fine at friends-playtest scale;
  revisit if the fleet outgrows it.
- Wrap edges are a fiction standing in for "neighbor not served"; players
  who cross an edge with no live neighbor re-enter the same quadrant.
- A departed server's players are down with it — no failover hosting,
  accepted for friends-scale ops simplicity.

## Precedents the build reuses

- Black-hole teleport path + swept-crossing detection (galaxy-expansion
  Phase 3, ADR-0012) — the wrap-around and wormhole mechanism
- `PendingRecoveries` failure-safety pattern (ADR-0014 shipyards)
- Per-server SQLite accounts + argon2 login gate (ADR-0007), including
  the "Already logged in." freeze that disconnect cleanup must clear
- `PROTOCOL_ID` in sv-protocol with sv-server re-export (sv-log's
  move-it-once precedent)
- The PIRATE_SYSTEM hardcode's three consumers — contract-board hazard
  math, initial pirate spawn, respawn timer — are what the per-system
  pirate flag (Phase 1) replaces

## Relationship to other roadmaps

- **ADR-0008 community shard**: quadrants are the "many operators, one
  galaxy" step; a single shard remains the default small deployment.
- **Come-back galaxy playtest**: friend-hosted shards can split the fleet
  load while sharing one map; sv-log already per-shard spectators the
  news feed.
- **Ghost pilots (ADR-0018)**: stay home-quadrant in Phase 5; their
  sidecar connects to exactly one server either way.
- **sv-log (accepted idea doc)**: one spectator per shard; a fleet view
  would run one per shard.

## Follow-up rules

- Any session picking this up: read this file + ADR-0021 once it exists,
  then check the Director log for newer decisions; do not re-litigate the
  locked scope answers above.
- Phase numbering here follows decision 01M64MAYVWP5T8W4KJNW62DGEH
  (six phases, 0-5); the roadmap open-item's "Phases 2-6" phrasing counted
  the same queue differently — this file is the canonical numbering.
- Server scoping was corrected 2026-10-29 to one-server-one-quadrant
  (decision 01M64P5YW09TTX04NAN5MQR5DJ): do not re-introduce
  multi-quadrant serving without a new owner decision.
- Two further owner refinements of 2026-10-29 are recorded as decisions
  01M64PN0BC4Y11Q0DAXJW3S463 (pirate systems are a per-system content
  flag; none is required) and 01M64PN0BWMZ3EXY814V8WP7WT (server-down:
  visitors return home, owned players go dark until their server
  returns).
- ADR numbering: 0021 is the next free number. docs/decisions carries a
  pre-existing duplicate-0016 collision; do not renumber unilaterally —
  the owner must bless a renumber or accept the drift.
- Every phase ships a benchmark + integration scenario in
  `content/scenarios/` (AGENTS.md house rule) before it is called done.