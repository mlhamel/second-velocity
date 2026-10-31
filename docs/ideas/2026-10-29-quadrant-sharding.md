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
- **Fleet transport** (owner, 2026-10-30): server-to-server traffic —
  gossip, handoffs, lease checkpoints — is **REST over HTTP with JSON
  payloads** between peers. Handoff is a two-party request/response
  (accept or reject with reason), JSON matches the server's existing
  serde_json state serialization (SQLite storage blobs), and curl/jq
  keep friends-scale ops debuggable — a single new dep, a standard Rust
  HTTP server crate (axum unless Phase 3 grooms otherwise). Rejected:
  pub/sub (a broker is a coordinator by another name, and there is no
  reply pattern for handoff accept/reject), gRPC (a protobuf toolchain
  and a third wire schema for ~4-6 endpoints), renet2-UDP reuse (would
  rebuild reliable RPC on a client netcode). Clients stay on renet2 UDP
  for gameplay — the fleet channel is a separate, tiny HTTP surface.
- **Identity**: per-server accounts + short-lived HMAC **handoff tickets**
  minted with a fleet-shared key. The target server *leases* a copy of
  the visiting account for the duration of the visit (`IncomingPilot`
  carries the argon2 hash); the home server remains the canonical owner.
  A handoff is refused loudly when the pilot's name collides with a
  native account on the target — the sender returns the pilot home with
  a notice. Two leases with the same pilot name but different home
  quadrants are distinct on the target (keyed by home + name); only a
  native collision refuses. Accepted friends-scale limitation.
- **Server-down semantics** (owner, 2026-10-29): a player's anchor is
  their home quadrant. When a server goes down, visiting players it
  hosted automatically return to their home quadrant; players owned by
  the departed quadrant simply disappear — no other server hosts them —
  until it returns, and its edges revert to wrap-around.
- **Visit lifecycle** (owner, 2026-10-29): death while visiting respawns
  in the visited quadrant (existing pipeline — death is never free
  fast-travel home); logging out while visiting camps you there, and
  re-login lands exactly where you were, against the visited server.
  Returning home is always an explicit flight. If the visited server is
  gone at re-login, the client falls back to the home server, which
  restores the fallback snapshot and treats the visit as ended. The
  client persists its home server endpoint alongside the camp record,
  so that fallback is always reachable.
- **Visitor-state sync** (owner, 2026-10-30): nothing streams home
  continuously — the visited server is fully authoritative for the
  player while the visit lasts (until return handoff, logout/camp, or
  home-server-down eviction); credits, purchases, and quest progress
  mutate the leased copy live. The home fallback snapshot refreshes at
  every handoff and at every logout (camp); the return handoff ends
  the lease and home's copy becomes the live authoritative state
  again, with everything gained on the trip. A visited-server crash
  can therefore only lose progress made since the last sync point —
  normally the current session.
- **Unserved boundary = wrap-around**: a quadrant behaves like a closed
  world; fly off one edge, reappear on the other (toroidal teleport
  reusing the black-hole teleport path). Edges open to handoffs only when
  the neighbor quadrant is actually served by another server — and not
  before Phase 3: until then every edge wraps regardless of neighbor
  status.
- **Cross-quadrant travel**: crossing a served boundary is the real way
  between quadrants — fly into an open edge and the pilot is handed to
  the neighbor server. **Wormhole black holes** (Phase 4) are optional,
  configurable shortcuts between served pairs; adjacent pairs do not
  need them — their value is non-adjacent jumps.
- **No cross-quadrant content** (owner, 2026-10-29): static content stays
  in its quadrant — quests, commodity price tables, faction-control
  system lists, and NPC routes never span a boundary; the loader rejects
  any that does. Cross-quadrant interaction is player-driven only.
- **Content layout** (owner, 2026-10-30): quadrant-scoped folders. The
  classification principle: a resource is quadrant content if it
  references systems/planets/ports, global if other content references
  it. `content/quadrants/<id>/` carries `systems/` (price tables and
  faction control ride inside system files), `quests/`, and `pilots/`
  (ghost pilots are home-anchored) plus a per-quadrant `manifest.ron`
  that doubles as the `QuadrantDef` (id, name, rectangle origin + size)
  — superseding the single `content/quadrants.ron` sketch. `ships/`,
  `outfits/`, `commodities/` (identity only), `factions/` (identity
  only), and `scenarios/` (global test fixtures, filtered to the served
  set by the scoped loader) stay at top level. First quadrant folder:
  `quadrants/sol-reach/`; the empty `missions/` leftover is deleted.
