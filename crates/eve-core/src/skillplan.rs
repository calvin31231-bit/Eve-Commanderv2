//! Skill training-time math and plan estimation.
//!
//! EVE's training model is fully deterministic, so these are pure functions:
//! - A skill's cumulative skillpoints at level `L` is
//!   `round(250 × rank × 2^(2.5·(L-1)))`.
//! - Training speed is `primary + secondary/2` **SP per minute** (attributes,
//!   not counting boosters/implants, which the caller can fold into the
//!   attribute values).
//!
//! From those two, the time to move a skill between levels — and to clear a
//! whole plan — falls straight out. Everything here is unit-tested; the SDE
//! supplies skill ranks and the ESI supplies the character's current levels.

use serde::{Deserialize, Serialize};

/// Cumulative skillpoints needed to *have* a skill trained to `level` (0..=5).
/// Level 0 is 0 SP. Pure.
pub fn sp_for_level(rank: i64, level: i64) -> i64 {
    if level <= 0 || rank <= 0 {
        return 0;
    }
    let level = level.min(5);
    let exp = 2.5 * (level - 1) as f64;
    (250.0 * rank as f64 * 2f64.powf(exp)).round() as i64
}

/// Skillpoints gained per minute from a primary/secondary attribute pair. Pure.
pub fn sp_per_minute(primary: f64, secondary: f64) -> f64 {
    primary + secondary / 2.0
}

/// Seconds to train `sp` skillpoints at the given attributes. Returns 0 when the
/// attributes are non-positive (avoids a divide-by-zero). Pure.
pub fn training_seconds(sp: i64, primary: f64, secondary: f64) -> i64 {
    let rate = sp_per_minute(primary, secondary);
    if sp <= 0 || rate <= 0.0 {
        return 0;
    }
    ((sp as f64 / rate) * 60.0).ceil() as i64
}

/// One step in a skill plan: train `skill_type_id` from `current_level` up to
/// `target_level`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanStep {
    pub skill_type_id: i64,
    pub rank: i64,
    pub current_level: i64,
    pub target_level: i64,
    /// Primary/secondary attribute values for this skill (already including any
    /// implant/booster bonuses the caller wants to model).
    pub primary: f64,
    pub secondary: f64,
}

/// A costed plan step: the SP and time it adds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostedStep {
    pub skill_type_id: i64,
    pub target_level: i64,
    pub sp: i64,
    pub seconds: i64,
}

/// A fully costed skill plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanCost {
    pub steps: Vec<CostedStep>,
    pub total_sp: i64,
    pub total_seconds: i64,
}

/// Cost a skill plan: per-step SP/time plus totals. Steps that don't raise the
/// level (target ≤ current) contribute zero. Pure.
pub fn cost_plan(steps: &[PlanStep]) -> PlanCost {
    let mut costed = Vec::with_capacity(steps.len());
    let mut total_sp = 0;
    let mut total_seconds = 0;
    for s in steps {
        let from = sp_for_level(s.rank, s.current_level);
        let to = sp_for_level(s.rank, s.target_level);
        let sp = (to - from).max(0);
        let seconds = training_seconds(sp, s.primary, s.secondary);
        total_sp += sp;
        total_seconds += seconds;
        costed.push(CostedStep {
            skill_type_id: s.skill_type_id,
            target_level: s.target_level,
            sp,
            seconds,
        });
    }
    PlanCost { steps: costed, total_sp, total_seconds }
}

/// A candidate skill plan described in ISK terms: how much income it unlocks and
/// how long it takes to train, so plans can be ranked by return — not just time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoiPlan {
    pub label: String,
    /// Training time for the plan (seconds), e.g. from `cost_plan`.
    pub train_seconds: i64,
    /// Extra ISK per hour this plan unlocks while the activity is run.
    pub isk_per_hour: f64,
    /// Hours per day the player expects to run the activity.
    pub hours_per_day: f64,
    /// One-off ISK outlay to use the plan (hull/modules/skillbooks). 0 if none.
    #[serde(default)]
    pub upfront_isk: f64,
}

/// A ranked skill plan with its computed economics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoiResult {
    pub label: String,
    pub train_days: f64,
    pub daily_gain: f64,
    /// Days for the unlocked income to repay training-time + upfront cost. The
    /// training time is valued at the plan's own daily gain (opportunity cost).
    pub payback_days: f64,
    /// ISK/day of income unlocked per day of training invested — the ranking key.
    /// Higher means "train this first".
    pub roi_score: f64,
}

