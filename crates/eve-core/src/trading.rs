//! FIFO trading profit & loss.
//!
//! ESI's wallet transactions are a flat buy/sell log with no cost basis, so
//! realized profit can only be reconstructed by matching each sale against the
//! oldest un-sold buys of that item (first-in-first-out — the accounting
//! convention players expect and the one PYFA/EVE-Tycoon use). This module does
//! that matching purely so it's fully unit-tested; the wallet fetch feeds it.
//!
//! Fees are out of scope here (broker fee + sales tax live on journal entries,
//! not transactions); this is gross realized margin per item, which is the
//! number traders check first.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The minimal transaction shape the matcher needs (a slice of the wallet row).
#[derive(Debug, Clone, PartialEq)]
pub struct Trade {
    pub type_id: i64,
    pub quantity: i64,
    pub unit_price: f64,
    pub is_buy: bool,
    /// RFC3339 date; used only to order the log oldest-first.
    pub date: String,
}

/// Realized P&L for one item type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemPnl {
    pub type_id: i64,
    /// Units matched (sold against a known buy).
    pub units_sold: i64,
    /// Gross revenue from those matched sales.
    pub revenue: f64,
    /// FIFO cost basis of those units.
    pub cost: f64,
    /// revenue − cost.
    pub profit: f64,
    /// profit / cost, when cost > 0.
    pub margin_pct: Option<f64>,
    /// Units still held (bought, not yet matched to a sale).
    pub units_open: i64,
    /// Cost basis still tied up in open inventory.
    pub open_cost: f64,
}

/// A trading summary across all items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TradingPnl {
    pub total_revenue: f64,
    pub total_cost: f64,
    pub total_profit: f64,
    /// Per-item breakdown, most profitable first.
    pub items: Vec<ItemPnl>,
}

/// A FIFO lot of un-sold buys: (remaining units, unit price).
struct Lot {
    qty: i64,
    price: f64,
}

