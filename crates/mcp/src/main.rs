use sv_mcp::harness::Harness;

use std::path::PathBuf;

use clap::Parser;
use rmcp::ServiceExt;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::JsonSchema;
use rmcp::tool;
use rmcp::tool_router;
use rmcp::transport::stdio;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

const SETTLE_TICKS: u32 = 10;
const MAX_TICKS: u32 = 600;

#[derive(Debug, Parser)]
#[command(
    name = "second-velocity-mcp",
    version,
    about = "Second Velocity as an MCP server over stdio; --bench runs scenario benchmarks instead"
)]
struct McpCli {
    /// Run benchmark scenarios (optionally one scenario id) instead of serving MCP
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    bench: Option<Option<String>>,
    /// Print scenario results as JSON instead of a markdown table
    #[arg(long)]
    json: bool,
    /// Baseline JSON report to annotate deltas against
    #[arg(long)]
    compare: Option<PathBuf>,
    /// Save the JSON report to a file
    #[arg(long)]
    save_json: Option<PathBuf>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct AdvanceArgs {
    /// Number of simulation ticks to run. Each tick is ~15ms of game time. Max 600.
    ticks: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SetFlightArgs {
    /// Forward thrust, 0.0 to 1.0
    throttle: f32,
    /// Turn input, -1.0 (left) to 1.0 (right)
    turn: f32,
    /// Hold weapons firing
    firing: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct LandArgs {
    /// Planet id to land at (see observe.planets); must be within landing range
    planet: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DockArgs {
    /// Owning system id from observe.stations or observe.planets.
    system: String,
    /// Station or planet id; the server checks actual spatial docking range.
    location: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TradeArgs {
    /// Commodity id (see observe.market while docked)
    commodity: String,
    /// Quantity to trade
    qty: u32,
    /// "buy" or "sell"
    direction: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WarpArgs {
    /// Enable warp drive
    on: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RespondArgs {
    /// Encounter option id (e.g. "accept_quest", "turn_in", "accept_offer", "pay_tribute", "decline") or 1-based option number ("1", "2")
    choice: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RunScenarioArgs {
    /// Optional scenario id from content/scenarios/ (omit to run all scenarios)
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BuyShipArgs {
    ship: String,
}
#[derive(Debug, Deserialize, JsonSchema)]
struct OutfitArgs {
    outfit: String,
}

enum Command {
    BuyShip(String),
    BuyOutfit(String),
    SellOutfit(String),
    Advance(u32),
    Observe,
    SetFlight {
        throttle: f32,
        turn: f32,
        firing: bool,
    },
    Land(String),
    Dock {
        system: String,
        location: String,
    },
    LiftOff,
    Trade {
        commodity: String,
        qty: u32,
        buy: bool,
    },
    SetWarp {
        on: bool,
    },
    Hail,
    Respond(String),
    RunScenario(Option<String>),
}

#[derive(Clone)]
struct GameMcp {
    tx: mpsc::Sender<(Command, oneshot::Sender<String>)>,
}

impl GameMcp {
    fn new(content_root: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel(32);
        std::thread::spawn(move || game_thread(content_root, rx));
        Self { tx }
    }

    async fn call(&self, command: Command) -> String {
        let (reply_tx, reply_rx) = oneshot::channel();
        if self.tx.send((command, reply_tx)).await.is_err() {
            return "the game thread is gone".to_owned();
        }
        reply_rx
            .await
            .unwrap_or_else(|_| "the game thread is gone".to_owned())
    }
}

#[tool_router(server_handler)]
impl GameMcp {
    #[tool(
        description = "Buy a hull at your docked port using a 50% old-hull trade-in. Cargo and equipment must fit; outfits transfer."
    )]
    async fn buy_ship(&self, Parameters(BuyShipArgs { ship }): Parameters<BuyShipArgs>) -> String {
        self.call(Command::BuyShip(ship)).await
    }
    #[tool(
        description = "Buy and install one outfit from your docked port's catalog. Server validates credits and slots."
    )]
    async fn buy_outfit(
        &self,
        Parameters(OutfitArgs { outfit }): Parameters<OutfitArgs>,
    ) -> String {
        self.call(Command::BuyOutfit(outfit)).await
    }
    #[tool(description = "Sell one installed outfit at a docked outfitter for 50% of list price.")]
    async fn sell_outfit(
        &self,
        Parameters(OutfitArgs { outfit }): Parameters<OutfitArgs>,
    ) -> String {
        self.call(Command::SellOutfit(outfit)).await
    }
    #[tool(
        description = "Capture fresh README screenshots using a separate desktop client tour. Requires a working desktop display and the client built alongside the MCP server. Overwrites the six fixed PNG files in docs/screenshots only after the complete tour succeeds. May take up to 180 seconds. Does not capture or alter the headless MCP game session."
    )]
    async fn capture_screenshots(&self) -> rmcp::model::CallToolResult {
        let result = tokio::task::spawn_blocking(|| {
            let root = content_root()
                .canonicalize()
                .map_err(|err| err.to_string())?
                .parent()
                .ok_or_else(|| "content directory has no parent".to_owned())?
                .to_owned();
            let client = std::env::current_exe()
                .map_err(|err| err.to_string())?
                .with_file_name("second-velocity");
            sv_mcp::screenshots::capture_readme_screenshots(&root, &client)
        })
        .await;
        match result.unwrap_or_else(|error| Err(error.to_string())) {
            Ok(paths) => rmcp::model::CallToolResult::structured(serde_json::json!({
                "screenshots": paths,
                "session": "separate desktop capture tour",
            })),
            Err(error) => rmcp::model::CallToolResult::structured_error(serde_json::json!({
                "error": error,
            })),
        }
    }
    #[tool(description = "Advance the game simulation. Ticks are ~15ms each; max 600 per call.")]
    async fn advance(&self, Parameters(AdvanceArgs { ticks }): Parameters<AdvanceArgs>) -> String {
        self.call(Command::Advance(ticks.min(MAX_TICKS))).await
    }

    #[tool(
        description = "Observe the full authoritative game state as JSON: ship, planets, ships, market, encounter, quests, notices."
    )]
    async fn observe(&self) -> String {
        self.call(Command::Observe).await
    }

    #[tool(
        description = "Set flight inputs: throttle (0..1), turn (-1..1), and firing hold. Returns the updated state."
    )]
    async fn set_flight(
        &self,
        Parameters(SetFlightArgs {
            throttle,
            turn,
            firing,
        }): Parameters<SetFlightArgs>,
    ) -> String {
        self.call(Command::SetFlight {
            throttle,
            turn,
            firing,
        })
        .await
    }

    #[tool(
        description = "Request landing at a planet. Fly within landing range first. Returns the updated state."
    )]
    async fn land(&self, Parameters(LandArgs { planet }): Parameters<LandArgs>) -> String {
        self.call(Command::Land(planet)).await
    }

    #[tool(
        description = "Dock at a station or planet identified by owning system and location id. Fly into range first; orbital stations move. Returns the updated state."
    )]
    async fn dock(
        &self,
        Parameters(DockArgs { system, location }): Parameters<DockArgs>,
    ) -> String {
        self.call(Command::Dock { system, location }).await
    }

    #[tool(description = "Undock from your current planet or station. Returns the updated state.")]
    async fn lift_off(&self) -> String {
        self.call(Command::LiftOff).await
    }

    #[tool(
        description = "Buy or sell a commodity while docked. Returns the updated state; check notices for the result."
    )]
    async fn trade(
        &self,
        Parameters(TradeArgs {
            commodity,
            qty,
            direction,
        }): Parameters<TradeArgs>,
    ) -> String {
        match direction.as_str() {
            "buy" => {
                self.call(Command::Trade {
                    commodity,
                    qty,
                    buy: true,
                })
                .await
            }
            "sell" => {
                self.call(Command::Trade {
                    commodity,
                    qty,
                    buy: false,
                })
                .await
            }
            other => format!("direction must be \"buy\" or \"sell\", got {other:?}"),
        }
    }

    #[tool(description = "Toggle warp drive. Warp multiplies speed by 3 and drains fuel faster.")]
    async fn warp(&self, Parameters(args): Parameters<WarpArgs>) -> String {
        self.call(Command::SetWarp { on: args.on }).await
    }

    #[tool(
        description = "Hail the nearest friendly NPC ship in range while undocked to talk or trade in space. Returns the updated state."
    )]
    async fn hail(&self) -> String {
        self.call(Command::Hail).await
    }

    #[tool(
        description = "Respond to a planet chief or hailed NPC ship encounter (see observe.encounter.options). Returns the updated state."
    )]
    async fn respond(&self, Parameters(RespondArgs { choice }): Parameters<RespondArgs>) -> String {
        self.call(Command::Respond(choice)).await
    }

    #[tool(
        description = "Run one or all benchmark scenarios from content/scenarios/ and return tick latency percentiles, entity load, and gameplay metrics as JSON."
    )]
    async fn run_scenario(
        &self,
        Parameters(RunScenarioArgs { id }): Parameters<RunScenarioArgs>,
    ) -> String {
        self.call(Command::RunScenario(id)).await
    }
}

