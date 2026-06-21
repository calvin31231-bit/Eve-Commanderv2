//! Ship insurance prices (public ESI `GET /insurance/prices/`).
//!
//! One day-cached call returns every insurable ship type with its tiered
//! premium/payout levels. We index it by type id so the market browser can show
//! insurance tiers (and net payout) for a selected ship.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::esi::EsiClient;

/// One insurance tier for a ship type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsuranceLevel {
    pub name: String,
    /// Premium paid up front.
    pub cost: f64,
    /// Payout on loss.
    pub payout: f64,
}

/// Insurance levels for one ship type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsuranceType {
    pub type_id: i64,
    #[serde(default)]
    pub levels: Vec<InsuranceLevel>,
}

/// Reads + indexes the public insurance price list.
#[derive(Clone)]
pub struct InsuranceClient {
    esi: EsiClient,
}

impl InsuranceClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    /// The full insurance price list.
    pub async fn prices(&self) -> Result<Vec<InsuranceType>> {
        self.esi
            .get_public_json::<Vec<InsuranceType>>("/latest/insurance/prices/")
            .await
    }

    /// Insurance tiers for one ship type, if insurable.
    pub async fn for_type(&self, type_id: i64) -> Result<Option<Vec<InsuranceLevel>>> {
        let prices = self.prices().await?;
        Ok(prices
            .into_iter()
            .find(|p| p.type_id == type_id)
            .map(|p| p.levels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_insurance_type() {
        let json = r#"{
            "type_id": 587,
            "levels": [
                {"name": "Basic", "cost": 100.0, "payout": 1000.0},
                {"name": "Standard", "cost": 500.0, "payout": 5000.0}
            ]
        }"#;
        let t: InsuranceType = serde_json::from_str(json).unwrap();
        assert_eq!(t.type_id, 587);
        assert_eq!(t.levels.len(), 2);
        assert_eq!(t.levels[1].payout, 5000.0);
    }
}
