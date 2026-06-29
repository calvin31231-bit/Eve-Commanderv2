//! Abyssal Deadspace run tracker.
//!
//! Players log each abyss run (tier, weather, ship/fit, time, loot, whether they
//! survived); this module turns a history of runs into the stats that matter —
//! ISK/hr, survival rate, and a per-tier breakdown. The aggregation is pure and
//! unit-tested; persistence lives in `db::abyss`.

use serde::{Deserialize, Serialize};

/// One logged abyssal run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbyssRun {
    pub id: i64,
    /// When the run happened (epoch seconds).
    pub ran_at: i64,
    /// Difficulty tier 1–6 (Calm … Cataclysmic); 0 if unspecified.
    pub tier: i64,
    /// Weather/typhoon (Dark, Gamma, Electrical, Exotic, Firestorm); free text.
    pub weather: String,
    pub ship: String,
    /// Optional fit name or EFT.
    pub fit: String,
    pub duration_seconds: i64,
    pub loot_value: f64,
    /// False if the run killed the ship.
    pub survived: bool,
    pub notes: String,
}

/// Per-tier rollup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierStat {
    pub tier: i64,
    pub runs: usize,
    pub avg_loot: f64,
    pub avg_seconds: i64,
}

/// Aggregate stats across a run history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbyssStats {
    pub runs: usize,
    pub deaths: usize,
    /// Fraction of runs survived (0–1); 0 when there are no runs.
    pub survival_rate: f64,
    pub total_loot: f64,
    pub avg_loot: f64,
    pub total_seconds: i64,
    pub avg_seconds: i64,
    /// Loot per hour across all logged run time.
    pub isk_per_hour: f64,
    pub best_loot: f64,
    /// Per-tier breakdown, tier ascending.
    pub by_tier: Vec<TierStat>,
}

/// Summarize a run history into headline stats + a per-tier breakdown. Pure.
pub fn summarize(runs: &[AbyssRun]) -> AbyssStats {
    use std::collections::BTreeMap;

    if runs.is_empty() {
        return AbyssStats {
            runs: 0,
            deaths: 0,
            survival_rate: 0.0,
            total_loot: 0.0,
            avg_loot: 0.0,
            total_seconds: 0,
            avg_seconds: 0,
            isk_per_hour: 0.0,
            best_loot: 0.0,
            by_tier: Vec::new(),
        };
    }

    let n = runs.len();
    let deaths = runs.iter().filter(|r| !r.survived).count();
    let total_loot: f64 = runs.iter().map(|r| r.loot_value).sum();
    let total_seconds: i64 = runs.iter().map(|r| r.duration_seconds).sum();
    let best_loot = runs.iter().map(|r| r.loot_value).fold(0.0, f64::max);
    let isk_per_hour = if total_seconds > 0 {
        total_loot / (total_seconds as f64 / 3600.0)
    } else {
        0.0
    };

    // Per-tier: (count, loot sum, seconds sum).
    let mut tiers: BTreeMap<i64, (usize, f64, i64)> = BTreeMap::new();
    for r in runs {
        let e = tiers.entry(r.tier).or_insert((0, 0.0, 0));
        e.0 += 1;
        e.1 += r.loot_value;
        e.2 += r.duration_seconds;
    }
    let by_tier = tiers
        .into_iter()
        .map(|(tier, (count, loot, secs))| TierStat {
            tier,
            runs: count,
            avg_loot: loot / count as f64,
            avg_seconds: secs / count as i64,
        })
        .collect();

    AbyssStats {
        runs: n,
        deaths,
        survival_rate: (n - deaths) as f64 / n as f64,
        total_loot,
        avg_loot: total_loot / n as f64,
        total_seconds,
        avg_seconds: total_seconds / n as i64,
        isk_per_hour,
        best_loot,
        by_tier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(tier: i64, secs: i64, loot: f64, survived: bool) -> AbyssRun {
        AbyssRun {
            id: 0,
            ran_at: 0,
            tier,
            weather: "Dark".into(),
            ship: "Gila".into(),
            fit: String::new(),
            duration_seconds: secs,
            loot_value: loot,
            survived,
            notes: String::new(),
        }
    }

    #[test]
    fn empty_history_is_zeroed() {
        let s = summarize(&[]);
        assert_eq!(s.runs, 0);
        assert_eq!(s.isk_per_hour, 0.0);
        assert!(s.by_tier.is_empty());
    }

    #[test]
    fn aggregates_totals_rate_and_isk_per_hour() {
        let runs = vec![
            run(3, 600, 30_000_000.0, true),  // 10 min, 30M
            run(3, 600, 50_000_000.0, true),  // 10 min, 50M
            run(4, 1200, 0.0, false),         // 20 min, died, 0 loot
        ];
        let s = summarize(&runs);
        assert_eq!(s.runs, 3);
        assert_eq!(s.deaths, 1);
        assert!((s.survival_rate - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(s.total_loot, 80_000_000.0);
        assert_eq!(s.total_seconds, 2400);
        assert_eq!(s.best_loot, 50_000_000.0);
        // 80M over 2400s = 40 min → 120M/hr.
        assert!((s.isk_per_hour - 120_000_000.0).abs() < 1.0);
    }

    #[test]
    fn per_tier_breakdown() {
        let runs = vec![
            run(3, 600, 30_000_000.0, true),
            run(3, 600, 50_000_000.0, true),
            run(5, 900, 90_000_000.0, true),
        ];
        let s = summarize(&runs);
        assert_eq!(s.by_tier.len(), 2);
        assert_eq!(s.by_tier[0].tier, 3);
        assert_eq!(s.by_tier[0].runs, 2);
        assert_eq!(s.by_tier[0].avg_loot, 40_000_000.0);
        assert_eq!(s.by_tier[1].tier, 5);
    }
}
