# PvP Policy (MMO)

[stable] — implemented (M2): fines + NPC patrol on player-vs-player hits in
policed space. M4: policing follows the controlling faction and happens in
the attacker's actual system (ADR-0017)

Government zones are the ruleset — and since M4 the zone rules move with the
war:

- **Policed space** (Confederation- or Rebellion-controlled systems):
  attacking players brings instant NPC response, a fine, and a faction
  reputation penalty. Possible, never profitable.
- **Contested / independent / pirate / lawless systems**: open PvP. No
  warning, no guards — until a war faction wins control of the system.
- **Hyperlanes**: TBD — origin-system rules or destination rules apply
- Hated pilots (standing ≤ −50 with the controller) are locked onto by
  patrols on sight in policed space

Design intent: EV's government/faction logic literally becomes the PvP
design. Player pirates get real space to be pirates; traders get real risk
arcs; and the front line between the two is visible on the galaxy map (M).