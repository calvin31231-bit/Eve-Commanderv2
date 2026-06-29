//! Wormhole rolling calculator.
//!
//! A wormhole has a total mass budget and a per-jump mass limit. Pushing heavy
//! ships back and forth ("rolling") collapses it. Mass shows as stable (>50% of
//! nominal remaining), reduced (10–50%), or critical (<10%), and the *nominal*
//! total carries a ±10% variance — so a hole can collapse anywhere between 90%
//! and 110% of nominal mass spent. This computes, for a given hole + rolling
//! ship, how many passes reach each stage and how many are guaranteed safe (no
//! collapse risk). Pure and unit-tested; all masses in kilograms.

use serde::{Deserialize, Serialize};

/// The result of a rolling plan for one hole + ship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollPlan {
    /// Mass removed per pass (the ship's jump mass).
    pub per_pass_mass: f64,
    /// Passes until the hole shows "reduced" (≤50% nominal remaining).
    pub passes_to_reduced: i64,
    /// Passes until the hole shows "critical" (≤10% nominal remaining).
    pub passes_to_critical: i64,
    /// Passes you can make with no collapse risk (the hole cannot collapse
    /// before this many, even at the light end of the variance).
    pub safe_passes: i64,
    /// Earliest pass that could collapse the hole (light end, 90% of nominal).
    pub collapse_earliest: i64,
    /// Latest pass that will definitely collapse the hole (heavy end, 110%).
    pub collapse_latest: i64,
    /// Set when the ship is too heavy to use the hole at all.
    pub warning: Option<String>,
}

/// Smallest integer `k` with `k * pass >= target` (and at least 1).
fn passes_for(target: f64, pass: f64) -> i64 {
    if pass <= 0.0 {
        return 0;
    }
    (target / pass).ceil().max(1.0) as i64
}

/// Plan a roll: how many passes of a ship (`ship_pass_mass`) it takes to walk a
/// hole (`nominal_total_mass`, `max_jump_mass`) through its mass stages and
/// collapse it. Pure.
pub fn roll_plan(nominal_total_mass: f64, max_jump_mass: f64, ship_pass_mass: f64) -> RollPlan {
    let none = RollPlan {
        per_pass_mass: ship_pass_mass,
        passes_to_reduced: 0,
        passes_to_critical: 0,
        safe_passes: 0,
        collapse_earliest: 0,
        collapse_latest: 0,
        warning: None,
    };

    if ship_pass_mass <= 0.0 || nominal_total_mass <= 0.0 {
        return RollPlan { warning: Some("Enter the hole and ship masses.".into()), ..none };
    }
    if max_jump_mass > 0.0 && ship_pass_mass > max_jump_mass {
        return RollPlan {
            warning: Some("Ship exceeds the hole's per-jump mass limit — it can't pass.".into()),
            ..none
        };
    }

    let collapse_earliest = passes_for(0.9 * nominal_total_mass, ship_pass_mass);
    RollPlan {
        per_pass_mass: ship_pass_mass,
        passes_to_reduced: passes_for(0.5 * nominal_total_mass, ship_pass_mass),
        passes_to_critical: passes_for(0.9 * nominal_total_mass, ship_pass_mass),
        safe_passes: (collapse_earliest - 1).max(0),
        collapse_earliest,
        collapse_latest: passes_for(1.1 * nominal_total_mass, ship_pass_mass),
        warning: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_roll_stages() {
        // 2.0 Gg hole, 300 Mkg jump limit, 200 Mkg/pass battleship.
        let p = roll_plan(2_000_000_000.0, 300_000_000.0, 200_000_000.0);
        assert!(p.warning.is_none());
        assert_eq!(p.passes_to_reduced, 5); // 1.0Gg / 200Mkg
        assert_eq!(p.passes_to_critical, 9); // 1.8Gg / 200Mkg
        assert_eq!(p.collapse_earliest, 9); // light hole (90%)
        assert_eq!(p.collapse_latest, 11); // heavy hole (110%): 2.2Gg / 200Mkg
        assert_eq!(p.safe_passes, 8);
    }

    #[test]
    fn ship_too_heavy_warns() {
        let p = roll_plan(2_000_000_000.0, 300_000_000.0, 400_000_000.0);
        assert!(p.warning.is_some());
        assert_eq!(p.safe_passes, 0);
    }

    #[test]
    fn zero_inputs_are_safe() {
        let p = roll_plan(0.0, 0.0, 0.0);
        assert!(p.warning.is_some());
    }
}
