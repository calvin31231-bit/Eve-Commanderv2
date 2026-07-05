//! Courier / hauling economics.
//!
//! For a hauling contract (or your own move), the numbers that matter are the
//! reward against the route length and cargo, and the collateral against the
//! reward (a high collateral-to-reward ratio is the classic gank bait). The
//! metrics are pure and unit-tested; the calling command pairs them with the
//! ESI route + kill heatmap for per-hop risk.

use serde::{Deserialize, Serialize};

/// Reward / collateral / route economics for one hauling job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CourierEstimate {
    pub jumps: i64,
    pub volume: f64,
    pub reward: f64,
    pub collateral: f64,
    /// Reward per jump (0 jumps ⇒ same-station, treated as 1 to avoid div0).
    pub reward_per_jump: f64,
    /// Reward per m³ of cargo.
    pub reward_per_m3: f64,
    /// Collateral ÷ reward — how much value rides on the haul per ISK earned.
    /// High values (≫ 20) are gank bait.
    pub collateral_ratio: f64,
}

/// Compute courier economics. Pure.
pub fn estimate(volume: f64, collateral: f64, reward: f64, jumps: i64) -> CourierEstimate {
    let eff_jumps = jumps.max(1) as f64;
    CourierEstimate {
        jumps,
        volume,
        reward,
        collateral,
        reward_per_jump: reward / eff_jumps,
        reward_per_m3: if volume > 0.0 { reward / volume } else { 0.0 },
        collateral_ratio: if reward > 0.0 { collateral / reward } else { 0.0 },
    }
}

/// A fair courier reward for a haul, using typical player-service rates: a base
/// fee, a per-jump rate, a small per-m³ rate, plus a collateral premium and a
/// low/null-sec surcharge (risk pricing). Pure — a starting figure to offer or
/// expect, not a fixed tariff.
pub fn suggested_reward(volume: f64, collateral: f64, jumps: i64, lowsec_hops: i64) -> f64 {
    const BASE: f64 = 100_000.0; // flat acceptance fee
    const PER_JUMP: f64 = 250_000.0; // per-jump haul rate
    const PER_M3: f64 = 40.0; // small cargo-size premium
    const COLLATERAL_PCT: f64 = 0.01; // 1% of collateral
    const LOWSEC_JUMP: f64 = 750_000.0; // extra per low/null hop
    BASE + PER_JUMP * jumps.max(0) as f64
        + PER_M3 * volume.max(0.0)
        + COLLATERAL_PCT * collateral.max(0.0)
        + LOWSEC_JUMP * lowsec_hops.max(0) as f64
}

/// A verdict string for how attractive/risky a haul looks. Pure.
pub fn verdict(est: &CourierEstimate, lowsec_hops: i64, kills_on_route: i64) -> String {
    let mut notes = Vec::new();
    if est.collateral_ratio > 20.0 {
        notes.push("high collateral-to-reward (gank bait)".to_string());
    }
    if lowsec_hops > 0 {
        notes.push(format!("{lowsec_hops} low/null hop(s)"));
    }
    if kills_on_route > 0 {
        notes.push(format!("{kills_on_route} kills on route this hour"));
    }
    if notes.is_empty() {
        "Looks clean — high-sec and well-rewarded for the distance.".to_string()
    } else {
        format!("Caution: {}.", notes.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_computes_ratios() {
        let e = estimate(320_000.0, 2_000_000_000.0, 50_000_000.0, 10);
        assert!((e.reward_per_jump - 5_000_000.0).abs() < 1e-6);
        assert!((e.reward_per_m3 - 156.25).abs() < 1e-6);
        assert!((e.collateral_ratio - 40.0).abs() < 1e-9);
    }

    #[test]
    fn zero_jumps_and_volume_are_safe() {
        let e = estimate(0.0, 0.0, 1_000.0, 0);
        assert_eq!(e.reward_per_jump, 1_000.0); // jumps clamped to 1
        assert_eq!(e.reward_per_m3, 0.0);
        assert_eq!(e.collateral_ratio, 0.0);
    }

    #[test]
    fn suggested_reward_scales_with_route_cargo_collateral_risk() {
        // Base 100k + 10 jumps*250k + 1000 m3*40 + 1% of 100M + 0 lowsec
        // = 100k + 2.5M + 40k + 1M = 3.64M.
        let r = suggested_reward(1_000.0, 100_000_000.0, 10, 0);
        assert!((r - 3_640_000.0).abs() < 1.0);
        // A low/null hop adds the surcharge; more jumps/collateral raise it.
        assert!(suggested_reward(1_000.0, 100_000_000.0, 10, 1) > r);
        assert!(suggested_reward(1_000.0, 100_000_000.0, 20, 0) > r);
    }

    #[test]
    fn verdict_flags_gank_bait_and_lowsec() {
        let e = estimate(10_000.0, 5_000_000_000.0, 10_000_000.0, 5); // ratio 500
        let v = verdict(&e, 2, 7);
        assert!(v.contains("gank bait"));
        assert!(v.contains("low/null"));
        assert!(v.contains("kills"));

        let clean = estimate(10_000.0, 100_000_000.0, 20_000_000.0, 5); // ratio 5
        assert!(verdict(&clean, 0, 0).starts_with("Looks clean"));
    }
}
