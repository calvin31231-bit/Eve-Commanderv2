//! Reprocessing / refining calculator.
//!
//! EVE reprocesses items in fixed **portions** (ore is 100 units; most items 1).
//! CCP's `invTypeMaterials` gives the materials one portion yields at 100%
//! efficiency; the real yield is `floor(base × efficiency)` per portion, summed
//! over the whole portions that fit in the input. The reduction here is pure and
//! unit-tested; the SDE supplies the yield rows and the market supplies prices.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::prices::PriceMap;
use crate::sde::{Material, Sde};

/// One refined material line (id, amount produced, ISK value).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineYield {
    pub type_id: i64,
    pub quantity: i64,
    pub value: f64,
}

/// The outcome of refining a quantity of one item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineResult {
    /// Whole portions that fit in the input (`floor(units / portion_size)`).
    pub portions: i64,
    /// Units that don't fill a portion and can't be refined.
    pub leftover_units: i64,
    pub yields: Vec<RefineYield>,
    /// Total ISK value of the refined materials at the supplied prices.
    pub refined_value: f64,
}

/// Compute the refined output of `units` of an item. `base_yields` are the
/// per-portion materials at 100%; `efficiency` is the total refine rate (0..1,
/// e.g. 0.5 for a no-skill NPC station, ~0.9 with skills/structure). Pure.
pub fn reprocess(
    units: i64,
    portion_size: i64,
    efficiency: f64,
    base_yields: &[Material],
    prices: &PriceMap,
) -> RefineResult {
    let portion_size = portion_size.max(1);
    let portions = units / portion_size;
    let leftover_units = units - portions * portion_size;
    let eff = efficiency.clamp(0.0, 1.0);

    let mut yields = Vec::with_capacity(base_yields.len());
    let mut refined_value = 0.0;
    for m in base_yields {
        let per_portion = (m.quantity as f64 * eff).floor() as i64;
        let quantity = per_portion * portions;
        if quantity == 0 {
            continue;
        }
        let value = prices.value(m.type_id, quantity);
        refined_value += value;
        yields.push(RefineYield { type_id: m.type_id, quantity, value });
    }
    RefineResult { portions, leftover_units, yields, refined_value }
}

/// Refine-vs-sell verdict for a stack of ore/items.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefineVsSell {
    pub refined_value: f64,
    /// Value of selling the unrefined item at its own reference price.
    pub sell_value: f64,
    /// `refined_value - sell_value` (positive ⇒ refining wins).
    pub advantage: f64,
}

/// Compare refining `units` to selling them whole. Pure.
pub fn refine_vs_sell(
    units: i64,
    refined_value: f64,
    unit_price: f64,
) -> RefineVsSell {
    let sell_value = unit_price * units as f64;
    RefineVsSell {
        refined_value,
        sell_value,
        advantage: refined_value - sell_value,
    }
}

/// SDE + price backed reprocessing calculator.
#[derive(Clone)]
pub struct ReprocessClient {
    sde: Sde,
}

impl ReprocessClient {
    pub fn new(sde: Sde) -> Self {
        Self { sde }
    }

    /// Refine `units` of `type_id` at `efficiency`, valued with `prices`.
    /// Returns `None` when the SDE has no reprocessing data for the type (e.g.
    /// the common-items seed without the full prebuilt SDE).
    pub async fn refine(
        &self,
        type_id: i64,
        units: i64,
        efficiency: f64,
        prices: &PriceMap,
    ) -> Result<Option<RefineResult>> {
        let base = self.sde.reprocess_materials(type_id).await?;
        if base.is_empty() {
            return Ok(None);
        }
        let portion = self.sde.portion_size(type_id).await?;
        Ok(Some(reprocess(units, portion, efficiency, &base, prices)))
    }
}

/// A material list as a flat `type_id → quantity` map (handy for tests/callers).
pub fn as_map(materials: &[Material]) -> HashMap<i64, i64> {
    materials.iter().map(|m| (m.type_id, m.quantity)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{PriceMap, TypePrice};

    fn prices() -> PriceMap {
        PriceMap::from_prices(&[
            TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 },
            TypePrice { type_id: 35, average_price: 10.0, adjusted_price: 0.0 },
        ])
    }

    #[test]
    fn reprocess_floors_per_portion_and_scales() {
        // Veldspar-like: 100-unit portion → 415 Tritanium at 100%.
        let yields = vec![Material { type_id: 34, quantity: 415 }];
        // 250 units → 2 whole portions, 50 leftover. 70% efficiency.
        let r = reprocess(250, 100, 0.7, &yields, &prices());
        assert_eq!(r.portions, 2);
        assert_eq!(r.leftover_units, 50);
        // floor(415 * 0.7) = 290 per portion × 2 = 580.
        assert_eq!(r.yields[0].quantity, 580);
        assert!((r.refined_value - 580.0 * 5.0).abs() < 1e-9);
    }

    #[test]
    fn reprocess_drops_zero_yield_materials() {
        // A tiny trace material that floors to zero is omitted.
        let yields = vec![
            Material { type_id: 34, quantity: 100 },
            Material { type_id: 35, quantity: 1 }, // floor(1 * 0.5) = 0
        ];
        let r = reprocess(100, 100, 0.5, &yields, &prices());
        assert_eq!(r.yields.len(), 1);
        assert_eq!(r.yields[0].type_id, 34);
    }

    #[test]
    fn refine_vs_sell_picks_advantage() {
        let v = refine_vs_sell(100, 1200.0, 10.0);
        assert!((v.sell_value - 1000.0).abs() < 1e-9);
        assert!((v.advantage - 200.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn client_refines_from_sde_or_reports_missing() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_type(1230, "Veldspar", None).await.unwrap();
        sde.set_portion_size(1230, 100).await.unwrap();
        sde.insert_type_material(1230, 34, 415).await.unwrap();
        let client = ReprocessClient::new(sde);

        let r = client.refine(1230, 100, 1.0, &prices()).await.unwrap().unwrap();
        assert_eq!(r.yields[0].quantity, 415);

        // A type with no yield rows reports None rather than an empty refine.
        assert!(client.refine(9999, 100, 1.0, &prices()).await.unwrap().is_none());
    }
}
