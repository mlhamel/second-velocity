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

    pub const fn default_max_energy(self) -> f32 {
        match self {
            Self::Light => 60.0,
            Self::Medium => 85.0,
            Self::Heavy => 120.0,
            Self::Capital => 160.0,
        }
    }

    pub const fn default_energy_regen(self) -> f32 {
        match self {
            Self::Light => 10.0,
            Self::Medium => 13.0,
            Self::Heavy => 16.0,
            Self::Capital => 20.0,
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
    pub max_energy: Option<f32>,
    #[serde(default)]
    pub energy_regen: Option<f32>,
    #[serde(default)]
    pub default_outfits: Vec<String>,
    #[serde(default)]
    pub sprite: Option<String>,
}

impl ShipDef {
    pub fn base_max_energy(&self) -> f32 {
        self.max_energy
            .unwrap_or_else(|| self.size.default_max_energy())
    }

    pub fn base_energy_regen(&self) -> f32 {
        self.energy_regen
            .unwrap_or_else(|| self.size.default_energy_regen())
    }
}
