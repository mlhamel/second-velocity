use bevy::prelude::*;
use sv_data::{ContentPack, QuestDef, QuestKind, SystemDef};

use super::EncounterRng;
use crate::Content;

pub const CONTRACT_REFRESH_SECS: f32 = 300.0;
pub const CONTRACT_EXTRAS: usize = 6;
const CONTRACT_MAX_QTY: u32 = 15;
const CONTRACT_MIN_QTY: u32 = 5;
const NEARBY_HOSTS: usize = 8;

#[derive(Resource, Debug)]
pub struct ContractBoard {
    pub contracts: Vec<QuestDef>,
    rng: EncounterRng,
    refresh_accum: f32,
}

impl Default for ContractBoard {
    fn default() -> Self {
        Self {
            contracts: Vec::new(),
            rng: EncounterRng::default(),
            refresh_accum: CONTRACT_REFRESH_SECS,
        }
    }
}

struct HostPort {
    system: String,
    port: String,
    port_name: String,
    system_pos: (f32, f32),
    commodities: Vec<String>,
}

/// Deterministic per seed: the same rng state reproduces the same board.
pub fn generate_board(pack: &ContentPack, rng: &mut EncounterRng, extras: usize) -> Vec<QuestDef> {
    let hosts = host_ports(pack);
    if hosts.is_empty() {
        return Vec::new();
    }
    let pirates = pirate_systems(pack);
    let mut contracts = Vec::new();
    for host in &hosts {
        if let Some(contract) = generate_contract(pack, rng, host, &hosts, &pirates) {
            contracts.push(contract);
        }
    }
    for _ in 0..extras {
        let idx = rng.next_u32(hosts.len() as u32) as usize;
        let host = &hosts[idx];
        if let Some(contract) = generate_contract(pack, rng, host, &hosts, &pirates) {
            contracts.push(contract);
        }
    }
    for (index, contract) in contracts.iter_mut().enumerate() {
        contract.id = format!("contract-{index}");
    }
    contracts
}

