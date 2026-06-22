//! PvE content: live incursions + faction-warfare system status.
//!
//! Both are public ESI reads (no scopes, no character needed), so they work for
//! anyone: where incursions are staged and how farmed they are, and which FW
//! systems are contested. Light reductions; the fetches are exercised live.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::esi::EsiClient;

/// One active incursion (ESI `GET /incursions/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Incursion {
    #[serde(default)]
    pub constellation_id: i64,
    #[serde(default)]
    pub staging_solar_system_id: i64,
    #[serde(default)]
    pub state: String,
    /// 0.0–1.0; drops as runners farm the constellation.
    #[serde(default)]
    pub influence: f64,
    #[serde(default)]
    pub has_boss: bool,
    #[serde(default)]
    pub infested_solar_systems: Vec<i64>,
    #[serde(default)]
    pub faction_id: i64,
}

/// One faction-warfare system (ESI `GET /fw/systems/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FwSystem {
    pub solar_system_id: i64,
    #[serde(default)]
    pub owner_faction_id: i64,
    #[serde(default)]
    pub occupier_faction_id: i64,
    #[serde(default)]
    pub contested: String,
    #[serde(default)]
    pub victory_points: i64,
    #[serde(default)]
    pub victory_points_threshold: i64,
}

impl FwSystem {
    /// Contest progress 0.0–1.0 toward the next flip.
    pub fn progress(&self) -> f64 {
        if self.victory_points_threshold > 0 {
            (self.victory_points as f64 / self.victory_points_threshold as f64).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

/// Reads public PvE-content endpoints.
#[derive(Clone)]
pub struct PveClient {
    esi: EsiClient,
}

impl PveClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    /// All active incursions, freshest (highest influence) first.
    pub async fn incursions(&self) -> Result<Vec<Incursion>> {
        let mut out = self
            .esi
            .get_public_json::<Vec<Incursion>>("/latest/incursions/")
            .await?;
        out.sort_by(|a, b| {
            b.influence
                .partial_cmp(&a.influence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    /// Faction-warfare systems that are actively contested, most-contested first.
    pub async fn contested_fw_systems(&self) -> Result<Vec<FwSystem>> {
        let mut systems = self
            .esi
            .get_public_json::<Vec<FwSystem>>("/latest/fw/systems/")
            .await?;
        systems.retain(|s| s.contested == "contested" || s.victory_points > 0);
        systems.sort_by(|a, b| {
            b.progress()
                .partial_cmp(&a.progress())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(systems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fw_progress_ratio() {
        let s = FwSystem {
            solar_system_id: 1,
            owner_faction_id: 500001,
            occupier_faction_id: 500002,
            contested: "contested".into(),
            victory_points: 1500,
            victory_points_threshold: 3000,
        };
        assert!((s.progress() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn fw_progress_handles_zero_threshold() {
        let s = FwSystem {
            solar_system_id: 1,
            owner_faction_id: 0,
            occupier_faction_id: 0,
            contested: "uncontested".into(),
            victory_points: 0,
            victory_points_threshold: 0,
        };
        assert_eq!(s.progress(), 0.0);
    }

    #[test]
    fn incursion_deserializes() {
        let json = r#"{"constellation_id":20000001,"staging_solar_system_id":30000001,
            "state":"established","influence":0.85,"has_boss":true,
            "infested_solar_systems":[30000001,30000002],"faction_id":500019}"#;
        let inc: Incursion = serde_json::from_str(json).unwrap();
        assert_eq!(inc.state, "established");
        assert!(inc.has_boss);
        assert_eq!(inc.infested_solar_systems.len(), 2);
    }
}
