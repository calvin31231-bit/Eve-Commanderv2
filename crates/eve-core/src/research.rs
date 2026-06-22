//! R&D agents: datacore (research-point) accumulation.
//!
//! Each running R&D agent produces `points_per_day` research points that pile up
//! until spent on datacores — passive income. ESI reports the per-day rate and a
//! `remainder_points` baseline at `started_at`; current accrued RP is
//! [`accumulated_points`], pure + unit-tested. The fetch is exercised live.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One running R&D agent (ESI `GET /characters/{id}/agents_research/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchAgent {
    pub agent_id: i64,
    #[serde(default)]
    pub skill_type_id: i64,
    #[serde(default)]
    pub points_per_day: f64,
    #[serde(default)]
    pub remainder_points: f64,
    #[serde(default)]
    pub started_at: String,
}

/// Research points accrued since `started_at`: the baseline remainder plus the
/// per-day rate over the elapsed days. Pure.
pub fn accumulated_points(remainder_points: f64, points_per_day: f64, days_elapsed: f64) -> f64 {
    remainder_points + points_per_day * days_elapsed.max(0.0)
}

/// Authenticated R&D-agent reads over the cache-first ESI client.
#[derive(Clone)]
pub struct ResearchClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl ResearchClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's running R&D agents.
    pub async fn agents(&self, character_id: i64) -> Result<Vec<ResearchAgent>> {
        let ep = endpoint("agents_research")
            .ok_or_else(|| Error::other("unknown endpoint 'agents_research'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<Vec<ResearchAgent>>(&ep.path_for(character_id), &token)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accrues_over_time() {
        // 1000 baseline + 50/day * 10 days = 1500.
        assert!((accumulated_points(1000.0, 50.0, 10.0) - 1500.0).abs() < 1e-9);
        // Negative elapsed clamps to the baseline.
        assert!((accumulated_points(1000.0, 50.0, -5.0) - 1000.0).abs() < 1e-9);
    }
}