fn game_thread(content_root: PathBuf, mut rx: mpsc::Receiver<(Command, oneshot::Sender<String>)>) {
    let mut harness = Harness::new(&content_root);
    while let Some((command, reply)) = rx.blocking_recv() {
        harness.ensure_connected();
        let _ = reply.send(match command {
            Command::BuyShip(id) => {
                harness.buy_ship(&id);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::BuyOutfit(id) => {
                harness.buy_outfit(&id);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::SellOutfit(id) => {
                harness.sell_outfit(&id);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }

            Command::Advance(ticks) => {
                harness.advance(ticks);
                format!("advanced {ticks} ticks (tick {})", harness.ticks)
            }
            Command::Observe => json(&harness.observe()),
            Command::SetFlight {
                throttle,
                turn,
                firing,
            } => {
                harness.set_flight(throttle, turn, firing);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::Dock { system, location } => {
                harness.dock(&system, &location);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::Land(planet) => {
                harness.land(&planet);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::LiftOff => {
                harness.lift_off();
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::Trade {
                commodity,
                qty,
                buy,
            } => {
                if buy {
                    harness.buy(&commodity, qty);
                } else {
                    harness.sell(&commodity, qty);
                }
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::SetWarp { on } => {
                harness.set_warp(on);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::Hail => {
                harness.hail();
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::Respond(choice) => {
                harness.respond(&choice);
                harness.advance(SETTLE_TICKS);
                json(&harness.observe())
            }
            Command::RunScenario(id) => match sv_mcp::run_scenarios(&content_root, id.as_deref()) {
                Ok(reports) => serde_json::to_string_pretty(&reports)
                    .unwrap_or_else(|err| format!("serialize failed: {err}")),
                Err(err) => serde_json::json!({ "error": err }).to_string(),
            },
        });
    }
}

fn json(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|err| format!("serialize failed: {err}"))
}

fn content_root() -> PathBuf {
    if std::path::Path::new("content").exists() {
        PathBuf::from("content")
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content")
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = McpCli::parse();
    let Some(scenario) = cli.bench else {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async move {
            let service = GameMcp::new(content_root());
            let server = service.serve(stdio()).await?;
            server.waiting().await?;
            Ok::<(), Box<dyn std::error::Error>>(())
        })?;
        return Ok(());
    };

    let filter_id = scenario.filter(|id| !id.is_empty());
    let baseline = cli
        .compare
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<Vec<sv_mcp::ScenarioReport>>(&raw).ok());
    let reports = sv_mcp::run_scenarios(&content_root(), filter_id.as_deref())?;
    let json_str = serde_json::to_string_pretty(&reports)?;
    if let Some(save_path) = cli.save_json {
        std::fs::write(save_path, &json_str)?;
    }
    if cli.json {
        println!("{json_str}");
    } else {
        print!(
            "{}",
            sv_mcp::format_markdown_table_with_baseline(&reports, baseline.as_deref())
        );
    }
    if reports.iter().any(|report| !report.passed) {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_parses_serve_default_and_bench_modes() {
        let serve = McpCli::try_parse_from(["second-velocity-mcp"]).unwrap();
        assert_eq!(serve.bench, None);

        let all = McpCli::try_parse_from(["second-velocity-mcp", "--bench"]).unwrap();
        assert_eq!(all.bench, Some(Some(String::new())));
        assert!(!all.json);

        let one = McpCli::try_parse_from([
            "second-velocity-mcp",
            "--bench",
            "sun-heat-damage",
            "--json",
            "--save-json",
            "reports.json",
        ])
        .unwrap();
        assert_eq!(one.bench, Some(Some("sun-heat-damage".to_owned())));
        assert!(one.json);
        assert_eq!(one.save_json, Some(PathBuf::from("reports.json")));

        let flag_after_bench =
            McpCli::try_parse_from(["second-velocity-mcp", "--bench", "--json"]).unwrap();
        assert_eq!(flag_after_bench.bench, Some(Some(String::new())));
        assert!(flag_after_bench.json);
    }
}