- **Market evolution: per quadrant** (owner, 2026-10-29): each server runs
  its quadrant's living market (ADR-0015) independently — supply drift,
  candles, and price news never cross a boundary, and the same commodity
  may diverge freely across quadrants. The only cross-market channel is
  physical: players hauling cargo through handoffs. A departed server's
  market freezes with it and resumes from its storage on return.
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
  for the pilot with a short-lived HMAC ticket; the receiver leases the
  pilot for the visit (argon2 hash travels in `IncomingPilot`) while the
  home server stays canonical owner. No central account store to operate
  or compromise.
- Server-down stays simple: no failover hosting, no split-brain — a
  departed server's players are down with it (permanently, if it never
  comes back), and everyone else's visitors fall back home.
- Per-quadrant markets keep each shard's economy self-contained — price
  history, supply drift, and news are one server's story, and nothing
  economic ever has to stay in sync across a boundary.

## Phase plan

First milestone = **Phase 0 + Phase 1**. Every phase ships with a
benchmark scenario + tests per house rules (AGENTS.md). Phases 2-5 are
queued: groom each at its phase-start session, not in advance.

### Phase 0 — Content schema + validation

- [ ] Content reorg per the locked layout: `content/quadrants/sol-reach/`
      with `systems/`, `quests/`, `pilots/` and a per-quadrant
      `manifest.ron` that IS the `QuadrantDef` (id, name, rectangle
      origin + size); top-level `content/manifest.ron` gains the
      explicit quadrants list; two-level loader in sv-data (global +
      selected quadrant manifests, all when unflagged); delete the
      empty `missions/`
- [ ] Every system belongs to exactly one quadrant (folder + loader
      check); sol-reach covers today's 15 systems
- [ ] Coordinate model: galaxy position = quadrant origin + local offset
      (sol-reach sits at the origin, so today's positions are the local
      offsets, numerically unchanged)
- [ ] Planet/station ids globally unique (enforced by the loader)
- [ ] Hyperlinks stay in-quadrant; cross-quadrant travel is spatial
      only — boundary crossings now, optional wormhole shortcuts later
      — never hyperlink jumps
- [ ] Content containment: the loader rejects any static reference
      spanning a quadrant boundary — quest destinations, commodity
      price-table systems, faction-control lists, NPC routes all stay
      in-quadrant
- [ ] Edge clearance: systems sit strictly more than 800px
      (`SYSTEM_MEMBERSHIP_RADIUS`, `crates/server/src/gameplay.rs:49`)
      from any quadrant edge, so membership discs never touch a boundary
- [ ] Galaxy map quadrant overlay in the client
- [ ] ADR-0021 written in this session

### Phase 1 — One server, one quadrant

- [ ] `--quadrant <id>` CLI flag (sv-server): fleet servers serve exactly
      one quadrant; omitted flag serves all (offline embedded default)
- [ ] `spawn_world` (planets, market state, price evolution), membership
      candidates, `ContractBoard`, `FactionState`, and NPC rosters scoped
      to the served quadrant (all when no flag is passed); scenarios
      filtered to those resolvable in the served set
- [ ] `PIRATE_SYSTEM` hardcode (`crates/server/src/gameplay.rs:31`) must
      become a per-system content flag — no system or quadrant is
      required to be a pirate system; flagging is per system. The flag
      feeds every consumer: contract-board hazard math
      (`crates/server/src/gameplay/contracts.rs`), the initial pirate
      spawn, and the pirate respawn spawn site (the respawn cooldown
      itself is a global timer). A served scope with no flagged pirate
      systems generates no pirate-hazard jobs and no pirates
- [ ] Scenario + tests for scoping (a two-quadrant pack serves one and
      ignores the other)

### Phase 2 — Boundaries + wrap

- Boundary detection; wrap-around teleport reusing the black-hole
  teleport path. All edges wrap in this phase — served edges become
  handoffs in Phase 3.

### Phase 3 — Gossip mesh + cross-server handoff

- Seed peers, keepalive, full-state quadrant map exchange, any-time joins;
  with one server per quadrant the map is a flat quadrant → endpoint
  table. Fleet channel: REST over HTTP with JSON (owner, 2026-10-30) —
  a small surface (~4-6 endpoints: gossip, handoff, lease-checkpoint,
  ops status); exact crate choice and endpoint shapes groom here.
- Crossing a served boundary is the real traversal (owner, 2026-10-29):
  flying into an open edge hands the pilot to the neighbor server.
  Wormholes (Phase 4) remain optional shortcuts, not the primary road.
- Handoff leases a `PlayerState` blob to the target server
  (`PendingRecoveries`-style failure safety, ADR-0014 pattern) so a
  dropped handoff never strands a pilot; the home server stays canonical
  owner and keeps fallback state for its players who are out visiting,
  refreshed at every handoff and every logout (owner, 2026-10-30).
