use super::progression::{dock_earth, ready};
use super::*;
use crate::gameplay::market;
use sv_protocol::{DockableTag, PortMarket};

fn market_price(h: &Harness, system: &str, port: &str, commodity: &str) -> Option<u32> {
    market::market_price(h.server.world(), system, port, commodity)
}

fn client_port_market(h: &mut Harness, system: &str, port: &str) -> Option<PortMarket> {
    let mut state = h
        .client
        .world_mut()
        .query_filtered::<(&DockableTag, &PortMarket), With<Remote>>();
    let world = h.client.world();
    state
        .iter(world)
        .find(|(tag, _)| tag.system == system && tag.id == port)
        .map(|(_, prices)| prices.clone())
}

#[test]
fn trades_move_supply_and_ticks_move_prices() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);

    assert_eq!(market_price(&h, "sol", "earth", "food"), Some(8));

    h.send(BuyCommodity {
        commodity: "food".into(),
        qty: 20,
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<Wallet>(entity)
            .is_some_and(|w| w.0.credits == 340)
    });
    assert_eq!(
        market_price(&h, "sol", "earth", "food"),
        Some(8),
        "prices must stay frozen between economy ticks"
    );

    market::force_market_ticks(h.server.world_mut(), 1);
    h.step_until(100, |h| {
        client_port_market(h, "sol", "earth").is_some_and(|m| {
            m.prices
                .iter()
                .any(|p| p.commodity == "food" && p.price == 10)
        })
    });
    assert_eq!(market_price(&h, "sol", "earth", "food"), Some(10));

    h.send(SellCommodity {
        commodity: "food".into(),
        qty: 20,
    });
    h.step_until(100, |h| {
        h.server
            .world()
            .get::<Wallet>(entity)
            .is_some_and(|w| w.0.credits == 540)
    });

    market::force_market_ticks(h.server.world_mut(), 2);
    h.step_until(100, |h| {
        client_port_market(h, "sol", "earth").is_some_and(|m| {
            m.prices
                .iter()
                .any(|p| p.commodity == "food" && p.price == 8)
        })
    });
    assert_eq!(market_price(&h, "sol", "earth", "food"), Some(8));
}

#[test]
fn news_fires_only_on_large_tick_moves() {
    let (mut h, _entity) = ready();
    dock_earth(&mut h);

    h.send(BuyCommodity {
        commodity: "food".into(),
        qty: 1,
    });
    for _ in 0..10 {
        h.step();
    }
    market::force_market_ticks(h.server.world_mut(), 1);
    for _ in 0..20 {
        h.step();
    }
    let notices = h.drain_notices();
    assert!(
        !notices
            .iter()
            .any(|n| n.contains("runs scarce") || n.contains("floods the docks")),
        "a one-unit trade must not make the news, got {notices:?}"
    );

    h.send(SellCommodity {
        commodity: "food".into(),
        qty: 1,
    });
    for _ in 0..10 {
        h.step();
    }
    h.drain_notices();

    h.send(BuyCommodity {
        commodity: "food".into(),
        qty: 20,
    });
    for _ in 0..10 {
        h.step();
    }
    market::force_market_ticks(h.server.world_mut(), 1);
    h.step_until(200, |h| {
        h.drain_notices().iter().any(|n| n.contains("runs scarce"))
    });
}

#[test]
fn npc_scale_deliveries_flood_ports_and_drop_prices() {
    let (mut h, _entity) = ready();

    h.server
        .world_mut()
        .resource_mut::<market::MarketState>()
        .shift_supply(
            market::MarketKey {
                system: "sol".into(),
                port: "wayfarer-exchange".into(),
                commodity: "food".into(),
            },
            30,
            30,
        );
    assert_eq!(
        market_price(&h, "sol", "wayfarer-exchange", "food"),
        Some(18),
        "supply alone must not move the frozen price"
    );

    market::force_market_ticks(h.server.world_mut(), 1);
    assert_eq!(
        market_price(&h, "sol", "wayfarer-exchange", "food"),
        Some(13),
        "a 30-unit delivery must sink the food price at Wayfarer"
    );
    h.step_until(200, |h| {
        h.drain_notices()
            .iter()
            .any(|n| n.contains("floods the docks"))
    });
}

#[test]
fn retired_commodities_drop_from_the_market_at_ticks() {
    let (mut h, entity) = ready();
    dock_earth(&mut h);

    assert_eq!(market_price(&h, "sol", "earth", "ore"), Some(40));
    h.server
        .world_mut()
        .resource_scope(|_, mut content: Mut<Content>| {
            if let Some(earth) = content
                .0
                .systems
                .iter_mut()
                .find(|s| s.id == "sol")
                .and_then(|s| s.planets.first_mut())
            {
                earth.commodities.retain(|c| c.id != "ore");
            }
        });

    market::force_market_ticks(h.server.world_mut(), 1);
    assert_eq!(
        market_price(&h, "sol", "earth", "ore"),
        None,
        "retired entries must leave the market book at the tick"
    );

    h.send(BuyCommodity {
        commodity: "ore".into(),
        qty: 1,
    });
    h.step_until(100, |h| {
        h.drain_notices()
            .iter()
            .any(|n| n.contains("never heard of ore"))
    });
    assert!(
        h.server
            .world()
            .get::<Wallet>(entity)
            .is_some_and(|w| w.0.credits == 500),
        "a retired commodity must not be buyable"
    );
}

#[test]
fn market_state_restores_from_storage_with_supply_prices_and_candles() {
    let db = crate::SqliteStorage::open_memory().expect("storage opens");
    db.save_market(&crate::MarketSnapshot {
        elapsed_secs: 86_430.0,
        supplies: vec![crate::MarketSupply {
            system: "sol".into(),
            port: "earth".into(),
            commodity: "food".into(),
            supply: -40,
        }],
        candles: vec![crate::MarketCandle {
            day: 1,
            system: "sol".into(),
            port: "earth".into(),
            commodity: "food".into(),
            open: 8,
            high: 12,
            low: 8,
            close: 12,
            volume: 60,
        }],
    })
    .expect("snapshot saves");

    let storage = crate::gameplay::StorageResource {
        storage: Some(Box::new(db)),
    };
    let content_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content");
    let pack = sv_data::ContentPack::load_dir(&content_root).expect("pack loads");
    let state = market::init_market_state(&storage, &pack);

    assert_eq!(state.day, 1);
    let food_key = market::MarketKey {
        system: "sol".into(),
        port: "earth".into(),
        commodity: "food".into(),
    };
    assert_eq!(
        state.prices[&food_key], 12,
        "restored supply must price through the curve"
    );
    assert_eq!(
        state
            .candles
            .get(&food_key)
            .map(|candle| (candle.open, candle.high, candle.volume)),
        Some((8, 12, 60)),
        "the current day's candle must restore into live state"
    );
}
