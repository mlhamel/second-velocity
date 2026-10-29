use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::Deserialize;
use sv_data::{ContentPack, PilotDef};
use sv_protocol::QuestTarget;

use crate::action::{PilotAction, TradeDirection, parse_and_validate, validate_action};
use crate::digest::{DecisionPoint, DigestInput, render_digest};

pub const MIN_DECISION_INTERVAL: Duration = Duration::from_secs(15);
pub const OLLAMA_TIMEOUT: Duration = Duration::from_secs(20);

const SYSTEM_PROMPT: &str = r#"You are a spaceship pilot in Second Velocity.
Given a compact state digest, return exactly ONE JSON object matching PilotAction:
- {"action":"fly_to","system":"<id>","location":"<optional_port_id>"}
- {"action":"dock","system":"<id>","location":"<port_id>"}
- {"action":"trade","direction":"buy"|"sell","commodity":"<id>","qty":<u32>}
- {"action":"buy_ship","ship":"<ship_id>"}
- {"action":"respond","choice":"<option_id>"}
- {"action":"flee","system":"<optional_system_id>"}
- {"action":"idle"}
Do not include markdown or extra keys."#;

pub trait PilotBrain: Send + 'static {
    fn complete(&mut self, prompt: &str) -> Result<String, String>;
    fn is_mock(&self) -> bool {
        false
    }
}

pub struct OllamaBrain {
    pub url: String,
    pub model: String,
    pub timeout: Duration,
}

impl OllamaBrain {
    pub fn new(url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            model: model.into(),
            timeout: OLLAMA_TIMEOUT,
        }
    }
}

#[derive(Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

impl PilotBrain for OllamaBrain {
    fn complete(&mut self, prompt: &str) -> Result<String, String> {
        let endpoint = format!("{}/api/generate", self.url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "system": SYSTEM_PROMPT,
            "prompt": prompt,
            "stream": false,
            "format": "json",
            "options": {
                "temperature": 0.2,
                "num_predict": 128,
            }
        });
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .build();
        let agent: ureq::Agent = config.into();
        let mut response = agent
            .post(&endpoint)
            .send_json(&body)
            .map_err(|err| format!("ollama request failed: {err}"))?;
        let parsed: OllamaGenerateResponse = response
            .body_mut()
            .read_json()
            .map_err(|err| format!("ollama response parse failed: {err}"))?;
        Ok(parsed.response)
    }
}

/// Deterministic scripted brain for `--mock` and CI integration tests.
#[derive(Default)]
pub struct MockBrain {
    pub scripted_raw: Vec<String>,
}

impl MockBrain {
    pub fn new() -> Self {
        Self {
            scripted_raw: Vec::new(),
        }
    }

    pub fn with_scripted_json(responses: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut raw: Vec<String> = responses.into_iter().map(Into::into).collect();
        raw.reverse();
        Self { scripted_raw: raw }
    }
}

impl PilotBrain for MockBrain {
    fn complete(&mut self, _prompt: &str) -> Result<String, String> {
        if let Some(next) = self.scripted_raw.pop() {
            Ok(next)
        } else {
            Err("use_deterministic_policy".into())
        }
    }

    fn is_mock(&self) -> bool {
        true
    }
}

/// Evaluates one decision cycle (with up to one error-feedback retry on invalid
/// JSON or failed state validation, then deterministic fallback).
pub fn decide_once(
    brain: &mut dyn PilotBrain,
    pilot: &PilotDef,
    pack: &ContentPack,
    input: &DigestInput,
) -> PilotAction {
    let digest = render_digest(pilot, pack, input);
    let ctx = input.validation_context(pack);

    match brain.complete(&digest) {
        Ok(raw) => match parse_and_validate(&raw, pack, &ctx) {
            Ok(action) => return action,
            Err(first_err) => {
                let retry_prompt = format!(
                    "{digest}\nPREVIOUS_ATTEMPT_ERROR: {first_err}\nFix the error and return ONE valid PilotAction JSON object."
                );
                if let Ok(raw_retry) = brain.complete(&retry_prompt)
                    && let Ok(action) = parse_and_validate(&raw_retry, pack, &ctx)
                {
                    return action;
                }
            }
        },
        Err(err) if brain.is_mock() && err == "use_deterministic_policy" => {
            return deterministic_policy(pilot, pack, input);
        }
        Err(_) => {}
    }

    fallback_action(pilot, pack, input)
}

