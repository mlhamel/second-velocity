use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum FactionSide {
    Confederation,
    Rebellion,
    Neutral,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FactionDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub color: (f32, f32, f32),
    pub side: FactionSide,
}