fn generate_contract(
    pack: &ContentPack,
    rng: &mut EncounterRng,
    host: &HostPort,
    hosts: &[HostPort],
    pirates: &[&SystemDef],
) -> Option<QuestDef> {
    let wants_delivery =
        !host.commodities.is_empty() && (pirates.is_empty() || rng.next_u32(10) < 6);
    if wants_delivery {
        let commodity =
            host.commodities[rng.next_u32(host.commodities.len() as u32) as usize].clone();
        let mut nearby: Vec<&HostPort> = hosts
            .iter()
            .filter(|other| other.port != host.port || other.system != host.system)
            .collect();
        nearby.sort_by(|a, b| {
            distance(host.system_pos, a.system_pos)
                .partial_cmp(&distance(host.system_pos, b.system_pos))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        nearby.truncate(NEARBY_HOSTS);
        if !nearby.is_empty()
            && let Some(destination) = nearby
                .get(rng.next_u32(nearby.len() as u32) as usize)
                .copied()
        {
            let qty = CONTRACT_MIN_QTY + rng.next_u32(CONTRACT_MAX_QTY - CONTRACT_MIN_QTY + 1);
            let dist = distance(host.system_pos, destination.system_pos);
            let hazard = system_hazard(pack, &destination.system);
            let timed = rng.next_u32(2) == 0;
            return Some(QuestDef {
                id: String::new(),
                name: format!("Parcel Run to {}", destination.port_name),
                giver: (host.system.clone(), host.port.clone()),
                kind: QuestKind::Delivery {
                    commodity,
                    qty,
                    destination: (destination.system.clone(), destination.port.clone()),
                    deadline_secs: timed.then(|| sv_core::contract::contract_deadline(dist)),
                },
                reward_credits: sv_core::contract::contract_reward(
                    sv_core::contract::DELIVERY_BASE,
                    dist,
                    hazard,
                ),
                description: format!(
                    "{qty} tons of cargo, sealed and stamped. {} wants it before the manifest goes stale.{}",
                    destination.port_name,
                    if timed { " The clock is running." } else { "" }
                ),
            });
        }
    }
    if pirates.is_empty() {
        return None;
    }
    let target_sys = if pirates.len() == 1 {
        pirates[0]
    } else {
        pirates[rng.next_u32(pirates.len() as u32) as usize]
    };
    let dist = distance(host.system_pos, target_sys.position);
    let hazard = system_hazard(pack, &target_sys.id);
    let timed = rng.next_u32(2) == 0;
    let system_name = &target_sys.name;
    Some(QuestDef {
        id: String::new(),
        name: format!("Pirate Warrant: {system_name}"),
        giver: (host.system.clone(), host.port.clone()),
        kind: QuestKind::Bounty {
            system: target_sys.id.clone(),
            target: "pirate".to_owned(),
            deadline_secs: timed.then(|| sv_core::contract::contract_deadline(dist)),
        },
        reward_credits: sv_core::contract::contract_reward(
            sv_core::contract::BOUNTY_BASE,
            dist,
            hazard,
        ),
        description: format!(
            "One confirmed pirate kill in the {system_name} approach. The registry pays on telemetry.{}",
            if timed { " The clock is running." } else { "" }
        ),
    })
}

pub fn contract_refresh(time: Res<Time>, mut board: ResMut<ContractBoard>, content: Res<Content>) {
    board.refresh_accum += time.delta_secs();
    if board.refresh_accum < CONTRACT_REFRESH_SECS {
        return;
    }
    board.refresh_accum = 0.0;
    regenerate(&mut board, &content.0);
}

/// Scenario pacing: runs the same refresh the scheduled system runs.
pub fn force_contract_refresh(world: &mut World, refreshes: u32) {
    for _ in 0..refreshes {
        world.resource_scope(|world, mut board: Mut<ContractBoard>| {
            let content = world.resource::<Content>();
            regenerate(&mut board, &content.0);
        });
    }
}

fn regenerate(board: &mut ContractBoard, pack: &ContentPack) {
    board.contracts = generate_board(pack, &mut board.rng, CONTRACT_EXTRAS);
}

fn pirate_systems(pack: &ContentPack) -> Vec<&SystemDef> {
    pack.systems.iter().filter(|sys| sys.pirate).collect()
}

fn host_ports(pack: &ContentPack) -> Vec<HostPort> {
    let mut hosts = Vec::new();
    for system in &pack.systems {
        for planet in &system.planets {
            if planet.chief.is_some() {
                hosts.push(HostPort {
                    system: system.id.clone(),
                    port: planet.id.clone(),
                    port_name: planet.name.clone(),
                    system_pos: (system.position.0, system.position.1),
                    commodities: planet.commodities.iter().map(|c| c.id.clone()).collect(),
                });
            }
        }
        for station in &system.stations {
            if station.master.is_some() {
                hosts.push(HostPort {
                    system: system.id.clone(),
                    port: station.id.clone(),
                    port_name: station.name.clone(),
                    system_pos: (system.position.0, system.position.1),
                    commodities: station.commodities.iter().map(|c| c.id.clone()).collect(),
                });
            }
        }
        for (_, moon) in system.landable_moons() {
            if moon.chief.is_some() {
                hosts.push(HostPort {
                    system: system.id.clone(),
                    port: moon.id.clone(),
                    port_name: moon.name.clone(),
                    system_pos: (system.position.0, system.position.1),
                    commodities: moon.commodities.iter().map(|c| c.id.clone()).collect(),
                });
            }
        }
    }
    hosts
}

fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

fn system_hazard(pack: &ContentPack, system_id: &str) -> f32 {
    pack.system(system_id)
        .map(|sys| {
            let mut hazard = 0.0;
            if sys.black_hole.is_some() {
                hazard += 0.4;
            }
            if sys.pirate {
                hazard += 0.25;
            }
            if sys.meteorites.is_some() {
                hazard += 0.2;
            }
            if sys.star.is_some() {
                hazard += 0.1;
            }
            hazard
        })
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
    }

    #[test]
    fn same_seed_generates_identical_boards() {
        let pack = ContentPack::load_dir(&content_root()).expect("pack loads");
        let mut first = EncounterRng(42);
        let mut second = EncounterRng(42);
        let board_a = generate_board(&pack, &mut first, CONTRACT_EXTRAS);
        let board_b = generate_board(&pack, &mut second, CONTRACT_EXTRAS);
        assert_eq!(board_a, board_b, "generation must be seeded-deterministic");
    }

    #[test]
    fn every_host_port_gets_a_contract_and_extras() {
        let pack = ContentPack::load_dir(&content_root()).expect("pack loads");
        let hosts = host_ports(&pack);
        let mut rng = EncounterRng(7);
        let board = generate_board(&pack, &mut rng, CONTRACT_EXTRAS);
        assert_eq!(board.len(), hosts.len() + CONTRACT_EXTRAS);
        for host in &hosts {
            assert!(
                board
                    .iter()
                    .any(|c| c.giver.0 == host.system && c.giver.1 == host.port),
                "host {}/{} must always have work",
                host.system,
                host.port
            );
        }
    }

    #[test]
    fn contracts_pay_at_least_base_and_bounties_target_pirates() {
        let pack = ContentPack::load_dir(&content_root()).expect("pack loads");
        let mut rng = EncounterRng(123);
        let board = generate_board(&pack, &mut rng, CONTRACT_EXTRAS);
        assert!(!board.is_empty());
        for contract in &board {
            assert!(
                contract.reward_credits
                    >= sv_core::contract::DELIVERY_BASE.min(sv_core::contract::BOUNTY_BASE),
                "rewards must respect the curve floor"
            );
            if let QuestKind::Bounty { system, target, .. } = &contract.kind {
                assert_eq!(target, "pirate", "bounties must use the pirate wildcard");
                assert!(
                    pack.system(system).is_some_and(|s| s.pirate),
                    "bounty target system must be flagged pirate"
                );
            }
            assert!(contract.id.starts_with("contract-"));
            assert!(!contract.description.is_empty());
        }
    }

    #[test]
    fn scope_without_pirate_systems_generates_no_bounties_and_no_pirate_hazard() {
        let mut pack = ContentPack::load_dir(&content_root()).expect("pack loads");
        let with_pirate_hazard = system_hazard(&pack, "alpha-centauri");
        for sys in &mut pack.systems {
            sys.pirate = false;
        }
        let without_pirate_hazard = system_hazard(&pack, "alpha-centauri");
        assert!(
            with_pirate_hazard > without_pirate_hazard,
            "pirate flag must feed system_hazard"
        );
        let mut rng = EncounterRng(123);
        let board = generate_board(&pack, &mut rng, CONTRACT_EXTRAS);
        assert!(!board.is_empty());
        assert!(
            board
                .iter()
                .all(|c| matches!(c.kind, QuestKind::Delivery { .. })),
            "scope with no flagged pirate systems must generate no pirate bounty jobs"
        );
    }
}
