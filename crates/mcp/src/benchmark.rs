use std::f64::consts::PI;
use std::path::Path;

use evr_data::{ContentPack, ScenarioDef, ScenarioStep};
use evr_protocol::{PlanetTag, Position};
use evr_server::Content;
use evr_server::gameplay::EncounterRng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::harness::{Harness, TickSample};

const SETTLE_TICKS: u32 = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioReport {
    pub id: String,
    pub name: String,
    pub passed: bool,
    pub error: Option<String>,
    pub total_ticks: u64,
    pub assertions_passed: usize,
    pub avg_tick_us: u64,
    pub p50_tick_us: u64,
    pub p95_tick_us: u64,
    pub p99_tick_us: u64,
    pub max_tick_us: u64,
    pub server_avg_us: u64,
    pub client_avg_us: u64,
    pub peak_server_entities: usize,
    pub peak_client_entities: usize,
    pub peak_projectiles: usize,
    pub credits_delta: i64,
    pub fuel_consumed: f32,
    pub hull_damage_taken: f32,
    pub quests_completed: usize,
}

pub fn run_scenarios(
    content_root: &Path,
    filter_id: Option<&str>,
) -> Result<Vec<ScenarioReport>, String> {
    let pack = ContentPack::load_dir(content_root)
        .map_err(|err| format!("failed to load content pack: {err}"))?;
    let scenarios: Vec<&ScenarioDef> = match filter_id {
        Some(id) => {
            let def = pack
                .scenario(id)
                .ok_or_else(|| format!("unknown scenario '{id}'"))?;
            vec![def]
        }
        None => pack.scenarios.iter().collect(),
    };
    Ok(scenarios
        .into_iter()
        .map(|def| run_scenario(content_root, def))
        .collect())
}

