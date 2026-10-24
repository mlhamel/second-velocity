pub const SUPPLY_SENSITIVITY: i32 = 64;
pub const PRICE_FLOOR_MULT: f32 = 0.5;
pub const PRICE_CEIL_MULT: f32 = 2.0;
pub const NEWS_MOVE_NUM: u32 = 15;
pub const NEWS_MOVE_DEN: u32 = 100;

/// Tick-frozen price from a signed supply counter. Positive supply (flooded
/// market) pushes below the list price; negative (drained market) above it.
/// The result is clamped to [0.5x, 2.0x] list and never drops below 1.
pub fn price(list_price: u32, supply: i32) -> u32 {
    if list_price == 0 {
        return 0;
    }
    let mult = f32::exp2(-(supply as f32) / (SUPPLY_SENSITIVITY as f32))
        .clamp(PRICE_FLOOR_MULT, PRICE_CEIL_MULT);
    ((list_price as f32) * mult).round().max(1.0) as u32
}

/// Mean reversion for the arbitrage damping: each economy tick removes about
/// a tenth of the counter, always at least one unit, without overshooting
/// past zero.
pub fn decay(supply: i32) -> i32 {
    if supply == 0 {
        return 0;
    }
    let step = (supply.abs() + 9) / 10;
    supply - supply.signum() * step
}

/// Whether a tick-over-tick price move is large enough to broadcast as news.
pub fn is_news_worthy(old_price: u32, new_price: u32) -> bool {
    old_price > 0 && old_price.abs_diff(new_price) * NEWS_MOVE_DEN >= old_price * NEWS_MOVE_NUM
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candle {
    pub open: u32,
    pub high: u32,
    pub low: u32,
    pub close: u32,
    pub volume: u32,
}

impl Candle {
    pub fn new(open: u32) -> Self {
        Self {
            open,
            high: open,
            low: open,
            close: open,
            volume: 0,
        }
    }

    pub fn record_price(&mut self, price: u32) {
        self.close = price;
        self.high = self.high.max(price);
        self.low = self.low.min(price);
    }

    pub fn record_volume(&mut self, qty: u32) {
        self.volume = self.volume.saturating_add(qty);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_supply_trades_at_list() {
        assert_eq!(price(8, 0), 8);
        assert_eq!(price(45, 0), 45);
        assert_eq!(price(0, 0), 0);
    }

    #[test]
    fn flood_lowers_and_drain_raises() {
        assert!(price(8, 20) < 8, "flooded market must trade below list");
        assert!(price(8, -20) > 8, "drained market must trade above list");
        assert!(price(8, 20) < price(8, 10));
        assert!(price(8, -10) < price(8, -20));
    }

    #[test]
    fn price_is_clamped_to_half_and_double_list() {
        assert_eq!(price(8, 1_000_000), 4);
        assert_eq!(price(8, -1_000_000), 16);
        assert_eq!(price(45, i32::MAX), 23);
        assert_eq!(price(45, i32::MIN), 90);
    }

    #[test]
    fn price_never_drops_below_one() {
        assert_eq!(price(1, 1_000_000), 1);
        assert_eq!(price(1, 0), 1);
    }

    #[test]
    fn decay_pulls_both_signs_toward_zero_without_overshoot() {
        assert_eq!(decay(100), 90);
        assert_eq!(decay(-100), -90);
        assert_eq!(decay(10), 9);
        assert_eq!(decay(5), 4);
        assert_eq!(decay(1), 0);
        assert_eq!(decay(-1), 0);
        assert_eq!(decay(0), 0);
        let mut supply = -97;
        for _ in 0..64 {
            supply = decay(supply);
        }
        assert_eq!(supply, 0, "long decay chains must terminate at zero");
    }

    #[test]
    fn news_fires_at_fifteen_percent_or_more() {
        assert!(is_news_worthy(8, 10));
        assert!(is_news_worthy(100, 115));
        assert!(!is_news_worthy(100, 114));
        assert!(!is_news_worthy(8, 9));
        assert!(!is_news_worthy(8, 8));
        assert!(!is_news_worthy(0, 10), "no baseline price means no news");
    }

    #[test]
    fn candle_tracks_high_low_close_and_volume() {
        let mut candle = Candle::new(8);
        candle.record_price(12);
        candle.record_price(6);
        candle.record_price(9);
        candle.record_volume(20);
        candle.record_volume(5);
        assert_eq!(
            candle,
            Candle {
                open: 8,
                high: 12,
                low: 6,
                close: 9,
                volume: 25,
            }
        );
    }

    #[test]
    fn candle_volume_saturates_instead_of_overflowing() {
        let mut candle = Candle::new(8);
        candle.record_volume(u32::MAX);
        candle.record_volume(1);
        assert_eq!(candle.volume, u32::MAX);
    }
}
