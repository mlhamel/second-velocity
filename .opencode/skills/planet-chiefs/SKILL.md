---
name: planet-chiefs
description: Build playbook for the planet chiefs feature - docking encounters with animated chiefs (resource offers, tribute demands) and quests as a new content resource. Use when continuing, reviewing, or extending the chiefs/quests feature work.
---

# Planet chiefs — docking encounters and quests

Feature: when a player lands on a planet, an encounter panel opens with the
planet's chief — an animated NPC who offers resources, demands tribute, or
offers a quest. Quests are a dedicated content resource (`content/quests/`).

All design decisions below are locked with the owner. Do not re-litigate
without asking; update this file when a phase completes.

## Locked decisions

- Presentation: side overlay panel (minimap-panel pattern), animated portrait
- Trigger: auto-open on every dock; Esc dismisses; T reopens while live
- Quest types: delivery, bounty, timed (all three in v1)
- Tribute refusal: flavor notice + reputation stub only (real consequences
  wait for the M4 reputation system)
- Quests persist across login sessions (SQLite column + migration)
- Encounter coexists with trading (market keys stay live)

## Architecture

Server-authoritative per ADR-0004; content is data per ADR-0002; the
encounter ships as a replicated component (not an event) so it survives the
MCP idle reconnect.

### Content layer

- `crates/data/src/quest.rs` (new): `QuestDef { id, name, giver: (system,
  planet), kind, reward_credits, description }` where kind is
  `Delivery { commodity, qty, destination }` or `Bounty { system, target }`,
  both with optional `deadline_secs`. Export in `crates/data/src/lib.rs`.
- `ChiefDef { name, portrait, greeting, offer_text, tribute_text,
  quest_text }` added as `#[serde(default)] chief: Option<ChiefDef>` on
  `PlanetDef` in `crates/data/src/system.rs`.
- Manifest gains `quests: []` — must use `#[serde(default)]` on the manifest
  struct field, or the six inline manifest literals in
  `crates/data/tests/pack.rs` all break.
- Loader: parse quest files, validate quest-to-planet references, add a
  `quest()` accessor, extend `base_pack_loads` counts and negative tests.
- `validate_sprites` must validate chief portraits.
- Chief dialog text follows docs/gdd/writing-style.md: character-voiced,
  dry wit, ≤120 words per briefing block.
- v1 content: chiefs for earth + newhaven; 3-4 starter quests (one per
  kind). Placeholder portraits `assets/textures/chiefs/<planet-id>.png` via
  a new `chief()` draw function in `ops/generate_placeholder_art.py`;
  add the row to docs/gdd/art-guide.md.
- Ship the `add-quest` skill (.opencode/skills/add-quest/SKILL.md) with this
  phase, mirroring add-ship/add-system.

### Protocol layer (crates/protocol/src/lib.rs)

- `ChiefEncounter` replicated component on the player (model: `DockedAt`):
  chief name, portrait, mood enum (model for enums: `ShipKind`), dialog
  text, options list (string ids + labels).
- `ActiveQuests` replicated component: quest states with deadlines.
- `RespondToEncounter { choice: String }` client event (model:
  `BuyCommodity` — string id, server re-validates).
- Register everything in `register(app)`; it must stay symmetric across
  server, client, and MCP harness (protocol hash).

### Server layer (crates/server/src/gameplay.rs)

- On RequestLand success: roll the encounter with a seedable `EncounterRng`
  resource (tests need determinism). Mood priority: TurnIn (quest is
  completable here) > weighted offer/tribute/quest; skip quests already
  completed or active. Insert `ChiefEncounter`; remove it on lift-off.
- Observer on RespondToEncounter: validate choice against the live
  encounter. Offer = discounted batch or cargo gift (capacity-checked).
  Tribute = credits transfer on pay; refuse = notice flavor only. Quest
  accept = delivery cargo granted directly (courier model — the player
  never buys quest cargo).
- `quest_progress` system: tick timed deadlines to failure notice; watch
  pirate deaths (projectile owner attribution, see `projectile_hits` +
  `ship_deaths`) to complete bounties. Turn-in at the destination (delivery)
  or giver planet (bounty).
- Storage (crates/server/src/storage.rs): `PlayerState` gains
  `active_quests_json` (serde_json like cargo). Schema uses CREATE TABLE IF
  EXISTS, so existing DB files need an ALTER TABLE migration path. Touch:
  state_columns, row_to_state, create_account INSERT, save_player UPDATE,
  both PlayerState constructions in gameplay.rs (auto_save + ship_deaths),
  and quest restore in on_login_request + respawn_players. Death fails
  active quests (cargo is lost anyway).

