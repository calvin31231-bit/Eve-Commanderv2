//! Loyalty-Point store optimizer.
//!
//! A corp's LP store offers (ESI `/loyalty/stores/{corp}/offers/`, public) each
//! trade LP + ISK + items for an output. The value that matters is **ISK per
//! LP**: `(market value of the output − ISK cost − cost of required items) ÷ LP
//! cost`. We compute and rank that. The reduction is pure + unit-tested; the
//! market supplies prices and ESI supplies offers.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::esi::EsiClient;
use crate::prices::PriceMap;

/// A required item in an LP offer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequiredItem {
    pub type_id: i64,
    #[serde(default)]
    pub quantity: i64,
}

/// One LP store offer (ESI `/loyalty/stores/{corp}/offers/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LpOffer {
    pub offer_id: i64,
    pub type_id: i64,
    #[serde(default = "one")]
    pub quantity: i64,
    #[serde(default)]
    pub lp_cost: i64,
    #[serde(default)]
    pub isk_cost: i64,
    #[serde(default)]
    pub required_items: Vec<RequiredItem>,
}

fn one() -> i64 {
    1
}

/// A valued LP offer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LpValue {
    pub offer_id: i64,
    pub type_id: i64,
    pub quantity: i64,
    pub lp_cost: i64,
    /// ISK cost (store ISK + market cost of required items).
    pub total_isk_cost: f64,
    /// Market value of the output.
    pub output_value: f64,
    /// `output_value − total_isk_cost`.
    pub profit: f64,
    /// `profit / lp_cost` (0 when the offer costs no LP).
    pub isk_per_lp: f64,
}

/// Value one offer against market prices. Pure.
pub fn value_offer(offer: &LpOffer, prices: &PriceMap) -> LpValue {
    let output_value = prices.value(offer.type_id, offer.quantity);
    let items_cost: f64 = offer
        .required_items
        .iter()
        .map(|r| prices.value(r.type_id, r.quantity))
        .sum();
    let total_isk_cost = offer.isk_cost as f64 + items_cost;
    let profit = output_value - total_isk_cost;
    let isk_per_lp = if offer.lp_cost > 0 {
        profit / offer.lp_cost as f64
    } else {
        0.0
    };
    LpValue {
        offer_id: offer.offer_id,
        type_id: offer.type_id,
        quantity: offer.quantity,
        lp_cost: offer.lp_cost,
        total_isk_cost,
        output_value,
        profit,
        isk_per_lp,
    }
}

/// Value + rank a store's offers by ISK/LP, best first. Pure.
pub fn rank_offers(offers: &[LpOffer], prices: &PriceMap) -> Vec<LpValue> {
    let mut out: Vec<LpValue> = offers.iter().map(|o| value_offer(o, prices)).collect();
    out.sort_by(|a, b| {
        b.isk_per_lp
            .partial_cmp(&a.isk_per_lp)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

/// Reads public LP store offers.
#[derive(Clone)]
pub struct LpClient {
    esi: EsiClient,
}

impl LpClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    /// All offers in a corporation's LP store (public).
    pub async fn offers(&self, corporation_id: i64) -> Result<Vec<LpOffer>> {
        let path = format!("/latest/loyalty/stores/{corporation_id}/offers/");
        self.esi.get_public_json::<Vec<LpOffer>>(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{PriceMap, TypePrice};

    fn prices() -> PriceMap {
        PriceMap::from_prices(&[
            TypePrice { type_id: 100, average_price: 1_000_000.0, adjusted_price: 0.0 }, // output
            TypePrice { type_id: 200, average_price: 50_000.0, adjusted_price: 0.0 },     // required
        ])
    }

    #[test]
    fn values_isk_per_lp() {
        let offer = LpOffer {
            offer_id: 1,
            type_id: 100,
            quantity: 1,
            lp_cost: 1000,
            isk_cost: 100_000,
            required_items: vec![RequiredItem { type_id: 200, quantity: 2 }],
        };
        let v = value_offer(&offer, &prices());
        // cost = 100k isk + 2 * 50k = 200k. output 1M. profit 800k. /1000 lp = 800.
        assert!((v.total_isk_cost - 200_000.0).abs() < 1e-6);
        assert!((v.profit - 800_000.0).abs() < 1e-6);
        assert!((v.isk_per_lp - 800.0).abs() < 1e-9);
    }

    #[test]
    fn ranks_best_first() {
        let offers = vec![
            LpOffer { offer_id: 1, type_id: 100, quantity: 1, lp_cost: 2000, isk_cost: 0, required_items: vec![] },
            LpOffer { offer_id: 2, type_id: 100, quantity: 1, lp_cost: 500, isk_cost: 0, required_items: vec![] },
        ];
        let ranked = rank_offers(&offers, &prices());
        // Offer 2 (fewer LP for the same output) has higher ISK/LP.
        assert_eq!(ranked[0].offer_id, 2);
    }
}
