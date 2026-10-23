# Combat

[active] — first pass live (M1)

Real-time top-down 2D, in the spirit of EV.

- Shields absorb first, armor takes the spill. Armor is the hull — armor gone
  means destroyed (`evr-core` `ShipState`)
- Shields regenerate slowly in flight (4.0/s); armor does not repair
- Escape is a legitimate tactic; hyperjump fuel is a combat resource
- Death: lose cargo, keep credits. The yard builds you a new hull (3 s)
- PvP follows government zones (see pvp-policy.md)
- Server-authoritative: clients send intents, server resolves hits

Live weapon: Light Blaster (`content/outfits/light-blaster.ron`) — damage 6,
cooldown 0.25 s, projectile speed 420, ttl 1.2 s. Pirates fly Turncoats and
hunt players in their system; attacking a player in Confederation space earns
a fine and a Patrol Cutter on your tail (see pvp-policy.md).

## Environmental hazards

Stars are the first environmental hazard (ADR-0012). Featured systems (sol,
sirius, cygnus-x1, hawking) carry an actual sun placed by content far from
spawn points and planet lanes.

- Heat ramp: zero damage beyond 1.8x the star's radius, ~25 dps at the
  surface, lethal (500 dps) inside the core fraction (0.5x radius)
- Shields absorb heat first, like any damage; armor takes the spill
- Undocked ships only: docking at a planet ends the threat. NPCs burn too —
  the universe is consistent, so a fleeing pirate can die in a corona
- A cooldown-gated notice warns on heat-zone entry
- Black holes (gravity pull + seeded 50/50 destroy/teleport) are the next
  hazard type, on the galaxy-expansion roadmap (Phase 3)

Open questions:

- The EV energy juggle (one pool for weapons, shields, afterburner) — arrives
  with the outfit system
- More weapon classes: beams, turrets, missiles
- Ramming damage: yes/no?