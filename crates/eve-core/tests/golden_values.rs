//! Golden-value regression suite — the "calculation trust" quality gate.
//!
//! Players abandon a tool the moment its numbers are wrong, so the core
//! calculators are pinned here against **independently known reference values**
//! (EVE mechanics that are public and stable, cross-checked against EVEMon /
//! PYFA / in-game / the EVE University wiki). These are deliberately separate
//! from each module's own unit tests: a unit test guards the code's internal
//! logic; a golden test guards the code against *reality*. If a refactor changes
//! one of these, either the math regressed or a game constant genuinely moved —
//! both warrant a hard look, not a silent update.

use eve_core::courier;
use eve_core::dogma::{self, DamageProfile, Layer, Weapon};
use eve_core::industry_plan::{self};
use eve_core::marketdata::{self, TradeFees};
use eve_core::reprocess;
use eve_core::sde::Material;
use eve_core::skillplan;

const EPS: f64 = 1e-6;

fn approx(a: f64, b: f64) {
    assert!((a - b).abs() < (b.abs().max(1.0)) * 1e-9 + EPS, "expected {b}, got {a}");
}

// ---------------------------------------------------------------------------
// Skills & training — the SP curve is a fixed EVE formula everyone knows.
// ---------------------------------------------------------------------------

#[test]
fn golden_skillpoints_per_level() {
    // Canonical rank-1 skill totals (EVE University "Skills" page): a rank-1
    // skill needs 250 / 1 415 / 8 000 / 45 255 / 256 000 SP for L1..L5.
    assert_eq!(skillplan::sp_for_level(1, 1), 250);
    assert_eq!(skillplan::sp_for_level(1, 2), 1_414);
    assert_eq!(skillplan::sp_for_level(1, 3), 8_000);
    assert_eq!(skillplan::sp_for_level(1, 4), 45_255);
    assert_eq!(skillplan::sp_for_level(1, 5), 256_000);
    // Rank scales the whole curve linearly: a rank-5 L5 skill is 5× a rank-1.
    assert_eq!(skillplan::sp_for_level(5, 5), 5 * 256_000);
    // A rank-8 skill (e.g. many capital skills) to L5 = 2,048,000 SP.
    assert_eq!(skillplan::sp_for_level(8, 5), 2_048_000);
}

#[test]
fn golden_training_rate() {
    // SP/min = primary + secondary/2 (a fixed EVE formula). Perfect implants +
    // remap give a common ~2700 SP/hr benchmark at Int 27 / Mem 21 (+5 implants
    // → 32/26): 32 + 13 = 45 SP/min = 2700 SP/hr.
    approx(skillplan::sp_per_minute(32.0, 26.0), 45.0);
    // Time to train a rank-1 skill to L5 (256k SP) at 45 SP/min:
    // 256000 / 45 = 5688.9 min → 341,334 s (ceil).
    assert_eq!(skillplan::training_seconds(256_000, 32.0, 26.0), 341_334);
    // Zero/negative attributes never divide by zero.
    assert_eq!(skillplan::training_seconds(256_000, 0.0, 0.0), 0);
}

// ---------------------------------------------------------------------------
// Fitting — EHP, DPS, turret falloff, cap: the PYFA-validated core.
// ---------------------------------------------------------------------------

#[test]
fn golden_ehp_from_resists() {
    // A layer's EHP = raw HP / average resonance. With a flat 50% resist across
    // the board (resonance 0.5) against an omni profile, EHP doubles.
    let half_resist = Layer { hp: 5000.0, em: 0.5, thermal: 0.5, kinetic: 0.5, explosive: 0.5 };
    approx(half_resist.ehp(&DamageProfile::uniform()), 10_000.0);

    // Tackle-tanked example: 1200 shield / 3000 armor / 1500 hull, all at the
    // EVE-default 0% hull / typical resists — pin the summed total.
    let shield = Layer { hp: 1200.0, em: 0.0, thermal: 0.8, kinetic: 0.6, explosive: 0.5 };
    let armor = Layer { hp: 3000.0, em: 0.5, thermal: 0.6, kinetic: 0.75, explosive: 0.9 };
    let hull = Layer { hp: 1500.0, em: 1.0, thermal: 1.0, kinetic: 1.0, explosive: 1.0 };
    let ehp = dogma::total_ehp(&shield, &armor, &hull, &DamageProfile::uniform());
    // Shield avg resonance (0+0.8+0.6+0.5)/4 = 0.475 → 1200/0.475 = 2526.3158.
    approx(ehp.shield, 1200.0 / 0.475);
    // Armor avg resonance (0.5+0.6+0.75+0.9)/4 = 0.6875 → 3000/0.6875 = 4363.636.
    approx(ehp.armor, 3000.0 / 0.6875);
    // Hull is unresisted → 1500.
    approx(ehp.hull, 1500.0);
    approx(ehp.total, ehp.shield + ehp.armor + ehp.hull);
}

#[test]
fn golden_dps_and_volley() {
    // A single weapon doing 100 total damage every 5 s = 20 DPS, 100 volley.
    let w = Weapon {
        em: 40.0, thermal: 60.0, kinetic: 0.0, explosive: 0.0,
        multiplier: 1.0, rof_seconds: 5.0, optimal_m: 0.0, falloff_m: 0.0,
    };
    approx(w.volley(), 100.0);
    approx(w.dps(), 20.0);
    // A 1.5× damage multiplier (e.g. a turret with damage mods) scales both.
    let boosted = Weapon { multiplier: 1.5, ..w };
    approx(boosted.volley(), 150.0);
    approx(boosted.dps(), 30.0);
}

