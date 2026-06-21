//! Public regional market data: live order book quotes + daily price history.
//!
//! This is the market-*browser* layer (distinct from a character's own orders in
//! [`crate::market`]). It reads the public, region-wide endpoints —
//! `/markets/{region}/orders/` (paginated) and `/markets/{region}/history/` —
//! and reduces them to the figures a trader wants: best buy/sell, spread, and
//! recent price/volume stats. The reductions are pure and unit-tested.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::esi::EsiClient;

/// The Forge — Jita's region, the default market hub.
pub const THE_FORGE: i64 = 10_000_002;

/// One order from the regional order book.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionOrder {
    pub price: f64,
    #[serde(default)]
    pub volume_remain: i64,
    #[serde(default)]
    pub is_buy_order: bool,
    #[serde(default)]
    pub location_id: i64,
}

/// Best-price summary of an item's order book in a region.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketQuote {
    pub best_sell: Option<f64>,
    pub best_buy: Option<f64>,
    /// best_sell - best_buy (only when both sides exist).
    pub spread: Option<f64>,
    /// Spread as a fraction of best_sell (margin), when both sides exist.
    pub spread_pct: Option<f64>,
    pub sell_volume: i64,
    pub buy_volume: i64,
    pub sell_orders: usize,
    pub buy_orders: usize,
}

/// Reduce a region order book to best buy/sell + spread + volumes. Pure.
pub fn quote_from_orders(orders: &[RegionOrder]) -> MarketQuote {
    let mut best_sell: Option<f64> = None;
    let mut best_buy: Option<f64> = None;
    let mut sell_volume = 0;
    let mut buy_volume = 0;
    let mut sell_orders = 0;
    let mut buy_orders = 0;

    for o in orders {
        if o.is_buy_order {
            buy_volume += o.volume_remain;
            buy_orders += 1;
            best_buy = Some(best_buy.map_or(o.price, |b| b.max(o.price)));
        } else {
            sell_volume += o.volume_remain;
            sell_orders += 1;
            best_sell = Some(best_sell.map_or(o.price, |s| s.min(o.price)));
        }
    }

    let (spread, spread_pct) = match (best_sell, best_buy) {
        (Some(s), Some(b)) if s > 0.0 => (Some(s - b), Some((s - b) / s)),
        _ => (None, None),
    };

    MarketQuote {
        best_sell,
        best_buy,
        spread,
        spread_pct,
        sell_volume,
        buy_volume,
        sell_orders,
        buy_orders,
    }
}

/// One day of market history (ESI `/markets/{region}/history/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryDay {
    pub date: String,
    pub average: f64,
    pub highest: f64,
    pub lowest: f64,
    #[serde(default)]
    pub volume: i64,
    #[serde(default)]
    pub order_count: i64,
}

/// Rolled-up history stats for the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryStats {
    /// Most recent day's average price.
    pub last_average: Option<f64>,
    pub avg_30d: f64,
    pub high_30d: f64,
    pub low_30d: f64,
    pub daily_volume_30d: i64,
    /// Average price per day over the last 30 days, oldest first (for a chart).
    pub recent: Vec<f64>,
}

/// Reduce daily history to recent-window stats (last 30 entries). Pure.
pub fn history_stats(days: &[HistoryDay]) -> HistoryStats {
    let window: Vec<&HistoryDay> = days.iter().rev().take(30).collect();
    if window.is_empty() {
        return HistoryStats {
            last_average: None,
            avg_30d: 0.0,
            high_30d: 0.0,
            low_30d: 0.0,
            daily_volume_30d: 0,
            recent: Vec::new(),
        };
    }
    let n = window.len() as f64;
    let avg_30d = window.iter().map(|d| d.average).sum::<f64>() / n;
    let high_30d = window.iter().map(|d| d.highest).fold(f64::MIN, f64::max);
    let low_30d = window.iter().map(|d| d.lowest).fold(f64::MAX, f64::min);
    let total_volume: i64 = window.iter().map(|d| d.volume).sum();
    // window is newest-first; recent should be oldest-first for a left→right chart.
    let recent: Vec<f64> = window.iter().rev().map(|d| d.average).collect();

    HistoryStats {
        last_average: days.last().map(|d| d.average),
        avg_30d,
        high_30d,
        low_30d,
        daily_volume_30d: total_volume / window.len() as i64,
        recent,
    }
}

/// Reads public regional market data.
#[derive(Clone)]
pub struct MarketDataClient {
    esi: EsiClient,
}

impl MarketDataClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    /// Best buy/sell quote for a type in a region.
    pub async fn quote(&self, region_id: i64, type_id: i64) -> Result<MarketQuote> {
        let path = format!("/latest/markets/{region_id}/orders/?order_type=all&type_id={type_id}");
        let orders = self.esi.get_public_json_paged::<RegionOrder>(&path).await?;
        Ok(quote_from_orders(&orders))
    }

    /// Daily price history for a type in a region.
    pub async fn history(&self, region_id: i64, type_id: i64) -> Result<HistoryStats> {
        let path = format!("/latest/markets/{region_id}/history/?type_id={type_id}");
        let days = self.esi.get_public_json::<Vec<HistoryDay>>(&path).await?;
        Ok(history_stats(&days))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(price: f64, vol: i64, buy: bool) -> RegionOrder {
        RegionOrder { price, volume_remain: vol, is_buy_order: buy, location_id: 60003760 }
    }

    #[test]
    fn quote_picks_best_sides_and_spread() {
        let orders = vec![
            order(5.5, 1000, false), // sell
            order(5.2, 500, false),  // sell (better)
            order(5.0, 2000, true),  // buy (best)
            order(4.8, 1000, true),  // buy
        ];
        let q = quote_from_orders(&orders);
        assert_eq!(q.best_sell, Some(5.2));
        assert_eq!(q.best_buy, Some(5.0));
        assert!((q.spread.unwrap() - 0.2).abs() < 1e-9);
        assert_eq!(q.sell_volume, 1500);
        assert_eq!(q.buy_volume, 3000);
        assert_eq!(q.sell_orders, 2);
        assert_eq!(q.buy_orders, 2);
    }

    #[test]
    fn quote_handles_one_sided_book() {
        let q = quote_from_orders(&[order(10.0, 5, false)]);
        assert_eq!(q.best_sell, Some(10.0));
        assert_eq!(q.best_buy, None);
        assert_eq!(q.spread, None);
    }

    #[test]
    fn history_stats_windows_and_orders_chart() {
        let days: Vec<HistoryDay> = (1..=40)
            .map(|i| HistoryDay {
                date: format!("2026-05-{i:02}"),
                average: i as f64,
                highest: i as f64 + 1.0,
                lowest: i as f64 - 1.0,
                volume: 100,
                order_count: 10,
            })
            .collect();
        let s = history_stats(&days);
        assert_eq!(s.last_average, Some(40.0));
        assert_eq!(s.recent.len(), 30);
        // Oldest-first within the 30-day window: day 11..40.
        assert_eq!(s.recent[0], 11.0);
        assert_eq!(*s.recent.last().unwrap(), 40.0);
        assert_eq!(s.high_30d, 41.0);
        assert_eq!(s.low_30d, 10.0);
    }

    #[test]
    fn history_stats_empty_is_zeroed() {
        let s = history_stats(&[]);
        assert_eq!(s.last_average, None);
        assert!(s.recent.is_empty());
    }
}
