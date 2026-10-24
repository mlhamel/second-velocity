use std::path::Path;
use std::time::{Duration, Instant};

use bevy::app::PostUpdate;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_replicon::prelude::*;
use bevy_replicon_renet2::netcode::{
    ClientSocket, MemorySocketClient, NetcodeClientTransport, NetcodeServerTransport,
};
use bevy_replicon_renet2::renet2::{ConnectionConfig, RenetClient, RenetServer};
use bevy_replicon_renet2::{RenetChannelsExt, RepliconRenetPlugins};
use evr_protocol::{
    AccountState, ActiveQuests, BuyCommodity, ChiefEncounter, ChiefMood, DockedAt, HailShip,
    Heading, Hull, Meteorite, Notice, PlanetTag, PlayerDied, Position, Projectile, QuestTarget,
    RequestLand, RequestLiftOff, RespondToEncounter, SellCommodity, SetFiring, SetThrottle,
    SetTurn, SetWarp, ShipInfo, SystemMembership,
};
use evr_server::{
    Content, EMBEDDED_CLIENT_ID, TickMode, attach_memory, build_server_app,
    memory_reconnect_transport,
};

#[derive(Resource, Default)]
pub struct NoticeLog(pub Vec<String>);

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct TickSample {
    pub server_us: u64,
    pub client_us: u64,
    pub total_us: u64,
    pub server_entities: usize,
    pub client_entities: usize,
    pub projectiles: usize,
}

pub struct Harness {
    pub server: App,
    pub client: App,
    pub ticks: u64,
    pub samples: Vec<TickSample>,
    client_socket: MemorySocketClient,
}

impl Harness {
    pub fn new(content_root: &Path) -> Self {
        let mut server = build_server_app(content_root, TickMode::PerUpdate, None)
            .expect("server app must build");
        let (transport, client_socket) = attach_memory(&mut server);
        server.finish();

        let mut client = App::new();
        client.add_plugins(MinimalPlugins);
        client.add_plugins(StatesPlugin);
        client.add_plugins(RepliconPlugins.set(ServerPlugin::new(PostUpdate)));
        client.add_plugins(RepliconRenetPlugins);
        evr_protocol::register(&mut client);
        client.init_resource::<NoticeLog>();
        client.add_observer(record_notice);
        client.add_observer(record_death);

        let (server_channels, client_channels) = {
            let channels = client.world().resource::<RepliconChannels>();
            (channels.server_configs(), channels.client_configs())
        };
        let renet_client = RenetClient::new(
            ConnectionConfig::from_channels(server_channels, client_channels),
            false,
        );
        client.insert_resource(renet_client);
        client.insert_resource(transport);
        client.finish();

        let mut harness = Self {
            server,
            client,
            ticks: 0,
            samples: Vec::new(),
            client_socket,
        };
        let mut connect_steps = 0;
        while !harness
            .client
            .world()
            .resource::<RenetClient>()
            .is_connected()
        {
            harness.step();
            connect_steps += 1;
            assert!(connect_steps < 300, "client must connect");
        }
        for _ in 0..10 {
            harness.step();
        }
        harness.ticks = 0;
        harness.samples.clear();
        harness
    }

    pub fn step(&mut self) {
        let t0 = Instant::now();
        self.server.update();
        let server_us = t0.elapsed().as_micros() as u64;
        let t1 = Instant::now();
        self.client.update();
        let client_us = t1.elapsed().as_micros() as u64;

        let server_entities = self.server.world().entities().len() as usize;
        let client_entities = self.client.world().entities().len() as usize;
        let projectiles = {
            let mut state = self.server.world_mut().query::<&Projectile>();
            let world = self.server.world();
            state.iter(world).count()
        };
        self.samples.push(TickSample {
            server_us,
            client_us,
            total_us: server_us + client_us,
            server_entities,
            client_entities,
            projectiles,
        });

        std::thread::sleep(Duration::from_millis(15));
        self.ticks += 1;
    }

    pub fn advance(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }

