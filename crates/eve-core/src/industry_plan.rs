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
    /// Base success chance for invention jobs (None for deterministic
    /// manufacturing/reaction). Effective odds apply skill/decryptor multipliers
    /// on top via [`invention_probability`].
    pub probability: Option<f64>,
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
#[allow(clippy::too_many_arguments)]
pub fn build_plan(
    product_type_id: i64,
    runs: i64,
    me: i64,
    output_per_run: i64,
    base_materials: &[Material],
    prices: &PriceMap,
    product_price: f64,
    probability: Option<f64>,
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
        probability,
    }
}

/// Effective invention success chance from the SDE base probability and the
/// player's combined skill + decryptor multipliers. Clamped to [0, 1]. Pure.
pub fn invention_probability(base: f64, skill_mult: f64, decryptor_mult: f64) -> f64 {
    (base * skill_mult * decryptor_mult).clamp(0.0, 1.0)
}

/// Runs needed to produce at least `quantity` units when each run yields
/// `output_per_run`. Pure.
pub fn runs_for(quantity: i64, output_per_run: i64) -> i64 {
    if quantity <= 0 || output_per_run <= 0 {
        return 0;
    }
    (quantity + output_per_run - 1) / output_per_run
}

/// Whether to build an intermediate rather than buy it: build only when it's
/// strictly cheaper than its market buy price (a tie buys, since buying avoids
/// job fees and time). Pure.
pub fn should_build(build_cost: f64, buy_price: f64) -> bool {
    buy_price > 0.0 && build_cost < buy_price
}

/// One node in a multi-level bill-of-materials tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomNode {
    pub type_id: i64,
    /// Total units of this item the parent needs.
    pub quantity: i64,
    /// This item has a blueprint (could be built).
    pub buildable: bool,
    /// The build-vs-buy decision taken for this node (false = buy as a leaf).
    pub build: bool,
    /// Reference buy price per unit.
    pub unit_buy_price: f64,
    /// Cheapest cost for `quantity` units (min of build vs buy).
    pub cost: f64,
    /// Sub-materials, present only when `build` is true.
    pub children: Vec<BomNode>,
}

/// A fully-expanded build tree with the flattened raw-materials shopping list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BomTree {
    pub root_type_id: i64,
    pub root: BomNode,
    /// Leaf items to actually acquire (bought intermediates + raw materials),
    /// summed across the tree, most expensive first.
    pub shopping_list: Vec<PlanLine>,
    /// Total cost of the shopping list (what you buy).
    pub build_cost: f64,
    /// Cost of just buying the finished product outright, for comparison.
    pub buy_cost: f64,
}

/// Flatten a decided BOM tree into a summed shopping list of the leaves that are
/// actually bought (nodes where `build` is false). Pure.
pub fn flatten_shopping_list(root: &BomNode) -> Vec<PlanLine> {
    let mut acc: std::collections::HashMap<i64, (i64, f64)> = std::collections::HashMap::new();
    collect_leaves(root, &mut acc);
    let mut list: Vec<PlanLine> = acc
        .into_iter()
        .map(|(type_id, (quantity, unit_price))| PlanLine {
            type_id,
            quantity,
            unit_price,
            value: unit_price * quantity as f64,
        })
        .collect();
    list.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
    list
}

