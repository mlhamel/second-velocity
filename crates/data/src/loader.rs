use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::commodity::CommodityDef;
use crate::error::{ContentError, validate_asset, validate_ids, validate_reference};
use crate::faction::FactionDef;
use crate::manifest::Manifest;
use crate::outfit::OutfitDef;
use crate::pilot::PilotDef;
use crate::quest::{QuestDef, QuestKind};
use crate::scenario::{ScenarioDef, ScenarioStep};
use crate::ship::ShipDef;
use crate::system::{DockableDef, PLANET_RADIUS, StationPlacement, SystemDef};

#[derive(Debug)]
pub struct ContentPack {
    pub manifest: Manifest,
    pub ships: Vec<ShipDef>,
    pub systems: Vec<SystemDef>,
    pub outfits: Vec<OutfitDef>,
    pub commodities: Vec<CommodityDef>,
    pub quests: Vec<QuestDef>,
    pub scenarios: Vec<ScenarioDef>,
    pub factions: Vec<FactionDef>,
    pub pilots: Vec<PilotDef>,
}

impl ContentPack {
    pub fn load_pilot(path: &Path) -> Result<PilotDef, ContentError> {
        parse_ron(&path.to_path_buf())
    }

    pub fn load_dir(root: &Path) -> Result<Self, ContentError> {
        let manifest: Manifest = parse_ron(&root.join("manifest.ron"))?;

        let mut ships: Vec<ShipDef> = Vec::with_capacity(manifest.ships.len());
        for path in &manifest.ships {
            ships.push(parse_ron(&root.join(path))?);
        }

        let mut systems: Vec<SystemDef> = Vec::with_capacity(manifest.systems.len());
        for path in &manifest.systems {
            systems.push(parse_ron(&root.join(path))?);
        }

        let mut outfits: Vec<OutfitDef> = Vec::with_capacity(manifest.outfits.len());
        for path in &manifest.outfits {
            outfits.push(parse_ron(&root.join(path))?);
        }

        let mut commodities: Vec<CommodityDef> = Vec::with_capacity(manifest.commodities.len());
        for path in &manifest.commodities {
            commodities.push(parse_ron(&root.join(path))?);
        }

        let mut quests: Vec<QuestDef> = Vec::with_capacity(manifest.quests.len());
        for path in &manifest.quests {
            quests.push(parse_ron(&root.join(path))?);
        }

        let mut scenarios: Vec<ScenarioDef> = Vec::with_capacity(manifest.scenarios.len());
        for path in &manifest.scenarios {
            scenarios.push(parse_ron(&root.join(path))?);
        }

        let mut factions: Vec<FactionDef> = Vec::with_capacity(manifest.factions.len());
        for path in &manifest.factions {
            factions.push(parse_ron(&root.join(path))?);
        }

        let mut pilots: Vec<PilotDef> = Vec::with_capacity(manifest.pilots.len());
        for path in &manifest.pilots {
            pilots.push(parse_ron(&root.join(path))?);
        }

        Self::validate(
            &ships,
            &systems,
            &outfits,
            &commodities,
            &quests,
            &scenarios,
            &factions,
            &pilots,
        )?;
        Self::validate_assets(root, &ships, &systems, &outfits, &commodities)?;

        Ok(Self {
            manifest,
            ships,
            systems,
            outfits,
            commodities,
            quests,
            scenarios,
            factions,
            pilots,
        })
    }

    fn validate_assets(
        root: &Path,
        ships: &[ShipDef],
        systems: &[SystemDef],
        outfits: &[OutfitDef],
        commodities: &[CommodityDef],
    ) -> Result<(), ContentError> {
        let assets_root = root.parent().unwrap_or(root).join("assets");
        for ship in ships {
            if let Some(sprite) = &ship.sprite {
                validate_asset(&assets_root, sprite)?;
            }
        }
        for outfit in outfits {
            if let Some(icon) = &outfit.icon {
                validate_asset(&assets_root, icon)?;
            }
        }
        for commodity in commodities {
            if let Some(icon) = &commodity.icon {
                validate_asset(&assets_root, icon)?;
            }
        }
        for system in systems {
            for planet in &system.planets {
                if let Some(sprite) = &planet.sprite {
                    validate_asset(&assets_root, sprite)?;
                }
                if let Some(chief) = &planet.chief
                    && let Some(portrait) = &chief.portrait
                {
                    validate_asset(&assets_root, portrait)?;
                }
            }
            if let Some(star) = &system.star
                && let Some(sprite) = &star.sprite
            {
                validate_asset(&assets_root, sprite)?;
            }
            if let Some(hole) = &system.black_hole
                && let Some(sprite) = &hole.sprite
            {
                validate_asset(&assets_root, sprite)?;
            }
            for station in &system.stations {
                if let Some(sprite) = &station.sprite {
                    validate_asset(&assets_root, sprite)?;
                }
                if let Some(portrait) = station
                    .master
                    .as_ref()
                    .and_then(|host| host.portrait.as_ref())
                {
                    validate_asset(&assets_root, portrait)?;
                }
            }
        }
        Ok(())
    }

