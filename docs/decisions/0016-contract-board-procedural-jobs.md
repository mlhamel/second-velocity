# ADR 0016 — Contract board: procedural repeatable jobs

- Status: accepted
- Date: 2026-10-24

## Decision

Chiefs and station masters offer procedurally generated contracts through the
existing encounter pipeline. Generated contracts are `QuestDef`-shaped at
runtime and ride the whole quest machinery unchanged: offer via
`roll_chief_encounter`, accept through `RespondToEncounter`, progress through
the delivery/bounty/deadline systems, turn-in at the giver or destination,
and persistence through the existing active-quests JSON.

The board is a server resource refreshed on a sim-time timer (5 simulated
minutes) plus once on boot. Every port with a host is guaranteed at least one
contract; the pool covers hosts first, extras second. Authored quests keep
absolute priority: a chief offers generated work only when no authored quest
is available at that port for that player, so curated arcs never compete with
procedural filler and existing deterministic flows are preserved.

Generated contracts are repeatable. A new `procedural: bool` (serde default
`false`) on the replicated `ActiveQuest` marks them; turn-in skips the
one-shot `completed` ledger for procedural entries, so the completed list
stays authored-only and bounded. Generation ids (`contract-N`) may repeat
across refreshes; offer filtering excludes only currently-active ids.

Kinds: Delivery (a commodity the giver port actually lists, a quantity that
fits light holds, delivered to another hosted port) and Bounty (the `"pirate"`
wildcard target in the pirate-inhabited system). Roughly half carry a
deadline scaled generously with distance. Rewards scale with the system
distance between giver and job and with destination hazards (black hole,
meteorite field, star); the curves are pure functions in `evr-core` with unit
tests. Contracts are never persisted as offers: accepted ones live in
active-quests JSON, unaccepted ones simply regenerate.

Generation draws from the seeded `EncounterRng`, so a fixed seed reproduces
a fixed board; the scenario step `AdvanceContractRefreshes` invokes the same
refresh the scheduled system runs, mirroring the economy-tick pattern.
Scenario `contract-refresh.ron` proves offer, accept, refresh survival, and
completion.

## Consequences

No client or protocol flow changes beyond the additive `ActiveQuest` field —
encounter UI, controls, and the turn-in experience are identical for authored
and procedural jobs. Bounty contracts exist where pirates actually spawn
today; widening pirate habitats later widens bounty generation for free.
This is a friends-scale mission board: no negotiation, no reputation
coupling (phase 4), no per-player boards, and no contract history.