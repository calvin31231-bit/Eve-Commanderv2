//! Market orders: the character's open buy/sell orders, with escrow / value
//! rollups and client-side expiry countdowns.
//!
//! An order expires at `issued + duration` days — deterministic once fetched, so
//! the countdown is computed locally (no re-polling). The [`summarize_orders`]
//! derivation is pure and unit-tested.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One open market order (ESI `GET /characters/{id}/orders/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketOrder {
    pub order_id: i64,
    pub type_id: i64,
    #[serde(default)]
    pub region_id: i64,
    #[serde(default)]
    pub location_id: i64,
    #[serde(default)]
    pub is_buy_order: bool,
    pub price: f64,
    #[serde(default)]
    pub volume_total: i64,
    pub volume_remain: i64,
    /// Order lifetime in days from `issued`.
    pub duration: i64,
    pub issued: String,
    /// ISK held in escrow (buy orders only).
    #[serde(default)]
    pub escrow: Option<f64>,
}

impl MarketOrder {
    /// When this order expires (`issued + duration` days), if `issued` parses.
    pub fn expires_at(&self) -> Option<OffsetDateTime> {
        OffsetDateTime::parse(&self.issued, &Rfc3339)
            .ok()
            .map(|issued| issued + Duration::days(self.duration))
    }
}

/// One order flattened for the UI with a client-side expiry countdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderView {
    pub order_id: i64,
    pub type_id: i64,
    pub is_buy_order: bool,
    pub price: f64,
    pub volume_remain: i64,
    pub volume_total: i64,
    pub seconds_remaining: i64,
}

/// Rollup of a character's open orders.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarketSummary {
    pub buy_count: usize,
    pub sell_count: usize,
    /// Total ISK locked in buy-order escrow.
    pub total_escrow: f64,
    /// Total listed value of sell orders (price × volume remaining).
    pub sell_value: f64,
    /// Orders soonest-expiry first.
    pub orders: Vec<OrderView>,
}

/// Summarize open orders: buy/sell counts, escrow and sell value, and per-order
/// countdowns (soonest expiry first). Pure (time injected) → unit-tested.
pub fn summarize_orders(orders: &[MarketOrder], now: OffsetDateTime) -> MarketSummary {
    let mut buy_count = 0;
    let mut sell_count = 0;
    let mut total_escrow = 0.0;
    let mut sell_value = 0.0;

    let mut views: Vec<OrderView> = Vec::with_capacity(orders.len());
    for o in orders {
        if o.is_buy_order {
            buy_count += 1;
            total_escrow += o.escrow.unwrap_or(0.0);
        } else {
            sell_count += 1;
            sell_value += o.price * o.volume_remain as f64;
        }
        let seconds_remaining = o
            .expires_at()
            .map(|e| (e - now).whole_seconds().max(0))
            .unwrap_or(0);
        views.push(OrderView {
            order_id: o.order_id,
            type_id: o.type_id,
            is_buy_order: o.is_buy_order,
            price: o.price,
            volume_remain: o.volume_remain,
            volume_total: o.volume_total,
            seconds_remaining,
        });
    }

    views.sort_by_key(|v| v.seconds_remaining);

    MarketSummary {
        buy_count,
        sell_count,
        total_escrow,
        sell_value,
        orders: views,
    }
}

/// Whether a competing best price beats this order: a sell order is undercut by
/// a strictly lower region best-sell; a buy order is outbid by a strictly
/// higher region best-buy. (The region best may include our own order — a tie
/// therefore never flags, only a strictly better competitor does.) Pure.
pub fn is_beaten(order: &MarketOrder, best_sell: Option<f64>, best_buy: Option<f64>) -> bool {
    if order.is_buy_order {
        best_buy.is_some_and(|b| b > order.price)
    } else {
        best_sell.is_some_and(|b| b < order.price)
    }
}