    pub fn ship(&self, id: &str) -> Option<&ShipDef> {
        self.ships.iter().find(|ship| ship.id == id)
    }

    pub fn system(&self, id: &str) -> Option<&SystemDef> {
        self.systems.iter().find(|system| system.id == id)
    }

    pub fn dockable(&self, system: &str, location: &str) -> Option<DockableDef<'_>> {
        self.system(system)?.dockable(location)
    }

    pub fn outfit(&self, id: &str) -> Option<&OutfitDef> {
        self.outfits.iter().find(|outfit| outfit.id == id)
    }

    pub fn commodity(&self, id: &str) -> Option<&CommodityDef> {
        self.commodities.iter().find(|commodity| commodity.id == id)
    }

    pub fn quest(&self, id: &str) -> Option<&QuestDef> {
        self.quests.iter().find(|quest| quest.id == id)
    }

    pub fn scenario(&self, id: &str) -> Option<&ScenarioDef> {
        self.scenarios.iter().find(|scenario| scenario.id == id)
    }

    pub fn faction(&self, id: &str) -> Option<&FactionDef> {
        self.factions.iter().find(|faction| faction.id == id)
    }

    pub fn pilot(&self, id: &str) -> Option<&PilotDef> {
        self.pilots.iter().find(|pilot| pilot.id == id)
    }

    pub fn validate_pilot(&self, pilot: &PilotDef) -> Result<(), ContentError> {
        let ship_ids: std::collections::HashSet<_> =
            self.ships.iter().map(|s| s.id.clone()).collect();
        let commodity_ids: std::collections::HashSet<_> =
            self.commodities.iter().map(|c| c.id.clone()).collect();
        let dockable_refs: std::collections::HashSet<String> = self
            .systems
            .iter()
            .flat_map(|s| {
                s.planets
                    .iter()
                    .map(move |p| format!("{}/{}", s.id, p.id))
                    .chain(
                        s.stations
                            .iter()
                            .map(move |st| format!("{}/{}", s.id, st.id)),
                    )
            })
            .collect();
        validate_pilot_def(pilot, &dockable_refs, &commodity_ids, &ship_ids)
    }

    #[allow(clippy::too_many_arguments)]
    fn validate(
        ships: &[ShipDef],
        systems: &[SystemDef],
        outfits: &[OutfitDef],
        commodities: &[CommodityDef],
        quests: &[QuestDef],
        scenarios: &[ScenarioDef],
        factions: &[FactionDef],
        pilots: &[PilotDef],
    ) -> Result<(), ContentError> {
        validate_ids("ship", ships.iter().map(|ship| ship.id.clone()))?;
        validate_ids("system", systems.iter().map(|system| system.id.clone()))?;
        validate_ids("outfit", outfits.iter().map(|outfit| outfit.id.clone()))?;
        validate_ids(
            "commodity",
            commodities.iter().map(|commodity| commodity.id.clone()),
        )?;
        validate_ids("quest", quests.iter().map(|quest| quest.id.clone()))?;
        validate_ids(
            "scenario",
            scenarios.iter().map(|scenario| scenario.id.clone()),
        )?;
        validate_ids("faction", factions.iter().map(|faction| faction.id.clone()))?;
        validate_ids("pilot", pilots.iter().map(|pilot| pilot.id.clone()))?;
        validate_ids(
            "pilot username",
            pilots.iter().map(|pilot| pilot.username.clone()),
        )?;

        let faction_ids: std::collections::HashSet<_> =
            factions.iter().map(|f| f.id.clone()).collect();
        for faction in factions {
            let in_range = [faction.color.0, faction.color.1, faction.color.2]
                .into_iter()
                .all(|axis| axis.is_finite() && (0.0..=1.0).contains(&axis));
            if !in_range {
                return Err(ContentError::UnknownReference {
                    kind: "faction color",
                    id: faction.id.clone(),
                });
            }
        }
        if !factions.is_empty() {
            for system in systems {
                validate_reference("system government", &system.government, &faction_ids)?;
            }
        }

        let outfit_ids: std::collections::HashSet<_> =
            outfits.iter().map(|o| o.id.clone()).collect();
        let commodity_ids: std::collections::HashSet<_> =
            commodities.iter().map(|c| c.id.clone()).collect();
        for outfit in outfits {
            let positive = |value: f32| value.is_finite() && value > 0.0;
            let valid = outfit.price > 0
                && outfit.mass.is_finite()
                && outfit.mass >= 0.0
                && match &outfit.kind {
                    crate::OutfitKind::Weapon(w) => {
                        [w.damage, w.cooldown, w.projectile_speed, w.projectile_ttl]
                            .into_iter()
                            .all(positive)
                            && w.energy_cost.is_none_or(|c| c.is_finite() && c >= 0.0)
                            && match w.mount {
                                crate::WeaponMount::Fixed | crate::WeaponMount::Turret => true,
                                crate::WeaponMount::Missile { turn_rate } => positive(turn_rate),
                            }
                    }
                    crate::OutfitKind::ShieldGenerator { max_shields } => positive(*max_shields),
                    crate::OutfitKind::FuelTank { fuel } => positive(*fuel),
                    crate::OutfitKind::ArmorPlate { armor } => positive(*armor),
                    crate::OutfitKind::Reactor {
                        max_energy,
                        energy_regen,
                    } => positive(*max_energy) && positive(*energy_regen),
                };
            if !valid {
                return Err(ContentError::UnknownReference {
                    kind: "outfit config",
                    id: outfit.id.clone(),
                });
            }
        }
        for ship in ships {
            if ship.price == 0 {
                return Err(ContentError::UnknownReference {
                    kind: "ship price",
                    id: ship.id.clone(),
                });
            }
            for outfit in &ship.default_outfits {
                validate_reference("outfit", outfit, &outfit_ids)?;
            }
            crate::loadout::stats_for(ship, outfits, &ship.default_outfits).map_err(|_| {
                ContentError::UnknownReference {
                    kind: "ship loadout",
                    id: ship.id.clone(),
                }
            })?;
        }

        let system_ids: std::collections::HashSet<_> =
            systems.iter().map(|s| s.id.clone()).collect();
        let ship_ids = ships.iter().map(|s| s.id.clone()).collect();
        for system in systems {
            for port in system
                .planets
                .iter()
                .map(DockableDef::Planet)
                .chain(system.stations.iter().map(DockableDef::Station))
            {
                if let Some(yard) = port.shipyard() {
                    validate_ids("shipyard stock", yard.ships.iter().cloned())?;
                    for id in &yard.ships {
                        validate_reference("shipyard ship", id, &ship_ids)?;
                    }
                }
                if let Some(shop) = port.outfitter() {
                    validate_ids("outfitter stock", shop.outfits.iter().cloned())?;
                    for id in &shop.outfits {
                        validate_reference("outfitter outfit", id, &outfit_ids)?;
                    }
                }
                for entry in port.commodities() {
                    validate_reference("port commodity", &entry.id, &commodity_ids)?;
                }
            }
            validate_ids(
                "docking location",
                system
                    .planets
                    .iter()
                    .map(|p| p.id.clone())
                    .chain(system.stations.iter().map(|s| s.id.clone())),
            )?;
            for station in &system.stations {
                let valid = station.radius.is_finite()
                    && station.radius > 0.0
                    && match &station.placement {
                        StationPlacement::Fixed { position } => {
                            position.0.is_finite()
                                && position.1.is_finite()
                                && (system.position.0 + position.0).is_finite()
                                && (system.position.1 + position.1).is_finite()
                                && [
                                    system.position.0 + position.0,
                                    system.position.1 + position.1,
                                ]
                                .into_iter()
                                .all(|axis| {
                                    f64::from(axis).abs() + f64::from(station.radius) + 64.0
                                        <= f64::from(f32::MAX)
                                })
                        }
                        StationPlacement::Orbit {
                            planet,
                            radius,
                            period_secs,
                            phase,
                        } => {
                            system.planet_position(planet).is_some_and(|center| {
                                [center.0, center.1].into_iter().all(|axis| {
                                    axis.is_finite()
                                        && (f64::from(axis).abs()
                                            + f64::from(*radius)
                                            + f64::from(station.radius))
                                            <= f64::from(f32::MAX)
                                })
                            }) && radius.is_finite()
                                && *radius > PLANET_RADIUS + station.radius + 20.0
                                && period_secs.is_finite()
                                && *period_secs > 0.0
                                && phase.is_finite()
                        }
                    };
                if !valid {
                    return Err(ContentError::UnknownReference {
                        kind: "station config",
                        id: format!("{}/{}", system.id, station.id),
                    });
                }
            }
            if let Some(field) = &system.meteorites
                && field.count > 0
                && (field.radius <= 0.0
                    || (field.stationary_weight == 0
                        && field.slow_weight == 0
                        && field.fast_weight == 0))
            {
                return Err(ContentError::UnknownReference {
                    kind: "meteorite field config",
                    id: system.id.clone(),
                });
            }
            if let Some(star) = &system.star
                && star.radius <= 0.0
            {
                return Err(ContentError::UnknownReference {
                    kind: "star config",
                    id: system.id.clone(),
                });
            }
            if let Some(hole) = &system.black_hole
                && (!hole.position.0.is_finite()
                    || !hole.position.1.is_finite()
                    || !hole.radius.is_finite()
                    || hole.radius <= 0.0
                    || !hole.influence.is_finite()
                    || hole.influence <= hole.radius
                    || !hole.pull_strength.is_finite()
                    || hole.pull_strength <= 0.0
                    || systems.len() < 2)
            {
                return Err(ContentError::UnknownReference {
                    kind: "black hole config",
                    id: system.id.clone(),
                });
            }
            for link in &system.links {
                validate_reference("system link", link, &system_ids)?;
                let target_has_back_link = systems
                    .iter()
                    .find(|target| &target.id == link)
                    .is_some_and(|target| target.links.contains(&system.id));
                if !target_has_back_link {
                    return Err(ContentError::UnknownReference {
                        kind: "reciprocal link",
                        id: format!("{}->{}", system.id, link),
                    });
                }
            }
        }

        let planet_refs: std::collections::HashSet<String> = systems
            .iter()
            .flat_map(|s| s.planets.iter().map(move |p| format!("{}/{}", s.id, p.id)))
            .collect();
        let planet_ids: std::collections::HashSet<String> = systems
            .iter()
            .flat_map(|s| s.planets.iter().map(|p| p.id.clone()))
            .collect();
        let quest_ids: std::collections::HashSet<String> =
            quests.iter().map(|q| q.id.clone()).collect();
        let station_refs: std::collections::HashSet<String> = systems
            .iter()
            .flat_map(|s| {
                s.stations
                    .iter()
                    .map(move |station| format!("{}/{}", s.id, station.id))
            })
            .collect();
        let dockable_refs: std::collections::HashSet<String> =
            planet_refs.union(&station_refs).cloned().collect();
        let dockable_ids: std::collections::HashSet<String> = planet_ids
            .iter()
            .cloned()
            .chain(
                systems
                    .iter()
                    .flat_map(|s| s.stations.iter().map(|station| station.id.clone())),
            )
            .collect();

        for quest in quests {
            if quest.id.starts_with("contract-") {
                return Err(ContentError::UnknownReference {
                    kind: "quest id prefix",
                    id: quest.id.clone(),
                });
            }
            let giver_ref = format!("{}/{}", quest.giver.0, quest.giver.1);
            validate_reference("quest giver planet", &giver_ref, &dockable_refs)?;
            validate_station_quest_host(systems, &quest.giver)?;
            match &quest.kind {
                QuestKind::Delivery { destination, .. } => {
                    let dest_ref = format!("{}/{}", destination.0, destination.1);
                    validate_reference("quest destination planet", &dest_ref, &dockable_refs)?;
                    validate_station_quest_host(systems, destination)?;
                }
                QuestKind::Bounty { system, .. } => {
                    validate_reference("quest target system", system, &system_ids)?;
                }
            }
        }

        for scenario in scenarios {
            for step in &scenario.steps {
                match step {
                    ScenarioStep::BuyShip { ship } | ScenarioStep::AssertShip(ship) => {
                        validate_reference("scenario ship", ship, &ship_ids)?;
                    }
                    ScenarioStep::BuyOutfit { outfit } | ScenarioStep::SellOutfit { outfit } => {
                        validate_reference("scenario outfit", outfit, &outfit_ids)?;
                    }
                    ScenarioStep::AssertLoadout { outfits } => {
                        for id in outfits {
                            validate_reference("scenario loadout", id, &outfit_ids)?;
                        }
                    }
                    ScenarioStep::FlyToStation {
                        system, station, ..
                    } => {
                        validate_reference(
                            "scenario station",
                            &format!("{system}/{station}"),
                            &station_refs,
                        )?;
                    }
                    ScenarioStep::Dock { system, location } => {
                        validate_reference(
                            "scenario dock target",
                            &format!("{system}/{location}"),
                            &dockable_refs,
                        )?;
                    }
                    ScenarioStep::FlyToPlanet { system, planet, .. } => {
                        let target_ref = format!("{system}/{planet}");
                        validate_reference("scenario target planet", &target_ref, &planet_refs)?;
                    }
                    ScenarioStep::FlyToSystem { system } | ScenarioStep::AssertSystem(system) => {
                        validate_reference("scenario system", system, &system_ids)?;
                    }
                    ScenarioStep::FlyToStar { system, .. } => {
                        validate_reference("scenario system", system, &system_ids)?;
                        let has_star = systems
                            .iter()
                            .find(|target| &target.id == system)
                            .is_some_and(|target| target.star.is_some());
                        if !has_star {
                            return Err(ContentError::UnknownReference {
                                kind: "scenario star target",
                                id: system.clone(),
                            });
                        }
                    }
                    ScenarioStep::Land { planet } => {
                        validate_reference("scenario planet", planet, &planet_ids)?;
                    }
                    ScenarioStep::FlyToBlackHole { system } => {
                        validate_reference("scenario system", system, &system_ids)?;
                        if !systems
                            .iter()
                            .find(|target| &target.id == system)
                            .is_some_and(|target| target.black_hole.is_some())
                        {
                            return Err(ContentError::UnknownReference {
                                kind: "scenario black hole target",
                                id: system.clone(),
                            });
                        }
                    }
                    ScenarioStep::AssertDocked(Some(planet)) => {
                        validate_reference("scenario docked planet", planet, &dockable_ids)?;
                    }
                    ScenarioStep::AssertPrice {
                        system,
                        location,
                        commodity,
                        ..
                    } => {
                        validate_reference(
                            "scenario price port",
                            &format!("{system}/{location}"),
                            &dockable_refs,
                        )?;
                        validate_reference("scenario price commodity", commodity, &commodity_ids)?;
                    }
                    ScenarioStep::AssertCompletedQuest(quest_id) => {
                        validate_reference("scenario quest", quest_id, &quest_ids)?;
                    }
                    ScenarioStep::AssertControl { system, .. } => {
                        validate_reference("scenario control system", system, &system_ids)?;
                    }
                    ScenarioStep::AssertReputation { faction, .. } => {
                        validate_reference("scenario reputation faction", faction, &faction_ids)?;
                    }
                    _ => {}
                }
            }
        }

        for pilot in pilots {
            validate_pilot_def(pilot, &dockable_refs, &commodity_ids, &ship_ids)?;
        }

        Ok(())
    }
}

