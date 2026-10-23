# Space Stations

[active] — implemented per ADR-0013

Stations are docking locations with the same refueling, commodity trading and
quest encounters as planets. They are indestructible ports, not NPC ships.

## Placement

Define stations in a system's optional `stations: [...]` list:

- Identity: `id`, `name`, `description`, `radius`, optional `sprite`.
- `placement: Fixed(position: (x, y))` is an offset from the owning system
  center. A freeport may sit outside every system's membership disc.
- `placement: Orbit(planet: "earth", radius: 240.0, period_secs: 180.0,
  phase: 1.5707964)` circles a planet in the same system counterclockwise.
  `phase` is radians and defaults to zero; orbital radius is center-to-center.
- The server computes motion from simulation time and replicates positions.
  Clients do not simulate orbits. An orbit restarts with the shard.
- IDs must be unique across planets and stations within a system. Orbits
  require finite values, a positive period, and clearance beyond the parent
  planet's surface plus station radius and a 20-unit margin.

The base pack includes **Lin Relay**, orbiting Earth every 180 seconds, and
**Wayfarer Exchange**, fixed at Sol offset `(-1000, 900)`. Their entire orbital
sweep/footprint clears planet surfaces, spawns, stellar heat and black-hole
influence zones, enforced by pack tests. Nearby drifting meteorites remain
ordinary flight hazards.

## Docking and services

- `L` lands/docks at the nearest eligible port; `U` undocks. Range is the
  existing 220-unit allowance plus the target's radius.
- Range and ownership are checked on the server against actual positions,
  not against the ship's possibly stale system-membership tag.
- A docked ship follows its station-relative docking offset. Undocking
  releases it beside the station's current position with safe separation.
- Membership remains pinned to the port's owning system while docked.
  Docked ships retain combat, meteorite, stellar heat and black-hole immunity.
- Each port has its own commodity list/prices. An empty list means no market.
  Refueling costs 1 cr per missing fuel unit, as on planets.
- Optional `master` uses the same fields as a planet's `chief`. Without one,
  no encounter opens, but docking/refueling/trading still work.
- Master encounters use the existing weighted offers, tribute and quest
  rules. Deliveries can go between either kind of port; bounty rewards may
  be collected at a station. Completed contracts remain non-repeatable.
- Shipyards, repairs, interiors, equipment shops and station fleet routing
  are outside this feature.

## Agents and compatibility

MCP `observe()` includes all `stations` with owning system, ID, radius and
current position; `docked_location` identifies the full system/ID/kind.
`dock(system, location)` supports either kind. Existing `land(planet)`,
`lift_off`, planet observations and scenario steps remain supported.

Quest `(system, location)` tuples retain their existing syntax. Saved quest
JSON retains the legacy `giver_planet` and `destination_planet` field names,
which now carry either kind of port ID. Existing planet content and save
progress remain intact; docking state itself is not persisted, as before.

Scenarios use `FlyToStation(system, station, dock)` to track a moving target,
and `Dock(system, location)` when already nearby. The orbital-trade and
station-delivery scenarios exercise the same live authoritative code as play.
