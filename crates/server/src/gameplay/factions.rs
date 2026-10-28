use std::collections::{BTreeMap, BTreeSet};

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy_replicon::prelude::*;
use sv_data::{ContentPack, FactionSide};
use sv_protocol::{Notice, PlayerReputation, ShipKind, SystemControl, SystemMembership};

use super::{
    Aggro, ClientRef, LastDamagedBy, Npc, Owner, POLICE_SHIP, PlayerShip, Ship, StorageResource,
    notice, spawn_ship,
};
use crate::Content;

pub const CONTROL_MAX: i32 = 100;
pub const CONTROL_FLIP: i32 = 50;
pub const CONTROL_INIT_WAR: i32 = 60;
pub const CONTROL_QUEST_COMPLETE: i32 = 10;
pub const CONTROL_PIRATE_KILL: i32 = 2;
pub const CONTROL_PATROL_KILL: i32 = 5;
pub const REP_MAX: i32 = 100;
pub const REP_HOSTILE: i32 = -50;
pub const REP_QUEST_COMPLETE: i32 = 5;
pub const REP_TRIBUTE_PAID: i32 = 3;
pub const REP_PIRATE_KILL: i32 = 2;
pub const REP_PATROL_KILL: i32 = -10;
pub const REP_CIVILIAN_KILL: i32 = -5;
pub const REP_FINE: i32 = -2;
pub const DISPATCH_PERIOD_SECS: f32 = 7.0 * 86_400.0;
const DISPATCH_WATCHPOINTS: usize = 3;

pub fn side_sign(pack: &ContentPack, faction: &str) -> i32 {
    match pack.faction(faction).map(|def| def.side) {
        Some(FactionSide::Confederation) => 1,
        Some(FactionSide::Rebellion) => -1,
        _ => 0,
    }
}

pub fn war_faction(pack: &ContentPack, side: FactionSide) -> Option<String> {
    pack.factions
        .iter()
        .find(|def| def.side == side)
        .map(|def| def.id.clone())
}

pub fn controller_for(pack: &ContentPack, system: &str, meter: i32) -> String {
    if meter >= CONTROL_FLIP {
        war_faction(pack, FactionSide::Confederation)
            .unwrap_or_else(|| base_government(pack, system))
    } else if meter <= -CONTROL_FLIP {
        war_faction(pack, FactionSide::Rebellion).unwrap_or_else(|| base_government(pack, system))
    } else {
        base_government(pack, system)
    }
}

fn base_government(pack: &ContentPack, system: &str) -> String {
    pack.system(system)
        .map(|def| def.government.clone())
        .unwrap_or_else(|| "independent".to_owned())
}

pub fn system_center(pack: &ContentPack, system: &str) -> Vec2 {
    pack.system(system)
        .map(|def| Vec2::new(def.position.0, def.position.1))
        .unwrap_or_default()
}

pub fn spawn_patrol(
    commands: &mut Commands,
    pack: &ContentPack,
    system: &str,
    offset: Vec2,
    aggro_owner: u64,
) -> Entity {
    let police = spawn_ship(
        commands,
        pack,
        POLICE_SHIP,
        ShipKind::Police,
        None,
        system,
        offset,
    );
    commands.entity(police).insert((Npc, Aggro(aggro_owner)));
    police
}

pub fn apply_rep(standings: &mut Vec<(String, i32)>, faction: &str, delta: i32) {
    match standings.iter_mut().find(|(id, _)| id == faction) {
        Some((_, value)) => *value = (*value + delta).clamp(-REP_MAX, REP_MAX),
        None => standings.push((faction.to_owned(), delta.clamp(-REP_MAX, REP_MAX))),
    }
}

#[derive(Resource, Debug, Default)]
pub struct FactionState {
    pub meters: BTreeMap<String, i32>,
    pub controllers: BTreeMap<String, String>,
    pub news: Vec<String>,
    pub dispatch_accum: f32,
    pub last_dispatch: Option<String>,
    dirty: BTreeSet<String>,
}

impl FactionState {
    pub fn meter(&self, system: &str) -> i32 {
        self.meters.get(system).copied().unwrap_or(0)
    }

    pub fn controller(&self, system: &str) -> &str {
        self.controllers
            .get(system)
            .map(String::as_str)
            .unwrap_or("independent")
    }

    pub fn take_dirty(&mut self) -> BTreeSet<String> {
        std::mem::take(&mut self.dirty)
    }

