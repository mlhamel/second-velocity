use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SystemDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub government: String,
    pub position: (f32, f32),
    pub links: Vec<String>,
    pub planets: Vec<PlanetDef>,
    #[serde(default)]
    pub star: Option<StarDef>,
    #[serde(default)]
    pub black_hole: Option<BlackHoleDef>,
    #[serde(default)]
    pub meteorites: Option<MeteoriteFieldDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlackHoleDef {
    pub position: (f32, f32),
    pub radius: f32,
    pub influence: f32,
    pub pull_strength: f32,
    #[serde(default)]
    pub sprite: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StarDef {
    pub position: (f32, f32),
    pub radius: f32,
    #[serde(default)]
    pub sprite: Option<String>,
}

fn default_field_radius() -> f32 {
    1200.0
}

fn default_stationary_weight() -> u32 {
    2
}

fn default_slow_weight() -> u32 {
    5
}

fn default_fast_weight() -> u32 {
    3
}

#[derive(Debug, Clone, Deserialize)]
pub struct MeteoriteFieldDef {
    pub count: u32,
    #[serde(default = "default_field_radius")]
    pub radius: f32,
    #[serde(default = "default_stationary_weight")]
    pub stationary_weight: u32,
    #[serde(default = "default_slow_weight")]
    pub slow_weight: u32,
    #[serde(default = "default_fast_weight")]
    pub fast_weight: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlanetDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub sprite: Option<String>,
    #[serde(default)]
    pub chief: Option<ChiefDef>,
    pub commodities: Vec<CommodityPrice>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChiefDef {
    pub name: String,
    #[serde(default)]
    pub portrait: Option<String>,
    pub greeting: String,
    pub offer_text: String,
    pub tribute_text: String,
    pub quest_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommodityPrice {
    pub id: String,
    pub price: u32,
}
