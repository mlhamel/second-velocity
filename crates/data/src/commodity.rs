use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CommodityDef {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub icon: Option<String>,
}
