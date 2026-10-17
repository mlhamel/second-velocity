---
name: living-galaxy-npcs
description: Build playbook for the living galaxy NPC feature - 14 new multi-race ships across 4 hull sizes (Light, Medium, Heavy, Capital), autonomous Merchant/Defender/Explorer AI traveling between systems, ship-to-ship hailing/visiting, and universal retaliation when fired upon. Use when implementing, reviewing, or extending NPC fleets and ship races.
---

# Living Galaxy NPCs — Multi-Race Fleets, Inter-System AI & Ship Hailing

Feature: populate space with autonomous NPC ships of diverse sizes and races.
Some are **Merchants** buying and hauling commodity stock from one system to
another; some are **Defenders / Bounty Hunters** hunting and fighting pirates;
some are **Explorers** discovering and surveying star systems. Players can fly
up to any non-hostile NPC ship to visit/talk (`HailShip`), and if the player
ever fires on an NPC ship, it immediately retaliates.

All design decisions align with
[ADR-0011](../../../docs/decisions/0011-living-galaxy-npc-fleets-races-and-hailing.md).

## Locked decisions

- **5 Races / Cultures**:
  1. `terran` — Human Confederation & frontier independents: utilitarian steel,
     titanium-white, and industrial amber.
  2. `vellari` — Vellari Trade Guild: crystalline/avian nomads with sleek
     iridescent gold-and-opal solar sails.
  3. `korath` — Korath Foundry: heavy lithoid engineers building brutalist,
     slab-armored gunmetal-and-magma vessels.
  4. `sylph` — Sylph Resonant Collective: deep-space mystics in organic,
     bioluminescent violet-and-cyan crescent hulls.
  5. `auroran` — Auroran Free Clans: honor-bound warrior houses flying
     forward-swept crimson-and-brass dueling hulls.
- **4 Hull Size Classes (`ShipSize`)**:
  - `Light` — world size `36×36 px`, hit radius `14.0`
  - `Medium` — world size `48×48 px`, hit radius `18.0`
  - `Heavy` — world size `68×68 px`, hit radius `24.0`
  - `Capital` — world size `92×92 px`, hit radius `32.0`
- **14 New Ship Hulls (`content/ships/*.ron`, 17 total with existing 3)**:
  1. `terran-hauler` (**Mule Hauler**, `terran`, `Medium`, `Merchant`)
  2. `terran-bulk-freighter` (**Atlas Starlifter**, `terran`, `Capital`, `Merchant`)
  3. `terran-gunboat` (**Vanguard Corvette**, `terran`, `Heavy`, `Defender`)
  4. `terran-surveyor` (**Pathfinder Scout**, `terran`, `Light`, `Explorer`)
  5. `vellari-clipper` (**Sun-Sail Clipper**, `vellari`, `Medium`, `Merchant`)
  6. `vellari-ark` (**Guild Caravel**, `vellari`, `Heavy`, `Merchant`)
  7. `vellari-seeker` (**Lumen Skiff**, `vellari`, `Light`, `Explorer`)
  8. `korath-ironclad` (**Korath Ironclad**, `korath`, `Heavy`, `Defender`)
  9. `korath-dreadnought` (**Basalt Bastion**, `korath`, `Capital`, `Defender`)
  10. `korath-barge` (**Slag Tender**, `korath`, `Medium`, `Merchant`)
  11. `sylph-wanderer` (**Aether Drift**, `sylph`, `Light`, `Explorer`)
  12. `sylph-cantor` (**Chorus Spire**, `sylph`, `Heavy`, `Explorer`)
  13. `auroran-corsair` (**Ash-Vane Lancer**, `auroran`, `Medium`, `Defender`)
  14. `auroran-galleon` (**Solaris Sovereign**, `auroran`, `Capital`, `Merchant`)
- **3 Autonomous NPC Behaviors**:
  - **Merchants (`ShipKind::Merchant`)**: Carry real commodity stock (`Wallet`)
    between linked star systems. They undock from a source planet, fly across
    the system link to a neighboring system's planet, dock briefly to sell their
    load and buy local stock, and depart on their next trade leg.
  - **Defenders (`ShipKind::Defender`)**: Patrol star systems and actively hunt
    `ShipKind::Pirate` ships (as well as any ship they hold `Aggro` against),
    engaging with blasters.
  - **Explorers (`ShipKind::Explorer`)**: Travel from system to system along
    `system.links`, visiting planets and outer system coordinates before
    charting a course to the next star system.
