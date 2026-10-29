use serde::{Deserialize, Serialize};
use sv_data::{ContentPack, PilotDef};
use sv_protocol::{ActiveQuest, ChiefEncounter, EncounterOption, QuestTarget};

use crate::action::{ValidationContext, allowed_actions_for};

pub const DIGEST_MAX_BYTES: usize = 1550;
pub const DIGEST_MAX_TOKENS_EST: usize = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionPoint {
    Docked,
    EncounterOffered,
    HostileClose,
    QuestComplete,
    LowFuel,
    IdleHeartbeat,
    RespawnedAfterDeath,
}

impl DecisionPoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Docked => "docked",
            Self::EncounterOffered => "encounter_offered",
            Self::HostileClose => "hostile_close",
            Self::QuestComplete => "quest_complete",
            Self::LowFuel => "low_fuel",
            Self::IdleHeartbeat => "idle_heartbeat",
            Self::RespawnedAfterDeath => "respawned_after_death",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThreatSummary {
    pub kind: String,
    pub name: String,
    pub distance: f32,
}

#[derive(Debug, Clone)]
pub struct DigestInput {
    pub reason: DecisionPoint,
    pub ship_name: String,
    pub ship_id: String,
    pub shields: f32,
    pub max_shields: f32,
    pub armor: f32,
    pub max_armor: f32,
    pub fuel: f32,
    pub max_fuel: f32,
    pub credits: u32,
    pub cargo: Vec<(String, u32)>,
    pub cargo_capacity: u32,
    pub system: String,
    pub docked: Option<(String, String)>,
    pub market: Vec<(String, u32)>,
    pub shipyard: Vec<(String, u32, u32)>,
    pub encounter: Option<ChiefEncounter>,
    pub quests: Vec<ActiveQuest>,
    pub threats: Vec<ThreatSummary>,
}

impl DigestInput {
    pub fn validation_context(&self, pack: &ContentPack) -> ValidationContext {
        let current_ship_price = pack.ship(&self.ship_id).map_or(0, |s| s.price);
        let encounter_options = self
            .encounter
            .as_ref()
            .map(|e| e.options.iter().map(|o| o.id.clone()).collect())
            .unwrap_or_default();
        ValidationContext {
            system: self.system.clone(),
            docked: self.docked.clone(),
            credits: self.credits,
            current_ship_id: self.ship_id.clone(),
            current_ship_price,
            cargo: self.cargo.clone(),
            cargo_capacity: self.cargo_capacity,
            market: self.market.clone(),
            shipyard: self.shipyard.clone(),
            encounter_options,
        }
    }
}

pub fn estimate_tokens(text: &str) -> usize {
    let words = text.split_whitespace().count();
    let chars_div4 = text.len().div_ceil(4);
    words.max(chars_div4)
}

pub fn render_digest(pilot: &PilotDef, pack: &ContentPack, input: &DigestInput) -> String {
    let ctx = input.validation_context(pack);
    let allowed = allowed_actions_for(&ctx, input.reason);

    let mut out = String::with_capacity(1024);
    let persona_short = truncate_chars(&pilot.persona, 160);
    out.push_str(&format!(
        "PILOT: {} ({}) home={}/{}\nROLE: {}\nREASON: {}\n",
        pilot.name,
        pilot.id,
        pilot.home_system,
        pilot.home_port,
        persona_short,
        input.reason.as_str()
    ));

    let loc_str = match &input.docked {
        Some((sys, port)) => format!("docked@{sys}/{port}"),
        None => format!("space@{}", input.system),
    };
    let cargo_used: u32 = input.cargo.iter().map(|(_, q)| *q).sum();
    let cargo_list = if input.cargo.is_empty() {
        "empty".to_owned()
    } else {
        input
            .cargo
            .iter()
            .map(|(c, q)| format!("{c}:{q}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    out.push_str(&format!(
        "STATUS: {} hull={} sh={:.0}/{:.0} ar={:.0}/{:.0} fuel={:.0}/{:.0} cr={} cargo={}/{} [{}]\n",
        loc_str,
        input.ship_id,
        input.shields,
        input.max_shields,
        input.armor,
        input.max_armor,
        input.fuel,
        input.max_fuel,
        input.credits,
        cargo_used,
        input.cargo_capacity,
        cargo_list
    ));

    if let Some(sys) = pack.system(&input.system) {
        let ports = sys
            .planets
            .iter()
            .map(|p| p.id.as_str())
            .chain(sys.stations.iter().map(|s| s.id.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let links = sys.links.join(",");
        out.push_str(&format!("NAV: ports=[{ports}] links=[{links}]\n"));
    }

    if !input.market.is_empty() {
        let items = input
            .market
            .iter()
            .map(|(c, p)| format!("{c}:{p}cr"))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("MARKET: {items}\n"));
    }

    if !input.shipyard.is_empty() {
        let hulls = input
            .shipyard
            .iter()
            .map(|(s, p, cap)| format!("{s}:{p}cr({cap}t)"))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!("SHIPYARD: {hulls}\n"));
    }

    if !input.quests.is_empty() {
        let q_str = input
            .quests
            .iter()
            .take(3)
            .map(|q| match &q.target {
                QuestTarget::Delivery {
                    commodity,
                    qty,
                    destination_system,
                    destination_planet,
                } => format!(
                    "{}:deliver {qty} {commodity}->{destination_system}/{destination_planet} (+{}cr)",
                    q.id, q.reward_credits
                ),
                QuestTarget::Bounty {
                    system,
                    target,
                    killed,
                } => format!(
                    "{}:bounty {target}@{system} killed={killed} return={}/{} (+{}cr)",
                    q.id, q.giver_system, q.giver_planet, q.reward_credits
                ),
            })
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!("QUESTS: {q_str}\n"));
    }

    if !input.threats.is_empty() {
        let t_str = input
            .threats
            .iter()
            .take(3)
            .map(|t| format!("{}:{}@{:.0}u", t.kind, t.name, t.distance))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("THREATS: {t_str}\n"));
    }

    if let Some(enc) = &input.encounter {
        let text = truncate_chars(&enc.text.replace('\n', " "), 240);
        let opts = format_options(&enc.options);
        out.push_str(&format!(
            "ENCOUNTER ({}): \"{}\"\nOPTIONS: {}\n",
            enc.chief_name, text, opts
        ));
    }

    out.push_str(&format!(
        "ALLOWED_ACTIONS: [{}]\nReturn ONE JSON object matching PilotAction.",
        allowed.join(", ")
    ));

    enforce_digest_cap(out)
}