/// Deterministic policy for `--mock` / stub-brain runs: handles encounters,
/// turns in quests, buys/sells cargo, upgrades ships when affordable, and
/// routes between ports and quest destinations.
pub fn deterministic_policy(
    pilot: &PilotDef,
    pack: &ContentPack,
    input: &DigestInput,
) -> PilotAction {
    let ctx = input.validation_context(pack);

    // 1. If an encounter is open, respond deterministically:
    if let Some(enc) = &input.encounter {
        for preferred in [
            "turn_in",
            "accept_quest",
            "accept_offer",
            "accept_hail_gift",
            "refuse_tribute",
            "decline",
        ] {
            if enc.options.iter().any(|o| o.id == preferred) {
                let candidate = PilotAction::Respond {
                    choice: preferred.to_owned(),
                };
                if validate_action(&candidate, pack, &ctx).is_ok() {
                    return candidate;
                }
            }
        }
        if let Some(first) = enc.options.first() {
            return PilotAction::Respond {
                choice: first.id.clone(),
            };
        }
    }

    // 2. If hostile close while undocked, flee:
    if input.reason == DecisionPoint::HostileClose && input.docked.is_none() {
        let flee_sys = pack
            .system(&input.system)
            .and_then(|s| s.links.first().cloned())
            .or_else(|| Some(pilot.home_system.clone()));
        return PilotAction::Flee { system: flee_sys };
    }

    // 3. If low fuel while undocked, dock at nearest port in current system or home:
    if input.reason == DecisionPoint::LowFuel && input.docked.is_none() {
        if let Some(sys) = pack.system(&input.system)
            && let Some(port) = sys
                .planets
                .first()
                .map(|p| p.id.clone())
                .or_else(|| sys.stations.first().map(|s| s.id.clone()))
        {
            return PilotAction::Dock {
                system: input.system.clone(),
                location: port,
            };
        }
        return PilotAction::Dock {
            system: pilot.home_system.clone(),
            location: pilot.home_port.clone(),
        };
    }

    // 4. While docked:
    if let Some((dock_sys, dock_port)) = &input.docked {
        // 4a. Check if a preferred ship at the shipyard is affordable:
        for pref_ship in &pilot.preferred_ships {
            let candidate = PilotAction::BuyShip {
                ship: pref_ship.clone(),
            };
            if validate_action(&candidate, pack, &ctx).is_ok() {
                return candidate;
            }
        }

        let at_home = (dock_sys, dock_port) == (&pilot.home_system, &pilot.home_port);

        // 4b. Sell non-quest cargo if listed at this port (avoid selling preferred export cargo at home port):
        let quest_hold = quest_reserved_cargo(&input.quests);
        for (comm, held) in &input.cargo {
            if at_home && pilot.preferred_commodities.contains(comm) {
                continue;
            }
            let reserved = quest_hold.get(comm).copied().unwrap_or(0);
            let sellable = held.saturating_sub(reserved);
            if sellable > 0 && input.market.iter().any(|(c, _)| c == comm) {
                let candidate = PilotAction::Trade {
                    direction: TradeDirection::Sell,
                    commodity: comm.clone(),
                    qty: sellable,
                };
                if validate_action(&candidate, pack, &ctx).is_ok() {
                    return candidate;
                }
            }
        }

        // 4c. If we have an active delivery quest to another port, depart for it
        // (after buying a small trade parcel if at home port with spare room & credits):
        let active_dest = input.quests.first().map(|q| match &q.target {
            QuestTarget::Delivery {
                destination_system,
                destination_planet,
                ..
            } => (destination_system.clone(), destination_planet.clone()),
            QuestTarget::Bounty { system, killed, .. } => {
                if *killed {
                    (q.giver_system.clone(), q.giver_planet.clone())
                } else {
                    (system.clone(), String::new())
                }
            }
        });

        // Buy preferred commodity at home port if we have empty non-quest cargo and spare credits:
        let non_quest_held: u32 = input
            .cargo
            .iter()
            .map(|(c, q)| q.saturating_sub(quest_hold.get(c).copied().unwrap_or(0)))
            .sum();
        if at_home && non_quest_held == 0 && ctx.cargo_used() < ctx.cargo_capacity {
            let buy_choice = pilot
                .preferred_commodities
                .iter()
                .find_map(|pref| input.market.iter().find(|(c, _)| c == pref))
                .or_else(|| input.market.iter().min_by_key(|(_, price)| *price));
            if let Some((comm, price)) = buy_choice
                && *price > 0
            {
                let spare_credits = input.credits.saturating_sub(100);
                let max_afford = spare_credits / *price;
                let room = (ctx.cargo_capacity - ctx.cargo_used()).min(4);
                let qty = max_afford.min(room);
                if qty > 0 {
                    let candidate = PilotAction::Trade {
                        direction: TradeDirection::Buy,
                        commodity: comm.clone(),
                        qty,
                    };
                    if validate_action(&candidate, pack, &ctx).is_ok() {
                        return candidate;
                    }
                }
            }
        }

        if let Some((dest_sys, dest_port)) = active_dest {
            if !dest_port.is_empty() && (&dest_sys, &dest_port) != (dock_sys, dock_port) {
                return PilotAction::Dock {
                    system: dest_sys,
                    location: dest_port,
                };
            } else if dest_port.is_empty() && &dest_sys != dock_sys {
                return PilotAction::FlyTo {
                    system: dest_sys,
                    location: None,
                };
            }
        }

        // Otherwise, travel to a linked system's first port or back to home port:
        if (dock_sys, dock_port) != (&pilot.home_system, &pilot.home_port) {
            return PilotAction::Dock {
                system: pilot.home_system.clone(),
                location: pilot.home_port.clone(),
            };
        }
        if let Some(sys) = pack.system(dock_sys)
            && let Some(next_sys_id) = sys.links.first()
            && let Some(next_sys) = pack.system(next_sys_id)
            && let Some(port) = next_sys
                .planets
                .first()
                .map(|p| p.id.clone())
                .or_else(|| next_sys.stations.first().map(|s| s.id.clone()))
        {
            return PilotAction::Dock {
                system: next_sys.id.clone(),
                location: port,
            };
        }
    }

    // 5. In space (undocked): if we have a quest destination, dock there; else dock at home port:
    if let Some((dest_sys, dest_port)) = input.quests.iter().find_map(|q| match &q.target {
        QuestTarget::Delivery {
            destination_system,
            destination_planet,
            ..
        } => Some((destination_system.clone(), destination_planet.clone())),
        QuestTarget::Bounty { killed: true, .. } => {
            Some((q.giver_system.clone(), q.giver_planet.clone()))
        }
        _ => None,
    }) {
        return PilotAction::Dock {
            system: dest_sys,
            location: dest_port,
        };
    }

    PilotAction::Dock {
        system: pilot.home_system.clone(),
        location: pilot.home_port.clone(),
    }
}

