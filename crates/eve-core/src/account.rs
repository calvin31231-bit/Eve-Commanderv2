//! Account-wide aggregation across all of a user's characters.
//!
//! The "one app for your whole account" view: each character's net worth
//! (wallet + asset value), SP, and balances rolled into combined totals plus a
//! per-character breakdown. Fetching each character's figures is done by the
//! shell (network); the [`aggregate`] roll-up here is pure and unit-tested.

use serde::{Deserialize, Serialize};

/// One character's headline figures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterWorth {
    pub character_id: i64,
    pub name: String,
    pub wallet_balance: f64,
    pub asset_value: f64,
    pub total_sp: i64,
    /// wallet + assets.
    pub net_worth: f64,
}

impl CharacterWorth {
    pub fn new(
        character_id: i64,
        name: impl Into<String>,
        wallet_balance: f64,
        asset_value: f64,
        total_sp: i64,
    ) -> Self {
        Self {
            character_id,
            name: name.into(),
            wallet_balance,
            asset_value,
            total_sp,
            net_worth: wallet_balance + asset_value,
        }
    }
}

/// Combined account figures plus the per-character breakdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountOverview {
    /// Per-character, richest first.
    pub characters: Vec<CharacterWorth>,
    pub total_net_worth: f64,
    pub total_wallet: f64,
    pub total_asset_value: f64,
    pub total_sp: i64,
}

/// Roll up per-character figures into account totals (characters sorted by net
/// worth, descending).
pub fn aggregate(mut characters: Vec<CharacterWorth>) -> AccountOverview {
    characters.sort_by(|a, b| {
        b.net_worth
            .partial_cmp(&a.net_worth)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.character_id.cmp(&b.character_id))
    });

    AccountOverview {
        total_net_worth: characters.iter().map(|c| c.net_worth).sum(),
        total_wallet: characters.iter().map(|c| c.wallet_balance).sum(),
        total_asset_value: characters.iter().map(|c| c.asset_value).sum(),
        total_sp: characters.iter().map(|c| c.total_sp).sum(),
        characters,
    }
}

/// A net-worth trend read: the ISK/day velocity fitted across the snapshot
/// series and a linear projection forward. The "portfolio velocity / MER" number
/// no web tool can give, since it needs our persisted history.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NetWorthTrend {
    /// Least-squares slope in ISK per day (negative = bleeding ISK).
    pub per_day: f64,
    /// Latest observed value + `per_day` × `horizon_days`.
    pub projected: f64,
    /// Days ahead the projection covers.
    pub horizon_days: i64,
    /// Fit quality 0.0–1.0 (R²); low means the trend is noisy/unreliable.
    pub r_squared: f64,
    /// Number of snapshots the fit used.
    pub points: usize,
}

/// Fit a straight line to a net-worth series (`(epoch_secs, value)`, assumed in
/// time order) and project it `horizon_days` forward. Pure — ordinary
/// least-squares on days-since-start. Fewer than two distinct-time points yields
/// a flat, zero-confidence trend. Unit-tested.
pub fn networth_trend(series: &[(i64, f64)], horizon_days: i64) -> NetWorthTrend {
    let flat = |projected: f64| NetWorthTrend {
        per_day: 0.0,
        projected,
        horizon_days,
        r_squared: 0.0,
        points: series.len(),
    };
    let last_value = series.last().map(|&(_, v)| v).unwrap_or(0.0);
    if series.len() < 2 {
        return flat(last_value);
    }
    let t0 = series[0].0;
    // x in days since the first snapshot; y in ISK.
    let xs: Vec<f64> = series.iter().map(|&(t, _)| (t - t0) as f64 / 86_400.0).collect();
    let ys: Vec<f64> = series.iter().map(|&(_, v)| v).collect();
    let n = xs.len() as f64;
    let mean_x = xs.iter().sum::<f64>() / n;
    let mean_y = ys.iter().sum::<f64>() / n;

    let mut sxx = 0.0;
    let mut sxy = 0.0;
    let mut syy = 0.0;
    for (&x, &y) in xs.iter().zip(ys.iter()) {
        let dx = x - mean_x;
        let dy = y - mean_y;
        sxx += dx * dx;
        sxy += dx * dy;
        syy += dy * dy;
    }
    if sxx <= 0.0 {
        // All snapshots share a timestamp — no slope to fit.
        return flat(last_value);
    }
    let per_day = sxy / sxx;
    let r_squared = if syy > 0.0 {
        (sxy * sxy / (sxx * syy)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    NetWorthTrend {
        per_day,
        projected: last_value + per_day * horizon_days as f64,
        horizon_days,
        r_squared,
        points: series.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_worth_is_wallet_plus_assets() {
        let c = CharacterWorth::new(1, "Pilot", 1_000_000.0, 4_000_000.0, 5_000_000);
        assert_eq!(c.net_worth, 5_000_000.0);
    }

    #[test]
    fn aggregate_totals_and_orders_by_net_worth() {
        let overview = aggregate(vec![
            CharacterWorth::new(1, "Alt", 500_000.0, 500_000.0, 2_000_000), // 1.0M
            CharacterWorth::new(2, "Main", 10_000_000.0, 40_000_000.0, 80_000_000), // 50M
        ]);
        // Totals.
        assert_eq!(overview.total_wallet, 10_500_000.0);
        assert_eq!(overview.total_asset_value, 40_500_000.0);
        assert_eq!(overview.total_net_worth, 51_000_000.0);
        assert_eq!(overview.total_sp, 82_000_000);
        // Richest first.
        assert_eq!(overview.characters[0].name, "Main");
        assert_eq!(overview.characters[1].name, "Alt");
    }

    #[test]
    fn empty_account_is_zeroed() {
        let overview = aggregate(vec![]);
        assert_eq!(overview.total_net_worth, 0.0);
        assert!(overview.characters.is_empty());
    }

    #[test]
    fn trend_fits_a_clean_upward_line() {
        // +1M ISK/day over 5 days, perfectly linear.
        let day = 86_400;
        let series: Vec<(i64, f64)> = (0..5)
            .map(|d| (d * day, 10_000_000.0 + d as f64 * 1_000_000.0))
            .collect();
        let t = networth_trend(&series, 30);
        assert!((t.per_day - 1_000_000.0).abs() < 1.0);
        assert!((t.r_squared - 1.0).abs() < 1e-6);
        // Last value 14M + 1M/day × 30 = 44M.
        assert!((t.projected - 44_000_000.0).abs() < 1.0);
        assert_eq!(t.points, 5);
    }

    #[test]
    fn trend_degrades_gracefully_on_thin_or_flat_data() {
        // Single point → flat, zero confidence, projects the value itself.
        let one = networth_trend(&[(0, 5_000_000.0)], 30);
        assert_eq!(one.per_day, 0.0);
        assert_eq!(one.r_squared, 0.0);
        assert_eq!(one.projected, 5_000_000.0);

        // Same-timestamp points can't yield a slope.
        let stacked = networth_trend(&[(100, 1.0), (100, 2.0)], 30);
        assert_eq!(stacked.per_day, 0.0);

        // Downward trend reads negative.
        let day = 86_400;
        let falling: Vec<(i64, f64)> =
            (0..4).map(|d| (d * day, 20_000_000.0 - d as f64 * 2_000_000.0)).collect();
        assert!(networth_trend(&falling, 7).per_day < 0.0);
    }
}
