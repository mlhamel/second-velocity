---
status: accepted
---

# The come-back galaxy: M4 retention roadmap

Multi-phase roadmap toward the first friends-scale playtest, approved
2026-10-23. Each phase is one workstream-sized unit, independently shippable,
landing with the full validation battery (workspace tests, clippy/fmt, make
bench, @review). Build cadence: one phase per session, like the galaxy
expansion (see `2026-10-21-galaxy-expansion.md`).

Scope answers locked with the owner on 2026-10-23:

- **First playtest audience**: friends/Discord group (~10–30 people) — light
  ops, presence matters, depth over moderation tooling
- **First slice**: progression (shipyard + outfit shops) — money must matter
  before anything else pays off
- **Come-back hook for week 1**: "the galaxy changed" — the living economy
  (phase 2) is the return hook, ahead of repeatable contracts
- **Cadence**: one phase per session with full validation + benchmark +
  @review per phase

## The retention problem (why this roadmap exists)

The core sim is deep (15 systems, stations, suns, black holes, NPC fleets,
quests, SQLite persistence, govt-zone PvP) but every loop dead-ends today:

| Gap | Why it kills the MMO loop |
|---|---|
| Money can't buy anything | Ships have prices but no shipyard exists; 1 weapon in the game. Credits are worthless → no goals → no retention |
| Content runs out | 12 hand-authored one-shot quests; a player sees everything in ~2 hours |
| Static economy | Fixed prices = memorizable routes = dead in days; nothing changes while away |
| No return hook | The galaxy you come back to is identical to the one you left |
| No social fabric | No chat, no online list; players fly past each other in silence |
| No identity | Reputation is designed but unbuilt; the galaxy doesn't remember what you did |

## Phase 1 — Progression: money must matter (done)

- [x] Schema: `ShipyardDef` / `OutfitterDef` optional per port (port-level
      lists of what's for sale; content is data, never code); new outfit
      kinds beyond `Weapon` — shield generator (max_shields), fuel tank
      (max fuel), armor plate; prices on outfits; `weapon_slots` per ship
      already exists and becomes load-limiting
- [x] Protocol: `BuyShip`, `BuyOutfit`, `SellOutfit` intents; replicated
      loadout component; protocol hash moves client+server together
- [x] Server: purchase validation at the docked port (credits, slots,
      availability); trade-in credit for the old hull (EV-style); ship swap
      preserves cargo rules; **death/respawn uses the owned ship** — replaces
      the hardcoded `PLAYER_SHIP` in `ship_deaths`/respawn/persistence;
      `PlayerState` persists owned ship + equipped outfits (save schema
      grows — storage migration per ADR-0007)
- [x] Client: docked-panel tabs (Market / Shipyard / Outfitter)
- [x] Content: shipyards on 3–4 ports (Earth, Sirius Prime, Hawking Stand,
      Wayfarer Exchange); first outfit set (2 weapons, 2 shield tiers,
      1 fuel tank); price sanity pass — a shuttle pilot affords a courier in
      ~2–3 sessions of trading/missions
- [x] Scenario: `shipyard-upgrade.ron` (earn → buy → assert new hull stats
      fly); server unit tests (insufficient funds, wrong port, slot limits,
      death-preserves-ship); loader validation
- [x] Docs: progression.md `[draft]` → `[active]`; ADR if the persistence
      shape change warrants one
- [x] Full validation battery + @review

## Phase 2 — Living markets: the world moves while you sleep (done)

- [x] Per-port supply/demand counters; player trades move supply
- [x] Economy tick (minutes, server-side — not per trade), prices drift from
      list price by supply (shared-economy.md is the design source)
- [x] NPC merchant fleets feed the model (they already haul commodities —
      tie their trades into supply)
- [x] News notices ("Isotope demand spikes at Vega") + persisted market log
      (daily OHLCV candles — nothing finer is stored, nothing is deleted)
- [x] Arbitrage damping: simple, tuned for friends scale
- [x] Commodity definitions move to `content/commodities/`
- [x] Scenario: `market-drift.ron` (flood a port, assert price moves);
      tolerance review of existing trade scenarios (credits asserts may need
      ranges); full battery + @review

## Phase 3 — Contract board: never run out of things to do

- [ ] Procedurally generated repeatable contracts (delivery / bounty / timed)
      offered by chiefs alongside authored quests; refreshed on a timer
- [ ] Session-sized 10–20 minute jobs; rewards scale with distance/hazard
- [ ] Repeatable bounty hunting on pirates; later, on player pirates
- [ ] Scenario: `contract-refresh.ron`; unit tests for generation determinism
      (seeded EncounterRng); full battery + @review

## Phase 4 — Factions and the scoreboard

- [ ] `content/factions/` content pack (Confederation, Rebellion, independents,
      pirates — factions.md is the design source)
- [ ] Per-player reputation: missions, kills, tribute move it; sticky
      (player-identity.md: no renames — reputation is identity)
- [ ] Collective system control shifted by player action; control changes
      policing and PvP rules locally (pvp-policy.md)
- [ ] Galaxy map as scoreboard; weekly dispatch (news post from server data)
- [ ] Scenario + unit tests; full battery + @review

## Phase 5 — Social + playtest launch

- [ ] System chat + online-players list + player name labels
- [ ] Hosted-shard ops runbook (ops/ deployment; UDP + SQLite backup)
- [ ] Onboarding mission chain (first-hour experience)
- [ ] Balance pass over ship prices vs earning rates with real playtest data
- [ ] Full battery + @review; then gather the Discord

## Sequencing notes

- A minimal "playtest night" is possible today (`cargo run -p evr-server` on
  any box); phases 1+2 are the minimum for it to be fun *and worth returning
  to*
- Chat (phase 5) can be pulled forward cheaply any time after phase 1 if the
  group wants presence earlier
- The loop must be fun solo before the meta-layers matter (pillar: freedom);
  but presence early makes a small playtest feel alive

## Follow-up rules

- Any session picking this up: read this file + the design docs each phase
  cites (progression.md, shared-economy.md, encounters.md, factions.md,
  faction-metagame.md, pvp-policy.md, player-identity.md), check the Director
  log for latest decisions, do not re-litigate locked scope answers.
- Energy juggling (EV single pool) is explicitly out of scope until the
  outfit system proves out — see combat.md open questions.
- Each phase ships with its benchmark scenario; scenario-count hardcoded
  assertions live in both `crates/data/tests/pack.rs` and
  `crates/mcp/tests/playthrough.rs`.