fn format_options(options: &[EncounterOption]) -> String {
    options
        .iter()
        .map(|o| format!("{}=\"{}\"", o.id, truncate_chars(&o.label, 48)))
        .collect::<Vec<_>>()
        .join(" | ")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_owned()
    } else {
        let mut s: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

fn enforce_digest_cap(mut text: String) -> String {
    if text.len() > DIGEST_MAX_BYTES {
        text.truncate(DIGEST_MAX_BYTES);
    }
    while estimate_tokens(&text) > DIGEST_MAX_TOKENS_EST && text.len() > 64 {
        let next_len = text.len().saturating_sub(32);
        text.truncate(next_len);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use sv_protocol::ChiefMood;

    #[test]
    fn digest_stays_within_400_token_budget_under_full_load() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let pack = ContentPack::load_dir(&root).expect("pack loads");
        let pilot = pack.pilot("cautious-trader").expect("pilot exists").clone();

        let input = DigestInput {
            reason: DecisionPoint::EncounterOffered,
            ship_name: "Shuttle".into(),
            ship_id: "shuttle".into(),
            shields: 40.0,
            max_shields: 40.0,
            armor: 25.0,
            max_armor: 25.0,
            fuel: 100.0,
            max_fuel: 100.0,
            credits: 1450,
            cargo: vec![("food".into(), 6), ("medicine".into(), 4)],
            cargo_capacity: 20,
            system: "sol".into(),
            docked: Some(("sol".into(), "earth".into())),
            market: vec![
                ("food".into(), 8),
                ("machinery".into(), 45),
                ("medicine".into(), 62),
                ("ore".into(), 40),
                ("textiles".into(), 18),
            ],
            shipyard: vec![
                ("shuttle".into(), 800, 20),
                ("terran-courier".into(), 2200, 25),
                ("terran-hauler".into(), 4500, 60),
            ],
            encounter: Some(ChiefEncounter {
                chief_name: "Port Director Lin".into(),
                portrait: None,
                mood: ChiefMood::Quest,
                text: "Berth seventeen. Keep your thrusters cold and your manifest colder.\nSealed Med-Crates for Newhaven — Carry 4 crates of medicine to Newhaven in Alpha Centauri. (Reward: 450 cr)".into(),
                options: vec![
                    EncounterOption {
                        id: "accept_quest".into(),
                        label: "Accept 'Sealed Med-Crates for Newhaven' (450 cr)".into(),
                    },
                    EncounterOption {
                        id: "decline".into(),
                        label: "Decline contract".into(),
                    },
                ],
            }),
            quests: vec![ActiveQuest {
                id: "newhaven-relief".into(),
                name: "Sealed Med-Crates for Newhaven".into(),
                giver_system: "sol".into(),
                giver_planet: "earth".into(),
                target: QuestTarget::Delivery {
                    commodity: "medicine".into(),
                    qty: 4,
                    destination_system: "alpha-centauri".into(),
                    destination_planet: "newhaven".into(),
                },
                reward_credits: 450,
                remaining_secs: Some(180.0),
                procedural: false,
            }],
            threats: vec![
                ThreatSummary {
                    kind: "pirate".into(),
                    name: "Turncoat".into(),
                    distance: 410.0,
                },
                ThreatSummary {
                    kind: "meteorite".into(),
                    name: "large".into(),
                    distance: 280.0,
                },
            ],
        };

        let digest = render_digest(&pilot, &pack, &input);
        let tokens = estimate_tokens(&digest);
        assert!(
            tokens <= DIGEST_MAX_TOKENS_EST,
            "digest tokens {tokens} exceeded {DIGEST_MAX_TOKENS_EST}:\n{digest}"
        );
        assert!(digest.contains("Port Director Lin"));
        assert!(digest.contains("accept_quest"));
        assert!(digest.contains("ALLOWED_ACTIONS"));
    }
}
