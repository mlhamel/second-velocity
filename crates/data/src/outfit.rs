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
    Reactor { max_energy: f32, energy_regen: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
pub enum WeaponMount {
    #[default]
    Fixed,
    Turret,
    Missile {
        turn_rate: f32,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeaponDef {
    pub damage: f32,
    pub cooldown: f32,
    pub projectile_speed: f32,
    pub projectile_ttl: f32,
    #[serde(default)]
    pub energy_cost: Option<f32>,
    #[serde(default)]
    pub mount: WeaponMount,
}

impl WeaponDef {
    pub fn effective_energy_cost(&self) -> f32 {
        self.energy_cost.unwrap_or(self.damage * 0.35)
    }
}