/// Deterministic fallback when a live model times out or fails validation twice:
/// decline/close blocking encounters or return to home port / continue orders.
pub fn fallback_action(pilot: &PilotDef, pack: &ContentPack, input: &DigestInput) -> PilotAction {
    let ctx = input.validation_context(pack);
    if let Some(enc) = &input.encounter {
        for safe in ["turn_in", "accept_quest", "decline", "refuse_tribute"] {
            if enc.options.iter().any(|o| o.id == safe) {
                let act = PilotAction::Respond {
                    choice: safe.to_owned(),
                };
                if validate_action(&act, pack, &ctx).is_ok() {
                    return act;
                }
            }
        }
        if let Some(first) = enc.options.first() {
            return PilotAction::Respond {
                choice: first.id.clone(),
            };
        }
    }
    if input.reason == DecisionPoint::HostileClose && input.docked.is_none() {
        return PilotAction::Flee {
            system: Some(pilot.home_system.clone()),
        };
    }
    if input.docked.is_none() {
        return PilotAction::Dock {
            system: pilot.home_system.clone(),
            location: pilot.home_port.clone(),
        };
    }
    PilotAction::Idle
}

fn quest_reserved_cargo(
    quests: &[sv_protocol::ActiveQuest],
) -> std::collections::BTreeMap<String, u32> {
    let mut map = std::collections::BTreeMap::new();
    for q in quests {
        if let QuestTarget::Delivery { commodity, qty, .. } = &q.target {
            *map.entry(commodity.clone()).or_insert(0) += *qty;
        }
    }
    map
}

