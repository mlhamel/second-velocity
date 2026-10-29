# ADR 0018 — Sidecar LLM ghost pilots (two-tier reflex + decision client)

- Status: accepted
- Date: 2026-10-27

## Context

The come-back galaxy playtest needs the shard to feel inhabited even when only
a few humans are online. Existing `NpcBrain` roles (merchants, defenders,
explorers, pirates) run inside the authoritative server tick as deterministic
state machines, but they cannot hold persistent player accounts, take authored
and procedural quests, upgrade their hulls at shipyards, or exercise
persona-shaped judgement at encounter dialogs.

At the same time, a local 3–9B LLM (via Ollama) takes 1–10 seconds per
completion, while the authoritative shard ticks every ~15ms (60 Hz) and
benchmark scenarios require deterministic server execution.

*(Note: originally slated as ADR-0015 in `docs/ideas/2026-10-23-ghost-pilots.md`
before living markets, contract boards, pixel art, and factions took
0015–0017.)*

## Decision

1. **Sidecar UDP client, never `NpcBrain::Pilot`**: Ghost pilots live in a
   standalone headless client crate (`crates/pilot`, binary
   `second-velocity-pilot` / `evr-pilot`) built on `MinimalPlugins +
   RepliconPlugins + RepliconRenetPlugins`. They connect over the exact same
   protocol (`sv-protocol`) as desktop players, authenticate via
   `LoginRequest` (auto-registering their persona account on a `--storage`
   shard), and persist credits, hull, outfits, cargo, reputation, and quests in
   SQLite (`PlayerState`). Zero server-side simulation changes are required.
2. **Two-tier architecture (reflex vs. decision)**:
   - **Reflex layer (deterministic Rust, every tick)**: A standing-orders
     executor (`StandingOrder::FlyTo`, `Flee`, `Idle`) owns continuous steering
     (`steer_approach`), deceleration-aware docking approach, hazard-aware
     tangent routing around sun heat discs (`1.8 * radius`) and black-hole
     influence discs, emergency pirate flee (`< flee_range`), and low-fuel
     return-to-port. The LLM never touches throttle or turn inputs.
   - **Decision layer (async worker thread, decision points only)**: At discrete
     decision points (`Docked`, `EncounterOffered`, `HostileClose`,
     `QuestComplete`, `LowFuel`, `IdleHeartbeat`, `RespawnedAfterDeath`), the
     main loop sends a compact ~400-token state digest (`render_digest`) to a
     background worker thread. The worker queries Ollama (`/api/generate` with
     `format: "json"`, 20s timeout, ≥15s cadence between requests) or a
     deterministic `MockBrain` (`--mock`), validates the returned `PilotAction`
     against live state and content (`parse_and_validate`), retries once with
     validation error feedback on failure, and falls back to deterministic
     standing orders if the model still fails.
3. **Personas are content (`content/pilots/`)**: `PilotDef` RON files define
   persona id, display name, account credentials, home system/port, role brief,
   preferred commodities, preferred shipyard upgrades, and reflex thresholds.
   `sv-data` validates every persona's home port, commodity ids, and ship ids
   against the content pack.

## Rejected alternatives

- **Server-internal `NpcBrain::Pilot` calling an LLM**: Couples authoritative
  tick latency or server thread pools to external HTTP calls, introduces
  non-determinism into `sv-server`, and creates a second persistence path
  parallel to `PlayerState`.

## Consequences

- LLM latency or malformed model output can never stall the shard tick or
  corrupt server state; worst case, a single pilot falls back to deterministic
  home-port autopilot.
- CI runs the deterministic `MockBrain` in-process over memory transport
  (`crates/pilot/tests/pilot_loop.rs`); live Ollama runs are ops/playtest
  daemons launched alongside `sv-server --storage`.
