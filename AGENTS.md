# Second Velocity: The Living Galaxy

Community-shard MMO inspired by Escape Velocity (1996): 2D top-down space
trading, missions, and combat in a shared persistent galaxy. Rust + Bevy.
Server-authoritative; the same server code embeds in the client for offline
single-player.

## Commands

- build: `cargo build --workspace`
- test: `cargo test --workspace` (single crate: `cargo test -p evr-core`)
- benchmark scenarios: `make bench` or `cargo run -p evr-mcp -- --bench [scenario_id] [--json]`
- lint: `cargo clippy --workspace -- -D warnings`
- format: `cargo fmt --all` (check: `cargo fmt --all --check`)
- commit check: `cog check` or `make commit-check` (install git hooks: `make hooks` / `cog install-hook --all`)
- run server: `cargo run -p evr-server [port] [--storage path.db] [--verbose]` (standalone shard; UDP, defaults to 3456; SQLite persistence with --storage)
- run client online: `cargo run -p evr-client -- --connect host:port [--username name] [--password pw] [--verbose]`
- run client: `cargo run -p evr-client [-- --verbose]` (desktop session required)
- capture screenshots: `cargo run -p evr-client -- --capture-tour` (F12 in-game)

## Structure

- `crates/core` — headless simulation (economy, factions, combat math); no Bevy, no I/O
- `crates/data` — RON content loader + validation; its structs are the content schema
- `crates/protocol` — client intents / server events + replicated components; register(app)
  is called identically by client and server (protocol hash must match)
- `crates/server` — authoritative simulation app; storage trait + SQLite persistence; login gate
- `crates/client` — Bevy desktop app
- `crates/mcp` — evr-mcp: the game as an MCP server (agents play the sim
  via observe/set_flight/land/lift_off/trade/jump/advance/run_scenario); registered as
  evr-game in opencode.json
- `content/` — RON content packs; data, never code
- `assets/` — textures referenced by content sprite fields; placeholders generated via ops/generate_placeholder_art.py (see docs/gdd/art-guide.md)
- `docs/gdd/` — canonical game design; README is the index
- `docs/decisions/` — ADRs; write one before non-trivial architecture changes
- `docs/ideas/` — idea inbox; status-tagged, never deleted
- `docs/reference/` — EV/EVO/EVN/Endless Sky research
- `prompts/` — curated prompt library
- `.opencode/skills/` — repeatable project workflows
- `ops/` — deployment configs (M3+)

## Conventions

- Content is data in `content/`, never code. Use the add-ship/add-system/add-quest/add-scenario skills.
- Every new gameplay feature must ship with a benchmark & integration test scenario in `content/scenarios/` (see `.opencode/skills/add-scenario/SKILL.md`).
- Server is authoritative. Clients send intents (`crates/protocol`); they never
  mutate game state.
- Offline play = the client embeds the server app (NonSendMut in evr-client) over a memory
  transport; no second code path. Online play swaps the transport for UDP; same wire protocol.
- Server app must be finished AFTER attaching its transport (attach_memory/attach_udp);
  build_server_app does not call finish() itself.
- `evr-core` stays Bevy-free and I/O-free. Presentation belongs in `evr-client`.
- Bevy/replicon/renet2 versions are pinned together (read docs/decisions/0005
  before upgrading). rustc >= 1.95 required by bevy 0.19.
- In-game text follows `docs/gdd/writing-style.md`.
- History lives in git: ADRs for decisions, `docs/ideas/` for everything considered.
  Never delete an idea; re-status it.
- All git commit messages must follow Conventional Commits (`<type>[optional scope]: <description>`),
  enforced via Cocogitto (`cog.toml`, `.githooks/commit-msg`, and GitHub Actions).
- No unsafe code (workspace lint).