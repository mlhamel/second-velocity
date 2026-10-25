---
name: add-system
description: Add a new star system with planets, commodities, and hyperlinks
---

# Add a star system

1. Create `content/systems/<id>.ron`:
   - Fields: id, name, government, description, position ((x, y) galaxy
     coordinates), links, planets
   - Each planet: id, name, description, commodities `[(id, price)]`
   - Descriptions follow docs/gdd/writing-style.md
2. Add `"systems/<id>.ron"` to `systems:` in `content/manifest.ron`
3. Update the links of every connected system to point back — links must be
   reciprocal or pack validation fails
4. Prices: design a profitable loop against existing systems
   (see docs/gdd/economy.md)
5. NPC presence is wired in `crates/server/src/gameplay.rs`, not in content
   (for now)
6. Validate: `cargo test -p sv-data`
   - Optional `stations: [...]` entries define ID/name/description/radius,
     optional sprite/master/commodities, and `placement` as either
     `Fixed(position: (x, y))` (system-relative offset) or
     `Orbit(planet: "<id>", radius: 240.0, period_secs: 180.0, phase: 0.0)`.
   - Planet/station IDs must be unique within the system. Orbital parents
     must exist, values must be finite, period positive, and orbital radius
     must clear the planet radius plus station radius and 20 units.
   - Fixed ports may lie outside membership discs: use the generic docking
     path, never membership-gated planet-only logic. Keep entire orbital
     sweeps clear of planets and stellar/black-hole danger zones.
7. Verify: `cargo test -p sv-server` (the embedded slice test exercises the
   whole pack end-to-end)