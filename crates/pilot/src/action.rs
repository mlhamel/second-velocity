use serde::{Deserialize, Serialize};
use sv_data::ContentPack;

use crate::digest::DecisionPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TradeDirection {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PilotAction {
    FlyTo {
        system: String,
        #[serde(default)]
        location: Option<String>,
    },
    Dock {
        system: String,
        location: String,
    },
    Trade {
        direction: TradeDirection,
        commodity: String,
        qty: u32,
    },
    BuyShip {
        ship: String,
    },
    Respond {
        choice: String,
    },
    Flee {
        #[serde(default)]
        system: Option<String>,
    },
    Idle,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidationContext {
    pub system: String,
    pub docked: Option<(String, String)>,
    pub credits: u32,
    pub current_ship_id: String,
    pub current_ship_price: u32,
    pub cargo: Vec<(String, u32)>,
    pub cargo_capacity: u32,
    pub market: Vec<(String, u32)>,
    pub shipyard: Vec<(String, u32, u32)>, // (id, price, cargo_capacity)
    pub encounter_options: Vec<String>,
}

impl ValidationContext {
    pub fn cargo_used(&self) -> u32 {
        self.cargo.iter().map(|(_, q)| *q).sum()
    }

    pub fn cargo_qty(&self, commodity: &str) -> u32 {
        self.cargo
            .iter()
            .find(|(c, _)| c == commodity)
            .map_or(0, |(_, q)| *q)
    }
}

pub fn parse_and_validate(
    raw_json: &str,
    pack: &ContentPack,
    ctx: &ValidationContext,
) -> Result<PilotAction, String> {
    let trimmed = strip_json_fence(raw_json);
    let action: PilotAction =
        serde_json::from_str(trimmed).map_err(|err| format!("invalid PilotAction JSON: {err}"))?;
    validate_action(&action, pack, ctx)?;
    Ok(action)
}

fn strip_json_fence(raw: &str) -> &str {
    let s = raw.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        rest.trim().strip_suffix("```").unwrap_or(rest).trim()
    } else if let Some(rest) = s.strip_prefix("```") {
        rest.trim().strip_suffix("```").unwrap_or(rest).trim()
    } else {
        s
    }
}

pub fn validate_action(
    action: &PilotAction,
    pack: &ContentPack,
    ctx: &ValidationContext,
) -> Result<(), String> {
    match action {
        PilotAction::FlyTo { system, location } => {
            let sys = pack
                .system(system)
                .ok_or_else(|| format!("unknown system '{system}'"))?;
            if let Some(loc) = location
                && sys.dockable(loc).is_none()
            {
                return Err(format!("unknown dockable '{system}/{loc}'"));
            }
            Ok(())
        }
        PilotAction::Dock { system, location } => {
            let sys = pack
                .system(system)
                .ok_or_else(|| format!("unknown system '{system}'"))?;
            if sys.dockable(location).is_none() {
                return Err(format!("unknown dockable '{system}/{location}'"));
            }
            Ok(())
        }
        PilotAction::Trade {
            direction,
            commodity,
            qty,
        } => {
            if ctx.docked.is_none() {
                return Err("trade requires being docked at a port".into());
            }
            if *qty == 0 {
                return Err("trade qty must be > 0".into());
            }
            let Some((_, price)) = ctx.market.iter().find(|(c, _)| c == commodity) else {
                return Err(format!(
                    "commodity '{commodity}' is not listed at this port"
                ));
            };
            match direction {
                TradeDirection::Buy => {
                    let cost = price
                        .checked_mul(*qty)
                        .ok_or_else(|| "trade cost overflow".to_owned())?;
                    if ctx.credits < cost {
                        return Err(format!(
                            "insufficient credits for buy ({} < {})",
                            ctx.credits, cost
                        ));
                    }
                    if ctx.cargo_used() + *qty > ctx.cargo_capacity {
                        return Err(format!(
                            "insufficient cargo space ({} + {} > {})",
                            ctx.cargo_used(),
                            qty,
                            ctx.cargo_capacity
                        ));
                    }
                }
                TradeDirection::Sell => {
                    let held = ctx.cargo_qty(commodity);
                    if held < *qty {
                        return Err(format!(
                            "cannot sell {qty} {commodity}; only {held} in cargo"
                        ));
                    }
                }
            }
            Ok(())
        }
        PilotAction::BuyShip { ship } => {
            if ctx.docked.is_none() {
                return Err("buy_ship requires being docked at a shipyard".into());
            }
            if *ship == ctx.current_ship_id {
                return Err(format!("already own hull '{ship}'"));
            }
            let Some((_, price, cargo_cap)) = ctx.shipyard.iter().find(|(s, _, _)| s == ship)
            else {
                return Err(format!(
                    "hull '{ship}' is not stocked at this port's shipyard"
                ));
            };
            if ctx.cargo_used() > *cargo_cap {
                return Err(format!(
                    "new hull '{ship}' cargo ({cargo_cap}) cannot fit current cargo ({})",
                    ctx.cargo_used()
                ));
            }
            let budget = u64::from(ctx.credits) + u64::from(ctx.current_ship_price / 2);
            if budget < u64::from(*price) {
                return Err(format!(
                    "insufficient credits after trade-in ({budget} < {price})"
                ));
            }
            Ok(())
        }
        PilotAction::Respond { choice } => {
            if ctx.encounter_options.is_empty() {
                return Err("respond requires an active encounter".into());
            }
            let valid_id = ctx.encounter_options.iter().any(|opt| opt == choice);
            let valid_index = choice
                .parse::<usize>()
                .is_ok_and(|idx| idx >= 1 && idx <= ctx.encounter_options.len());
            if !valid_id && !valid_index {
                return Err(format!(
                    "choice '{choice}' is not among encounter options {:?}",
                    ctx.encounter_options
                ));
            }
            Ok(())
        }
        PilotAction::Flee { system } => {
            if let Some(sys) = system
                && pack.system(sys).is_none()
            {
                return Err(format!("unknown flee system '{sys}'"));
            }
            Ok(())
        }
        PilotAction::Idle => Ok(()),
    }
}

pub fn allowed_actions_for(ctx: &ValidationContext, reason: DecisionPoint) -> Vec<&'static str> {
    let mut allowed = Vec::new();
    if !ctx.encounter_options.is_empty() {
        allowed.push("respond");
    }
    if ctx.docked.is_some() {
        if !ctx.market.is_empty() {
            allowed.push("trade");
        }
        if !ctx.shipyard.is_empty() {
            allowed.push("buy_ship");
        }
        allowed.push("fly_to");
        allowed.push("dock");
        allowed.push("idle");
    } else {
        if reason == DecisionPoint::HostileClose {
            allowed.push("flee");
        }
        allowed.push("dock");
        allowed.push("fly_to");
        allowed.push("flee");
        allowed.push("idle");
    }
    allowed.dedup();
    allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn sample_pack() -> ContentPack {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        ContentPack::load_dir(&root).expect("pack loads")
    }

    fn docked_ctx() -> ValidationContext {
        ValidationContext {
            system: "sol".into(),
            docked: Some(("sol".into(), "earth".into())),
            credits: 500,
            current_ship_id: "shuttle".into(),
            current_ship_price: 800,
            cargo: vec![("food".into(), 2)],
            cargo_capacity: 20,
            market: vec![("food".into(), 8), ("machinery".into(), 45)],
            shipyard: vec![
                ("shuttle".into(), 800, 20),
                ("terran-courier".into(), 2200, 25),
            ],
            encounter_options: vec!["accept_quest".into(), "decline".into()],
        }
    }

    #[test]
    fn validates_all_action_variants() {
        let pack = sample_pack();
        let mut ctx = docked_ctx();

        assert_eq!(
            parse_and_validate(
                r#"{"action":"respond","choice":"accept_quest"}"#,
                &pack,
                &ctx
            )
            .unwrap(),
            PilotAction::Respond {
                choice: "accept_quest".into()
            }
        );

        assert_eq!(
            parse_and_validate(
                r#"{"action":"trade","direction":"buy","commodity":"food","qty":5}"#,
                &pack,
                &ctx
            )
            .unwrap(),
            PilotAction::Trade {
                direction: TradeDirection::Buy,
                commodity: "food".into(),
                qty: 5,
            }
        );

        assert!(
            parse_and_validate(
                r#"{"action":"trade","direction":"sell","commodity":"food","qty":5}"#,
                &pack,
                &ctx
            )
            .is_err()
        );

        assert!(
            parse_and_validate(
                r#"{"action":"buy_ship","ship":"terran-courier"}"#,
                &pack,
                &ctx
            )
            .is_err()
        );

        ctx.credits = 2000;
        assert_eq!(
            parse_and_validate(
                r#"{"action":"buy_ship","ship":"terran-courier"}"#,
                &pack,
                &ctx
            )
            .unwrap(),
            PilotAction::BuyShip {
                ship: "terran-courier".into()
            }
        );

        assert_eq!(
            parse_and_validate(
                r#"{"action":"dock","system":"alpha-centauri","location":"newhaven"}"#,
                &pack,
                &ctx
            )
            .unwrap(),
            PilotAction::Dock {
                system: "alpha-centauri".into(),
                location: "newhaven".into()
            }
        );

        assert!(
            parse_and_validate(
                r#"{"action":"dock","system":"sol","location":"atlantis"}"#,
                &pack,
                &ctx
            )
            .is_err()
        );
    }
}