fn validate_pilot_def(
    pilot: &PilotDef,
    dockable_refs: &std::collections::HashSet<String>,
    commodity_ids: &std::collections::HashSet<String>,
    ship_ids: &std::collections::HashSet<String>,
) -> Result<(), ContentError> {
    let valid_config = !pilot.id.trim().is_empty()
        && !pilot.name.trim().is_empty()
        && !pilot.username.trim().is_empty()
        && !pilot.password.trim().is_empty()
        && !pilot.persona.trim().is_empty()
        && pilot.idle_interval_secs.is_finite()
        && pilot.idle_interval_secs >= 15.0
        && pilot.flee_range.is_finite()
        && pilot.flee_range > 0.0
        && pilot.low_fuel_ratio.is_finite()
        && (0.05..=0.95).contains(&pilot.low_fuel_ratio);
    if !valid_config {
        return Err(ContentError::UnknownReference {
            kind: "pilot config",
            id: pilot.id.clone(),
        });
    }
    let home_ref = format!("{}/{}", pilot.home_system, pilot.home_port);
    validate_reference("pilot home port", &home_ref, dockable_refs)?;
    for comm in &pilot.preferred_commodities {
        validate_reference("pilot preferred commodity", comm, commodity_ids)?;
    }
    for ship in &pilot.preferred_ships {
        validate_reference("pilot preferred ship", ship, ship_ids)?;
    }
    Ok(())
}

fn validate_station_quest_host(
    systems: &[SystemDef],
    target: &(String, String),
) -> Result<(), ContentError> {
    if systems
        .iter()
        .find(|s| s.id == target.0)
        .and_then(|s| s.stations.iter().find(|station| station.id == target.1))
        .is_some_and(|station| station.master.is_none())
    {
        return Err(ContentError::UnknownReference {
            kind: "quest station master",
            id: format!("{}/{}", target.0, target.1),
        });
    }
    Ok(())
}

fn parse_ron<T: DeserializeOwned>(path: &PathBuf) -> Result<T, ContentError> {
    let text = fs::read_to_string(path)?;
    ron::from_str(&text).map_err(|err| ContentError::Parse {
        path: path.clone(),
        message: err.to_string(),
    })
}