pub struct DecisionWorker {
    tx: Mutex<Sender<DigestInput>>,
    rx: Mutex<Receiver<(DecisionPoint, PilotAction)>>,
    pub in_flight: bool,
    pub last_sent: Option<Instant>,
    pub min_interval: Duration,
}

impl DecisionWorker {
    pub fn spawn(
        mut brain: Box<dyn PilotBrain>,
        pilot: PilotDef,
        pack: ContentPack,
        min_interval: Duration,
    ) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<DigestInput>();
        let (res_tx, res_rx) = mpsc::channel::<(DecisionPoint, PilotAction)>();
        std::thread::spawn(move || {
            while let Ok(input) = req_rx.recv() {
                let reason = input.reason;
                let action = decide_once(brain.as_mut(), &pilot, &pack, &input);
                if res_tx.send((reason, action)).is_err() {
                    break;
                }
            }
        });
        Self {
            tx: Mutex::new(req_tx),
            rx: Mutex::new(res_rx),
            in_flight: false,
            last_sent: None,
            min_interval,
        }
    }

    pub fn can_request(&self) -> bool {
        if self.in_flight {
            return false;
        }
        match self.last_sent {
            Some(last) => last.elapsed() >= self.min_interval,
            None => true,
        }
    }

    pub fn request(&mut self, input: DigestInput) -> bool {
        if !self.can_request() {
            return false;
        }
        let sent = self
            .tx
            .lock()
            .map(|tx| tx.send(input).is_ok())
            .unwrap_or(false);
        if sent {
            self.in_flight = true;
            self.last_sent = Some(Instant::now());
            true
        } else {
            false
        }
    }

    pub fn try_recv(&mut self) -> Option<(DecisionPoint, PilotAction)> {
        let received = self.rx.lock().ok().and_then(|rx| rx.try_recv().ok());
        if let Some(pair) = received {
            self.in_flight = false;
            Some(pair)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use sv_protocol::{ChiefEncounter, ChiefMood, EncounterOption};

    fn test_pack_and_pilot() -> (ContentPack, PilotDef) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content");
        let pack = ContentPack::load_dir(&root).expect("pack loads");
        let pilot = pack.pilot("cautious-trader").expect("pilot exists").clone();
        (pack, pilot)
    }

    fn base_input() -> DigestInput {
        DigestInput {
            reason: DecisionPoint::EncounterOffered,
            ship_name: "Shuttle".into(),
            ship_id: "shuttle".into(),
            shields: 40.0,
            max_shields: 40.0,
            armor: 25.0,
            max_armor: 25.0,
            fuel: 100.0,
            max_fuel: 100.0,
            credits: 500,
            cargo: Vec::new(),
            cargo_capacity: 20,
            system: "sol".into(),
            docked: Some(("sol".into(), "earth".into())),
            market: vec![("food".into(), 8)],
            shipyard: vec![("shuttle".into(), 800, 20)],
            encounter: Some(ChiefEncounter {
                chief_name: "Port Director Lin".into(),
                portrait: None,
                mood: ChiefMood::Quest,
                text: "Take this contract.".into(),
                options: vec![
                    EncounterOption {
                        id: "accept_quest".into(),
                        label: "Accept".into(),
                    },
                    EncounterOption {
                        id: "decline".into(),
                        label: "Decline".into(),
                    },
                ],
            }),
            quests: Vec::new(),
            threats: Vec::new(),
        }
    }

    #[test]
    fn retries_once_with_error_feedback_then_succeeds() {
        let (pack, pilot) = test_pack_and_pilot();
        let input = base_input();
        let mut brain = MockBrain::with_scripted_json([
            r#"{"action":"respond","choice":"not_an_option"}"#,
            r#"{"action":"respond","choice":"accept_quest"}"#,
        ]);

        let action = decide_once(&mut brain, &pilot, &pack, &input);
        assert_eq!(
            action,
            PilotAction::Respond {
                choice: "accept_quest".into()
            }
        );
    }

    #[test]
    fn falls_back_after_two_invalid_responses() {
        let (pack, pilot) = test_pack_and_pilot();
        let input = base_input();
        let mut brain = MockBrain::with_scripted_json(["not json", "still not json"]);

        let action = decide_once(&mut brain, &pilot, &pack, &input);
        assert_eq!(
            action,
            PilotAction::Respond {
                choice: "accept_quest".into()
            }
        );
    }
}