- **Visiting & Talking (`HailShip`)**:
  - Pressing `H` in the client (or calling `hail` in `evr-mcp`) within
    `HAIL_RANGE` (`200.0` units) of a non-hostile NPC ship opens the encounter
    communication panel (`ChiefEncounter`) with captain dialog tailored to the
    ship's race and role:
    - **Merchant**: offers to sell a small batch of the cargo stock they are
      currently hauling at a fair mid-space price (`accept_offer` / `decline`).
    - **Explorer**: shares navigation intel on their destination system and
      offers a small free fuel transfer (`accept_offer` / `decline`).
    - **Defender / Police**: shares tactical patrol status (`decline` / `"Clear skies"`).
- **Universal Retaliation (`Aggro`)**:
  - Every NPC ship has a `Blaster` weapon component.
  - If any player or ship fires a projectile that hits ANY NPC ship
    (`Merchant`, `Defender`, `Explorer`, `Pirate`, `Police`), that NPC ship
    immediately receives `Aggro(attacker_owner)`, drops any peaceful trade
    pause, and fights back against the attacker.

## Architecture

### 1. Content Schema & 14 Ship Definitions (`crates/data` & `content/ships/`)

- In `crates/data/src/ship.rs`, add:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
  pub enum ShipSize {
      Light,
      #[default]
      Medium,
      Heavy,
      Capital,
  }
  ```
  and extend `ShipDef` with:
  - `#[serde(default = "default_race")] pub race: String` (default `"terran"`)
  - `#[serde(default)] pub size: ShipSize`
- Create the 14 new `content/ships/<id>.ron` files, register them in
  `content/manifest.ron`, and generate distinct top-down sprites in
  `assets/textures/ships/<id>.png` (plus update `ops/generate_placeholder_art.py`).

### 2. Protocol Layer (`crates/protocol/src/lib.rs`)

- Extend `ShipKind`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  pub enum ShipKind {
      Player,
      Pirate,
      Police,
      Merchant,
      Defender,
      Explorer,
  }
  ```
- Add client intent `HailShip` (registered in `protocol::register(app)`):
  ```rust
  #[derive(Event, Debug, Clone, PartialEq, Serialize, Deserialize)]
  pub struct HailShip;
  ```

### 3. Server Simulation (`crates/server/src/gameplay.rs`)

- **NPC Route & Role State (`NpcBrain`)**:
  ```rust
  #[derive(Component, Debug, Clone)]
  pub enum NpcBrain {
      Merchant {
          commodity: String,
          qty: u32,
          target_system: String,
          target_planet: String,
          pause_timer: f32,
      },
      Defender {
          patrol_target: Vec2,
      },
      Explorer {
          target_system: String,
          waypoint: Vec2,
          pause_timer: f32,
      },
  }
  ```
- **Spawning (`spawn_world`)**:
  - Spawn a diverse mix of `Merchant`, `Defender`, and `Explorer` NPC ships
    across the galaxy's systems using the 14 new multi-race hulls.
  - Ensure every NPC ship has a `Blaster` (defaulting to `light-blaster` if
    `default_outfits` is empty) so every NPC can defend itself when attacked.
- **Unified AI (`npc_ai`)**:
  1. **Combat priority**:
     - If `Aggro(wanted_owner)` is present on ANY NPC ship, pursue and fire at
       `wanted_owner` (if undocked and in sight range).
     - If `ShipKind::Pirate`, pursue nearest undocked player or `Merchant` /
       `Defender` within sight range.
     - If `ShipKind::Defender`, scan for the nearest undocked `ShipKind::Pirate`
       in the same system; if found within `PIRATE_SIGHT_RANGE`, pursue and fire!
  2. **Peaceful navigation (when not in combat)**:
     - `Merchant`: fly toward `target_planet` in `target_system`; on arrival
       (`distance <= 140.0`), pause briefly (`pause_timer`), swap cargo stock to
       a commodity from that planet, and pick a new destination planet in a
       linked system (`system.links`).
     - `Explorer`: fly toward `waypoint` in `target_system`; on arrival, pause
       briefly and pick the next linked star system from `system.links` and a
       new survey waypoint.
     - `Defender` (when no pirates nearby): cruise between patrol points around
       the system's planets and jump lanes.
- **Retaliation in `projectile_hits`**:
  - When a projectile from `attacker_owner` hits an NPC ship (`With<Npc>`),
    insert `Aggro(attacker_owner)` on `victim_entity` so the NPC immediately
    fights back!
  - Also allow NPC projectiles (`owner == 0` or NPC entity ID) to hit opposing
    NPCs (e.g., `Defender` projectiles hit `Pirate` ships and vice versa) by
    storing the shooter's entity bits or faction owner on `Projectile`.
- **Visiting / Hailing (`on_hail_ship` observer)**:
  - Find the nearest non-hostile NPC ship within `HAIL_RANGE` (`220.0`) of the
    player ship.
  - Construct and insert a `ChiefEncounter` (and `PendingEncounter`) on the
    player ship voiced by the hailed NPC captain (`"[Vellari Merchant] Guild
    Caravel"`, etc.) with interactive options (`accept_offer` / `decline`).

