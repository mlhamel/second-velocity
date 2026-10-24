# ADR 0015 — Living markets: supply, drift, and daily candles

- Status: accepted
- Date: 2026-10-24

## Decision

Every port keeps a signed supply counter per commodity (0 = list price).
Player buys drain supply at the port; player sales and NPC merchant
deliveries flood it. Counters move immediately per trade; prices do not.
Prices recompute only on a server-side economy tick (30 simulated seconds,
accumulated from update deltas), from the content list price through a
damped function of supply, clamped to [0.5x, 2.0x] list. Ticks also decay
each counter ~10% toward zero (mean reversion), which is the arbitrage
damping: stale spreads heal within minutes, and watching the news beats
memorizing routes. Entries whose system/port/commodity no longer resolve
in content are dropped at the tick rather than panicking.

News broadcasts (the existing Notice event, sent to all connected clients)
fire on the tick when a price moved at least 15% since the previous tick.
News is ephemeral: it is a broadcast, not a stored row.

Persistence keeps two things and deletes nothing. `market_state` is a
single row holding the accumulated market clock and the supply map;
`market_candles` is one OHLCV candle per (sim-day, system, port,
commodity) — open/high/low/close and traded volume — upserted for dirty
entries on each tick. Daily candles are the whole history: no per-tick or
per-event log exists, so the table grows by roughly the number of active
port-commodity pairs per day and never needs retention. Intraday spikes
survive as candle highs; trends for a future galaxy scoreboard (Phase 4)
are exactly this shape. Sim-days derive from the persisted accumulated
clock (elapsed / 86400), so restarts and offline play keep day numbering
stable. Trades and NPC deliveries land in the running candle's volume and
high/low immediately; closes track tick prices.

The live price surface reaches clients as a replicated `PortMarket`
component on each port entity, refreshed at boot from persisted supply
and on every economy tick. Trades are charged at the current frozen
price between ticks, so a single mega-trade cannot move its own price.
Scenario runs invoke the same tick function directly
(`AdvanceEconomyTicks`) instead of waiting on wall time; runtime pacing
and scenario pacing share one code path. Commodity definitions move to
`content/commodities/` (id, name, description, icon); per-port entries
keep only id and list price, and the loader validates port entries
against the commodity pack.

## Consequences

Buy and sell both execute at the frozen price, so no spread exists yet.
NPC deliveries currently flood only the arrival port; loading at origin
would double-count routes and stays out. The economy tick, decay, and
candle math live in `evr-core` as pure functions for unit testing.
Existing trade scenarios keep exact assertions only where no economy
tick intervenes; range tolerances cover the rest. This is friends-scale
market making, not a futures exchange: no player limits, no buy/sell
spread, no per-trade price impact, and no pruning of candle history.