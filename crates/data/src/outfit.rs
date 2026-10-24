use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct OutfitDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: u32,
    pub mass: f32,
    pub kind: OutfitKind,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum OutfitKind {
    Weapon(WeaponDef),
    ShieldGenerator { max_shields: f32 },
    FuelTank { fuel: f32 },
    ArmorPlate { armor: f32 },
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponDef {
    pub damage: f32,
    pub cooldown: f32,
    pub projectile_speed: f32,
    pub projectile_ttl: f32,
}
