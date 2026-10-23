# Docking Encounters & Quests

[active] — implemented per ADR-0009

When a player docks at a planet with a chief or station with a master, an
encounter panel opens automatically alongside the docked HUD. Instead of
burying jobs in a separate spaceport bar screen, the port's host steps forward.

## Planet chiefs and station masters

Each planet in `content/systems/*.ron` may define an optional `chief` block.
Stations may instead define an optional `master` with the same fields and
portrait behavior. Ports without a host do not open an encounter.

- **Identity**: `name` and `portrait` (`textures/chiefs/<planet-id>.png`,
  128×128 RGBA with procedural fallback).
- **Voice**: `greeting`, `offer_text`, `tribute_text`, and `quest_text`, written
  in character per [writing-style.md](writing-style.md).

## Encounter moods and roll priority

On `RequestLand` or `RequestDock` success, the server rolls an encounter for the
docked port's chief or station master:

1. **Turn-in** (highest priority): if the player holds an active quest whose
   turn-in location is this port and whose objective is met (delivery cargo
   onboard or bounty target destroyed), the chief opens with a turn-in prompt
   and pays `reward_credits` on completion.
2. **Weighted roll** (when no quest is ready to turn in):
   - **Resource offer**: a discounted commodity parcel or free surplus cargo
     (checked against free hold space).
   - **Tribute demand**: a docking levy or shake-down in credits. Paying
     deducts credits cleanly; refusing logs a dry flavor notice (and a
     reputation hook once the faction reputation system lands).
   - **Quest offer**: offers an available quest originating at this port
     (skipping quests already active or completed by this player).

## Quests (`content/quests/*.ron`)

Quests are standalone RON resources referenced from `content/manifest.ron`:

- **Delivery**: `Delivery { commodity, qty, destination: (system, location), deadline_secs }`.
  Uses a courier model — accepting the quest loads the parcel directly into the
  player's cargo hold (requires free capacity). Turning in at the destination
  removes the parcel and awards `reward_credits`.
- **Bounty**: `Bounty { system, target, deadline_secs }`. Requires destroying a
  pirate ship in the target system (attributed via projectile ownership) and
  returning to the giver planet or station to collect `reward_credits`.
- **Timed constraint**: either quest kind may set `deadline_secs: Some(u64)`.
  The server ticks remaining time; expiry fails the quest and emits a notice.
- **Death penalty**: player destruction clears active quests along with lost
  cargo.
- Giver/destination IDs share a planet/station namespace per system. A station
  referenced by a quest needs a master for offering or turn-in. Saved JSON
  keeps its legacy planet-named fields for compatibility.

## Controls & UI coexistence

- The chief encounter opens as a side overlay panel when docking.
- While the panel is visible, number keys (`1`–`9`) choose the corresponding
  encounter option.
- `Esc` dismisses the panel so number keys revert to market trading (`1`–`5` buy,
  `Shift+1`–`5` sell); `T` reopens the chief panel while still docked.
