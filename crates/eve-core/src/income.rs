//! Cross-content income optimizer.
//!
//! Ranks the income activities a player *can actually do right now* by realistic
//! ISK/hr — after risk, and against the time they have. The value of this view
//! is that one app holds the player's skills, assets, location and standings, so
//! it can filter the menu to what they're eligible for and rank by risk-adjusted
//! return. The math is deterministic and pure; the caller supplies the candidate
//! activities (their gross ISK/hr and gating facts).

use serde::{Deserialize, Serialize};

/// One candidate income activity the player might run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncomeActivity {
    pub name: String,
    /// Gross ISK/hr the activity yields at the player's competence.
    pub isk_per_hour: f64,
    /// Expected loss as a fraction of gross (0.0 = safe ratting in a cheap hull,
    /// 0.3 = risky lowsec content). Applied to discount the gross rate.
    #[serde(default)]
    pub risk: f64,
    /// One-off ISK outlay to start (hull/fit/standings grind). 0 if none.
    #[serde(default)]
    pub setup_cost: f64,
    /// Whether the player meets the requirements (skills/standings/access).
    /// Ineligible activities are reported but sorted last.
    #[serde(default)]
    pub eligible: bool,
}

/// A ranked activity with its computed economics for the session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IncomeRanking {
    pub name: String,
    pub eligible: bool,
    /// ISK/hr after risk discount.
    pub effective_isk_per_hour: f64,
    /// Net ISK over the session: effective rate × hours − setup cost.
    pub session_profit: f64,
}

/// Rank income activities by risk-adjusted return over `available_hours`. Eligible
/// activities sort first (by session profit, highest first), ineligible ones
/// after. Pure.
pub fn rank_income(activities: &[IncomeActivity], available_hours: f64) -> Vec<IncomeRanking> {
    let hours = available_hours.max(0.0);
    let mut out: Vec<IncomeRanking> = activities
        .iter()
        .map(|a| {
            let risk = a.risk.clamp(0.0, 1.0);
            let eff = (a.isk_per_hour.max(0.0)) * (1.0 - risk);
            IncomeRanking {
                name: a.name.clone(),
                eligible: a.eligible,
                effective_isk_per_hour: eff,
                session_profit: eff * hours - a.setup_cost.max(0.0),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        // Eligible first, then by session profit descending.
        b.eligible
            .cmp(&a.eligible)
            .then_with(|| {
                b.session_profit
                    .partial_cmp(&a.session_profit)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(name: &str, rate: f64, risk: f64, setup: f64, eligible: bool) -> IncomeActivity {
        IncomeActivity {
            name: name.into(),
            isk_per_hour: rate,
            risk,
            setup_cost: setup,
            eligible,
        }
    }

    #[test]
    fn ranks_eligible_by_risk_adjusted_profit() {
        let acts = vec![
            // High gross but very risky → discounted below the safe option.
            act("Lowsec ratting", 120_000_000.0, 0.5, 0.0, true),
            // Lower gross, safe → wins after risk discount.
            act("Highsec missions", 80_000_000.0, 0.0, 0.0, true),
        ];
        let r = rank_income(&acts, 3.0);
        assert_eq!(r[0].name, "Highsec missions");
        assert!((r[0].effective_isk_per_hour - 80_000_000.0).abs() < 1.0);
        assert!((r[1].effective_isk_per_hour - 60_000_000.0).abs() < 1.0);
    }

    #[test]
    fn ineligible_sorts_after_eligible() {
        let acts = vec![
            act("Incursions (need fit)", 200_000_000.0, 0.1, 0.0, false),
            act("Mining", 15_000_000.0, 0.0, 0.0, true),
        ];
        let r = rank_income(&acts, 2.0);
        assert_eq!(r[0].name, "Mining");
        assert!(!r[1].eligible);
    }

    #[test]
    fn setup_cost_reduces_session_profit() {
        let acts = vec![act("Ratting", 50_000_000.0, 0.0, 100_000_000.0, true)];
        let r = rank_income(&acts, 1.0);
        // 50M earned − 100M setup = −50M this session.
        assert!((r[0].session_profit + 50_000_000.0).abs() < 1.0);
    }
}
