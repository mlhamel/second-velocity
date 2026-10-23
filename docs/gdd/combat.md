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

Black holes are live in Cygnus X-1, south of the mining belt (ADR-0012).

- Content defines the position offset, event horizon radius, influence radius,
  pull strength, and optional sprite. The full influence disc stays clear of
  spawn zones, planet approaches, meteorite fields, stellar heat, and travel
  corridors; pack tests enforce the placement.
- Gravity is spatial, not membership-gated: undocked players and NPCs
  accelerate toward the hole. Pull ramps linearly from zero at the influence
  boundary to the configured strength at the horizon; no inverse-square
  singularity.
- Reaching the event horizon resolves one seeded 50/50 draw: destruction via
  the normal damage/death/respawn pipeline, or teleportation to a seeded-random
  other system's center with velocity zeroed. The server checks the full
  flight segment, so warp-speed and grazing crossings cannot skip the horizon.
- Teleportation preserves fuel, hull, credits, cargo, and quests. The server
  updates system membership from the authoritative position and notifies the
  player of the destination.
- Docked ships are immune to both pull and consumption. Dead ships do not
  consume more random outcomes.
- The dark disc marks the event horizon; a violet accretion ring and minimap
  dot distinguish it from a star. Clients present replicated state only.
- `black-hole-teleport` and `black-hole-death` benchmark scenarios pin horizon
  entry with `FlyToBlackHole { system }` so navigation timing cannot change the
  seeded outcome. Gravity is covered separately by core and server tests.

Open questions:

- The EV energy juggle (one pool for weapons, shields, afterburner) — arrives
  with the outfit system
- More weapon classes: beams, turrets, missiles
- Ramming damage: yes/no?