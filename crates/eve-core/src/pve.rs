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

/// Security band an incursion is staged in — the dominant driver of the fleet
/// people run there and therefore the ISK/hr, per community MER/fleet lore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncursionBand {
    HighSec,
    LowSec,
    NullSec,
}

impl IncursionBand {
    /// Classify by the staging system's security status.
    pub fn from_security(security: f64) -> Self {
        if security >= 0.45 {
            IncursionBand::HighSec
        } else if security > 0.0 {
            IncursionBand::LowSec
        } else {
            IncursionBand::NullSec
        }
    }
}

/// A community ISK/hr estimate for running an incursion, expressed as a range
/// (payout swings with site type, fleet quality, and how farmed the pocket is).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IncursionEstimate {
    pub band: IncursionBand,
    /// Low end of the per-pilot ISK/hr range (millions).
    pub low_m: f64,
    /// High end of the per-pilot ISK/hr range (millions).
    pub high_m: f64,
}

/// Rough per-pilot ISK/hr for an incursion, keyed off its staging-system band
/// and dampened as the constellation gets farmed (falling influence lengthens
/// spawns). Deterministic reference numbers — the real payout is the site
/// bounty + shared corp payout, which the community tracks by band.
///
/// Highsec HQ fleets sit around 130–180M/hr fresh; lowsec and nullsec pay more
/// per site but carry travel + risk overhead, so the working ranges are wider.
pub fn incursion_estimate(band: IncursionBand, influence: f64) -> IncursionEstimate {
    let (base_low, base_high) = match band {
        IncursionBand::HighSec => (100.0, 180.0),
        IncursionBand::LowSec => (120.0, 220.0),
        IncursionBand::NullSec => (150.0, 300.0),
    };
    // Influence 1.0 = untouched (full payout); it decays toward ~0.6× as the
    // pocket is farmed out and spawns slow down.
    let farm = 0.6 + 0.4 * influence.clamp(0.0, 1.0);
    IncursionEstimate {
        band,
        low_m: base_low * farm,
        high_m: base_high * farm,
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
    fn band_classifies_by_security() {
        assert_eq!(IncursionBand::from_security(0.9), IncursionBand::HighSec);
        assert_eq!(IncursionBand::from_security(0.45), IncursionBand::HighSec);
        assert_eq!(IncursionBand::from_security(0.3), IncursionBand::LowSec);
        assert_eq!(IncursionBand::from_security(0.0), IncursionBand::NullSec);
        assert_eq!(IncursionBand::from_security(-0.5), IncursionBand::NullSec);
    }

    #[test]
    fn estimate_scales_with_influence() {
        let fresh = incursion_estimate(IncursionBand::HighSec, 1.0);
        let farmed = incursion_estimate(IncursionBand::HighSec, 0.0);
        assert!((fresh.high_m - 180.0).abs() < 1e-9);
        assert!(farmed.high_m < fresh.high_m);
        // Farmed floor is 0.6× the fresh payout.
        assert!((farmed.high_m - 180.0 * 0.6).abs() < 1e-9);
        // Nullsec pays more than highsec at the same influence.
        let null = incursion_estimate(IncursionBand::NullSec, 1.0);
        assert!(null.high_m > fresh.high_m);
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