/// Rank candidate skill plans by ISK return on training time, best first. Pure.
/// Plans with no daily gain sort last with an infinite payback.
pub fn rank_roi(plans: &[RoiPlan]) -> Vec<RoiResult> {
    let mut out: Vec<RoiResult> = plans
        .iter()
        .map(|p| {
            let train_days = (p.train_seconds as f64 / 86_400.0).max(0.0);
            let daily_gain = (p.isk_per_hour * p.hours_per_day).max(0.0);
            let (payback_days, roi_score) = if daily_gain > 0.0 {
                // Repay the upfront outlay plus the income forgone while training.
                let train_cost = daily_gain * train_days;
                ((p.upfront_isk + train_cost) / daily_gain, daily_gain / train_days.max(0.01))
            } else {
                (f64::INFINITY, 0.0)
            };
            RoiResult {
                label: p.label.clone(),
                train_days,
                daily_gain,
                payback_days,
                roi_score,
            }
        })
        .collect();
    out.sort_by(|a, b| b.roi_score.partial_cmp(&a.roi_score).unwrap_or(std::cmp::Ordering::Equal));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sp_table_matches_known_values() {
        // Rank 1 reference points from the in-game skill sheet.
        assert_eq!(sp_for_level(1, 1), 250);
        assert_eq!(sp_for_level(1, 5), 256_000);
        // Rank scales linearly.
        assert_eq!(sp_for_level(3, 1), 750);
        assert_eq!(sp_for_level(1, 0), 0);
    }

    #[test]
    fn rate_and_time() {
        // 20 primary + 15 secondary → 27.5 SP/min.
        assert!((sp_per_minute(20.0, 15.0) - 27.5).abs() < 1e-9);
        // 27500 SP at 27.5/min = 1000 min = 60000 s.
        assert_eq!(training_seconds(27_500, 20.0, 15.0), 60_000);
        assert_eq!(training_seconds(100, 0.0, 0.0), 0);
    }

    #[test]
    fn cost_plan_sums_levels_and_skips_completed() {
        let steps = vec![
            // Rank-1 skill IV→V: 256000 - 45255 = 210745 SP.
            PlanStep { skill_type_id: 3300, rank: 1, current_level: 4, target_level: 5, primary: 20.0, secondary: 15.0 },
            // Already at target → contributes nothing.
            PlanStep { skill_type_id: 3301, rank: 1, current_level: 5, target_level: 5, primary: 20.0, secondary: 15.0 },
        ];
        let cost = cost_plan(&steps);
        assert_eq!(cost.steps[0].sp, 256_000 - sp_for_level(1, 4));
        assert_eq!(cost.steps[1].sp, 0);
        assert_eq!(cost.total_sp, cost.steps[0].sp);
        assert!(cost.total_seconds > 0);
    }

    #[test]
    fn roi_ranks_high_return_first() {
        let plans = vec![
            // Big earner, quick train → should rank first.
            RoiPlan {
                label: "Marauder".into(),
                train_seconds: 10 * 86_400,
                isk_per_hour: 200_000_000.0,
                hours_per_day: 3.0,
                upfront_isk: 1_500_000_000.0,
            },
            // Smaller earner, long train.
            RoiPlan {
                label: "Hauler".into(),
                train_seconds: 40 * 86_400,
                isk_per_hour: 20_000_000.0,
                hours_per_day: 2.0,
                upfront_isk: 0.0,
            },
        ];
        let r = rank_roi(&plans);
        assert_eq!(r[0].label, "Marauder");
        assert!(r[0].roi_score > r[1].roi_score);
        assert!(r[0].payback_days > 0.0 && r[0].payback_days.is_finite());
    }

    #[test]
    fn roi_zero_gain_sorts_last_with_infinite_payback() {
        let plans = vec![
            RoiPlan { label: "Dead".into(), train_seconds: 86_400, isk_per_hour: 0.0, hours_per_day: 4.0, upfront_isk: 0.0 },
            RoiPlan { label: "Live".into(), train_seconds: 86_400, isk_per_hour: 10.0, hours_per_day: 4.0, upfront_isk: 0.0 },
        ];
        let r = rank_roi(&plans);
        assert_eq!(r[0].label, "Live");
        assert!(r[1].payback_days.is_infinite());
    }
}
