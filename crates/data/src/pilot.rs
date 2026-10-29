use serde::{Deserialize, Serialize};

fn default_idle_interval_secs() -> f32 {
    35.0
}

fn default_flee_range() -> f32 {
    600.0
}

fn default_low_fuel_ratio() -> f32 {
    0.25
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PilotDef {
    pub id: String,
    pub name: String,
    pub username: String,
    pub password: String,
    pub home_system: String,
    pub home_port: String,
    pub persona: String,
    #[serde(default)]
    pub preferred_commodities: Vec<String>,
    #[serde(default)]
    pub preferred_ships: Vec<String>,
    #[serde(default = "default_idle_interval_secs")]
    pub idle_interval_secs: f32,
    #[serde(default = "default_flee_range")]
    pub flee_range: f32,
    #[serde(default = "default_low_fuel_ratio")]
    pub low_fuel_ratio: f32,
}
