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

/// The major trade-hub regions for cross-hub price comparison.
pub const HUBS: &[(i64, &str)] = &[
    (10_000_002, "Jita"),
    (10_000_043, "Amarr"),
    (10_000_032, "Dodixie"),
    (10_000_030, "Rens"),
    (10_000_042, "Hek"),
];

/// Best buy/sell for an item at one trade hub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HubQuote {
    pub hub: String,
    pub best_sell: Option<f64>,
    pub best_buy: Option<f64>,
}

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

/// Broker fee + sales tax assumptions for station-trade profit math. Fractions
/// (0.03 = 3%). Defaults are typical mid-skill highsec values; the UI lets the
/// user override them.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TradeFees {
    /// Broker fee charged when placing an order (applied to both the buy and
    /// the sell order in a station flip).
    pub broker_fee: f64,
    /// Sales tax charged on the value of a sell order.
    pub sales_tax: f64,
}

impl Default for TradeFees {
    fn default() -> Self {
        Self { broker_fee: 0.03, sales_tax: 0.045 }
    }
}

/// Per-unit station-trade economics for one item (buy low / sell high in place).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeMetrics {
    pub buy_price: f64,
    pub sell_price: f64,
    /// Net profit as a fraction of acquisition cost (after fees + tax).
    pub margin_pct: f64,
    /// Net ISK profit per unit flipped.
    pub profit_per_unit: f64,
    pub daily_volume: i64,
    /// Rough daily profit ceiling = profit_per_unit × daily_volume.
    pub daily_potential: f64,
}

/// Station-flip economics for one item from its quote. Returns `None` when the
/// book is one-sided or prices are non-positive. Pure.
pub fn trade_metrics(quote: &MarketQuote, daily_volume: i64, fees: TradeFees) -> Option<TradeMetrics> {
    let buy = quote.best_buy?;
    let sell = quote.best_sell?;
    if buy <= 0.0 || sell <= 0.0 {
        return None;
    }
    // Acquire by outbidding the best buy order (pay the broker fee on it);
    // realise by undercutting the best sell (pay broker fee + sales tax).
    let cost = buy * (1.0 + fees.broker_fee);
    let revenue = sell * (1.0 - fees.broker_fee - fees.sales_tax);
    let profit = revenue - cost;
    let margin_pct = if cost > 0.0 { profit / cost } else { 0.0 };
    Some(TradeMetrics {
        buy_price: buy,
        sell_price: sell,
        margin_pct,
        profit_per_unit: profit,
        daily_volume,
        daily_potential: profit * daily_volume as f64,
    })
}

/// A profitable station-trade candidate (type + its economics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradeOpportunity {
    pub type_id: i64,
    pub metrics: TradeMetrics,
}

/// The best cross-hub buy-low / sell-high flip for an item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HubArbitrage {
    /// Hub to buy at (lowest sell price).
    pub buy_hub: String,
    /// Hub to sell at (highest buy price).
    pub sell_hub: String,
    /// Price paid per unit (best sell at `buy_hub`).
    pub buy_price: f64,
    /// Price received per unit (best buy at `sell_hub`).
    pub sell_price: f64,
    /// Net profit per unit after sales tax on the sale.
    pub profit_per_unit: f64,
    /// Profit as a fraction of the buy price.
    pub margin_pct: f64,
}

