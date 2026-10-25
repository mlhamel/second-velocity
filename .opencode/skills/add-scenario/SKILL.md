---
name: add-scenario
description: Add a new gameplay benchmark and integration test scenario to content/scenarios/ and verify it with sv-mcp
---

# Add a benchmark scenario

Every new gameplay feature should ship with a scenario in `content/scenarios/`
that both validates its behavior as an integration test and measures its tick
latency, entity load, and gameplay efficiency.

1. Create `content/scenarios/<id>.ron`:
   - Fields: `id`, `name`, `description`, optional `encounter_seed`,
     `max_ticks`, and `steps: [...]`
   - Combine high-level action steps (`FlyToPlanet`, `FlyToSystem`, `Land`,
     `LiftOff`, `Trade`, `Respond`, `SetFlight`, `SetWarp`, `Advance`) with
     assertion steps (`AssertDocked`, `AssertSystem`, `AssertCredits`,
     `AssertCargo`, `AssertEncounter`, `AssertActiveQuests`,
     `AssertCompletedQuest`, `AssertAlive`)
   - Reference only valid system, planet, commodity, and quest IDs from
     `content/`
2. Add `"scenarios/<id>.ron"` to `scenarios:` in `content/manifest.ron`
   - For stations, use `FlyToStation(system: "<id>", station: "<id>", dock: true)`
     to track the moving replicated position. `Dock(system: "<id>", location:
     "<id>")` docks when already nearby. `AssertDocked(Some("<station-id>"))`
     works for stations too. Explicit docking checks the complete system/ID.
3. Validate content schema and cross-references:
   `cargo test -p sv-data`
4. Run the scenario as a benchmark and integration test:
   `cargo run -p sv-mcp -- --bench <id>` (or `make bench`) and
   `cargo test -p sv-mcp`
5. Verify lints:
   `cargo clippy --workspace --all-targets -- -D warnings`
