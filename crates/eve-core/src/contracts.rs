//! Character contracts (ESI `GET /characters/{id}/contracts/`, paginated).

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub contract_id: i64,
    /// item_exchange | auction | courier | loan.
    #[serde(rename = "type")]
    pub contract_type: String,
    /// outstanding | in_progress | finished | cancelled | …
    pub status: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub date_issued: String,
    #[serde(default)]
    pub date_expired: String,
    #[serde(default)]
    pub price: f64,
    #[serde(default)]
    pub reward: f64,
    #[serde(default)]
    pub collateral: f64,
    #[serde(default)]
    pub volume: f64,
    #[serde(default)]
    pub for_corporation: bool,
}

impl Contract {
    /// Whether the contract is still live (outstanding or in progress).
    pub fn is_active(&self) -> bool {
        matches!(self.status.as_str(), "outstanding" | "in_progress")
    }
}

/// Sort contracts active-first, then by issue date descending.
pub fn sort_for_display(contracts: &mut [Contract]) {
    contracts.sort_by(|a, b| {
        b.is_active()
            .cmp(&a.is_active())
            .then_with(|| b.date_issued.cmp(&a.date_issued))
    });
}

/// Typed, authenticated contract reads.
#[derive(Clone)]
pub struct ContractsClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl ContractsClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// Every page of the character's contracts, sorted for display.
    pub async fn contracts(&self, character_id: i64) -> Result<Vec<Contract>> {
        let ep = endpoint("contracts")
            .ok_or_else(|| Error::other("unknown endpoint 'contracts'"))?;
        let token = self.tokens.access_token(character_id).await?;
        let mut out = self
            .esi
            .get_auth_json_paged::<Contract>(&ep.path_for(character_id), &token)
            .await?;
        sort_for_display(&mut out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_and_flags_active() {
        let json = r#"{
            "contract_id": 1, "type": "courier", "status": "in_progress",
            "title": "Jita to Amarr", "date_issued": "2026-06-20T00:00:00Z",
            "date_expired": "2026-06-27T00:00:00Z", "reward": 1000000.0,
            "collateral": 50000000.0, "volume": 10000.0
        }"#;
        let c: Contract = serde_json::from_str(json).unwrap();
        assert_eq!(c.contract_type, "courier");
        assert!(c.is_active());
        assert_eq!(c.reward, 1_000_000.0);
    }

    #[test]
    fn sorts_active_first_then_newest() {
        let mut cs = vec![
            Contract { contract_id: 1, contract_type: "item_exchange".into(), status: "finished".into(), title: String::new(), date_issued: "2026-06-22".into(), date_expired: String::new(), price: 0.0, reward: 0.0, collateral: 0.0, volume: 0.0, for_corporation: false },
            Contract { contract_id: 2, contract_type: "courier".into(), status: "outstanding".into(), title: String::new(), date_issued: "2026-06-20".into(), date_expired: String::new(), price: 0.0, reward: 0.0, collateral: 0.0, volume: 0.0, for_corporation: false },
        ];
        sort_for_display(&mut cs);
        assert_eq!(cs[0].contract_id, 2); // active first despite older
    }
}
