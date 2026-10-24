# Economy

[active] — baseline trading live (M1)

## Baseline (live)

- Commodities per planet or station with fixed list prices; buy and sell at list
- The first profitable loop: buy food @ 8 on Earth, sell @ 14 at Newhaven;
  buy machinery @ 30 at Newhaven, sell @ 45 on Earth
- Landing refuels at 1 cr per unit of fuel
- Station docking follows the same refuel rules. Lin Relay sells food at 10 cr;
  Wayfarer Exchange buys it at 18 cr. Stations with empty markets offer no trade.
- Credits survive death; cargo does not

## Shared economy (MMO) — live

- Prices respond to actual player trading: selling floods, buying drains —
  through per-port supply counters that move on every trade but reprice only
  on the server's economy tick (see shared-economy.md and ADR 0015)
- NPC freighters deliver into the supply model, keeping spreads from being
  permanent
- Commodity definitions live in `content/commodities/`; per-port entries keep
  only id and list price