### 4. Client Presentation (`crates/client`)

- **Size-scaled rendering (`crates/client/src/visuals.rs`)**:
  - Scale each ship's sprite `custom_size` by its `ShipDef.size` (`Light => 36`,
    `Medium => 48`, `Heavy => 68`, `Capital => 92`) so capital freighters and
    dreadnoughts tower over light scouts and shuttles!
  - Color untextured fallback ships distinctly for `Merchant` (gold),
    `Defender` (teal/emerald), and `Explorer` (violet).
- **Hailing key (`crates/client/src/input.rs` & `hud.rs`)**:
  - Bind `KeyCode::KeyH` to send `HailShip` when undocked.
  - Show a HUD prompt when within `HAIL_RANGE` of a non-hostile NPC ship
    (`"[H] Hail <Ship Name>"`).

### 5. MCP & Benchmark Scenario (`crates/mcp` & `content/scenarios/`)

- Add `hail` tool to `evr-mcp` (`Command::Hail`) and include `race`, `size`,
  and `hostile` in `observe()["ships"]`.
- Add scenario steps (`HailNearestShip`, `AttackNearestNpc`, `AssertShipHostile`)
  and `content/scenarios/living-galaxy-npcs.ron` verifying:
  1. Multi-race NPC ships (`Merchant`, `Defender`, `Explorer`) are active in
     space.
  2. Hailing a friendly NPC ship opens a communication encounter and completes
     an in-space interaction.
  3. Firing on an NPC ship causes it to retaliate and damage the attacker.

## Phases (verify after each; keep `cargo test` green)

1. **Docs & Skill**: `docs/decisions/0011-living-galaxy-npc-fleets-races-and-hailing.md`
   and `.opencode/skills/living-galaxy-npcs/SKILL.md`.
2. **Content & Art (14 new ships + 5 races + 4 sizes)**: `ShipSize` and `race`
   in `crates/data/src/ship.rs`, 14 `content/ships/*.ron` files, sprites in
   `assets/textures/ships/`, `manifest.ron`. Verify: `cargo test -p evr-data`.
3. **Protocol & Server AI / Hailing / Retaliation**: `ShipKind` variants +
   `HailShip` in `evr-protocol`; `NpcBrain`, `npc_ai`, defender-vs-pirate
   combat, universal retaliation on hit, and `on_hail_ship` in `evr-server`.
   Verify: `cargo test -p evr-server`.
4. **Client Visuals & Input**: size-based sprite scaling (`36..92 px`), `H` key
   to hail nearby NPCs, HUD hail indicator. Verify: `cargo clippy --workspace -- -D warnings`.
5. **MCP & Benchmark Scenario**: `hail` MCP tool, `content/scenarios/living-galaxy-npcs.ron`.
   Verify: `make bench` and `make check`.

## Status

- 2026-10-17: ADR-0011 and `living-galaxy-npcs` skill created. Ready for implementation.