    /// Restores the connection after an idle gap. The netcode transport drops
    /// the connection after 15 seconds without traffic, so callers that pause
    /// between commands must call this before touching the game.
    ///
    /// The renet client only notices the drop one update later, so this checks
    /// the transport's own netcode state, which is updated synchronously.
    pub fn ensure_connected(&mut self) {
        self.step();
        let connected = {
            let transport = self
                .client
                .world()
                .resource::<bevy_replicon_renet2::netcode::NetcodeClientTransport>();
            transport.is_connected()
        };
        if connected {
            return;
        }
        self.force_reconnect();
    }

    /// Replaces the client transport and re-establishes the connection over
    /// the same memory socket. Server-side game state is untouched; the
    /// server rebinds the existing player to the new client entity.
    pub fn force_reconnect(&mut self) {
        self.server.world_mut().resource_scope(
            |world, mut transport: Mut<NetcodeServerTransport>| {
                world.resource_scope(|_, mut renet: Mut<RenetServer>| {
                    transport.disconnect_all(&mut renet);
                });
            },
        );

        {
            let mut buffer = vec![0u8; 4096];
            while self.client_socket.try_recv(&mut buffer).is_ok() {}
        }

        {
            let mut remote = self
                .client
                .world_mut()
                .query_filtered::<Entity, With<Remote>>();
            let world = self.client.world_mut();
            for entity in remote.iter(world).collect::<Vec<_>>() {
                world.despawn(entity);
            }
        }

        let (server_channels, client_channels) = {
            let channels = self.client.world().resource::<RepliconChannels>();
            (channels.server_configs(), channels.client_configs())
        };
        self.client
            .world_mut()
            .remove_resource::<NetcodeClientTransport>();
        self.client.world_mut().remove_resource::<RenetClient>();
        self.client.world_mut().insert_resource(RenetClient::new(
            ConnectionConfig::from_channels(server_channels, client_channels),
            false,
        ));
        self.client
            .world_mut()
            .insert_resource(memory_reconnect_transport(&self.client_socket));

        let mut connect_steps = 0;
        while !self.client.world().resource::<RenetClient>().is_connected() {
            self.step();
            connect_steps += 1;
            assert!(connect_steps < 300, "client must reconnect");
        }
        for _ in 0..10 {
            self.step();
        }
    }

    pub fn set_flight(&mut self, throttle: f32, turn: f32, firing: bool) {
        self.client
            .world_mut()
            .client_trigger(SetThrottle(throttle.clamp(0.0, 1.0)));
        self.client
            .world_mut()
            .client_trigger(SetTurn(turn.clamp(-1.0, 1.0)));
        self.client.world_mut().client_trigger(SetFiring(firing));
    }

    pub fn land(&mut self, planet: &str) {
        self.client.world_mut().client_trigger(RequestLand {
            planet: planet.to_owned(),
        });
    }

    pub fn lift_off(&mut self) {
        self.client.world_mut().client_trigger(RequestLiftOff);
    }

    pub fn dock(&mut self, system: &str, location: &str) {
        self.client
            .world_mut()
            .client_trigger(evr_protocol::RequestDock {
                system: system.to_owned(),
                location: location.to_owned(),
            });
    }

    pub fn buy(&mut self, commodity: &str, qty: u32) {
        self.client.world_mut().client_trigger(BuyCommodity {
            commodity: commodity.to_owned(),
            qty,
        });
    }

    pub fn sell(&mut self, commodity: &str, qty: u32) {
        self.client.world_mut().client_trigger(SellCommodity {
            commodity: commodity.to_owned(),
            qty,
        });
    }

    pub fn set_warp(&mut self, on: bool) {
        self.client.world_mut().client_trigger(SetWarp(on));
    }

    pub fn buy_ship(&mut self, ship: &str) {
        self.client
            .world_mut()
            .client_trigger(evr_protocol::BuyShip {
                ship: ship.to_owned(),
            });
    }

