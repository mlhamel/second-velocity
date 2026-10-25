---
name: benchmark-scenarios
description: Build playbook for the game scenario benchmark and integration testing system powered by sv-mcp. Use when implementing, running, or extending automated gameplay benchmarks and scenario-driven integration tests.
---

# Benchmark scenarios — MCP-driven gameplay benchmarks & integration tests

Feature: declarative gameplay scenarios (`content/scenarios/<id>.ron`) executed
through the `sv-mcp` harness that serve simultaneously as **end-to-end
integration tests** (`cargo test -p sv-mcp`) and **performance/gameplay
benchmarks** (`cargo run -p sv-mcp -- --bench` / MCP `run_scenario` tool).
Every new gameplay feature ships with a scenario.

All design decisions below are locked with the owner. Update this file as
phases complete.

## Locked decisions

- **Scenarios are content (`content/scenarios/`)**: authored in RON, listed in
  `content/manifest.ron` (`#[serde(default)] pub scenarios: Vec<String>`), and
  cross-reference-validated by `sv-data` at pack load time (ADR-0002).
- **Single source of truth for integration tests + benchmarks**: the same
  `.ron` scenario drives functional assertions in `cargo test` and collects
  timing + entity + gameplay metrics in benchmark mode.
- **Feature policy**: any new gameplay feature must include a scenario in
  `content/scenarios/` and pass both `make test` and `make bench`.
- **Three execution entrypoints**:
  1. `cargo test -p sv-mcp` — runs all scenarios in `content/manifest.ron` and
     asserts every step and invariant passes.
  2. `cargo run -p sv-mcp -- --bench [scenario_id] [--json]` (aliased as
     `make bench`) — runs one or all scenarios and prints a Markdown table (or
     JSON report) with tick latency percentiles, entity load, and gameplay stats.
  3. MCP tool `run_scenario(id)` — lets connected AI agents execute a named
     scenario (or the full suite) and inspect the structured JSON report.

## Metrics collected per scenario

1. **Tick compute latency** (wall-clock duration of `server.update()` and
   `client.update()`, excluding the 15ms transport sleep):
   - `avg_tick_us`, `p50_tick_us`, `p95_tick_us`, `p99_tick_us`, `max_tick_us`
   - `server_avg_us`, `client_avg_us`
2. **Entity & replication load**:
   - `peak_server_entities`, `peak_client_entities`, `peak_projectiles`
3. **Gameplay efficiency & outcome**:
   - `total_ticks`, `credits_delta`, `fuel_consumed`, `hull_damage_taken`,
     `quests_completed`, `assertions_passed`

## Architecture

### 1. Content layer (`crates/data/src/scenario.rs`)

- `ScenarioDef`:
  ```rust
  pub struct ScenarioDef {
      pub id: String,
      pub name: String,
      pub description: String,
      #[serde(default)]
      pub encounter_seed: Option<u64>,
      #[serde(default = "default_max_ticks")]
      pub max_ticks: u64,
      pub steps: Vec<ScenarioStep>,
  }
  ```
- `ScenarioStep` enum combines high-level MCP actions, autopilot helpers, and
  declarative assertions:
  - **Actions**:
    - `SetFlight { throttle: f32, turn: f32, fire: bool }`
    - `SetWarp(bool)`
    - `Advance { ticks: u32 }`
    - `FlyToPlanet { system: String, planet: String, land: bool }`
    - `FlyToSystem { system: String }`
    - `Land { planet: String }`
    - `LiftOff`
    - `Trade { action: String, commodity: String, qty: u32 }`
    - `Respond { choice: String }`
  - **Assertions**:
    - `AssertAlive(bool)`
    - `AssertSystem(String)`
    - `AssertDocked(Option<String>)`
    - `AssertCredits { min: Option<u32>, max: Option<u32> }`
    - `AssertCargo { commodity: String, min_qty: u32 }`
    - `AssertEncounter { mood: Option<String> }`
    - `AssertActiveQuests { count: usize }`
    - `AssertCompletedQuest(String)`
