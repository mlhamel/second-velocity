use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub version: (u32, u32, u32),
    #[serde(default)]
    pub quadrants: Vec<String>,
    pub ships: Vec<String>,
    pub outfits: Vec<String>,
    #[serde(default)]
    pub commodities: Vec<String>,
    #[serde(default)]
    pub scenarios: Vec<String>,
    #[serde(default)]
    pub factions: Vec<String>,
}
