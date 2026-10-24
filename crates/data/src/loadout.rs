use crate::{ContentPack, OutfitDef, OutfitKind, ShipDef};

#[derive(Debug, Clone, Copy)]
pub struct LoadoutStats {
    pub max_shields: f32,
    pub max_armor: f32,
    pub max_fuel: f32,
    pub weapon_slots_used: u32,
}

impl ContentPack {
    pub fn loadout_stats(&self, ship_id: &str, ids: &[String]) -> Result<LoadoutStats, String> {
        let ship = self
            .ship(ship_id)
            .ok_or_else(|| format!("Unknown hull: {ship_id}."))?;
        stats_for(ship, &self.outfits, ids)
    }
}

pub(crate) fn stats_for(
    ship: &ShipDef,
    outfits: &[OutfitDef],
    ids: &[String],
) -> Result<LoadoutStats, String> {
    let mut stats = LoadoutStats {
        max_shields: ship.max_shields,
        max_armor: ship.max_armor,
        max_fuel: ship.fuel as f32,
        weapon_slots_used: 0,
    };
    let (mut shield, mut fuel, mut armor) = (false, false, false);
    for id in ids {
        let outfit = outfits
            .iter()
            .find(|outfit| outfit.id == *id)
            .ok_or_else(|| format!("Unknown outfit: {id}."))?;
        match &outfit.kind {
            OutfitKind::Weapon(_) => {
                stats.weapon_slots_used = stats
                    .weapon_slots_used
                    .checked_add(1)
                    .ok_or("Too many weapons.")?;
                if stats.weapon_slots_used > ship.weapon_slots {
                    return Err("No free weapon slots.".into());
                }
            }
            OutfitKind::ShieldGenerator { max_shields } => {
                if shield {
                    return Err(
                        "Only one shield generator fits; sell the installed one first.".into(),
                    );
                }
                shield = true;
                stats.max_shields += max_shields;
            }
            OutfitKind::FuelTank { fuel: extra } => {
                if fuel {
                    return Err("Only one auxiliary fuel tank fits.".into());
                }
                fuel = true;
                stats.max_fuel += extra;
            }
            OutfitKind::ArmorPlate { armor: extra } => {
                if armor {
                    return Err("Only one armor plate fits.".into());
                }
                armor = true;
                stats.max_armor += extra;
            }
        }
    }
    if ![stats.max_shields, stats.max_armor, stats.max_fuel]
        .into_iter()
        .all(|v| v.is_finite() && v > 0.0)
    {
        return Err("Invalid equipped hull capacities.".into());
    }
    Ok(stats)
}