pub fn run_scenario(content_root: &Path, def: &ScenarioDef) -> ScenarioReport {
    let mut harness = Harness::new(content_root);
    if let Some(seed) = def.encounter_seed {
        harness
            .server
            .insert_resource(EncounterRng::with_roll(seed as u32));
    }

    let initial = harness.observe();
    let initial_credits = initial["ship"]["credits"].as_i64().unwrap_or(0);
    let mut prev_fuel = initial["ship"]["fuel"].as_f64().unwrap_or(0.0) as f32;
    let mut prev_hull = (initial["ship"]["shields"].as_f64().unwrap_or(0.0)
        + initial["ship"]["armor"].as_f64().unwrap_or(0.0)) as f32;
    let mut fuel_consumed = 0.0_f32;
    let mut hull_damage_taken = 0.0_f32;
    let mut assertions_passed = 0_usize;
    let mut error: Option<String> = None;

    for (idx, step) in def.steps.iter().enumerate() {
        if harness.ticks > def.max_ticks {
            error = Some(format!(
                "step {}: exceeded max_ticks ({} > {})",
                idx + 1,
                harness.ticks,
                def.max_ticks
            ));
            break;
        }
        let remaining_budget = def.max_ticks.saturating_sub(harness.ticks) as u32;
        if let Err(msg) = execute_step(
            &mut harness,
            step,
            remaining_budget,
            &mut assertions_passed,
            &mut prev_fuel,
            &mut prev_hull,
            &mut fuel_consumed,
            &mut hull_damage_taken,
        ) {
            error = Some(format!("step {} ({step:?}): {msg}", idx + 1));
            break;
        }
    }

    let final_state = harness.observe();
    update_resource_deltas(
        &final_state,
        &mut prev_fuel,
        &mut prev_hull,
        &mut fuel_consumed,
        &mut hull_damage_taken,
    );
    let final_credits = final_state["ship"]["credits"]
        .as_i64()
        .unwrap_or(initial_credits);
    let quests_completed = final_state["completed_quests"]
        .as_array()
        .map_or(0, Vec::len);

    let stats = compute_sample_stats(&harness.samples);

    ScenarioReport {
        id: def.id.clone(),
        name: def.name.clone(),
        passed: error.is_none(),
        error,
        total_ticks: harness.ticks,
        assertions_passed,
        avg_tick_us: stats.avg_us,
        p50_tick_us: stats.p50_us,
        p95_tick_us: stats.p95_us,
        p99_tick_us: stats.p99_us,
        max_tick_us: stats.max_us,
        server_avg_us: stats.server_avg_us,
        client_avg_us: stats.client_avg_us,
        peak_server_entities: stats.peak_server_entities,
        peak_client_entities: stats.peak_client_entities,
        peak_projectiles: stats.peak_projectiles,
        credits_delta: final_credits - initial_credits,
        fuel_consumed,
        hull_damage_taken,
        quests_completed,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_step(
    harness: &mut Harness,
    step: &ScenarioStep,
    remaining_budget: u32,
    assertions_passed: &mut usize,
    prev_fuel: &mut f32,
    prev_hull: &mut f32,
    fuel_consumed: &mut f32,
    hull_damage_taken: &mut f32,
) -> Result<(), String> {
    match step {
        ScenarioStep::SetFlight {
            throttle,
            turn,
            fire,
        } => {
            harness.set_flight(*throttle, *turn, *fire);
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::SetWarp(on) => {
            harness.set_warp(*on);
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::Advance { ticks } => {
            advance_tracking(
                harness,
                *ticks,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::FlyToPlanet {
            system,
            planet,
            land,
        } => {
            let target = planet_world_pos(harness, system, planet)
                .ok_or_else(|| format!("planet {system}/{planet} not found in server world"))?;
            fly_to_point(
                harness,
                target,
                240.0,
                remaining_budget,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            )?;
            if *land {
                harness.land(planet);
                advance_tracking(
                    harness,
                    SETTLE_TICKS,
                    prev_fuel,
                    prev_hull,
                    fuel_consumed,
                    hull_damage_taken,
                );
            }
            Ok(())
        }
        ScenarioStep::FlyToSystem { system } => {
            let target = system_world_pos(harness, system)
                .ok_or_else(|| format!("system {system} not found in content"))?;
            fly_to_point(
                harness,
                target,
                350.0,
                remaining_budget,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            )
        }
        ScenarioStep::Land { planet } => {
            harness.land(planet);
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::LiftOff => {
            harness.lift_off();
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::Trade {
            action,
            commodity,
            qty,
        } => {
            match action.as_str() {
                "buy" => harness.buy(commodity, *qty),
                "sell" => harness.sell(commodity, *qty),
                other => return Err(format!("invalid trade action '{other}'")),
            }
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::Respond { choice } => {
            harness.respond(choice);
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::AssertAlive(expected) => {
            let state = harness.observe();
            let actual = state["alive"].as_bool().unwrap_or(false);
            if actual != *expected {
                return Err(format!("expected alive={expected}, got {actual}"));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertSystem(expected) => {
            let state = harness.observe();
            let actual = state["system"].as_str().unwrap_or("");
            if actual != expected {
                return Err(format!("expected system='{expected}', got '{actual}'"));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertDocked(expected) => {
            let state = harness.observe();
            let actual = state["docked"].as_str().map(str::to_owned);
            if &actual != expected {
                return Err(format!("expected docked={expected:?}, got {actual:?}"));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertCredits { min, max } => {
            let state = harness.observe();
            let actual = state["ship"]["credits"].as_u64().unwrap_or(0) as u32;
            if let Some(min_val) = min
                && actual < *min_val
            {
                return Err(format!("expected credits >= {min_val}, got {actual}"));
            }
            if let Some(max_val) = max
                && actual > *max_val
            {
                return Err(format!("expected credits <= {max_val}, got {actual}"));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertCargo { commodity, min_qty } => {
            let state = harness.observe();
            let actual = state["ship"]["cargo"][commodity.as_str()]
                .as_u64()
                .unwrap_or(0) as u32;
            if actual < *min_qty {
                return Err(format!(
                    "expected cargo '{commodity}' >= {min_qty}, got {actual}"
                ));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertEncounter { mood } => {
            let state = harness.observe();
            if state["encounter"].is_null() {
                return Err("expected active chief encounter, found null".to_owned());
            }
            if let Some(expected_mood) = mood {
                let actual_mood = state["encounter"]["mood"].as_str().unwrap_or("");
                if actual_mood != expected_mood {
                    return Err(format!(
                        "expected encounter mood '{expected_mood}', got '{actual_mood}'"
                    ));
                }
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertActiveQuests { count } => {
            let state = harness.observe();
            let actual = state["quests"].as_array().map_or(0, Vec::len);
            if actual != *count {
                return Err(format!("expected {count} active quests, got {actual}"));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertCompletedQuest(quest_id) => {
            let state = harness.observe();
            let completed = state["completed_quests"]
                .as_array()
                .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(quest_id.as_str())));
            if !completed {
                return Err(format!("expected quest '{quest_id}' to be completed"));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::ClearMeteorites => {
            let state = harness.observe();
            let sys = state["system"].as_str().unwrap_or("sol").to_owned();
            let entities: Vec<bevy::prelude::Entity> = harness
                .server
                .world_mut()
                .query_filtered::<(bevy::prelude::Entity, &evr_protocol::SystemMembership), bevy::prelude::With<evr_protocol::Meteorite>>()
                .iter(harness.server.world())
                .filter(|(_, m)| m.0 == sys)
                .map(|(e, _)| e)
                .collect();
            for entity in entities {
                harness.server.world_mut().despawn(entity);
            }
            advance_tracking(
                harness,
                2,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::SpawnMeteorite {
            size,
            offset,
            velocity,
        } => {
            let parsed_size = match size.as_str() {
                "large" => evr_protocol::MeteoriteSize::Large,
                "medium" => evr_protocol::MeteoriteSize::Medium,
                "small" => evr_protocol::MeteoriteSize::Small,
                other => return Err(format!("unknown meteorite size '{other}'")),
            };
            let state = harness.observe();
            let sys = state["system"].as_str().unwrap_or("sol").to_owned();
            let pos_arr = state["ship"]["position"]
                .as_array()
                .ok_or_else(|| "missing ship.position".to_owned())?;
            let ship_pos = bevy::math::Vec2::new(
                pos_arr[0].as_f64().unwrap_or(0.0) as f32,
                pos_arr[1].as_f64().unwrap_or(0.0) as f32,
            );
            let spawn_pos = ship_pos + bevy::math::Vec2::new(offset.0, offset.1);
            let vel = bevy::math::Vec2::new(velocity.0, velocity.1);
            let spin = if vel.length_squared() > 0.0 { 1.4 } else { 0.0 };
            let mut cmds = harness.server.world_mut().commands();
            evr_server::gameplay::spawn_meteorite(
                &mut cmds,
                &sys,
                ship_pos,
                1200.0,
                spawn_pos,
                vel,
                parsed_size,
                spin,
                0,
            );
            harness.server.world_mut().flush();
            advance_tracking(
                harness,
                2,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::DestroyNearestMeteorite => {
            let state = harness.observe();
            let sys = state["system"].as_str().unwrap_or("sol").to_owned();
            let pos_arr = state["ship"]["position"]
                .as_array()
                .ok_or_else(|| "missing ship.position".to_owned())?;
            let ship_pos = bevy::math::Vec2::new(
                pos_arr[0].as_f64().unwrap_or(0.0) as f32,
                pos_arr[1].as_f64().unwrap_or(0.0) as f32,
            );
            let target_entity = {
                let mut q = harness.server.world_mut().query::<(
                    bevy::prelude::Entity,
                    &Position,
                    &evr_protocol::SystemMembership,
                    &evr_protocol::Meteorite,
                )>();
                let world = harness.server.world();
                q.iter(world)
                    .filter(|(_, _, m, _)| m.0 == sys)
                    .min_by_key(|(_, pos, _, met)| {
                        let tier_rank = match met.size {
                            evr_protocol::MeteoriteSize::Large => 0_u8,
                            evr_protocol::MeteoriteSize::Medium => 1_u8,
                            evr_protocol::MeteoriteSize::Small => 2_u8,
                        };
                        (tier_rank, pos.0.distance(ship_pos).to_bits())
                    })
                    .map(|(e, _, _, _)| e)
                    .ok_or_else(|| "no meteorite found in current system".to_owned())?
            };
            let heading = state["ship"]["heading"].as_f64().unwrap_or(0.0) as f32;
            let forward = bevy::math::Vec2::new(heading.cos(), heading.sin());
            if let Some(mut pos) = harness
                .server
                .world_mut()
                .get_mut::<Position>(target_entity)
            {
                pos.0 = ship_pos + forward * 85.0;
            }
            if let Some(mut motion) =
                harness
                    .server
                    .world_mut()
                    .get_mut::<evr_server::gameplay::MeteoriteMotion>(target_entity)
            {
                motion.velocity = bevy::math::Vec2::ZERO;
            }
            harness.set_flight(0.0, 0.0, true);
            let max_steps = remaining_budget.max(1);
            let mut destroyed = false;
            for _ in 0..max_steps {
                advance_tracking(
                    harness,
                    1,
                    prev_fuel,
                    prev_hull,
                    fuel_consumed,
                    hull_damage_taken,
                );
                if harness.server.world().get_entity(target_entity).is_err() {
                    destroyed = true;
                    break;
                }
            }
            harness.set_flight(0.0, 0.0, false);
            let projs: Vec<bevy::prelude::Entity> = harness
                .server
                .world_mut()
                .query_filtered::<bevy::prelude::Entity, bevy::prelude::With<evr_protocol::Projectile>>()
                .iter(harness.server.world())
                .collect();
            for p in projs {
                harness.server.world_mut().despawn(p);
            }
            if !destroyed {
                return Err("timed out firing at nearest meteorite".to_owned());
            }
            Ok(())
        }
        ScenarioStep::AssertMeteorites { count, size } => {
            let state = harness.observe();
            let arr = state["meteorites"]
                .as_array()
                .ok_or_else(|| "missing meteorites array in observe()".to_owned())?;
            let actual = match size {
                Some(s) => arr
                    .iter()
                    .filter(|m| m["size"].as_str() == Some(s.as_str()))
                    .count(),
                None => arr.len(),
            };
            if actual != *count {
                return Err(format!(
                    "expected {count} meteorites (size={size:?}), got {actual}"
                ));
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::AssertNpcKindsPresent { kinds } => {
            let state = harness.observe();
            let arr = state["ships"]
                .as_array()
                .ok_or_else(|| "missing ships array in observe()".to_owned())?;
            for expected_kind in kinds {
                let present = arr
                    .iter()
                    .any(|s| s["kind"].as_str() == Some(expected_kind.as_str()));
                if !present {
                    return Err(format!(
                        "expected NPC ship of kind '{expected_kind}' in world"
                    ));
                }
            }
            *assertions_passed += 1;
            Ok(())
        }
        ScenarioStep::HailNearestShip { kind } => {
            let state = harness.observe();
            let sys = state["system"].as_str().unwrap_or("sol").to_owned();
            let pos_arr = state["ship"]["position"]
                .as_array()
                .ok_or_else(|| "missing ship.position".to_owned())?;
            let ship_pos = bevy::math::Vec2::new(
                pos_arr[0].as_f64().unwrap_or(0.0) as f32,
                pos_arr[1].as_f64().unwrap_or(0.0) as f32,
            );

            let (target_entity, target_pos) = {
                let mut q = harness.server.world_mut().query::<(
                    bevy::prelude::Entity,
                    &evr_protocol::ShipInfo,
                    &Position,
                    &evr_protocol::SystemMembership,
                    Option<&evr_server::gameplay::Aggro>,
                )>();
                let world = harness.server.world();
                q.iter(world)
                    .filter(|(_, info, _, _, aggro)| {
                        aggro.is_none()
                            && info.kind != evr_protocol::ShipKind::Player
                            && info.kind != evr_protocol::ShipKind::Pirate
                            && kind
                                .as_ref()
                                .is_none_or(|k| info.kind.as_str() == k.as_str())
                    })
                    .min_by_key(|(_, _, p, m, _)| {
                        let same_sys_rank = if m.0 == sys { 0_u8 } else { 1_u8 };
                        (same_sys_rank, p.0.distance(ship_pos).to_bits())
                    })
                    .map(|(e, _, p, _, _)| (e, p.0))
                    .ok_or_else(|| format!("no friendly NPC ship (kind={kind:?}) in {sys}"))?
            };

            fly_to_point(
                harness,
                [f64::from(target_pos.x), f64::from(target_pos.y)],
                180.0,
                remaining_budget,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            )?;

            // Match position alongside target_entity within HAIL_RANGE before triggering HailShip:
            let refreshed_npc_pos = harness
                .server
                .world()
                .get::<Position>(target_entity)
                .map(|p| p.0);
            if let Some(npc_pos) = refreshed_npc_pos {
                let mut q = harness
                    .server
                    .world_mut()
                    .query::<(&evr_protocol::ShipInfo, &mut evr_server::gameplay::Ship)>();
                let world = harness.server.world_mut();
                for (info, mut s) in q.iter_mut(world) {
                    if info.owner == Some(evr_server::EMBEDDED_CLIENT_ID as u64) {
                        s.0.position = npc_pos - bevy::math::Vec2::new(60.0, 0.0);
                        s.0.velocity = bevy::math::Vec2::ZERO;
                    }
                }
            }
            advance_tracking(
                harness,
                1,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            harness.hail();
            advance_tracking(
                harness,
                SETTLE_TICKS,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            Ok(())
        }
        ScenarioStep::ProvokeNearestNpc { kind } => {
            let state = harness.observe();
            let sys = state["system"].as_str().unwrap_or("sol").to_owned();
            let pos_arr = state["ship"]["position"]
                .as_array()
                .ok_or_else(|| "missing ship.position".to_owned())?;
            let ship_pos = bevy::math::Vec2::new(
                pos_arr[0].as_f64().unwrap_or(0.0) as f32,
                pos_arr[1].as_f64().unwrap_or(0.0) as f32,
            );

            let (npc_entity, npc_pos) = {
                let mut q = harness.server.world_mut().query::<(
                    bevy::prelude::Entity,
                    &evr_protocol::ShipInfo,
                    &Position,
                    &evr_protocol::SystemMembership,
                )>();
                let world = harness.server.world();
                q.iter(world)
                    .filter(|(_, info, _, m)| {
                        m.0 == sys
                            && info.kind != evr_protocol::ShipKind::Player
                            && kind
                                .as_ref()
                                .is_none_or(|k| info.kind.as_str() == k.as_str())
                    })
                    .min_by_key(|(_, _, p, _)| p.0.distance(ship_pos).to_bits())
                    .map(|(e, _, p, _)| (e, p.0))
                    .ok_or_else(|| format!("no NPC ship (kind={kind:?}) in {sys}"))?
            };

            {
                let mut q = harness
                    .server
                    .world_mut()
                    .query::<(&evr_protocol::ShipInfo, &mut evr_server::gameplay::Ship)>();
                let world = harness.server.world_mut();
                for (info, mut s) in q.iter_mut(world) {
                    if info.owner == Some(evr_server::EMBEDDED_CLIENT_ID as u64) {
                        s.0.position = npc_pos - bevy::math::Vec2::new(75.0, 0.0);
                        s.0.rotation = 0.0;
                        s.0.velocity = bevy::math::Vec2::ZERO;
                    }
                }
            }
            let damage_before = *hull_damage_taken;
            harness.set_flight(0.0, 0.0, true);
            let mut retaliated = false;
            for _ in 0..remaining_budget.min(200) {
                advance_tracking(
                    harness,
                    1,
                    prev_fuel,
                    prev_hull,
                    fuel_consumed,
                    hull_damage_taken,
                );
                let aggroed = harness
                    .server
                    .world()
                    .get::<evr_server::gameplay::Aggro>(npc_entity)
                    .is_some();
                if aggroed && *hull_damage_taken > damage_before {
                    retaliated = true;
                    break;
                }
            }
            harness.set_flight(0.0, 0.0, false);
            if !retaliated {
                return Err("NPC did not retaliate after being fired upon".to_owned());
            }
            *assertions_passed += 1;
            Ok(())
        }
    }
}

fn advance_tracking(
    harness: &mut Harness,
    ticks: u32,
    prev_fuel: &mut f32,
    prev_hull: &mut f32,
    fuel_consumed: &mut f32,
    hull_damage_taken: &mut f32,
) {
    harness.advance(ticks);
    let state = harness.observe();
    update_resource_deltas(
        &state,
        prev_fuel,
        prev_hull,
        fuel_consumed,
        hull_damage_taken,
    );
}

fn update_resource_deltas(
    state: &Value,
    prev_fuel: &mut f32,
    prev_hull: &mut f32,
    fuel_consumed: &mut f32,
    hull_damage_taken: &mut f32,
) {
    if state["alive"].as_bool() == Some(true) {
        let fuel = state["ship"]["fuel"].as_f64().unwrap_or(0.0) as f32;
        if fuel < *prev_fuel {
            *fuel_consumed += *prev_fuel - fuel;
        }
        *prev_fuel = fuel;

        let hull = (state["ship"]["shields"].as_f64().unwrap_or(0.0)
            + state["ship"]["armor"].as_f64().unwrap_or(0.0)) as f32;
        if hull < *prev_hull {
            *hull_damage_taken += *prev_hull - hull;
        }
        *prev_hull = hull;
    }
}

fn planet_world_pos(harness: &mut Harness, system_id: &str, planet_id: &str) -> Option<[f64; 2]> {
    let mut query = harness
        .server
        .world_mut()
        .query::<(&PlanetTag, &Position)>();
    let world = harness.server.world();
    query
        .iter(world)
        .find(|(tag, _)| tag.system == system_id && tag.id == planet_id)
        .map(|(_, pos)| [f64::from(pos.0.x), f64::from(pos.0.y)])
}

fn system_world_pos(harness: &Harness, system_id: &str) -> Option<[f64; 2]> {
    let content = harness.server.world().resource::<Content>();
    content
        .0
        .system(system_id)
        .map(|s| [f64::from(s.position.0), f64::from(s.position.1)])
}

#[allow(clippy::too_many_arguments)]
fn fly_to_point(
    harness: &mut Harness,
    target: [f64; 2],
    stop_distance: f64,
    max_ticks: u32,
    prev_fuel: &mut f32,
    prev_hull: &mut f32,
    fuel_consumed: &mut f32,
    hull_damage_taken: &mut f32,
) -> Result<(), String> {
    let step_chunk = 10_u32;
    let iterations = (max_ticks / step_chunk).max(1);
    for _ in 0..iterations {
        let state = harness.observe();
        if state["alive"].as_bool() != Some(true) {
            return Err("ship was destroyed while flying to target".to_owned());
        }
        let pos_arr = state["ship"]["position"]
            .as_array()
            .ok_or_else(|| "missing ship.position".to_owned())?;
        let position = [
            pos_arr[0].as_f64().unwrap_or(0.0),
            pos_arr[1].as_f64().unwrap_or(0.0),
        ];
        let distance = (position[0] - target[0]).hypot(position[1] - target[1]);
        if distance <= stop_distance {
            harness.set_flight(0.0, 0.0, false);
            harness.set_warp(false);
            advance_tracking(
                harness,
                5,
                prev_fuel,
                prev_hull,
                fuel_consumed,
                hull_damage_taken,
            );
            return Ok(());
        }
        let heading = state["ship"]["heading"].as_f64().unwrap_or(0.0);
        let dx = target[0] - position[0];
        let dy = target[1] - position[1];
        let desired = dy.atan2(dx);
        let diff = (desired - heading + PI).rem_euclid(2.0 * PI) - PI;
        let turn = diff.signum() as f32;
        let throttle = if diff.abs() < 0.6 { 1.0 } else { 0.2 };
        let use_warp = distance > 450.0 && diff.abs() < 0.45;
        harness.set_warp(use_warp);
        harness.set_flight(throttle, turn, false);
        advance_tracking(
            harness,
            step_chunk,
            prev_fuel,
            prev_hull,
            fuel_consumed,
            hull_damage_taken,
        );
    }
    harness.set_flight(0.0, 0.0, false);
    harness.set_warp(false);
    Err(format!(
        "timed out flying to ({:.0}, {:.0}) within {max_ticks} ticks",
        target[0], target[1]
    ))
}

#[derive(Default)]
struct SampleStats {
    avg_us: u64,
    p50_us: u64,
    p95_us: u64,
    p99_us: u64,
    max_us: u64,
    server_avg_us: u64,
    client_avg_us: u64,
    peak_server_entities: usize,
    peak_client_entities: usize,
    peak_projectiles: usize,
}

fn compute_sample_stats(samples: &[TickSample]) -> SampleStats {
    if samples.is_empty() {
        return SampleStats::default();
    }
    let n = samples.len();
    let mut totals: Vec<u64> = samples.iter().map(|s| s.total_us).collect();
    totals.sort_unstable();

    let sum_total: u64 = totals.iter().sum();
    let sum_server: u64 = samples.iter().map(|s| s.server_us).sum();
    let sum_client: u64 = samples.iter().map(|s| s.client_us).sum();

    let percentile = |p: usize| -> u64 {
        let idx = ((n - 1) * p) / 100;
        totals[idx]
    };

    SampleStats {
        avg_us: sum_total / n as u64,
        p50_us: percentile(50),
        p95_us: percentile(95),
        p99_us: percentile(99),
        max_us: *totals.last().unwrap_or(&0),
        server_avg_us: sum_server / n as u64,
        client_avg_us: sum_client / n as u64,
        peak_server_entities: samples.iter().map(|s| s.server_entities).max().unwrap_or(0),
        peak_client_entities: samples.iter().map(|s| s.client_entities).max().unwrap_or(0),
        peak_projectiles: samples.iter().map(|s| s.projectiles).max().unwrap_or(0),
    }
}

pub fn format_markdown_table(reports: &[ScenarioReport]) -> String {
    format_markdown_table_with_baseline(reports, None)
}

pub fn format_markdown_table_with_baseline(
    reports: &[ScenarioReport],
    baseline: Option<&[ScenarioReport]>,
) -> String {
    let mut out = String::new();
    out.push_str(
        "| Scenario | Status | Ticks | Avg (us) | p50 (us) | p95 (us) | p99 (us) | Max (us) | Entities (S/C/Proj) | Credits | Fuel | Hull Dmg |\n",
    );
    out.push_str("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for r in reports {
        let prev = baseline.and_then(|b| b.iter().find(|p| p.id == r.id));
        let status = match (r.passed, baseline.is_some(), prev.is_some()) {
            (true, true, false) => "PASS (new)",
            (true, _, _) => "PASS",
            (false, true, false) => "FAIL (new)",
            (false, _, _) => "FAIL",
        };
        let ticks_cell = match prev {
            Some(p) => format_i64_delta(r.total_ticks, p.total_ticks),
            None => r.total_ticks.to_string(),
        };
        let avg_cell = match prev {
            Some(p) => format_pct_delta(r.avg_tick_us, p.avg_tick_us),
            None => r.avg_tick_us.to_string(),
        };
        let p50_cell = match prev {
            Some(p) => format_pct_delta(r.p50_tick_us, p.p50_tick_us),
            None => r.p50_tick_us.to_string(),
        };
        let p95_cell = match prev {
            Some(p) => format_pct_delta(r.p95_tick_us, p.p95_tick_us),
            None => r.p95_tick_us.to_string(),
        };
        let p99_cell = match prev {
            Some(p) => format_pct_delta(r.p99_tick_us, p.p99_tick_us),
            None => r.p99_tick_us.to_string(),
        };
        let max_cell = match prev {
            Some(p) => format_pct_delta(r.max_tick_us, p.max_tick_us),
            None => r.max_tick_us.to_string(),
        };

        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {}/{}/{} | {:+} | {:.1} | {:.1} |\n",
            r.id,
            status,
            ticks_cell,
            avg_cell,
            p50_cell,
            p95_cell,
            p99_cell,
            max_cell,
            r.peak_server_entities,
            r.peak_client_entities,
            r.peak_projectiles,
            r.credits_delta,
            r.fuel_consumed,
            r.hull_damage_taken,
        ));
        if let Some(err) = &r.error {
            out.push_str(&format!("  > Error in `{}`: {}\n", r.id, err));
        }
    }
    out
}

fn format_pct_delta(current: u64, previous: u64) -> String {
    if previous == 0 {
        return current.to_string();
    }
    let pct = ((current as f64 - previous as f64) / previous as f64) * 100.0;
    if pct.abs() < 0.05 {
        format!("{current} (0.0%)")
    } else {
        format!("{current} ({pct:+.1}%)")
    }
}

fn format_i64_delta(current: u64, previous: u64) -> String {
    let diff = current as i64 - previous as i64;
    if diff == 0 {
        format!("{current} (0)")
    } else {
        format!("{current} ({diff:+})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_table_with_baseline_includes_percentage_variations() {
        let prev = vec![ScenarioReport {
            id: "sol-centauri-trade".to_owned(),
            name: "Trade".to_owned(),
            passed: true,
            error: None,
            total_ticks: 600,
            assertions_passed: 5,
            avg_tick_us: 100,
            p50_tick_us: 100,
            p95_tick_us: 200,
            p99_tick_us: 300,
            max_tick_us: 400,
            server_avg_us: 60,
            client_avg_us: 40,
            peak_server_entities: 10,
            peak_client_entities: 10,
            peak_projectiles: 0,
            credits_delta: 44,
            fuel_consumed: 45.0,
            hull_damage_taken: 0.0,
            quests_completed: 0,
        }];
        let mut curr = prev.clone();
        curr[0].total_ticks = 615;
        curr[0].avg_tick_us = 95;
        curr[0].p95_tick_us = 210;

        let table = format_markdown_table_with_baseline(&curr, Some(&prev));
        assert!(table.contains("615 (+15)"));
        assert!(table.contains("95 (-5.0%)"));
        assert!(table.contains("100 (0.0%)"));
        assert!(table.contains("210 (+5.0%)"));
    }
}