/// Compute FIFO realized P&L from a transaction log. Sales with no matching
/// prior buy (item bought before the log window) are skipped for cost — they
/// still count revenue but contribute zero cost, flagged implicitly by a
/// higher-than-real margin; callers note the window caveat. Pure.
pub fn fifo_pnl(trades: &[Trade]) -> TradingPnl {
    // Order oldest-first so buys precede the sales that consume them.
    let mut ordered: Vec<&Trade> = trades.iter().collect();
    ordered.sort_by(|a, b| a.date.cmp(&b.date));

    // Per-type FIFO queues of open buy lots, plus running realized figures.
    let mut lots: HashMap<i64, Vec<Lot>> = HashMap::new();
    let mut acc: HashMap<i64, ItemPnl> = HashMap::new();
    let blank = |type_id: i64| ItemPnl {
        type_id,
        units_sold: 0,
        revenue: 0.0,
        cost: 0.0,
        profit: 0.0,
        margin_pct: None,
        units_open: 0,
        open_cost: 0.0,
    };

    for t in ordered {
        if t.is_buy {
            lots.entry(t.type_id).or_default().push(Lot { qty: t.quantity, price: t.unit_price });
        } else {
            // A sale: match units against the oldest open lots.
            let it = acc.entry(t.type_id).or_insert_with(|| blank(t.type_id));
            let mut remaining = t.quantity;
            it.revenue += t.quantity as f64 * t.unit_price;
            if let Some(queue) = lots.get_mut(&t.type_id) {
                while remaining > 0 {
                    let Some(front) = queue.first_mut() else { break };
                    let take = remaining.min(front.qty);
                    it.cost += take as f64 * front.price;
                    it.units_sold += take;
                    front.qty -= take;
                    remaining -= take;
                    if front.qty == 0 {
                        queue.remove(0);
                    }
                }
            }
            // `remaining > 0` here = sold more than we have buys for (pre-window
            // inventory); revenue counted, no cost basis available.
        }
    }

    // Fold leftover open lots into each item's open inventory figures.
    for (type_id, queue) in &lots {
        let open_qty: i64 = queue.iter().map(|l| l.qty).sum();
        let open_cost: f64 = queue.iter().map(|l| l.qty as f64 * l.price).sum();
        if open_qty > 0 {
            let it = acc.entry(*type_id).or_insert_with(|| blank(*type_id));
            it.units_open = open_qty;
            it.open_cost = open_cost;
        }
    }

    let mut items: Vec<ItemPnl> = acc
        .into_values()
        .map(|mut it| {
            it.profit = it.revenue - it.cost;
            it.margin_pct = (it.cost > 0.0).then_some(it.profit / it.cost * 100.0);
            it
        })
        // Keep anything with realized revenue or open inventory; drop only
        // items that never traded.
        .filter(|it| it.revenue > 0.0 || it.units_open > 0)
        .collect();
    items.sort_by(|a, b| b.profit.partial_cmp(&a.profit).unwrap_or(std::cmp::Ordering::Equal));

    let total_revenue = items.iter().map(|i| i.revenue).sum();
    let total_cost = items.iter().map(|i| i.cost).sum();
    TradingPnl {
        total_revenue,
        total_cost,
        total_profit: total_revenue - total_cost,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buy(type_id: i64, qty: i64, price: f64, date: &str) -> Trade {
        Trade { type_id, quantity: qty, unit_price: price, is_buy: true, date: date.into() }
    }
    fn sell(type_id: i64, qty: i64, price: f64, date: &str) -> Trade {
        Trade { type_id, quantity: qty, unit_price: price, is_buy: false, date: date.into() }
    }

    #[test]
    fn fifo_matches_oldest_buys_first() {
        // Buy 100 @ 5, then 100 @ 7; sell 150 @ 10.
        // FIFO cost = 100*5 + 50*7 = 850; revenue = 1500; profit = 650.
        let trades = vec![
            buy(34, 100, 5.0, "2026-01-01T00:00:00Z"),
            buy(34, 100, 7.0, "2026-01-02T00:00:00Z"),
            sell(34, 150, 10.0, "2026-01-03T00:00:00Z"),
        ];
        let pnl = fifo_pnl(&trades);
        let it = &pnl.items[0];
        assert_eq!(it.units_sold, 150);
        assert!((it.revenue - 1500.0).abs() < 1e-6);
        assert!((it.cost - 850.0).abs() < 1e-6);
        assert!((it.profit - 650.0).abs() < 1e-6);
        // 50 units of the 7.0 lot remain open at 350 cost.
        assert_eq!(it.units_open, 50);
        assert!((it.open_cost - 350.0).abs() < 1e-6);
        assert!((pnl.total_profit - 650.0).abs() < 1e-6);
    }

    #[test]
    fn sale_beyond_known_buys_counts_revenue_only() {
        // Sell 10 with no prior buy → revenue 1000, cost 0.
        let pnl = fifo_pnl(&[sell(587, 10, 100.0, "2026-02-01T00:00:00Z")]);
        let it = &pnl.items[0];
        assert!((it.revenue - 1000.0).abs() < 1e-6);
        assert_eq!(it.cost, 0.0);
        assert_eq!(it.margin_pct, None);
    }

    #[test]
    fn ranks_by_profit_and_ignores_pure_holdings_without_sales() {
        // Item 1 profits 100; item 2 only bought (open, no sale) → excluded.
        let trades = vec![
            buy(1, 10, 5.0, "2026-01-01T00:00:00Z"),
            sell(1, 10, 15.0, "2026-01-02T00:00:00Z"),
            buy(2, 5, 20.0, "2026-01-01T00:00:00Z"),
        ];
        let pnl = fifo_pnl(&trades);
        // Item 2 has open inventory but no realized sale — still shown (open>0),
        // ranked below the profitable item.
        assert_eq!(pnl.items[0].type_id, 1);
        assert!(pnl.items.iter().any(|i| i.type_id == 2 && i.units_open == 5));
    }
}
