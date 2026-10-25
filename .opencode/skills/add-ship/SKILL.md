---
name: add-ship
description: Add a new ship definition to content/ and validate the pack
---

# Add a ship

1. Create `content/ships/<id>.ron`:
   - Fields: id, name, description, price, mass, turn_rate, acceleration,
     max_speed, max_shields, max_armor, fuel, cargo, weapon_slots
   - Optional: default_outfits (outfit ids; must exist in the pack)
   - Description follows docs/gdd/writing-style.md
2. Add `"ships/<id>.ron"` to `ships:` in `content/manifest.ron`
3. Server spawn wiring (if needed): constants in
   `crates/server/src/gameplay.rs` (PLAYER_SHIP, PIRATE_SHIP) or spawn logic
4. Validate: `cargo test -p sv-data` (loads the pack, checks
   cross-references and duplicate ids)
5. Verify: `cargo clippy --workspace -- -D warnings`