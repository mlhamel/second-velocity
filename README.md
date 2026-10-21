# Second Velocity: The Living Galaxy

*One galaxy, shared by pilots and minds.*

A community-shard MMO inspired by Escape Velocity (1996): 2D top-down space
trading, missions, and combat in a shared, persistent galaxy. Rust + Bevy,
server-authoritative, playable offline single-player through an embedded
server.

## Quickstart

```bash
cargo run -p evr-server 3456 --storage shard.db     # standalone shard (UDP :3456, persistent)
cargo run -p evr-client                            # offline single-player (embedded server)
cargo run -p evr-client -- --connect host:port --username you --password secret
cargo test --workspace                              # unit + end-to-end tests
```

## Screenshots

![Flight near Earth](docs/screenshots/01-flight-earth.png)
![Duel with a Turncoat in Alpha Centauri](docs/screenshots/02-combat.png)
![Docked and trading at Newhaven](docs/screenshots/03-docked-trade.png)

Captured by the automated tour: `cargo run -p evr-client -- --capture-tour`
(F12 saves a screenshot anytime during play).

## Structure

| Path | What |
|---|---|
| `crates/` | evr-core (sim), evr-data (content), evr-protocol (wire), evr-server, evr-client, evr-mcp (MCP server) |
| `content/` | RON content packs — data, not code |
| `docs/gdd/` | game design (README is the index) |
| `docs/decisions/` | ADRs — the why behind everything |
| `docs/ideas/` | idea inbox, never deleted |
| `prompts/` | curated prompt library |
| `.opencode/skills/` | agent workflows |
| `ops/` | placeholder art generator, deployment (future) |

Agents: start at [AGENTS.md](AGENTS.md). Humans: start at
[docs/gdd/README.md](docs/gdd/README.md).

The game is also playable by agents: `evr-mcp` exposes the sim as an MCP
server (registered as `evr-game` in opencode.json) — observe, fly, trade,
fight, and warp through tool calls. See `crates/mcp`.

## Milestones

- **M0** — workspace + content pipeline ✅
- **M1** — offline vertical slice: fly, trade, fight, hyperjump ✅
- **M2** — real networking: standalone server, 2+ players, per-system rooms, govt-zone PvP ✅
- **M3** — persistence: accounts, login gate, SQLite player state ✅
- **M3.5** — continuous galaxy: warp drive replaces hyperjump, 11-system shared plane, minimap, MCP auto-reconnect ✅
- **M4** — community playtest: shared economy tick, faction meta-game, ops