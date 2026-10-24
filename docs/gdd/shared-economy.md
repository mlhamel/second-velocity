# Shared Economy (MMO)

[active] — supply model live (M4)

All players trade in the same persistent market.

- Every port keeps a signed supply counter per commodity; player buys drain
  it, player sales and NPC merchant deliveries flood it
- Server runs an economy tick every 30 simulated seconds — prices recompute
  only on the tick, never per trade, and decay ~10% toward zero (arbitrage
  damping); clamped to [0.5x, 2.0x] of the content list price
- NPC freighters feed the model at their arrival port, keeping routes from
  being permanently broken
- A trader who watches the galaxy's news — broadcasts on ≥15% tick moves —
  has an edge over one who memorized static routes
- History is daily OHLCV candles per port-commodity (open/high/low/close and
  volume), persisted in SQLite; nothing is ever deleted and nothing finer
  is stored (see ADR 0015)
- Supply and the market clock restore on boot; live prices replicate to
  clients through a per-port market component

Anti-abuse notes:

- Arbitrage damping: the per-tick decay above; tuned for friends scale
- Alt-account muling: accept it; designing around it costs more than it saves