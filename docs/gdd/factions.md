# Factions

[active] — implemented (M4): four-faction content pack, per-player sticky
reputation, controller-driven policing, galaxy map scoreboard (ADR-0017)

Confederation vs Rebellion — the core tension. Plus independents and pirates.

- Per-player reputation: missions, kills, and tribute move it; sticky by
  design — no renames, no resets (player-identity.md)
- Reputation rules: quest/contract turn-in +5 with the giver's faction,
  pirate kill +2 with the system owner, patrol kill −10 with the controller,
  merchant/defender/explorer kill −5 with the system owner, tribute paid +3
  with the port's faction, unlawful discharge fine −2 with the controller
- Hostility: standing ≤ −50 with a system's controller and their patrols
  lock onto you on sight
- EV rule: the game never forces you to pick a side, but the galaxy remembers
  what you did
- High reputation opens exclusive missions, ships, equipment (future work —
  the tracker and effects above are implemented); enemies close doors

MMO layer: reputation is personal, but system control is collective — see
faction-metagame.md. Faction definitions live in `content/factions/` and the
loader validates that every system's government references one.