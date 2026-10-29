use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: (u32, u32, u32),
    pub ships: Vec<String>,
    pub systems: Vec<String>,
    pub outfits: Vec<String>,
    #[serde(default)]
    pub commodities: Vec<String>,
    #[serde(default)]
    pub quests: Vec<String>,
    #[serde(default)]
    pub scenarios: Vec<String>,
    #[serde(default)]
    pub factions: Vec<String>,
    #[serde(default)]
    pub pilots: Vec<String>,
}