fn collect_leaves(node: &BomNode, acc: &mut std::collections::HashMap<i64, (i64, f64)>) {
    if node.build {
        for c in &node.children {
            collect_leaves(c, acc);
        }
    } else {
        let e = acc.entry(node.type_id).or_insert((0, node.unit_buy_price));
        e.0 += node.quantity;
    }
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
            product.probability,
        )))
    }

    /// Expand a full multi-level bill of materials for `quantity` units of
    /// `product_type_id`, deciding build-vs-buy at every intermediate (build only
    /// when strictly cheaper than buying), and flatten the leaves into one
    /// shopping list. `me` is applied uniformly at every tier (a simplification).
    /// Depth-capped and cycle-guarded so exotic blueprint loops can't runaway.
    pub async fn bom_tree(
        &self,
        product_type_id: i64,
        quantity: i64,
        me: i64,
        prices: &PriceMap,
    ) -> Result<BomTree> {
        let mut path = std::collections::HashSet::new();
        let root = self
            .expand_node(product_type_id, quantity.max(1), me, prices, 0, &mut path)
            .await?;
        let shopping_list = flatten_shopping_list(&root);
        let build_cost = shopping_list.iter().map(|l| l.value).sum();
        let buy_cost = prices.price(product_type_id).unwrap_or(0.0) * quantity.max(1) as f64;
        Ok(BomTree { root_type_id: product_type_id, root, shopping_list, build_cost, buy_cost })
    }

    /// Recursively cost one item: expand its blueprint (manufacturing, else
    /// reaction), decide build-vs-buy, and return the decided node. Boxed for
    /// async recursion.
    fn expand_node<'a>(
        &'a self,
        type_id: i64,
        quantity: i64,
        me: i64,
        prices: &'a PriceMap,
        depth: usize,
        path: &'a mut std::collections::HashSet<i64>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<BomNode>> + Send + 'a>> {
        Box::pin(async move {
            let unit_buy_price = prices.price(type_id).unwrap_or(0.0);
            let buy_cost = unit_buy_price * quantity as f64;
            let buy_leaf = |buildable: bool| BomNode {
                type_id,
                quantity,
                buildable,
                build: false,
                unit_buy_price,
                cost: buy_cost,
                children: Vec::new(),
            };

            // Stop expanding at the depth cap or on a cycle; treat as a bought leaf.
            if depth >= 8 || !path.insert(type_id) {
                return Ok(buy_leaf(false));
            }

            // Find a blueprint that makes this (manufacturing preferred, then reaction).
            let mut product = self.sde.blueprint_for_product(type_id, "manufacturing").await?;
            let mut activity = "manufacturing";
            if product.is_none() {
                product = self.sde.blueprint_for_product(type_id, "reaction").await?;
                activity = "reaction";
            }
            let Some(product) = product else {
                path.remove(&type_id);
                return Ok(buy_leaf(false));
            };
            let base = self.sde.blueprint_materials(product.blueprint_type_id, activity).await?;
            if base.is_empty() || product.quantity <= 0 {
                path.remove(&type_id);
                return Ok(buy_leaf(true));
            }

            let runs = runs_for(quantity, product.quantity);
            let mut children = Vec::with_capacity(base.len());
            let mut build_cost = 0.0;
            for m in &base {
                let need = material_required(m.quantity, runs, me);
                if need == 0 {
                    continue;
                }
                let child = self
                    .expand_node(m.type_id, need, me, prices, depth + 1, path)
                    .await?;
                build_cost += child.cost;
                children.push(child);
            }
            path.remove(&type_id);

            let build = should_build(build_cost, buy_cost);
            Ok(BomNode {
                type_id,
                quantity,
                buildable: true,
                build,
                unit_buy_price,
                cost: if build { build_cost } else { buy_cost },
                children,
            })
        })
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
        let plan = build_plan(587, 1, 10, 1, &base, &prices(), 500_000.0, None);
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

    #[test]
    fn runs_and_build_decision() {
        assert_eq!(runs_for(10, 1), 10);
        assert_eq!(runs_for(10, 3), 4); // ceil(10/3)
        assert_eq!(runs_for(0, 5), 0);
        assert!(should_build(90.0, 100.0)); // cheaper to build
        assert!(!should_build(120.0, 100.0)); // cheaper to buy
        assert!(!should_build(50.0, 0.0)); // unpriced product → buy
    }

    #[test]
    fn flatten_sums_only_bought_leaves() {
        // A built root with one built child (itself two bought leaves) plus a
        // directly-bought leaf; the built nodes must not appear in the list.
        let leaf = |type_id, quantity, price: f64| BomNode {
            type_id,
            quantity,
            buildable: false,
            build: false,
            unit_buy_price: price,
            cost: price * quantity as f64,
            children: vec![],
        };
        let child = BomNode {
            type_id: 100,
            quantity: 1,
            buildable: true,
            build: true,
            unit_buy_price: 0.0,
            cost: 30.0,
            children: vec![leaf(34, 10, 2.0), leaf(35, 5, 2.0)],
        };
        let root = BomNode {
            type_id: 587,
            quantity: 1,
            buildable: true,
            build: true,
            unit_buy_price: 0.0,
            cost: 50.0,
            children: vec![child, leaf(34, 5, 2.0)], // 34 also appears here → merges
        };
        let list = flatten_shopping_list(&root);
        // 34 merged: 10 + 5 = 15; 35: 5. Built nodes (587, 100) excluded.
        let trit = list.iter().find(|l| l.type_id == 34).unwrap();
        assert_eq!(trit.quantity, 15);
        assert!(list.iter().all(|l| l.type_id != 587 && l.type_id != 100));
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn bom_tree_expands_and_decides_build_vs_buy() {
        let sde = Sde::open_in_memory().await.unwrap();
        // Product 587 built from 5× component 200; component 200 built from
        // 100× mineral 34. Prices make building cheaper than buying at both tiers.
        sde.insert_type(587, "Widget", None).await.unwrap();
        sde.insert_type(200, "Component", None).await.unwrap();
        sde.insert_blueprint_product(681, "manufacturing", 587, 1, None, Some(6000))
            .await
            .unwrap();
        sde.insert_blueprint_material(681, "manufacturing", 200, 5).await.unwrap();
        sde.insert_blueprint_product(682, "manufacturing", 200, 1, None, Some(600))
            .await
            .unwrap();
        sde.insert_blueprint_material(682, "manufacturing", 34, 100).await.unwrap();

        let prices = PriceMap::from_prices(&[
            TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 },
            TypePrice { type_id: 200, average_price: 10_000.0, adjusted_price: 0.0 },
            TypePrice { type_id: 587, average_price: 1_000_000.0, adjusted_price: 0.0 },
        ]);
        let client = IndustryPlanClient::new(sde);
        let tree = client.bom_tree(587, 1, 0, &prices).await.unwrap();

        // Root builds (5 components × 100 trit × 5 = 2500 ISK << 1M buy).
        assert!(tree.root.build);
        // Component also builds (100 trit × 5 = 500 << 10k buy).
        assert!(tree.root.children[0].build);
        // Shopping list is pure trit: 5 components × 100 = 500 units.
        assert_eq!(tree.shopping_list.len(), 1);
        assert_eq!(tree.shopping_list[0].type_id, 34);
        assert_eq!(tree.shopping_list[0].quantity, 500);
        assert!((tree.build_cost - 2500.0).abs() < 1e-6);
        assert!((tree.buy_cost - 1_000_000.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn bom_tree_buys_when_cheaper_than_building() {
        let sde = Sde::open_in_memory().await.unwrap();
        // Component 200 built from 100× mineral 34 @ 5 = 500, but buys for 100.
        sde.insert_type(200, "Component", None).await.unwrap();
        sde.insert_blueprint_product(682, "manufacturing", 200, 1, None, Some(600))
            .await
            .unwrap();
        sde.insert_blueprint_material(682, "manufacturing", 34, 100).await.unwrap();
        let prices = PriceMap::from_prices(&[
            TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 },
            TypePrice { type_id: 200, average_price: 100.0, adjusted_price: 0.0 },
        ]);
        let client = IndustryPlanClient::new(sde);
        let tree = client.bom_tree(200, 1, 0, &prices).await.unwrap();
        // Buildable, but buying (100) beats building (500) → buy the component.
        assert!(tree.root.buildable);
        assert!(!tree.root.build);
        assert_eq!(tree.shopping_list.len(), 1);
        assert_eq!(tree.shopping_list[0].type_id, 200);
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
