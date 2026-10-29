---
status: accepted
---

# Making Combat & Flight Fun: Micro-Loop Roadmap

Captures the analysis and three-slice roadmap from 2026-10-28 for turning
Second Velocity's 30-second flight and combat loop from a flat stat-check
into a responsive, tactical *Escape Velocity*-style dogfight, alongside
galaxy-wide combat encounters.

## Locked scope answers for Option A (owner, 2026-10-28)

- **Afterburner (`Shift`)**: built-in on all ships (drains the energy capacitor
  while `Shift` is held to boost acceleration and top speed), with optional
  Reactor/Capacitor outfits to expand the energy pool and recharge rate.
- **Weapon slots & triggers**: unified `weapon_slots` — any weapon (`Blaster`,
  `Turret`, `Missile`) installs into `weapon_slots`; `Space` fires primary
  guns and auto-turrets, while `F` fires homing missiles.
- **Base Energy Capacitor stats**: default `max_energy` and `energy_regen`
  derived from `ShipSize` (with optional per-hull override in
  `content/ships/*.ron`), keeping existing ship RON files and scenarios clean.

## Diagnosis: why combat feels flat today

The macro-simulation is rich (15 systems, living OHLCV markets, procedural
contracts, faction control & reputation, stations, stars/black holes, ghost
pilots), but the moment-to-moment flight and combat loop has five bottlenecks:

1. **One combat verb ("Hold Spacebar & Turn")**: only two weapons exist
   (`light-blaster`, `pulse-blaster`), both fixed-forward dumbfire guns with
   infinite free shots, zero energy cost, and identical barrel spawn origins.
2. **Fighting blind**: no target lock, no enemy shield/armor readout, no
   off-screen target arrow, and no velocity lead pip for aiming fixed guns.
3. **Missing EV flight controls**: no retrograde brake (`S` / `ArrowDown` to
   auto-align opposite velocity) and no afterburner (`Shift`) to dodge or
   sprint out of danger.
4. **Only one pirate in the galaxy**: a single hardcoded `turncoat` in
   `alpha-centauri` (`PIRATE_SYSTEM`), with all procedural bounty contracts
   pointing to that lone ship while 7 heavy `Defender` patrols have nothing to
   fight in their systems.
5. **Minimal sensory feedback ("juice")**: silent vacuum (no synth/SFX), no
   engine plumes, no distinction between shield hits and armor sparks, and a
   rigidly locked camera.

---

## Option A — Tactical Combat Core (First Focus)

Brings the classic *Escape Velocity* energy juggle, flight maneuverability,
target lock, and weapon archetypes into the simulation (`sv-core`, `sv-data`,
`sv-protocol`, `sv-server`, `sv-client`, `sv-mcp`).

- [ ] **Energy Capacitor Pool**:
      - `energy` / `max_energy` and `energy_regen` on `ShipState` (replicated
        to clients and displayed as a fourth HUD bar below Fuel)
      - Weapons consume `energy_cost` per shot; firing pauses when capacitor
        is dry
      - Reactor / Battery outfit kind (`OutfitKind::Reactor` or capacitor
        upgrade) to expand capacity and regen rate
- [ ] **Retrograde Brake (`S` / `ArrowDown`)**:
      - Holding brake auto-steers the ship toward `-velocity` (retrograde)
        and zeroes turn once aligned, making flip-and-burn braking and combat
        pivots crisp
- [ ] **Afterburner (`Shift` / `OutfitKind::Afterburner`)**:
      - Drains capacitor energy (or fuel) while held to boost acceleration
        and `max_speed` by a configurable multiplier
- [x] **Target Lock (`T` cycle, `R` nearest hostile)**:
      - Client/server target selection (`CycleTarget` intent and replicated
        `TargetLock`)
      - When locked onto a target ship, weapon fire aims at the locked target
        with velocity lead calculation (`lead_target_position`)
      - Target bracket & velocity lead indicator pip in `visuals.rs`
      - Target telemetry on `hud.rs` (name, role/faction, live shield & armor
        percentages)
- [ ] **Distinct Weapon Archetypes (`content/outfits/`)**:
      - **Wing-offset / Twin mounts**: multiple weapon slots fire with lateral
        barrel offsets rather than stacking on one pixel
      - **Auto-Turrets**: aim toward the locked target (or nearest hostile in
        range) within an arc so heavy haulers and capital ships can fight
        agile interceptors
      - **Homing Missiles / Secondary Weapons**: track locked target with
        bounded turn rate (`turn_rate`), high hull damage, longer cooldown
- [ ] **Scenario & Validation**:
      - Benchmark & integration scenario in `content/scenarios/` exercising
        energy drain, afterburner, turrets/missiles, and combat resolution

---

## Option B — Galaxy-Wide Combat Opportunities, Smarter AI & Loot

Expands *who* you fight, *where* you fight, and *why* you fight across all 15
systems.

- [ ] **Multi-System Pirate Roamers & Tiered Hulls**:
      - Replace the singleton `PIRATE_SYSTEM` / `PirateRespawn` with per-system
        pirate rosters and threat tiers (e.g. `turncoat` scouts in frontier
        systems, `auroran-corsair` / `korath-cutter` raiders in mining belts,
        heavy `korath-reaver` packs in `ross-154`)
      - Pirates clash with the 7 `NpcBrain::Defender` warships in shared systems
- [ ] **Multi-System & Tiered Bounty Contracts**:
      - `ContractBoard` generates bounties across pirate-active systems or
        spawns named bounty targets matching contract difficulty (Light,
        Medium, Heavy boss)
- [ ] **Confederation vs. Rebellion Border Skirmishes**:
      - Contested systems spawn opposing faction strike craft that trade fire
        with local patrols; players can intervene for reputation and system
        control shifts
- [ ] **Smarter NPC Dogfighting**:
      - Attack runs & peel-off maneuvers at close range (`<120u`) for light
        fighters instead of stationary face-tanking
      - Velocity lead aiming for veteran NPCs
- [ ] **Loot Canisters, Merchant Plunder & Disabling/Boarding**:
      - Destroyed pirates, ambushed merchants, and cracked meteorites drop
        scoopable cargo canisters (`isotopes`, `ore`, etc.)
      - Ships below ~15% armor become **Disabled** (drifting inertly) so
        players can board (`B`) for credits/salvage or finish them off

---

## Option C — Visceral Combat Feel ("Juice", VFX & Synth Audio)

Pure presentation and client-feel upgrades in `sv-client`.

- [ ] **Shield vs. Armor Hit VFX**:
      - Cyan ripple/ring on shield absorption; orange-red flash + hot debris
        sparks when armor takes spill damage
- [ ] **Thruster Plumes & Afterburner Flare**:
      - Emissive exhaust sprites behind ships scaled by `throttle` (and
        flaring bright under Afterburner), leveraging the HDR Bloom pipeline
- [ ] **Camera Dynamics**:
      - Subtle velocity look-ahead on `camera_follow`, screen shake on armor
        hits / nearby explosions, and warp-streak effect on hyperjump (`J`)
- [ ] **Procedural Retro Synth SFX**:
      - Zero-asset-dependency procedural audio buffers generated at startup
        for blaster bolts, shield impacts, hull alarms, engine hum, docking,
        and hyperjump charge
