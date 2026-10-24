# ADR 0014 — Shipyards, equipped loadouts, and owned-ship recovery

- Status: accepted
- Date: 2026-10-23

## Decision

Ports optionally list ships/outfits for sale in content. Server intents buy a
hull or install/sell one equipped outfit while docked. Prices and stock are
server-authoritative; hull trade-in and outfit resale are 50% of base price.
Any docked outfitter accepts resale of installed outfits, whether or not it
stocks them. Installed outfits transfer to replacement hulls; no duplicate
factory equipment is granted. Reject swaps that cannot carry current cargo or
equipment.

Weapon copies consume weapon slots. Shield generators, fuel tanks and armor
plates each allow one installed copy/category. Effective maxima are recomputed
from the base hull and equipped IDs. Capacity changes preserve current resource
amounts, clamping on removal; outfitting is not a repair/refuel service.
Purchased/recovered hulls are fully commissioned. Independent weapon banks
fire through the existing projectile pipeline. Energy is outside this phase.

Keep a replicated equipped-ID list, not a separate equipment inventory.
Persist hull identity and nullable equipped-ID JSON alongside account state.
NULL/missing equipment means a legacy save and uses that hull's defaults;
an explicit empty list means deliberately unarmed. Migrate old SQLite tables
with an additive nullable column. Corrupt or invalid saved loadouts reject
login with a notice rather than silently resetting progress.

Apply progression purchases only after the candidate save succeeds when
storage is enabled. Keep existing in-memory embedded/MCP modes. Restore
absolute position and saved resources on login; persist actual owned hulls.
Death keeps hull/equipment/credits/completed quests, loses cargo/active quests,
and commissions the same hull after the existing recovery delay. Recovery is
tracked per owner so simultaneous deaths cannot overwrite each other. Death
snapshots drop outfits that no longer resolve in content; if the owned hull
itself no longer resolves, recovery issues the starter hull with its factory
loadout rather than saving an unrecoverable pilot.

## Consequences

Shop controls, MCP and scenarios share the same intents. Current packs without
shops still load. Client/server protocol changes ship together. This is a
survivable first-playtest policy, not insurance, storage inventory, repairs,
energy management, or a new offline save mode.
