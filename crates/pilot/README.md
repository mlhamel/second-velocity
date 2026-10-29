# sv-pilot (`second-velocity-pilot` / `evr-pilot`)

Headless sidecar daemon that runs LLM-driven "ghost pilots" as ordinary UDP
clients against a persistent `sv-server` shard (see
[ADR-0018](../../docs/decisions/0018-ghost-pilots-sidecar-daemon.md)).

## Architecture

Each pilot process is two-tier:

1. **Reflex layer (60 Hz, deterministic Rust)**: Executes standing orders
   (`FlyTo`, `Flee`, `Idle`), deceleration-aware docking, tangent routing around
   stellar corona heat zones (`1.8r`) and black-hole influence discs, emergency
   flee when pirates close within `flee_range`, and automatic refueling on
   landing.
2. **Decision layer (event-driven background thread)**: On discrete decision
   points (`docked`, `encounter_offered`, `hostile_close`, `quest_complete`,
   `low_fuel`, `idle_heartbeat`, `respawned_after_death`), renders a hard-capped
   `<= 400` token state digest and requests one `PilotAction` JSON object from
   Ollama (`format: "json"`, 20s timeout, one retry with error feedback, then
   deterministic fallback) or from the built-in deterministic `MockBrain`
   (`--mock`).

## Quickstart (same box: shard + Ollama + pilots)

1. Start a persistent shard (ghost pilots require `--storage` so `LoginRequest`
   auto-registers and persists each persona's `PlayerState`):

   ```sh
   cargo run -p sv-server -- 3456 --storage shard.db
   ```

2. Pull a small local model in Ollama (3B–9B):

   ```sh
   ollama pull qwen2.5:3b
   ```

3. Run a ghost pilot persona (or pass `--mock` to run the deterministic brain
   without Ollama):

   ```sh
   cargo run -p sv-pilot --bin second-velocity-pilot -- \
     --connect 127.0.0.1:3456 \
     --persona content/pilots/cautious-trader.ron \
     --model qwen2.5:3b \
     --verbose
   ```

Starter personas in `content/pilots/`:
- `content/pilots/cautious-trader.ron` — Maren Vos (`sol/earth`)
- `content/pilots/courier.ron` — Juno Kade (`sol/lin-relay`)
- `content/pilots/bounty-hunter.ron` — Sable Venn (`vega/vega-station`)

## Systemd template (`second-velocity-pilot@.service`)

```ini
[Unit]
Description=Second Velocity Ghost Pilot (%i)
After=network.target ollama.service second-velocity-server.service
Requires=second-velocity-server.service

[Service]
Type=simple
WorkingDirectory=/opt/second-velocity
ExecStart=/opt/second-velocity/second-velocity-pilot \
  --connect 127.0.0.1:3456 \
  --persona content/pilots/%i.ron \
  --ollama-url http://127.0.0.1:11434 \
  --model qwen2.5:3b
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Enable all three starter personas:

```sh
sudo systemctl enable --now \
  second-velocity-pilot@cautious-trader \
  second-velocity-pilot@courier \
  second-velocity-pilot@bounty-hunter
```
