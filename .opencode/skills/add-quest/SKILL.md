---
name: add-quest
description: Add a new quest definition, planet chief, or station master to content/ and validate the pack
---

# Add a quest

1. Create `content/quests/<id>.ron`:
   - Fields: `id`, `name`, `giver: ("<system-id>", "<location-id>")`, `kind`,
     `reward_credits`, `description`
   - `kind` is one of:
     - `Delivery(commodity: "<id>", qty: <u32>, destination: ("<system-id>", "<location-id>"), deadline_secs: Option<u64>)`
     - `Bounty(system: "<system-id>", target: "<ship-id>", deadline_secs: Option<u64>)`
   - `deadline_secs` is optional (`#[serde(default)]`); omit it for untimed
     contracts or set `deadline_secs: Some(180)` for timed runs
   - `giver` can be a planet with `chief: Some((...))` or station with
     `master: Some((...))`. Station delivery destinations also need a master
     for turn-in. Both share a per-system location-ID namespace.
   - Keep legacy saved quest JSON field names (`giver_planet`,
     `destination_planet`); their values now accept station IDs as well.
   - `description` follows `docs/gdd/writing-style.md` (character-voiced, dry
     wit, ≤120 words)
2. Add `"quests/<id>.ron"` to `quests:` in `content/manifest.ron`
3. If adding a chief or station master portrait, place a 128×128 RGBA PNG at
   `assets/textures/chiefs/<location-id>.png` (or add a row in
   `ops/generate_placeholder_art.py` and run it)
4. Validate: `cargo test -p sv-data` (checks manifest paths, unique quest ids,
   giver/destination docking-location references, target system references, and host
   portrait assets)
5. Verify: `cargo clippy --workspace -- -D warnings`
