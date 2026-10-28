# ADR 0017 — Factions, per-player reputation, and collective system control

- Status: accepted
- Date: 2026-10-26

## Context

Phase 4 of the come-back galaxy roadmap (docs/ideas/2026-10-23-come-back-galaxy.md):
the galaxy remembers what you did, and the war is bigger than any one pilot.
System content already carried free-text `government` strings, but only a stub
consumed them — the PvP fine/patrol check hardcoded Sol and ignored the
attacker's actual system. There was no Rebellion foothold, no reputation, and
no way to see the war.

Owner-locked scope answers (2026-10-26):

- Reputation model blessed as proposed (moves: quests/contracts +5 with the
  port-owner faction, pirate kills +2 with the system owner, patrol kills −10,
  merchant/defender kills −5, tribute +3; hostility at ≤ −50 with a
  system's controller: patrols aggro on sight)
- Control meter per system: i32 −100…+100 (Rebellion…Confederation), init by
  owner side (+60 / −60 / 0), shifted by quest/contract completion toward the
  giver's war side, +2 pirate kills, −5 patrol kills; controller flips at
  ±50 and reverts to the content government below
- Vega flips government to the Rebellion (foothold on the Confederation's
  doorstep)
- Weekly dispatch: standings composed from server control data every 7
  sim-days, broadcast as a Notice; forceable for tests
- Galaxy map on `M` is the scoreboard; ADR number for this feature is 0017
  (the deferred sv-log Phase C becomes 0018)

## Decision

**Factions are content** (`content/factions/`): four definitions with id,
name, description, color, and war side. The loader validates that every
system's `government` references a faction — governments stop being dangling
strings. Faction colors feed the galaxy map.

**Reputation is personal and sticky** (EV rule: the galaxy remembers; the game
never forces a side): per-player standings map, replicated via a
`PlayerReputation` component on the player ship, persisted as
`reputation_json` in `PlayerState` (serde-default migration, no renames —
player-identity.md).

**Control is collective and per-system**: a server-authoritative
`FactionState` resource owns the meters; `SystemControl` components
replicate one entity per system. The controlling war faction drives local
policing and PvP rules (ADR-0008's "government zones are the ruleset" becomes
dynamic), replacing the hardcoded Sol check — patrols now respond in the
attacker's actual system, notices and fines name the controller, and control
flips broadcast galaxy news. Meters persist in SQLite (`system_control`).

**The map is the scoreboard**: the client's `M` overlay draws every system at
its content position colored by controller, contested systems two-tone, with
the player's own standings. The weekly dispatch is composed from the same
control data.

## Consequences

- The wire grows two replicated components; protocol hash moves client and
  server together (same commit, pre-M3 norm).
- Reputation effects beyond hostility (exclusive missions/ships/equipment in
  factions.md) remain future work; this phase ships the tracker, the
  movement rules, hostility, and the metagame.
- Under the locked shift rules only war-owned systems' meters move (quests
  shift the giver's system; pirate/patrol kills shift where they happen),
  so independent and pirate systems hold at 0 this phase; winning them over
  needs a future "territory events" mechanic, not this one.
- Determinism: control and reputation math are pure functions of content +
  action events; scenario steps (AssertControl, AssertReputation,
  AdvanceDispatch) and the faction-control scenario ride the existing
  benchmark harness.