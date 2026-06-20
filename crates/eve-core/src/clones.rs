//! Jump clones and active implants for the Character hub.
//!
//! Two ESI reads back the clone view: `GET /characters/{id}/clones/` (home
//! station + the list of jump clones, each with its installed implants) and
//! `GET /characters/{id}/implants/` (the type ids currently plugged into the
//! *active* clone). The models mirror ESI; the [`ClonesSummary`] derivation is
//! pure and unit-tested.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// A clone/home location reference (ESI `location_id` + `location_type`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneLocation {
    pub location_id: i64,
    #[serde(default)]
    pub location_type: String,
}

/// One jump clone (ESI `GET /characters/{id}/clones/` → `jump_clones[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JumpClone {
    pub jump_clone_id: i64,
    pub location_id: i64,
    #[serde(default)]
    pub location_type: String,
    #[serde(default)]
    pub implants: Vec<i64>,
    #[serde(default)]
    pub name: Option<String>,
}

/// The clones response (ESI `GET /characters/{id}/clones/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clones {
    #[serde(default)]
    pub home_location: Option<CloneLocation>,
    #[serde(default)]
    pub jump_clones: Vec<JumpClone>,
    #[serde(default)]
    pub last_clone_jump_date: Option<String>,
}

/// Flattened, serializable clone view for the hub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClonesSummary {
    pub jump_clone_count: usize,
    pub home_location_id: Option<i64>,
    /// Implant type ids plugged into the active clone.
    pub active_implants: Vec<i64>,
}

impl ClonesSummary {
    /// Assemble the summary from the two source reads. Pure → unit-tested.
    pub fn assemble(clones: &Clones, active_implants: Vec<i64>) -> Self {
        Self {
            jump_clone_count: clones.jump_clones.len(),
            home_location_id: clones.home_location.as_ref().map(|l| l.location_id),
            active_implants,
        }
    }
}

/// Typed, authenticated clone reads over the cache-first ESI client.
#[derive(Clone)]
pub struct ClonesClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl ClonesClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    async fn auth_get<T: serde::de::DeserializeOwned>(
        &self,
        character_id: i64,
        endpoint_key: &str,
    ) -> Result<T> {
        let ep = endpoint(endpoint_key)
            .ok_or_else(|| Error::other(format!("unknown endpoint '{endpoint_key}'")))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<T>(&ep.path_for(character_id), &token)
            .await
    }

    /// The clones response (home + jump clones).
    pub async fn clones(&self, character_id: i64) -> Result<Clones> {
        self.auth_get(character_id, "clones").await
    }

    /// Implant type ids in the active clone.
    pub async fn active_implants(&self, character_id: i64) -> Result<Vec<i64>> {
        self.auth_get(character_id, "implants").await
    }

    /// Fetch both reads concurrently and assemble the summary.
    pub async fn summary(&self, character_id: i64) -> Result<ClonesSummary> {
        let (clones, implants) =
            tokio::join!(self.clones(character_id), self.active_implants(character_id));
        Ok(ClonesSummary::assemble(&clones?, implants?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_clones_response() {
        let json = r#"{
            "home_location": {"location_id": 60003760, "location_type": "station"},
            "jump_clones": [
                {"jump_clone_id": 1, "location_id": 60003760, "location_type": "station", "implants": [10212, 13255], "name": "PvP"},
                {"jump_clone_id": 2, "location_id": 60008494, "location_type": "station", "implants": []}
            ],
            "last_clone_jump_date": "2026-06-01T00:00:00Z"
        }"#;
        let clones: Clones = serde_json::from_str(json).unwrap();
        assert_eq!(clones.jump_clones.len(), 2);
        assert_eq!(clones.home_location.as_ref().unwrap().location_id, 60003760);
        assert_eq!(clones.jump_clones[0].implants, vec![10212, 13255]);
        assert_eq!(clones.jump_clones[0].name.as_deref(), Some("PvP"));
    }

    #[test]
    fn implants_is_a_bare_id_array() {
        let implants: Vec<i64> = serde_json::from_str("[10212, 10213, 10214]").unwrap();
        assert_eq!(implants, vec![10212, 10213, 10214]);
    }

    #[test]
    fn clones_response_tolerates_empty_character() {
        // A character with no jump clones / no home set.
        let clones: Clones = serde_json::from_str("{}").unwrap();
        assert!(clones.jump_clones.is_empty());
        assert!(clones.home_location.is_none());
    }

    #[test]
    fn assembles_summary() {
        let clones = Clones {
            home_location: Some(CloneLocation { location_id: 60003760, location_type: "station".into() }),
            jump_clones: vec![
                JumpClone { jump_clone_id: 1, location_id: 1, location_type: "station".into(), implants: vec![10212], name: None },
                JumpClone { jump_clone_id: 2, location_id: 2, location_type: "station".into(), implants: vec![], name: None },
            ],
            last_clone_jump_date: None,
        };
        let summary = ClonesSummary::assemble(&clones, vec![10212, 13255]);
        assert_eq!(summary.jump_clone_count, 2);
        assert_eq!(summary.home_location_id, Some(60003760));
        assert_eq!(summary.active_implants, vec![10212, 13255]);
    }
}