/// Typed, authenticated market-order reads over the cache-first ESI client.
#[derive(Clone)]
pub struct MarketClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl MarketClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's open market orders.
    pub async fn orders(&self, character_id: i64) -> Result<Vec<MarketOrder>> {
        let ep = endpoint("market_orders")
            .ok_or_else(|| Error::other("unknown endpoint 'market_orders'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<Vec<MarketOrder>>(&ep.path_for(character_id), &token)
            .await
    }

    /// Fetch and summarize open orders with countdowns.
    pub async fn summary(&self, character_id: i64) -> Result<MarketSummary> {
        let orders = self.orders(character_id).await?;
        Ok(summarize_orders(&orders, OffsetDateTime::now_utc()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_market_order() {
        let json = r#"{
            "order_id": 5,
            "type_id": 34,
            "region_id": 10000002,
            "location_id": 60003760,
            "is_buy_order": true,
            "price": 5.5,
            "volume_total": 1000000,
            "volume_remain": 750000,
            "duration": 90,
            "issued": "2026-06-20T00:00:00Z",
            "escrow": 4125000.0
        }"#;
        let o: MarketOrder = serde_json::from_str(json).unwrap();
        assert!(o.is_buy_order);
        assert_eq!(o.volume_remain, 750000);
        assert_eq!(o.escrow, Some(4_125_000.0));
    }

    fn order(id: i64, buy: bool, price: f64, remain: i64, duration: i64, issued: &str, escrow: Option<f64>) -> MarketOrder {
        MarketOrder {
            order_id: id,
            type_id: 34,
            region_id: 10000002,
            location_id: 60003760,
            is_buy_order: buy,
            price,
            volume_total: remain,
            volume_remain: remain,
            duration,
            issued: issued.into(),
            escrow,
        }
    }

    #[test]
    fn summarizes_counts_escrow_value_and_expiry() {
        let now = OffsetDateTime::parse("2026-06-20T00:00:00Z", &Rfc3339).unwrap();
        let orders = vec![
            // buy: escrow 4.125M, issued now, 90d duration
            order(1, true, 5.5, 750_000, 90, "2026-06-20T00:00:00Z", Some(4_125_000.0)),
            // sell: value 100 * 50 = 5000, issued now, 30d duration
            order(2, false, 100.0, 50, 30, "2026-06-20T00:00:00Z", None),
            // sell expiring sooner: 1d duration
            order(3, false, 10.0, 5, 1, "2026-06-20T00:00:00Z", None),
        ];
        let s = summarize_orders(&orders, now);
        assert_eq!(s.buy_count, 1);
        assert_eq!(s.sell_count, 2);
        assert_eq!(s.total_escrow, 4_125_000.0);
        assert_eq!(s.sell_value, 100.0 * 50.0 + 10.0 * 5.0);
        // Soonest expiry first → order 3 (1 day).
        assert_eq!(s.orders[0].order_id, 3);
        assert_eq!(s.orders[0].seconds_remaining, 86_400);
    }

    #[test]
    fn expired_order_clamps_to_zero() {
        let now = OffsetDateTime::parse("2026-07-01T00:00:00Z", &Rfc3339).unwrap();
        // 1-day order issued long before now → expired.
        let orders = vec![order(1, false, 1.0, 1, 1, "2026-06-20T00:00:00Z", None)];
        let s = summarize_orders(&orders, now);
        assert_eq!(s.orders[0].seconds_remaining, 0);
    }

    #[test]
    fn undercut_and_outbid_detection() {
        let sell = order(1, false, 100.0, 5, 10, "2026-06-20T00:00:00Z", None);
        // A strictly cheaper competing sell undercuts; a tie (our own order) doesn't.
        assert!(is_beaten(&sell, Some(99.9), None));
        assert!(!is_beaten(&sell, Some(100.0), None));
        assert!(!is_beaten(&sell, None, None));

        let buy = order(2, true, 50.0, 5, 10, "2026-06-20T00:00:00Z", Some(250.0));
        // A strictly higher competing buy outbids; a tie doesn't.
        assert!(is_beaten(&buy, None, Some(50.1)));
        assert!(!is_beaten(&buy, None, Some(50.0)));
    }
}
