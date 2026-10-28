# Faction Meta-game (MMO)

[active] — implemented (M4): per-system control meters, controller flips,
weekly dispatch, galaxy map scoreboard (ADR-0017)

The war is bigger than any one pilot.

- Every system has a control meter −100…+100 (Rebellion…Confederation),
  initialized by its owner's war side (+60 / −60 / 0)
- Player action shifts meters: quest/contract turn-ins push the giver's
  system toward the giver's war side (+10), pirate kills strengthen the
  local controller (+2), patrol kills weaken it (−5)
- At ±50 the controlling war faction flips and the news broadcasts it;
  below the threshold control reverts to the content government
- Control changes who polices a system (pvp-policy.md): patrols respond in
  the attacker's actual system and answer to the controller
- The galaxy map is the scoreboard: press M — every system colored by its
  controller, meters on the label, your own standings at the bottom
- The weekly dispatch (every 7 sim-days) broadcasts standings and watchpoints
  as a news post; control persists in SQLite across restarts

Open question (deferred): a weekly faction dispatch newsletter beyond the
in-game notice — e.g. posting to Discord in phase 5.