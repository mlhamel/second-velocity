---
status: implemented
---

# Ghost pilots: local-model player-like ships (evr-pilot sidecar daemon)

LLM-driven ships that live in the shard like players — trade, take quests,
buy ships, die, persist, and come back. Approved 2026-10-23.

## Sequencing (hard dependency)

`buy_ship` uses the `BuyShip` intent and owned-ship persistence from the
come-back roadmap's **Phase 1 (progression)** — built in the working tree
as of 2026-10-23 (ADR-0014 shipyards/loadouts), pending the owner's commit.
Ghost pilots build **after Phase 1 lands**; pairs best after Phase 2
(economy), when pilots become visible market agents.

## Locked scope answers (owner, 2026-10-23)

- **Brain**: small local model (3–9B via ollama) — digest hard-capped
  ~400 tokens; strict JSON contract via ollama's `format: "json"` option,
  one retry with error feedback, then deterministic fallback
- **Action set**: core loop + shipyard — `fly_to`, `dock`, `trade`,
  `buy_ship`, `respond`, `flee`, `idle` (refuel implicit on landing)
- **Deployment**: same-box UDP CLI; memory transport only for in-process
  integration tests; requires a `--storage`-backed shard
- **Cadence**: one fresh session builds the MVP from this doc

## Why this architecture (decision, see also ADR to be written)

A local LLM answers in seconds; the shard ticks at 15ms. So the pilot is
two-tier:

1. **Reflex layer** (deterministic Rust, every tick): standing-orders state
   machine (`FlyTo → Dock → TradeQueue → LiftOff…`) owning steering,
   deceleration-aware docking, hazard-aware routing (sun heat discs 1.8r and
   black-hole influence discs from content → tangent waypoints),
   flee-from-pirate, auto-refuel. The LLM never flies.
2. **Decision layer** (ollama, decision points only): a decision thread
   receives a ~400-token digest (persona, ship/fuel/credits/cargo, docked
   market + shipyard prices, verbatim encounter text + options — the same
   structured choices a human gets, top-3 threats, allowed-actions list)
   and returns one `PilotAction` JSON. ≥15s between calls, 20s timeout,
   fallback = continue orders / return to home port.

Server-authoritative + clients-send-intents means a pilot is just another
UDP client: LLM latency can never hurt the shard tick. `LoginRequest`
auto-registers persona accounts; `PlayerState` persists credits, ship,
outfits, quests — a pilot is a returning player, not a disposable bot.

Rejected alternative: server-internal `NpcBrain::Pilot` calling an LLM
service — couples the authoritative tick to LLM latency and breaks bench
determinism. (To be recorded as ADR-0015; 0013 is stations, 0014 is
shipyards/loadouts.)

## Live-demo lessons that shaped the design (2026-10-23 session)

1. Raw `observe()` is 500–1300 lines — pilots need the distilled digest
2. Manual docking at full burn overshot Earth — reflex owns approach and
   deceleration, never the model
3. Unattended flight through a meteorite band destroyed the ship —
   reflex needs hazard-aware routing

## Build checklist (one session, full battery + @review)

- [x] `crates/pilot` skeleton + CLI (`--connect host:port --persona
      content/pilots/<id>.ron [--mock]`); `PilotDef` RON schema; 3 starter
      personas (cautious trader, courier, bounty hunter) in `content/pilots/`
- [x] Headless client (MinimalPlugins + Replicon): connect → login →
      reflex loop; reconnect on netcode 15s idle-drop (`ensure_connected`
      pattern from `crates/mcp/src/harness.rs`)
- [x] Digest renderer (~400 tokens, format above)
- [x] `PilotAction` schema + validation; decision thread (ureq — first
      HTTP client dep in the workspace) with ollama `format:"json"`;
      mock brain (scripted deterministic responses) for CI
- [x] Reflex executor: steering (`tour.rs` `steer()` pattern), docking,
      hazard routing, flee, refuel
- [x] Decision points: docked, encounter offered, hostile < 600u, quest
      complete, fuel < 25%, idle heartbeat 30–60s, death
- [x] Integration test: stub-brain pilot vs embedded memory server —
      connect → login → fly to Earth → dock → trade → accept quest →
      deliver → turn in
- [x] ADR-0018 (sidecar LLM pilots; why not NpcBrain::Pilot); crate README
      runbook (shard + ollama same box, systemd example)
- [x] Full validation battery + @review

## Out of MVP (explicitly)

- Fighting back (flee only) — combat reflexes are the hardest tier
- Chat/social personas (belongs to come-back roadmap phase 5)
- Fleet intel sharing between pilots
- Prompt-injection hardening beyond friends-scale content trust

## Risks (accepted)

- Small models mis-decide → structured choices, strict JSON, fallback
  autopilot; worst case a pilot flies dumb, never the shard
- Pilots die — they respawn like players; death is a decision point
- No bench scenario, by design: AGENTS.md's "every gameplay feature ships
  with a benchmark scenario" rule covers sim mechanics; the pilot daemon is
  an external client/ops tool with no server-side sim change, so the
  deterministic stub-brain integration test covers the loop instead

## Precedents the build reuses

`udp_client_app` (server tests, gameplay.rs:3701) · `steer()` (tour.rs:61) ·
`ensure_connected`/`force_reconnect` (mcp harness.rs) · encounter options as
the model's action space · login auto-registration (on_login_request)

## Relationship to other roadmaps

Parallel idea to the come-back galaxy roadmap (docs/ideas/
2026-10-23-come-back-galaxy.md) — serves the same friends-playtest goal:
the shard feels alive when only 2 humans are online. Hard dependency on
its Phase 1 (see Sequencing above); pairs best after Phase 2 (economy).

## Follow-up rules

- Any session picking this up: read this file, the come-back roadmap,
  ADR-0004 (server-authoritative/clients-send-intents), ADR-0014
  (shipyards — for buy_ship), and the cited precedent code before writing
  Rust; check the Director log for latest decisions; do not re-litigate
  the locked scope answers above.
- The ADR for this feature is **0015** (0013 stations, 0014
  shipyards/loadouts are taken).
- The two-tier split is architectural, not stylistic: no amount of model
  quality justifies moving flight control into the LLM, and no server-side
  sim change is in scope for this workstream.
- Mock-brain determinism is load-bearing: CI runs the stub brain only;
  real-ollama runs are manual smoke tests on the shard box.