    pub fn shift_meter(&mut self, pack: &ContentPack, system: &str, delta: i32) {
        let Some(base) = self.meters.get_mut(system) else {
            return;
        };
        *base = (*base + delta).clamp(-CONTROL_MAX, CONTROL_MAX);
        let meter = *base;
        let controller = controller_for(pack, system, meter);
        self.dirty.insert(system.to_owned());
        if self.controllers.get(system).map(String::as_str) != Some(controller.as_str()) {
            self.controllers
                .insert(system.to_owned(), controller.clone());
            self.news.push(format!(
                "{} now holds {}.",
                faction_name(pack, &controller),
                system_name(pack, system)
            ));
        }
    }
}

pub fn init_faction_state(storage: &super::StorageResource, pack: &ContentPack) -> FactionState {
    let mut state = FactionState::default();
    for system in &pack.systems {
        let meter = CONTROL_INIT_WAR * side_sign(pack, &system.government);
        let controller = controller_for(pack, &system.id, meter);
        state.meters.insert(system.id.clone(), meter);
        state.controllers.insert(system.id.clone(), controller);
    }
    if let Some(db) = storage.storage.as_ref()
        && let Ok(rows) = db.load_control()
    {
        for row in rows {
            if let Some(base) = state.meters.get_mut(&row.system) {
                *base = row.meter.clamp(-CONTROL_MAX, CONTROL_MAX);
                let controller = controller_for(pack, &row.system, *base);
                state.controllers.insert(row.system.clone(), controller);
            }
        }
    }
    state
}