- Loader validation (`crates/data/src/loader.rs`):
  - Verify duplicate scenario IDs.
  - Validate that every `system`, `planet`, `commodity`, and `quest` referenced
    in `ScenarioStep` exists in the `ContentPack`.

### 2. Harness & Runner layer (`crates/mcp/src/harness.rs` & `crates/mcp/src/benchmark.rs`)

- `TickSample`:
  ```rust
  pub struct TickSample {
      pub server_us: u64,
      pub client_us: u64,
      pub total_us: u64,
      pub server_entities: usize,
      pub client_entities: usize,
      pub projectiles: usize,
  }
  ```
- `Harness` records `Vec<TickSample>` during `step()` by timing
  `self.server.update()` and `self.client.update()` around `Instant::now()`
  before the 15ms netcode sleep.
- `run_scenario(content_root: &Path, def: &ScenarioDef) -> ScenarioReport`:
  - Initializes a fresh `Harness`, optionally seeds `EncounterRng` on the
    server if `def.encounter_seed` is set.
  - Executes each `ScenarioStep` in order, enforcing `def.max_ticks`.
  - Autopilot steps (`FlyToPlanet`, `FlyToSystem`) use closed-loop steering
    over `observe()` + `set_flight()` + `set_warp()` so scenarios stay concise
    and readable.
  - Computes latency percentiles (`p50`, `p95`, `p99`, `max`), peak entity
    counts, and net deltas (`credits_delta`, `fuel_consumed`,
    `hull_damage_taken`).

### 3. Starter scenarios (`content/scenarios/`)

1. `sol-centauri-trade.ron` — dock on Earth, buy food, warp to Alpha Centauri,
   dock on Newhaven, sell food for a profit.
2. `quest-delivery-loop.ron` — dock on Earth, accept `newhaven-relief` from
   Director Vance, warp to Newhaven, turn in to Chief Okafor, verify reward.
3. `combat-dock-immunity.ron` — fly near Pirate in Alpha Centauri, exchange
   weapons fire, land on Newhaven while under attack, advance ticks and verify
   hull takes zero damage while docked.

### 4. MCP tool, CLI mode & Makefile (`crates/mcp/src/main.rs`, `Makefile`)

- If `sv-mcp` is invoked with `--bench [scenario_id] [--json]`, it runs the
  benchmark suite headlessly without starting the stdio MCP server, prints a
  formatted Markdown table (or JSON), and exits non-zero if any scenario fails.
- When running as an MCP server, `sv-mcp` exposes `run_scenario(id?: String)`
  so agents can run benchmarks on demand.
- `Makefile` gains `make bench` (and `make bench-json`).

## Phases (verify after each; keep `cargo test` green)

1. **Content schema & starter scenarios**: `crates/data/src/scenario.rs`,
   `scenarios` in `Manifest`, loader cross-reference validation, pack tests,
   3 starter `.ron` scenarios in `content/scenarios/`, and `add-scenario` skill.
   Verify: `cargo test -p sv-data`
2. **Timed harness & scenario runner**: `TickSample` instrumentation in
   `crates/mcp/src/harness.rs`, scenario executor + percentile aggregator +
   Markdown/JSON formatter in `crates/mcp/src/benchmark.rs`.
   Verify: `cargo test -p sv-mcp`
3. **MCP tool, CLI `--bench`, integration test & Makefile**: `--bench` flag and
   `run_scenario` tool in `crates/mcp/src/main.rs`, `all_pack_scenarios_pass`
   in `crates/mcp/tests/playthrough.rs`, `make bench` in `Makefile`.
   Verify: `make bench && cargo test --workspace && cargo clippy --workspace -- -D warnings`
4. **Docs & conventions**: document `make bench` and the scenario requirement
   for new features in `AGENTS.md`.

## Status

- 2026-10-16: Phases 1–4 implemented (`crates/data/src/scenario.rs`, `content/scenarios/*.ron`, `crates/mcp/src/benchmark.rs`, `--bench` CLI + `run_scenario` MCP tool, `all_pack_scenarios_pass` test, `make bench`, and `AGENTS.md`).
