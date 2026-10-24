use serde::{Deserialize, Serialize};

fn default_max_ticks() -> u64 {
    5_000
}

fn default_star_approach() -> f32 {
    1.3
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScenarioDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub encounter_seed: Option<u64>,
    /// Benchmark-only starting-balance fixture; never a game/MCP cheat intent.
    #[serde(default)]
    pub starting_credits: Option<u32>,
    #[serde(default = "default_max_ticks")]
    pub max_ticks: u64,
    pub steps: Vec<ScenarioStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ScenarioStep {
    BuyShip {
        ship: String,
    },
    BuyOutfit {
        outfit: String,
    },
    SellOutfit {
        outfit: String,
    },
    AssertShip(String),
    AssertLoadout {
        outfits: Vec<String>,
    },
    AssertShipStats {
        max_shields: f32,
        max_armor: f32,
        max_fuel: f32,
        cargo: u32,
        weapon_slots: u32,
    },
    SetFlight {
        throttle: f32,
        turn: f32,
        fire: bool,
    },
    SetWarp(bool),
    Advance {
        ticks: u32,
    },
    FlyToPlanet {
        system: String,
        planet: String,
        #[serde(default)]
        land: bool,
    },
    FlyToSystem {
        system: String,
    },
    FlyToStation {
        system: String,
        station: String,
        #[serde(default)]
        dock: bool,
    },
    Dock {
        system: String,
        location: String,
    },
    FlyToStar {
        system: String,
        #[serde(default = "default_star_approach")]
        approach: f32,
    },
    FlyToBlackHole {
        system: String,
    },
    Land {
        planet: String,
    },
    LiftOff,
    Trade {
        action: String,
        commodity: String,
        qty: u32,
    },
    Respond {
        choice: String,
    },
    AssertAlive(bool),
    AssertSystem(String),
    AssertHull {
        #[serde(default)]
        min: Option<f32>,
        #[serde(default)]
        max: Option<f32>,
    },
    AssertDocked(Option<String>),
    AssertCredits {
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    AssertCargo {
        commodity: String,
        min_qty: u32,
    },
    AssertEncounter {
        #[serde(default)]
        mood: Option<String>,
    },
    AssertActiveQuests {
        count: usize,
    },
    AssertCompletedQuest(String),
    ClearMeteorites,
    SpawnMeteorite {
        size: String,
        offset: (f32, f32),
        #[serde(default)]
        velocity: (f32, f32),
    },
    DestroyNearestMeteorite,
    AssertMeteorites {
        count: usize,
        #[serde(default)]
        size: Option<String>,
    },
    AssertNpcKindsPresent {
        kinds: Vec<String>,
    },
    HailNearestShip {
        #[serde(default)]
        kind: Option<String>,
    },
    ProvokeNearestNpc {
        #[serde(default)]
        kind: Option<String>,
    },
    AdvanceEconomyTicks {
        ticks: u32,
    },
    AssertPrice {
        system: String,
        location: String,
        commodity: String,
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
}
