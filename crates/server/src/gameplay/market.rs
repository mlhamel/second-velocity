use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use sv_core::market as math;
use sv_core::market::Candle;
use sv_data::{CommodityPrice, ContentPack};
use sv_protocol::{DockableTag, MarketPrice, Notice, PortMarket};

use super::StorageResource;
use crate::Content;
use crate::storage::{MarketCandle, MarketSnapshot, MarketSupply};

pub const ECONOMY_TICK_SECS: f32 = 30.0;
const SECONDS_PER_DAY: f32 = 86_400.0;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MarketKey {
    pub system: String,
    pub port: String,
    pub commodity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketChange {
    pub key: MarketKey,
    pub old: u32,
    pub new: u32,
}

#[derive(Resource, Debug, Default)]
pub struct MarketState {
    pub supplies: BTreeMap<MarketKey, i32>,
    pub prices: BTreeMap<MarketKey, u32>,
    pub candles: BTreeMap<MarketKey, Candle>,
    pub dirty: BTreeSet<MarketKey>,
    pub news: Vec<String>,
    pending_flush: Vec<MarketCandle>,
    pub day: u32,
    pub elapsed_secs: f32,
    tick_accum: f32,
}

impl MarketState {
    /// The tick-frozen price a trade executes at. Content list price is the
    /// fallback for entries the boot-time seeding has not covered.
    pub fn price(
        &self,
        pack: &ContentPack,
        system: &str,
        port: &str,
        commodity: &str,
    ) -> Option<u32> {
        let key = MarketKey {
            system: system.to_owned(),
            port: port.to_owned(),
            commodity: commodity.to_owned(),
        };
        self.prices
            .get(&key)
            .copied()
            .or_else(|| list_price(pack, system, port, commodity))
    }

    /// Moves a port-commodity counter and records traded volume into the
    /// running candle. Entries without a seeded price are ignored: the market
    /// only tracks commodities the port actually lists.
    pub fn shift_supply(&mut self, key: MarketKey, supply_delta: i32, volume: u32) {
        if !self.prices.contains_key(&key) {
            return;
        }
        if supply_delta != 0 {
            let next = self
                .supplies
                .get(&key)
                .copied()
                .unwrap_or(0)
                .saturating_add(supply_delta);
            if next == 0 {
                self.supplies.remove(&key);
            } else {
                self.supplies.insert(key.clone(), next);
            }
        }
        if volume > 0 {
            let open = self
                .prices
                .get(&key)
                .copied()
                .expect("price seeded before supply shift");
            self.candles
                .entry(key.clone())
                .or_insert_with(|| Candle::new(open))
                .record_volume(volume);
            self.dirty.insert(key);
        }
    }

    /// One economy tick: decay supplies, recompute prices, roll the sim day,
    /// collect news. Prices are frozen between ticks by design.
    pub fn run_tick(&mut self, pack: &ContentPack) -> Vec<MarketChange> {
        self.elapsed_secs += ECONOMY_TICK_SECS;
        let new_day = (self.elapsed_secs / SECONDS_PER_DAY) as u32;
        if new_day != self.day {
            for (key, candle) in &self.candles {
                self.pending_flush
                    .push(row_from_candle(self.day, key, candle));
            }
            self.day = new_day;
            let keys: Vec<MarketKey> = self.candles.keys().cloned().collect();
            for key in keys {
                if let Some(price) = self.prices.get(&key).copied() {
                    self.candles.insert(key.clone(), Candle::new(price));
                    self.dirty.insert(key);
                } else {
                    self.candles.remove(&key);
                }
            }
        }

        let mut changes = Vec::new();
        let keys: Vec<MarketKey> = self.prices.keys().cloned().collect();
        for key in keys {
            let Some(list) = list_price(pack, &key.system, &key.port, &key.commodity) else {
                self.prices.remove(&key);
                self.supplies.remove(&key);
                continue;
            };
            let supply = match self.supplies.get(&key).copied() {
                Some(value) => math::decay(value),
                None => 0,
            };
            if supply == 0 {
                self.supplies.remove(&key);
            } else {
                self.supplies.insert(key.clone(), supply);
            }
            let fresh = math::price(list, supply);
            let old = self.prices.insert(key.clone(), fresh).unwrap_or(list);
            if fresh != old {
                changes.push(MarketChange {
                    key: key.clone(),
                    old,
                    new: fresh,
                });
                self.candles
                    .entry(key.clone())
                    .or_insert_with(|| Candle::new(list))
                    .record_price(fresh);
                self.dirty.insert(key.clone());
            }
            if math::is_news_worthy(old, fresh) {
                self.news.push(format_news(pack, &key, old, fresh));
            }
        }
        let stale: Vec<MarketKey> = self
            .supplies
            .keys()
            .chain(self.candles.keys())
            .filter(|key| !self.prices.contains_key(*key))
            .cloned()
            .collect();
        for key in stale {
            self.supplies.remove(&key);
            self.candles.remove(&key);
        }
        changes
    }
}

pub fn economy_tick(
    time: Res<Time>,
    mut market: ResMut<MarketState>,
    content: Res<Content>,
    storage: Res<StorageResource>,
    mut ports: Query<(&DockableTag, &mut PortMarket)>,
) {
    market.tick_accum += time.delta_secs();
    while market.tick_accum >= ECONOMY_TICK_SECS {
        market.tick_accum -= ECONOMY_TICK_SECS;
        let changes = market.run_tick(&content.0);
        sync_port_markets(&market, &content.0, &changes, &mut ports);
        persist_market(&mut market, &storage);
    }
}

pub fn flush_market_news(mut commands: Commands, mut market: ResMut<MarketState>) {
    for text in market.news.drain(..) {
        info!("server: market news: {text}");
        commands.server_trigger(ToClients {
            targets: SendTargets::All,
            message: Notice { text },
        });
    }
}

/// Scenario pacing: runs the same tick the scheduled system runs, without
/// waiting on wall time.
pub fn force_market_ticks(world: &mut World, ticks: u32) {
    for _ in 0..ticks {
        let changes = world.resource_scope(|world, mut market: Mut<MarketState>| {
            let content = world.resource::<Content>();
            market.run_tick(&content.0)
        });
        let updates = world.resource_scope(|world, market: Mut<MarketState>| {
            let content = world.resource::<Content>();
            let mut changed = BTreeSet::new();
            for change in &changes {
                changed.insert((change.key.system.clone(), change.key.port.clone()));
            }
            let mut updates = BTreeMap::new();
            for (system, port) in changed {
                updates.insert(
                    (system.clone(), port.clone()),
                    port_prices(&market, &content.0, &system, &port),
                );
            }
            updates
        });
        let mut ports = world.query::<(&DockableTag, &mut PortMarket)>();
        for (tag, mut port_market) in ports.iter_mut(world) {
            if let Some(prices) = updates.get(&(tag.system.clone(), tag.id.clone())) {
                port_market.prices = prices.clone();
            }
        }
        world.resource_scope(|world, mut market: Mut<MarketState>| {
            let storage = world.resource::<StorageResource>();
            persist_market(&mut market, storage);
        });
    }
}

/// Current tick-frozen price for tests, scenarios, and MCP assertions.
pub fn market_price(world: &World, system: &str, port: &str, commodity: &str) -> Option<u32> {
    let market = world.resource::<MarketState>();
    let content = world.resource::<Content>();
    market.price(&content.0, system, port, commodity)
}

pub fn init_market_state(storage: &StorageResource, pack: &ContentPack) -> MarketState {
    let mut market = MarketState::default();
    let mut restored_candles = Vec::new();
    if let Some(db) = storage.storage.as_ref() {
        match db.load_market() {
            Ok(snapshot) => {
                market.elapsed_secs = snapshot.elapsed_secs;
                market.day = (snapshot.elapsed_secs / SECONDS_PER_DAY) as u32;
                for row in snapshot.supplies {
                    let key = MarketKey {
                        system: row.system,
                        port: row.port,
                        commodity: row.commodity,
                    };
                    market.supplies.insert(key, row.supply);
                }
                restored_candles = snapshot.candles;
            }
            Err(error) => warn!("market restore failed; starting fresh: {error}"),
        }
    }
    let seed =
        |market: &mut MarketState, system_id: &str, port_id: &str, entries: &[CommodityPrice]| {
            for entry in entries {
                let key = MarketKey {
                    system: system_id.to_owned(),
                    port: port_id.to_owned(),
                    commodity: entry.id.clone(),
                };
                let supply = market.supplies.get(&key).copied().unwrap_or(0);
                market.prices.insert(key, math::price(entry.price, supply));
            }
        };
    for system in pack.systems.iter() {
        for planet in &system.planets {
            seed(&mut market, &system.id, &planet.id, &planet.commodities);
        }
        for station in &system.stations {
            seed(&mut market, &system.id, &station.id, &station.commodities);
        }
        for (_, moon) in system.landable_moons() {
            seed(&mut market, &system.id, &moon.id, &moon.commodities);
        }
    }
    market
        .supplies
        .retain(|key, _| market.prices.contains_key(key));
    for row in restored_candles {
        if row.day != market.day {
            continue;
        }
        let key = MarketKey {
            system: row.system,
            port: row.port,
            commodity: row.commodity,
        };
        if !market.prices.contains_key(&key) {
            continue;
        }
        market.candles.insert(
            key,
            Candle {
                open: row.open,
                high: row.high,
                low: row.low,
                close: row.close,
                volume: row.volume,
            },
        );
    }
    market
}

/// Price list for a port's replicated component, in content order.
pub fn port_prices(
    market: &MarketState,
    pack: &ContentPack,
    system: &str,
    port: &str,
) -> Vec<MarketPrice> {
    let Some(dockable) = pack.dockable(system, port) else {
        return Vec::new();
    };
    dockable
        .commodities()
        .iter()
        .map(|entry| MarketPrice {
            commodity: entry.id.clone(),
            price: market
                .prices
                .get(&MarketKey {
                    system: system.to_owned(),
                    port: port.to_owned(),
                    commodity: entry.id.clone(),
                })
                .copied()
                .unwrap_or(entry.price),
        })
        .collect()
}

fn sync_port_markets(
    market: &MarketState,
    pack: &ContentPack,
    changes: &[MarketChange],
    ports: &mut Query<(&DockableTag, &mut PortMarket)>,
) {
    if changes.is_empty() {
        return;
    }
    let changed: BTreeSet<(String, String)> = changes
        .iter()
        .map(|change| (change.key.system.clone(), change.key.port.clone()))
        .collect();
    for (tag, mut port_market) in ports.iter_mut() {
        if changed.contains(&(tag.system.clone(), tag.id.clone())) {
            port_market.prices = port_prices(market, pack, &tag.system, &tag.id);
        }
    }
}

fn persist_market(market: &mut MarketState, storage: &StorageResource) {
    let Some(db) = storage.storage.as_ref() else {
        market.pending_flush.clear();
        return;
    };
    let pending = std::mem::take(&mut market.pending_flush);
    let dirty_rows: Vec<MarketCandle> = market
        .dirty
        .iter()
        .filter_map(|key| {
            market
                .candles
                .get(key)
                .map(|candle| row_from_candle(market.day, key, candle))
        })
        .collect();
    let snapshot = MarketSnapshot {
        elapsed_secs: market.elapsed_secs,
        supplies: market
            .supplies
            .iter()
            .map(|(key, supply)| MarketSupply {
                system: key.system.clone(),
                port: key.port.clone(),
                commodity: key.commodity.clone(),
                supply: *supply,
            })
            .collect(),
        candles: pending.iter().chain(dirty_rows.iter()).cloned().collect(),
    };
    match db.save_market(&snapshot) {
        Ok(()) => market.dirty.clear(),
        Err(error) => {
            warn!("market save failed: {error}");
            market.pending_flush = pending;
        }
    }
}

fn row_from_candle(day: u32, key: &MarketKey, candle: &Candle) -> MarketCandle {
    MarketCandle {
        day,
        system: key.system.clone(),
        port: key.port.clone(),
        commodity: key.commodity.clone(),
        open: candle.open,
        high: candle.high,
        low: candle.low,
        close: candle.close,
        volume: candle.volume,
    }
}

fn list_price(pack: &ContentPack, system: &str, port: &str, commodity: &str) -> Option<u32> {
    pack.dockable(system, port)?
        .commodities()
        .iter()
        .find(|entry| entry.id == commodity)
        .map(|entry| entry.price)
}

fn format_news(pack: &ContentPack, key: &MarketKey, old: u32, fresh: u32) -> String {
    let port_name = pack
        .dockable(&key.system, &key.port)
        .map(|port| port.name())
        .unwrap_or(&key.port);
    let system_name = pack
        .system(&key.system)
        .map(|system| system.name.as_str())
        .unwrap_or(&key.system);
    let commodity_name = pack
        .commodity(&key.commodity)
        .map(|commodity| commodity.name.as_str())
        .unwrap_or(&key.commodity);
    if fresh > old {
        format!(
            "{commodity_name} runs scarce at {port_name} ({system_name}): brokers ask {fresh} cr, up from {old}."
        )
    } else {
        format!(
            "{commodity_name} floods the docks at {port_name} ({system_name}): prices sink to {fresh} cr, down from {old}."
        )
    }
}