    pub fn buy_outfit(&mut self, outfit: &str) {
        self.client
            .world_mut()
            .client_trigger(evr_protocol::BuyOutfit {
                outfit: outfit.to_owned(),
            });
    }

    pub fn sell_outfit(&mut self, outfit: &str) {
        self.client
            .world_mut()
            .client_trigger(evr_protocol::SellOutfit {
                outfit: outfit.to_owned(),
            });
    }

    /// Scenario-only fixture, intentionally not exposed as an MCP tool.
    pub fn scenario_credits(&mut self, credits: u32) {
        evr_server::gameplay::set_scenario_credits(
            self.server.world_mut(),
            EMBEDDED_CLIENT_ID as u64,
            credits,
        );
    }

    pub fn respond(&mut self, choice: &str) {
        self.client.world_mut().client_trigger(RespondToEncounter {
            choice: choice.to_owned(),
        });
    }

    pub fn hail(&mut self) {
        self.client.world_mut().client_trigger(HailShip);
    }

    pub fn observe(&mut self) -> serde_json::Value {
        let mut notices = Vec::new();
        if let Some(log) = self.client.world().get_resource::<NoticeLog>() {
            notices = log.0.clone();
        }
        self.client
            .world_mut()
            .resource_mut::<NoticeLog>()
            .0
            .clear();

        let owner = EMBEDDED_CLIENT_ID as u64;
        let my_ship = {
            let mut state = self.server.world_mut().query::<(
                &ShipInfo,
                &evr_server::gameplay::Ship,
                &SystemMembership,
                Option<&DockedAt>,
                Option<&AccountState>,
                Option<&ChiefEncounter>,
                Option<&ActiveQuests>,
            )>();
            let world = self.server.world();
            state
                .iter(world)
                .find(|(info, _, _, _, _, _, _)| info.owner == Some(owner))
                .map(
                    |(info, ship, membership, docked, account, encounter, quests)| {
                        (
                            info.name.clone(),
                            info.def_id.clone(),
                            ship.0.clone(),
                            membership.0.clone(),
                            docked.cloned(),
                            account.cloned(),
                            encounter.cloned(),
                            quests.cloned(),
                        )
                    },
                )
        };

        let Some((ship_name, ship_id, ship, system, docked, account, encounter, quests)) = my_ship
        else {
            return serde_json::json!({
                "tick": self.ticks,
                "alive": false,
                "notices": notices,
            });
        };

        let market = {
            let mut ports = self
                .server
                .world_mut()
                .query::<(&evr_protocol::DockableTag, &evr_protocol::PortMarket)>();
            docked
                .as_ref()
                .and_then(|d| {
                    ports
                        .iter(self.server.world())
                        .find(|(tag, _)| tag.system == d.system && tag.id == d.location)
                        .map(|(_, prices)| {
                            prices
                                .prices
                                .iter()
                                .map(|entry| {
                                    serde_json::json!({ "id": entry.commodity, "price": entry.price })
                                })
                                .collect::<Vec<_>>()
                        })
                })
                .unwrap_or_default()
        };

        let encounter_json = encounter.map(|enc| {
            let mood = match enc.mood {
                ChiefMood::Offer => "offer",
                ChiefMood::Tribute => "tribute",
                ChiefMood::Quest => "quest",
                ChiefMood::TurnIn => "turn_in",
            };
            let options = enc
                .options
                .iter()
                .map(|o| serde_json::json!({ "id": o.id, "label": o.label }))
                .collect::<Vec<_>>();
            serde_json::json!({
                "chief_name": enc.chief_name,
                "portrait": enc.portrait,
                "mood": mood,
                "text": enc.text,
                "options": options,
            })
        });

        let completed_quests = quests
            .as_ref()
            .map(|aq| aq.completed.clone())
            .unwrap_or_default();

        let quests_json = quests
            .map(|aq| {
                aq.quests
                    .iter()
                    .map(|q| {
                        let target = match &q.target {
                            QuestTarget::Delivery {
                                commodity,
                                qty,
                                destination_system,
                                destination_planet,
                            } => serde_json::json!({
                                "kind": "delivery",
                                "commodity": commodity,
                                "qty": qty,
                                "destination_system": destination_system,
                                "destination_planet": destination_planet,
                            }),
                            QuestTarget::Bounty {
                                system,
                                target,
                                killed,
                            } => serde_json::json!({
                                "kind": "bounty",
                                "system": system,
                                "target": target,
                                "killed": killed,
                            }),
                        };
                        serde_json::json!({
                            "id": q.id,
                            "name": q.name,
                            "giver_system": q.giver_system,
                            "giver_planet": q.giver_planet,
                            "target": target,
                            "reward_credits": q.reward_credits,
                            "remaining_secs": q.remaining_secs,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let planets = {
            let mut state = self.server.world_mut().query::<(&PlanetTag, &Position)>();
            let world = self.server.world();
            state
                .iter(world)
                .filter(|(tag, _)| tag.system == system)
                .map(|(tag, position)| {
                    serde_json::json!({
                        "id": tag.id,
                        "system": tag.system,
                        "name": tag.name,
                        "position": [position.0.x, position.0.y],
                        "radius": tag.radius,
                    })
                })
                .collect::<Vec<_>>()
        };

        let ships = {
            let mut state = self
                .server
                .world_mut()
                .query::<(&ShipInfo, &Position, &Hull, &SystemMembership)>();
            let world = self.server.world();
            let content = world.resource::<Content>();
            state
                .iter(world)
                .filter(|(info, _, _, _)| info.owner != Some(owner))
                .map(|(info, position, hull, membership)| {
                    let ship_def = content.0.ship(&info.def_id);
                    serde_json::json!({
                        "def_id": info.def_id,
                        "name": info.name,
                        "race": ship_def.map_or("terran", |d| d.race.as_str()),
                        "size": ship_def.map_or("medium", |d| d.size.as_str()),
                        "kind": info.kind.as_str(),
                        "system": membership.0,
                        "position": [position.0.x, position.0.y],
                        "shields": hull.shields,
                        "armor": hull.armor,
                    })
                })
                .collect::<Vec<_>>()
        };

        let meteorites = {
            let mut state =
                self.server
                    .world_mut()
                    .query::<(&Meteorite, &Position, &Heading, &SystemMembership)>();
            let world = self.server.world();
            state
                .iter(world)
                .filter(|(_, _, _, membership)| membership.0 == system)
                .map(|(m, position, heading, _)| {
                    serde_json::json!({
                        "size": m.size.as_str(),
                        "position": [position.0.x, position.0.y],
                        "heading": heading.0,
                        "hp": m.hp,
                        "max_hp": m.max_hp,
                        "spin_rate": m.spin_rate,
                        "spinning": m.spin_rate.abs() > 0.0,
                    })
                })
                .collect::<Vec<_>>()
        };

        let equipped = {
            let mut query = self
                .server
                .world_mut()
                .query::<(&ShipInfo, &evr_protocol::EquippedOutfits)>();
            query
                .iter(self.server.world())
                .find(|(info, _)| info.owner == Some(owner))
                .map(|(_, equipped)| equipped.ids.clone())
                .unwrap_or_default()
        };
        let (weapon_slots, shops) = {
            let content = &self.server.world().resource::<Content>().0;
            let slots = content.ship(&ship_id).map_or(0, |s| s.weapon_slots);
            let port = docked
                .as_ref()
                .and_then(|d| content.dockable(&d.system, &d.location));
            let yard = port.and_then(|p| p.shipyard()).map(|yard| yard.ships.iter().filter_map(|id| content.ship(id))
                .map(|s| serde_json::json!({"id":s.id,"name":s.name,"price":s.price,"cargo":s.cargo,
                    "max_shields":s.max_shields,"max_armor":s.max_armor,"fuel":s.fuel,"weapon_slots":s.weapon_slots})).collect::<Vec<_>>());
            let outfitter = port.and_then(|p| p.outfitter()).map(|shop| shop.outfits.iter().filter_map(|id| content.outfit(id))
                .map(|o| {
                    let effect = match &o.kind {
                        evr_data::OutfitKind::Weapon(w) => serde_json::json!({"kind":"weapon","damage":w.damage,"cooldown":w.cooldown}),
                        evr_data::OutfitKind::ShieldGenerator { max_shields } => serde_json::json!({"kind":"shield","max_shields":max_shields}),
                        evr_data::OutfitKind::FuelTank { fuel } => serde_json::json!({"kind":"fuel","fuel":fuel}),
                        evr_data::OutfitKind::ArmorPlate { armor } => serde_json::json!({"kind":"armor","armor":armor}),
                    };
                    serde_json::json!({"id":o.id,"name":o.name,"price":o.price,"effect":effect,
                        "installed":equipped.iter().filter(|id| **id == o.id).count()})
                }).collect::<Vec<_>>());
            (
                slots,
                serde_json::json!({"shipyard":yard,"outfitter":outfitter}),
            )
        };

        let stations = {
            let mut query = self
                .server
                .world_mut()
                .query::<(&evr_protocol::DockableTag, &Position)>();
            query
                .iter(self.server.world())
                .filter(|(port, _)| port.kind == evr_protocol::DockableKind::Station)
                .map(|(port, position)| {
                    serde_json::json!({
                        "id": port.id, "system": port.system, "name": port.name,
                        "position": [position.0.x, position.0.y], "radius": port.radius,
                    })
                })
                .collect::<Vec<_>>()
        };

        let black_holes = {
            let mut query = self
                .server
                .world_mut()
                .query::<(&evr_protocol::BlackHoleTag, &Position)>();
            query
                .iter(self.server.world())
                .map(|(hole, position)| {
                    serde_json::json!({
                        "system": hole.system,
                        "position": [position.0.x, position.0.y],
                        "radius": hole.radius,
                        "influence": hole.influence,
                    })
                })
                .collect::<Vec<_>>()
        };

        let projectile_count = {
            let mut state = self.server.world_mut().query::<&Projectile>();
            let world = self.server.world();
            state.iter(world).count()
        };

        let account = account.unwrap_or(AccountState {
            credits: 0,
            cargo: Vec::new(),
            cargo_capacity: 0,
        });

        serde_json::json!({
            "tick": self.ticks,
            "alive": true,
            "system": system,
            "docked": docked.as_ref().map(|d| &d.location),
            "docked_location": docked.as_ref().map(|d| serde_json::json!({
                "system": d.system, "id": d.location, "kind": d.kind.as_str(),
            })),
            "ship": {
                "name": ship_name,
                "def_id": ship_id,
                "outfits": equipped,
                "max_shields": ship.max_shields,
                "max_armor": ship.max_armor,
                "max_fuel": ship.max_fuel,
                "weapon_slots": weapon_slots,
                "position": [ship.position.x, ship.position.y],
                "velocity": [ship.velocity.x, ship.velocity.y],
                "heading": ship.rotation,
                "shields": ship.shields,
                "armor": ship.armor,
                "fuel": ship.fuel,
                "credits": account.credits,
                "cargo": account.cargo.iter().cloned().collect::<std::collections::BTreeMap<String, u32>>(),
                "cargo_capacity": account.cargo_capacity,
            },
            "planets": planets,
            "stations": stations,
            "ships": ships,
            "meteorites": meteorites,
            "black_holes": black_holes,
            "market": market,
            "shops": shops,
            "encounter": encounter_json,
            "quests": quests_json,
            "completed_quests": completed_quests,
            "projectiles": projectile_count,
            "notices": notices,
        })
    }
}

fn record_notice(trigger: On<Notice>, mut log: ResMut<NoticeLog>) {
    log.0.push(trigger.event().text.clone());
}

fn record_death(_death: On<PlayerDied>, mut log: ResMut<NoticeLog>) {
    log.0.push("Your ship was destroyed.".to_owned());
}