- Visit lifecycle (owner, 2026-10-29): death while visiting respawns in
  the visited quadrant; logout camps the pilot on the visited server —
  the lease persists across sessions and re-login lands where they were;
  returning home is an explicit flight (return handoff). If the visited
  server is unreachable at re-login, the pilot falls back to the home
  server and the visit ends.
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

### Phase 4 — Wormholes (optional shortcuts)

- Optional, configurable wormhole black holes between served pairs —
  adjacent pairs already have their open edge, so the value is
  non-adjacent jumps. Reuses black-hole visuals/teleport machinery
  (ADR-0012).

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
- Login at home while leased out: home holds only a fallback snapshot
  while a visit is live, so a second login there would fork the pilot —
  refuse loudly, or reroute to the visited server? Lock in Phase 3.
- Faction metagame (ADR-0017): `FactionState` is per-shard today; does
  control of a system in an unattended quadrant drift for nobody, and
  does anything merge when a neighbor comes online?
- Galaxy map on a fleet-connected client: show all known quadrants or
  only the served set?

## Accepted limitations (owner, 2026-10-29)

- Per-server accounts with no global namespace: a name taken on another
  server is rejected loudly at handoff. Fine at friends-playtest scale;
  revisit if the fleet outgrows it.
- Wrap edges are a fiction standing in for "neighbor not served"; players
  who cross an edge with no live neighbor re-enter the same quadrant.
- A departed server's players are down with it — and if it never comes
  back, they are gone for good: their newest state exists only inside
  transient leases on other servers, and no other server may take over
  as the canonical home for those players. If that server disappears,
  you disappear; no failover, no transfer escape hatch — consciously
  accepted at friends-scale.

## Precedents the build reuses

- Black-hole teleport path + swept-crossing detection (galaxy-expansion
  Phase 3, ADR-0012) — the wrap-around and wormhole mechanism
- `PendingRecoveries` failure-safety pattern (ADR-0014 shipyards)
- Per-server SQLite accounts + argon2 login gate (ADR-0007), including
  the "Already logged in." freeze that disconnect cleanup must clear
- `PROTOCOL_ID` in sv-protocol with sv-server re-export (sv-log's
  move-it-once precedent)
- The PIRATE_SYSTEM hardcode's three consumers — contract-board hazard
  math, initial pirate spawn, respawn spawn site — are what the
  per-system pirate flag (Phase 1) replaces

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
- Owner refinements of 2026-10-29, all recorded as Director decisions in
  the quadrants area (do not re-litigate): one-server-one-quadrant
  (01M64P5YW09TTX04NAN5MQR5DJ), per-system pirate flag
  (01M64PN0BC4Y11Q0DAXJW3S463), server-down semantics
  (01M64PN0BWMZ3EXY814V8WP7WT), per-quadrant markets
  (01M65AXXV8YWH17DYHQZNQ1JZT), boundary-crossing traversal with optional
  wormholes (01M65BJYYQEACJKQC9X9H16NS3), visit lifecycle — death and
  logout (01M65BJYZ2T8YRJG6QW1NH06DM, 01M65BJYZAQ4H4WZ8VFQWG172R),
  connection-anchor loss (01M65BJYZJPGWN57VX41Y7Z3CM), and content
  containment (01M65BJYZTM4ADPXWENRBR81T4).
- Content layout decided 2026-10-30 (decisions 01M66QG3FCEFV8BJG1CH4Q6AVM
  and 01M67718B63JEA54XEMTBPEYX3, owner-confirmed): quadrant-scoped
  folders with the per-quadrant manifest as the `QuadrantDef` — first
  folder `quadrants/sol-reach/`, scenarios global, ghost pilots
  quadrant-bound, `missions/` deleted. Do not re-litigate the
  classification.
- Visitor-state sync points decided 2026-10-30 (decision
  01M680BQMF3YJTVKZQ8YFMMCJT): the home fallback snapshot refreshes at
  every handoff and at every logout — do not introduce continuous
  cross-server state streaming.
- Fleet transport decided 2026-10-30 (decision 01M680Y7GAME6AMC7M34T7G7FB): REST over HTTP
  with JSON between peers — do not reintroduce a broker (pub/sub) or a
  second wire-schema toolchain (gRPC) without a new owner decision.
- ADR numbering: 0021 is the next free number. docs/decisions carries a
  pre-existing duplicate-0016 collision; do not renumber unilaterally —
  the owner must bless a renumber or accept the drift.
- Every phase ships a benchmark + integration scenario in
  `content/scenarios/` (AGENTS.md house rule) before it is called done.