# ADR 0011 — Living galaxy NPC fleets, multi-race ship classes, inter-system AI, and ship hailing

- Status: accepted
- Date: 2026-10-17

## Context

Currently, the only non-player ships in space are a lone static pirate in
Alpha Centauri and Confederation police cutters that spawn only after a PvP
offense. The galaxy feels empty between planets, and every ship hull is human
and identical in scale.

To make space feel like a living Escape Velocity galaxy, we want:
1. **14 new ship classes across 5 distinct races/cultures and 4 hull sizes**
   (`Light`, `Medium`, `Heavy`, `Capital`).
2. **Autonomous NPC traffic** flying through and between star systems:
   - **Merchants** buying and carrying commodity stock from one system/planet
     to another.
   - **Defenders / Bounty Hunters** actively hunting and fighting pirates.
   - **Explorers** traveling across jump links to chart systems and outer belts.
3. **Ship-to-ship hailing / visiting**: players can fly alongside a non-hostile
   NPC ship and talk to its captain (trade tips, mid-space barter, system lore,
   or emergency fuel).
4. **Universal retaliation**: if a player fires on any NPC ship (even a
   peaceful merchant or explorer), that ship turns hostile (`Aggro`) and fights
   back.

## Decision

1. **Extended `ShipDef` metadata (`crates/data`)**:
   `ShipDef` gains optional content fields with sensible defaults so existing
   ship RON files remain valid:
   - `#[serde(default = "default_race")] pub race: String` (`"terran"`,
     `"vellari"`, `"korath"`, `"sylph"`, `"auroran"`)
   - `#[serde(default)] pub hull_size: ShipSize` (`Light`, `Medium`, `Heavy`,
     `Capital` — controls world render size `34..96 px` and hit radius)
   - `#[serde(default)] pub role: Option<String>` (`"merchant"`, `"defender"`,
     `"explorer"`, `"pirate"`, `"police"`)

2. **14 new multi-race ships (`content/ships/*.ron`)**:
   We add 14 new ship definitions across five races (`Terran`, `Vellari`,
   `Korath`, `Sylph`, `Auroran`) and four hull sizes (`Light`, `Medium`,
   `Heavy`, `Capital`), each with its own top-down sprite in
   `assets/textures/ships/<id>.png`.

3. **Extended `ShipKind` and `ShipInfo` (`crates/protocol`)**:
   `ShipKind` gains `Merchant`, `Defender`, and `Explorer` alongside `Player`,
   `Pirate`, and `Police`. `ShipInfo` carries `race: String` and
   `size: ShipSize` (or client looks up `content.ship(&info.def_id)`) plus
   replicated cargo/route summary so clients and MCP agents can inspect NPC
   ships in flight.

4. **Inter-system NPC AI state machine (`crates/server`)**:
   NPC ships run a unified `npc_ai` system:
   - **Retaliation override (`Aggro(owner)`)**: Any NPC ship hit by a
     projectile from `owner` immediately gains `Aggro(owner)`. While `Aggro` is
     present, the NPC abandons its peaceful route, turns toward the attacker,
     and fires its weapons.
   - **Defender (`ShipKind::Defender`)**: Scans its current system for
     `ShipKind::Pirate` (or `Aggro` targets); engages pirates with weapons, or
     patrols between planets and neighboring systems when clear.
   - **Merchant (`ShipKind::Merchant`)**: Holds an `NpcRoute` with a source
     planet, commodity cargo stock, destination `(system, planet)`, and docked
     pause timer. Flies to the destination system, lands briefly to exchange
     stock, and picks a new trade leg along `system.links`.
   - **Explorer (`ShipKind::Explorer`)**: Picks a sequence of survey waypoints
     (planets, asteroid belts, and linked star systems), flying from system to
     system across the galaxy.

5. **Visiting & Hailing NPC Ships (`HailShip` client intent)**:
   When an undocked player is within `HAIL_RANGE` (`200.0` units) of a
   non-hostile NPC ship and triggers `HailShip` (`H` key in `evr-client` or
   `hail` tool in `evr-mcp`), the server opens a `ChiefEncounter` dialog on the
   player entity voiced by that NPC's race and role (offering mid-space stock
   barter from merchants, survey notes/emergency fuel from explorers, or combat
   intel from defenders).

## Consequences

Star systems become busy, self-sustaining ecosystems where merchants haul real
cargo between worlds, defenders clash with pirates, explorers cross jump lanes,
and every ship on screen has a distinct silhouette, scale, race, and voice when
hailed — or teeth when provoked.
