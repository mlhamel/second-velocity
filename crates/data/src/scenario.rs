use serde::{Deserialize, Serialize};

fn default_max_ticks() -> u64 {
    5_000
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScenarioDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub encounter_seed: Option<u64>,
    #[serde(default = "default_max_ticks")]
    pub max_ticks: u64,
    pub steps: Vec<ScenarioStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ScenarioStep {
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
}
