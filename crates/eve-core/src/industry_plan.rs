//! Industry build planner: bill-of-materials, ME, job profit, invention odds.
//!
//! Given a blueprint's base inputs (from the SDE), this computes the materials a
//! manufacturing/reaction job actually consumes after Material Efficiency, prices
//! the bill against the market, and reports build-vs-buy profit. Invention success
//! odds are computed from the SDE base probability and skill/decryptor multipliers.
//! All formulas are pure and unit-tested; the SDE supplies blueprint rows.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::prices::PriceMap;
use crate::sde::{Material, Sde};

/// One material line in a build plan (post-ME quantity + price).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanLine {
    pub type_id: i64,
    pub quantity: i64,
    pub unit_price: f64,
    pub value: f64,
}

/// A priced bill-of-materials for a manufacturing/reaction job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuildPlan {
    pub product_type_id: i64,
    pub runs: i64,
    pub me: i64,
    /// Total units of product produced (per-run output × runs).
    pub output_units: i64,
    pub materials: Vec<PlanLine>,
    pub material_cost: f64,
    /// Market value of the produced output at its reference price.
    pub product_value: f64,
    /// `product_value - material_cost` (excludes facility/job fees).
    pub profit: f64,
    /// Profit as a fraction of material cost.
    pub margin_pct: f64,
}

/// Units of a material a job consumes after Material Efficiency. CCP applies the
/// ME modifier across the whole job and never lets a needed material drop below
/// one per run. Pure.
pub fn material_required(base_quantity: i64, runs: i64, me: i64) -> i64 {
    if base_quantity <= 0 || runs <= 0 {
        return 0;
    }
    let modifier = 1.0 - (me.clamp(0, 100) as f64) / 100.0;
    let raw = (base_quantity as f64) * (runs as f64) * modifier;
    (raw.ceil() as i64).max(runs)
}

/// Build a priced bill-of-materials. `base_materials` are the blueprint's ME-0
/// per-run inputs; `output_per_run` units of `product_type_id` come out each run.
/// Pure.
pub fn build_plan(
    product_type_id: i64,
    runs: i64,
    me: i64,
    output_per_run: i64,
    base_materials: &[Material],
    prices: &PriceMap,
    product_price: f64,
) -> BuildPlan {
    let mut materials = Vec::with_capacity(base_materials.len());
    let mut material_cost = 0.0;
    for m in base_materials {
        let quantity = material_required(m.quantity, runs, me);
        if quantity == 0 {
            continue;
        }
        let unit_price = prices.price(m.type_id).unwrap_or(0.0);
        let value = unit_price * quantity as f64;
        material_cost += value;
        materials.push(PlanLine { type_id: m.type_id, quantity, unit_price, value });
    }
    let output_units = output_per_run.max(0) * runs.max(0);
    let product_value = product_price * output_units as f64;
    let profit = product_value - material_cost;
    let margin_pct = if material_cost > 0.0 { profit / material_cost } else { 0.0 };
    BuildPlan {
        product_type_id,
        runs,
        me,
        output_units,
        materials,
        material_cost,
        product_value,
        profit,
        margin_pct,
    }
}

/// Effective invention success chance from the SDE base probability and the
/// player's combined skill + decryptor multipliers. Clamped to [0, 1]. Pure.
pub fn invention_probability(base: f64, skill_mult: f64, decryptor_mult: f64) -> f64 {
    (base * skill_mult * decryptor_mult).clamp(0.0, 1.0)
}

/// SDE + price backed build planner.
#[derive(Clone)]
pub struct IndustryPlanClient {
    sde: Sde,
}

impl IndustryPlanClient {
    pub fn new(sde: Sde) -> Self {
        Self { sde }
    }

    /// Plan a manufacturing (or reaction) job for `product_type_id`. Returns
    /// `None` when the SDE has no blueprint producing it via `activity` (e.g. the
    /// common-items seed without the full prebuilt SDE).
    pub async fn plan(
        &self,
        product_type_id: i64,
        runs: i64,
        me: i64,
        activity: &str,
        prices: &PriceMap,
    ) -> Result<Option<BuildPlan>> {
        let Some(product) = self.sde.blueprint_for_product(product_type_id, activity).await? else {
            return Ok(None);
        };
        let base = self
            .sde
            .blueprint_materials(product.blueprint_type_id, activity)
            .await?;
        if base.is_empty() {
            return Ok(None);
        }
        let product_price = prices.price(product_type_id).unwrap_or(0.0);
        Ok(Some(build_plan(
            product_type_id,
            runs,
            me,
            product.quantity,
            &base,
            prices,
            product_price,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prices::{PriceMap, TypePrice};

    fn prices() -> PriceMap {
        PriceMap::from_prices(&[
            TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 },
            TypePrice { type_id: 587, average_price: 500_000.0, adjusted_price: 0.0 },
        ])
    }

    #[test]
    fn me_reduces_but_floors_at_one_per_run() {
        // 1 base unit × 10 runs at ME 10 → raw 9, floored up to runs (10).
        assert_eq!(material_required(1, 10, 10), 10);
        // 100 base × 1 run at ME 10 → ceil(90) = 90.
        assert_eq!(material_required(100, 1, 10), 90);
        // ME 0 is the base.
        assert_eq!(material_required(100, 2, 0), 200);
    }

    #[test]
    fn build_plan_prices_and_reports_margin() {
        let base = vec![Material { type_id: 34, quantity: 1000 }];
        let plan = build_plan(587, 1, 10, 1, &base, &prices(), 500_000.0);
        // 1000 trit × ME10 = 900 × 5 = 4500 cost; product 500k.
        assert_eq!(plan.materials[0].quantity, 900);
        assert!((plan.material_cost - 4500.0).abs() < 1e-9);
        assert!((plan.profit - 495_500.0).abs() < 1e-9);
        assert_eq!(plan.output_units, 1);
    }

    #[test]
    fn invention_probability_clamps() {
        assert!((invention_probability(0.3, 1.5, 1.0) - 0.45).abs() < 1e-9);
        assert_eq!(invention_probability(0.5, 3.0, 2.0), 1.0); // clamps to 1
    }

    #[tokio::test]
    async fn client_plans_from_sde_or_reports_missing() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_type(587, "Rifter", None).await.unwrap();
        sde.insert_blueprint_product(681, "manufacturing", 587, 1, None, Some(6000))
            .await
            .unwrap();
        sde.insert_blueprint_material(681, "manufacturing", 34, 1000)
            .await
            .unwrap();
        let client = IndustryPlanClient::new(sde);

        let plan = client
            .plan(587, 1, 0, "manufacturing", &prices())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(plan.materials[0].quantity, 1000);

        // No blueprint for this product → None.
        assert!(client
            .plan(9999, 1, 0, "manufacturing", &prices())
            .await
            .unwrap()
            .is_none());
    }
}