pub fn compose_dispatch(pack: &ContentPack, state: &FactionState) -> String {
    let mut holdings: BTreeMap<&str, usize> = BTreeMap::new();
    for controller in state.controllers.values() {
        *holdings.entry(controller.as_str()).or_default() += 1;
    }
    let standings = holdings
        .iter()
        .map(|(faction, count)| {
            format!(
                "{} holds {}",
                pack.faction(faction)
                    .map(|def| def.name.as_str())
                    .unwrap_or(*faction),
                count
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let mut watchpoints: Vec<(String, i32)> = state
        .meters
        .iter()
        .filter(|(system, meter)| {
            let base = CONTROL_INIT_WAR
                * side_sign(
                    pack,
                    pack.system(system)
                        .map(|def| def.government.as_str())
                        .unwrap_or("independent"),
                );
            (**meter - base).abs() >= CONTROL_PATROL_KILL
        })
        .map(|(system, meter)| (system_name(pack, system).to_owned(), *meter))
        .collect();
    watchpoints.sort_by_key(|(name, _)| name.clone());
    watchpoints.sort_by_key(|(_, meter)| meter.abs());
    let front = if watchpoints.is_empty() {
        "The front lines hold.".to_owned()
    } else {
        let listed = watchpoints
            .into_iter()
            .take(DISPATCH_WATCHPOINTS)
            .map(|(name, meter)| format!("{name} at {meter:+}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Watchpoints: {listed}.")
    };
    format!("Weekly dispatch: {standings}. {front}")
}

pub fn sync_system_control(
    mut state: ResMut<FactionState>,
    storage: Res<StorageResource>,
    mut controls: Query<&mut SystemControl>,
) {
    for mut control in &mut controls {
        let Some(meter) = state.meters.get(&control.system).copied() else {
            continue;
        };
        let controller = state.controller(&control.system).to_owned();
        if control.meter != meter || control.controller != controller {
            control.meter = meter;
            control.controller = controller;
        }
    }
    let dirty = state.take_dirty();
    let Some(db) = storage.storage.as_ref() else {
        return;
    };
    if !dirty.is_empty() {
        let rows = dirty
            .iter()
            .map(|system| crate::storage::ControlRow {
                system: system.clone(),
                meter: state.meter(system),
            })
            .collect::<Vec<_>>();
        if let Err(error) = db.save_control(&rows) {
            warn!("server: control save failed: {error}");
        }
    }
}

pub fn flush_faction_news(mut commands: Commands, mut state: ResMut<FactionState>) {
    for text in state.news.drain(..) {
        info!("server: faction news: {text}");
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: Notice { text },
        });
    }
}

pub fn dispatch_tick(
    time: Res<Time>,
    content: Res<Content>,
    mut state: ResMut<FactionState>,
    mut commands: Commands,
) {
    state.dispatch_accum += time.delta_secs();
    if state.dispatch_accum < DISPATCH_PERIOD_SECS {
        return;
    }
    state.dispatch_accum = 0.0;
    broadcast_dispatch(&content.0, &mut state, &mut commands);
}

fn broadcast_dispatch(pack: &ContentPack, state: &mut FactionState, commands: &mut Commands) {
    let text = compose_dispatch(pack, state);
    info!("server: dispatch: {text}");
    state.last_dispatch = Some(text.clone());
    commands.server_trigger(ToClients {
        targets: SendTargets::All,
        message: Notice { text },
    });
}

/// Scenario pacing: runs the same broadcast the scheduled system runs, without
/// waiting on wall time. Returns the composed dispatch text.
pub fn force_dispatch(world: &mut World) -> Option<String> {
    let mut queue = CommandQueue::default();
    let text = world.resource_scope(|world, mut state: Mut<FactionState>| {
        let content = world.resource::<Content>();
        state.dispatch_accum = 0.0;
        let mut commands = Commands::new(&mut queue, world);
        broadcast_dispatch(&content.0, &mut state, &mut commands);
        state.last_dispatch.clone()
    });
    queue.apply(world);
    text
}

pub fn faction_kill_effects(
    content: Res<Content>,
    mut state: ResMut<FactionState>,
    mut dying_npcs: Query<
        (
            &Ship,
            &sv_protocol::ShipInfo,
            &SystemMembership,
            &mut LastDamagedBy,
        ),
        With<Npc>,
    >,
    mut players: Query<(&Owner, &mut PlayerReputation), With<PlayerShip>>,
) {
    for (ship, info, membership, mut last_damaged) in &mut dying_npcs {
        if !ship.0.is_destroyed() {
            continue;
        }
        let Some(killer) = last_damaged.0 else {
            continue;
        };
        last_damaged.0 = None;
        let system = membership.0.as_str();
        let owner_faction = base_government(&content.0, system);
        let controller = state.controller(system).to_owned();
        let (rep_faction, rep_delta, control_delta) = match info.kind {
            ShipKind::Pirate => (
                owner_faction.clone(),
                REP_PIRATE_KILL,
                CONTROL_PIRATE_KILL * side_sign(&content.0, &controller),
            ),
            ShipKind::Police => (
                controller.clone(),
                REP_PATROL_KILL,
                -CONTROL_PATROL_KILL * side_sign(&content.0, &controller),
            ),
            ShipKind::Merchant | ShipKind::Defender | ShipKind::Explorer => {
                (owner_faction.clone(), REP_CIVILIAN_KILL, 0)
            }
            ShipKind::Player => continue,
        };
        for (owner, mut reputation) in &mut players {
            if owner.0 == killer {
                apply_rep(&mut reputation.standings, &rep_faction, rep_delta);
                break;
            }
        }
        if control_delta != 0 {
            state.shift_meter(&content.0, system, control_delta);
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn patrol_hostility(
    mut commands: Commands,
    content: Res<Content>,
    state: Res<FactionState>,
    players: Query<
        (
            &Owner,
            &SystemMembership,
            &sv_protocol::Position,
            &PlayerReputation,
            Option<&ClientRef>,
        ),
        With<PlayerShip>,
    >,
    aggros: Query<&Aggro>,
) {
    for (owner, membership, position, reputation, client_ref) in &players {
        let system = membership.0.as_str();
        let controller = state.controller(system).to_owned();
        if side_sign(&content.0, &controller) == 0 {
            continue;
        }
        let hostile = reputation
            .standings
            .iter()
            .find(|(faction, _)| *faction == controller)
            .is_some_and(|(_, value)| *value <= REP_HOSTILE);
        if !hostile {
            continue;
        }
        if aggros.iter().any(|aggro| aggro.0 == owner.0) {
            continue;
        }
        let offset = (position.0 - system_center(&content.0, system)) + Vec2::new(320.0, 120.0);
        spawn_patrol(&mut commands, &content.0, system, offset, owner.0);
        if let Some(client_ref) = client_ref {
            notice(
                &mut commands,
                ClientId::Client(client_ref.0),
                format!(
                    "A {} patrol locks onto you. Your face is on file.",
                    faction_name(&content.0, &controller)
                ),
            );
        }
    }
}

pub fn parse_reputation(json: &str) -> PlayerReputation {
    PlayerReputation {
        standings: serde_json::from_str(json).unwrap_or_default(),
    }
}

pub fn encode_reputation(reputation: &PlayerReputation) -> String {
    serde_json::to_string(&reputation.standings).unwrap_or_else(|_| "[]".to_owned())
}

pub fn faction_name<'a>(pack: &'a ContentPack, faction: &'a str) -> &'a str {
    pack.faction(faction)
        .map(|def| def.name.as_str())
        .unwrap_or(faction)
}

pub fn system_name<'a>(pack: &'a ContentPack, system: &'a str) -> &'a str {
    pack.system(system)
        .map(|def| def.name.as_str())
        .unwrap_or(system)
}