/// Find the best cross-hub flip from per-hub quotes: buy where the sell price is
/// lowest, sell where the buy price is highest, net of sales tax. Returns `None`
/// unless the two hubs differ and the flip is profitable. Pure.
pub fn best_arbitrage(hubs: &[HubQuote], fees: TradeFees) -> Option<HubArbitrage> {
    // Cheapest place to buy (lowest best_sell) and richest place to sell
    // (highest best_buy).
    let buy = hubs
        .iter()
        .filter_map(|h| h.best_sell.map(|p| (h, p)))
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;
    let sell = hubs
        .iter()
        .filter_map(|h| h.best_buy.map(|p| (h, p)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;

    // A real haul moves between two different hubs.
    if buy.0.hub == sell.0.hub {
        return None;
    }
    let buy_price = buy.1;
    let sell_price = sell.1;
    let profit = sell_price * (1.0 - fees.sales_tax) - buy_price;
    if profit <= 0.0 || buy_price <= 0.0 {
        return None;
    }
    Some(HubArbitrage {
        buy_hub: buy.0.hub.clone(),
        sell_hub: sell.0.hub.clone(),
        buy_price,
        sell_price,
        profit_per_unit: profit,
        margin_pct: profit / buy_price,
    })
}

/// A profitable cross-hub haul candidate (type + its best flip).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub type_id: i64,
    pub flip: HubArbitrage,
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

    /// Best buy/sell for a type across the major trade hubs. Best-effort: a hub
    /// that fails to fetch is reported with empty sides rather than aborting the
    /// whole comparison.
    pub async fn compare(&self, type_id: i64) -> Result<Vec<HubQuote>> {
        let mut out = Vec::with_capacity(HUBS.len());
        for &(region, hub) in HUBS {
            let (best_sell, best_buy) = match self.quote(region, type_id).await {
                Ok(q) => (q.best_sell, q.best_buy),
                Err(e) => {
                    tracing::warn!("compare: {hub} quote failed: {e}");
                    (None, None)
                }
            };
            out.push(HubQuote { hub: hub.to_string(), best_sell, best_buy });
        }
        Ok(out)
    }

    /// Scan a set of types in a region for station-trade opportunities. Fetches
    /// each item's quote + daily volume best-effort (skipping ones that fail or
    /// are one-sided) and returns the profitable flips sorted by daily profit
    /// potential, highest first.
    pub async fn scan(
        &self,
        region_id: i64,
        type_ids: &[i64],
        fees: TradeFees,
    ) -> Result<Vec<TradeOpportunity>> {
        let mut out = Vec::new();
        for &type_id in type_ids {
            let quote = match self.quote(region_id, type_id).await {
                Ok(q) => q,
                Err(e) => {
                    tracing::warn!("scan: quote {type_id} failed: {e}");
                    continue;
                }
            };
            let daily_volume = self
                .history(region_id, type_id)
                .await
                .map(|h| h.daily_volume_30d)
                .unwrap_or(0);
            if let Some(m) = trade_metrics(&quote, daily_volume, fees) {
                if m.profit_per_unit > 0.0 {
                    out.push(TradeOpportunity { type_id, metrics: m });
                }
            }
        }
        out.sort_by(|a, b| {
            b.metrics
                .daily_potential
                .partial_cmp(&a.metrics.daily_potential)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    /// Scan a set of types for the best cross-hub flip per item. Fetches each
    /// item's per-hub quotes best-effort (skipping ones that fail or have no
    /// profitable flip) and returns opportunities sorted by per-unit profit,
    /// highest first.
    pub async fn arbitrage(
        &self,
        type_ids: &[i64],
        fees: TradeFees,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        let mut out = Vec::new();
        for &type_id in type_ids {
            let hubs = match self.compare(type_id).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!("arbitrage: compare {type_id} failed: {e}");
                    continue;
                }
            };
            if let Some(flip) = best_arbitrage(&hubs, fees) {
                out.push(ArbitrageOpportunity { type_id, flip });
            }
        }
        out.sort_by(|a, b| {
            b.flip
                .profit_per_unit
                .partial_cmp(&a.flip.profit_per_unit)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
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

    #[test]
    fn trade_metrics_nets_fees_and_tax() {
        // buy 100, sell 200, 3% broker, 4.5% tax.
        let q = quote_from_orders(&[order(200.0, 10, false), order(100.0, 10, true)]);
        let m = trade_metrics(&q, 500, TradeFees::default()).unwrap();
        // cost = 100 * 1.03 = 103; revenue = 200 * (1 - 0.075) = 185; profit = 82.
        assert!((m.profit_per_unit - 82.0).abs() < 1e-9);
        assert!((m.margin_pct - 82.0 / 103.0).abs() < 1e-9);
        assert!((m.daily_potential - 82.0 * 500.0).abs() < 1e-9);
    }

    #[test]
    fn trade_metrics_none_on_one_sided_book() {
        let q = quote_from_orders(&[order(10.0, 5, false)]);
        assert!(trade_metrics(&q, 100, TradeFees::default()).is_none());
    }

    #[test]
    fn arbitrage_picks_cheapest_buy_and_richest_sell() {
        let hubs = vec![
            HubQuote { hub: "Jita".into(), best_sell: Some(100.0), best_buy: Some(95.0) },
            HubQuote { hub: "Amarr".into(), best_sell: Some(140.0), best_buy: Some(130.0) },
            HubQuote { hub: "Hek".into(), best_sell: Some(110.0), best_buy: Some(90.0) },
        ];
        // Buy Jita @100, sell Amarr @130, 4.5% tax → 130*0.955 - 100 = 24.15.
        let a = best_arbitrage(&hubs, TradeFees::default()).unwrap();
        assert_eq!(a.buy_hub, "Jita");
        assert_eq!(a.sell_hub, "Amarr");
        assert!((a.profit_per_unit - 24.15).abs() < 1e-6);
    }

    #[test]
    fn arbitrage_none_when_unprofitable_or_same_hub() {
        // Best buy and best sell are the same hub → no haul.
        let single = vec![HubQuote { hub: "Jita".into(), best_sell: Some(100.0), best_buy: Some(99.0) }];
        assert!(best_arbitrage(&single, TradeFees::default()).is_none());

        // Cross-hub but spread doesn't beat the tax.
        let thin = vec![
            HubQuote { hub: "Jita".into(), best_sell: Some(100.0), best_buy: Some(80.0) },
            HubQuote { hub: "Amarr".into(), best_sell: Some(105.0), best_buy: Some(101.0) },
        ];
        // 101*0.955 - 100 = -3.5 → None.
        assert!(best_arbitrage(&thin, TradeFees::default()).is_none());
    }
}
