use serde::{Deserialize, Serialize};

fn default_race() -> String {
    "terran".to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub enum ShipSize {
    Light,
    #[default]
    Medium,
    Heavy,
    Capital,
}

impl ShipSize {
    pub const fn world_size(self) -> f32 {
        match self {
            Self::Light => 36.0,
            Self::Medium => 48.0,
            Self::Heavy => 68.0,
            Self::Capital => 92.0,
        }
    }

    pub const fn hit_radius(self) -> f32 {
        match self {
            Self::Light => 14.0,
            Self::Medium => 18.0,
            Self::Heavy => 24.0,
            Self::Capital => 32.0,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Medium => "medium",
            Self::Heavy => "heavy",
            Self::Capital => "capital",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShipDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default = "default_race")]
    pub race: String,
    #[serde(default)]
    pub size: ShipSize,
    pub price: u32,
    pub mass: f32,
    pub turn_rate: f32,
    pub acceleration: f32,
    pub max_speed: f32,
    pub max_shields: f32,
    pub max_armor: f32,
    pub fuel: u32,
    pub cargo: u32,
    pub weapon_slots: u32,
    #[serde(default)]
    pub default_outfits: Vec<String>,
    #[serde(default)]
    pub sprite: Option<String>,
}