### Client layer (crates/client/src)

- `chief.rs` plugin (new): framed panel built once like the HUD, shown on
  Added<ChiefEncounter>, hidden on Removed or Esc. ImageNode portrait via
  AssetServer with procedural fallback (textures.rs pattern). Numbered
  options; digits send RespondToEncounter; T reopens.
- Animation is manual (established pattern: debris_flight/tick_flashes in
  visuals.rs) — idle bob via sine on the portrait transform plus a subtle
  scale pulse while the panel is open. No bevy_animation wiring.
- HUD quest line: destination, commodity, deadline countdown
  (hud.rs HudLayout pattern).
- Keys wire in input.rs alongside the existing digit trade keys — while
  the encounter panel is open, digits route to encounter choices instead.

### MCP layer (crates/mcp)

- observe() gains `encounter` and `quests` JSON keys (query the server
  world directly, like the market array).
- New `respond(choice)` tool + Command variant + harness method.
- game_thread already calls ensure_connected() per command — new tools get
  that for free.

## Phases (verify after each; keep cargo test green)

1. Docs: ADR-0009 (docking encounters + quests as content),
   docs/gdd/encounters.md, amend core-loops.md venue (bars → planet chiefs)
2. Content schema: quest.rs, ChiefDef, manifest, loader validation, pack
   tests, placeholder art, add-quest skill. Verify: cargo test -p sv-data
3. Server engine: encounter roll, RespondToEncounter observer, quest
   state + turn-in, storage migration. Verify: cargo test -p sv-server
4. Client scene: chief.rs panel + animation, HUD quest line, input keys.
   Verify: cargo build -p sv-client + clippy
5. MCP + tests + tour: observe keys, respond tool, playthrough quest loop
   test (dock → accept → deliver → reward), tour capture stage.
   Verify: cargo test --workspace, cargo clippy -D warnings

## Status

- 2026-10-13: plan approved by owner; no phases started yet.
- 2026-10-16: Phases 1–5 completed and verified (`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all --check`):
  - **Phase 1 (Docs)**: Added `docs/decisions/0009-docking-encounters-and-quests.md`, `docs/gdd/encounters.md` (indexed in `docs/gdd/README.md`), and updated `docs/gdd/core-loops.md` mission venue from spaceport bars to planet chiefs.
  - **Phase 2 (Content schema & pack)**: Added `crates/data/src/quest.rs` (`QuestDef`, `QuestKind::{Delivery, Bounty}`, optional `deadline_secs`), `ChiefDef` on `PlanetDef`, `quests` on `Manifest`, cross-reference & portrait sprite validation in `crates/data/src/loader.rs`, 4 starter quests in `content/quests/`, chiefs on `earth` and `newhaven`, `assets/textures/chiefs/{earth,newhaven}.png` via `ops/generate_placeholder_art.py`, and `.opencode/skills/add-quest/SKILL.md`.
  - **Phase 3 (Protocol & Server engine)**: Added `ChiefEncounter`, `ChiefMood`, `EncounterOption`, `ActiveQuests`, `ActiveQuest`, `QuestTarget`, and `RespondToEncounter` in `crates/protocol`; implemented `EncounterRng`, docking encounter roll + auto-turn-in on `RequestLand`, `on_respond_to_encounter` observer, `quest_progress` system (deadline expiry + `LastDamagedBy` pirate kill bounty attribution), and SQLite `active_quests_json` migration (`PRAGMA table_info(accounts)`) in `crates/server`.
  - **Phase 4 (Client scene)**: Added `crates/client/src/chief.rs` overlay panel with portrait bob/scale pulse animation, active quest HUD line in `crates/client/src/hud.rs`, digit key priority routing (`1`–`5` and `X` dismiss) in `crates/client/src/input.rs`, and procedural fallback portrait texture in `crates/client/src/textures.rs`.
  - **Phase 5 (MCP, tests, tour)**: Extended `observe()` with `encounter` and `quests` JSON fields, added `respond(choice)` MCP tool and harness method, added `quest_loop_playthrough` integration test in `crates/mcp/tests/playthrough.rs`, and added `02b-chief-earth` capture stage to `crates/client/src/tour.rs`.