#[test]
fn golden_turret_falloff_curve() {
    // The EVE turret hit formula: 0.5^((over/falloff)^2), where `over` is metres
    // past optimal. Pinned at the textbook points.
    // Inside optimal → full hits.
    approx(dogma::hit_fraction(4_000.0, 10_000.0, 5_000.0), 1.0);
    // Exactly at optimal → full.
    approx(dogma::hit_fraction(10_000.0, 10_000.0, 5_000.0), 1.0);
    // One falloff past optimal → 0.5^1 = 50% (the canonical "half at falloff").
    approx(dogma::hit_fraction(15_000.0, 10_000.0, 5_000.0), 0.5);
    // Two falloffs past → 0.5^4 = 6.25%.
    approx(dogma::hit_fraction(20_000.0, 10_000.0, 5_000.0), 0.0625);
    // Missiles (no optimal) apply full damage at any range.
    approx(dogma::hit_fraction(50_000.0, 0.0, 0.0), 1.0);
}

#[test]
fn golden_cap_peak_recharge() {
    // Peak passive recharge (at ~25% cap) = 2.5 × capacity / recharge_time — the
    // standard EVE capacitor formula. 5000 GJ over 500 s → 25 GJ/s peak.
    let stable = dogma::cap_stats(5000.0, 500.0, 20.0);
    approx(stable.peak_recharge, 25.0);
    approx(stable.net_at_peak, 5.0);
    assert!(stable.stable);
    // Load above peak → unstable.
    let unstable = dogma::cap_stats(5000.0, 500.0, 40.0);
    assert!(!unstable.stable);
    assert!(unstable.seconds_to_empty.is_some());
}

// ---------------------------------------------------------------------------
// Industry & reprocessing — ME rounding and refine flooring are exact rules.
// ---------------------------------------------------------------------------

#[test]
fn golden_material_efficiency() {
    // CCP applies ME across the whole job and never drops a material below one
    // per run. 1,000,000 base × 1 run: ME 10 → 900,000; ME 5 → 950,000.
    assert_eq!(industry_plan::material_required(1_000_000, 1, 10), 900_000);
    assert_eq!(industry_plan::material_required(1_000_000, 1, 5), 950_000);
    // Across 34 runs at ME 10, values round up per job (ceil), not per run.
    assert_eq!(industry_plan::material_required(1, 34, 10), 34); // floored at runs
    assert_eq!(industry_plan::material_required(100, 34, 10), (100.0 * 34.0 * 0.9_f64).ceil() as i64);
}

#[test]
fn golden_reprocess_flooring() {
    // Reprocessing floors per portion (invTypeMaterials rule). A Veldspar-like
    // 100-unit portion yielding 415 Tritanium at 100%, refined at 70%:
    // floor(415 × 0.7) = 290 per portion. 300 units = 3 portions → 870 trit.
    let base = vec![Material { type_id: 34, quantity: 415 }];
    let prices = marketdata_prices();
    let r = reprocess::reprocess(300, 100, 0.7, &base, &prices);
    assert_eq!(r.portions, 3);
    assert_eq!(r.leftover_units, 0);
    assert_eq!(r.yields[0].quantity, 870);
    // Value at 5 ISK/unit tritanium = 4350.
    approx(r.refined_value, 870.0 * 5.0);
}

fn marketdata_prices() -> eve_core::prices::PriceMap {
    use eve_core::prices::TypePrice;
    eve_core::prices::PriceMap::from_prices(&[
        TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 },
    ])
}

// ---------------------------------------------------------------------------
// Market & hauling — fee math and the service-rate reward formula.
// ---------------------------------------------------------------------------

#[test]
fn golden_station_trade_fees() {
    // Buy at 100, sell at 150, with 3% broker (both orders) + 4.5% sales tax on
    // the sell. Net profit/unit = 150 − 100 − (150+100)×0.03 − 150×0.045
    // = 50 − 7.5 − 6.75 = 35.75.
    let quote = marketdata::MarketQuote {
        best_sell: Some(150.0),
        best_buy: Some(100.0),
        spread: Some(50.0),
        spread_pct: Some(50.0 / 150.0),
        sell_volume: 1000,
        buy_volume: 1000,
        sell_orders: 10,
        buy_orders: 10,
    };
    let m = marketdata::trade_metrics(&quote, 1000, TradeFees { broker_fee: 0.03, sales_tax: 0.045 })
        .expect("priced quote");
    approx(m.profit_per_unit, 35.75);
}

#[test]
fn golden_courier_suggested_reward() {
    // Service-rate formula: 100k base + 250k/jump + 40/m³ + 1% collateral +
    // 750k/lowsec-hop. 10 jumps, 320,000 m³, 500M collateral, 2 lowsec hops:
    // 100k + 2.5M + 12.8M + 5M + 1.5M = 21,900,000.
    let reward = courier::suggested_reward(320_000.0, 500_000_000.0, 10, 2);
    approx(reward, 21_900_000.0);

    // Economics of accepting a 20M reward on that haul.
    let est = courier::estimate(320_000.0, 500_000_000.0, 20_000_000.0, 10);
    approx(est.reward_per_jump, 2_000_000.0);
    approx(est.reward_per_m3, 62.5);
    approx(est.collateral_ratio, 25.0); // 500M / 20M — gank-bait territory
}
