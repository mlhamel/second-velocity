# ADR 0013 — Space stations and shared docking locations

- Status: accepted
- Date: 2026-10-23

## Context

Planets currently own all docking, markets, refueling and quest encounters.
Stations need those same services, but may orbit a planet or sit beyond any
system membership disc. Duplicating the planet service code would create two
gameplay paths and make saved quest references ambiguous.

## Decision

- Systems gain a serde-defaulted `stations` list. A station has identity,
  radius, optional sprite, commodity prices, optional master, and either
  `Fixed(position)` or `Orbit(planet, radius, period_secs, phase)` placement.
  Fixed positions are offsets from the owning system center; orbital parents
  are planets in that system. Orbits are scripted circles, not gravity.
- Planet and station IDs share a per-system namespace. A borrowed dockable
  content view supplies the same market and host interface for both.
- Replicate a `DockableTag` on both kinds and preserve existing `PlanetTag`.
  `DockedAt` names the owning system, location and kind. `RequestDock` accepts
  an explicit system/location; `RequestLand` remains a planet-only adapter.
- The server validates ownership, living state and actual spatial range;
  system membership is not a docking authorization boundary. Docking pins
  membership to the port, including deep-space stations.
- Server-owned station motion precedes docked-ship attachment updates. Docked
  ships follow a station-relative offset and remain immune to combat and
  environmental hazards. Undocking places the ship at the current port with
  safe separation. Clients render replicated positions only.
- Trading, refueling, host encounters and quest turn-in share one path.
  Markets and masters are optional. The existing encounter weighting and
  completion rules remain unchanged.
- Quest tuples `(system, location)` can reference either kind. The legacy
  serialized `giver_planet`/`destination_planet` fields retain their names
  and carry dockable IDs, preserving SQLite quest JSON without a DB migration.
  Existing planet definitions, quests and named station-like planets remain
  intact.
- Add station visuals/minimap markers, `L` land/dock, MCP `dock` and station
  observations, moving-target scenario navigation, and deterministic orbital
  docking/trading and station delivery benchmarks.

## Consequences

Offline, UDP and MCP simulations use identical authoritative station behavior.
Old packs load with zero stations. Protocol changes ship with client and
server together. There are no station interiors, shipyards, repair services,
destructible stations, or station NPC routing changes in this pass.
