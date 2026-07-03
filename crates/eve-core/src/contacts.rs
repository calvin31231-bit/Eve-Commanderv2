//! Contacts & standings editor (`esi-characters` read/write_contacts).
//!
//! Read the character's contact list and manage it: add a contact at a
//! standing, change a standing, remove a contact. Standings drive the threat
//! scanner's blue/red logic, so keeping them editable in-app closes the loop —
//! see a hostile in the scanner, red them here. ESI-sanctioned writes only.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::Result;
use crate::esi::EsiClient;

/// One contact (ESI `GET /characters/{id}/contacts/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contact {
    pub contact_id: i64,
    #[serde(default)]
    pub contact_type: String,
    #[serde(default)]
    pub standing: f64,
    #[serde(default)]
    pub is_blocked: Option<bool>,
    #[serde(default)]
    pub is_watched: Option<bool>,
}

/// Clamp a standing to EVE's valid −10.0..=10.0 range. Pure.
pub fn clamp_standing(standing: f64) -> f64 {
    standing.clamp(-10.0, 10.0)
}

/// Authenticated contact reads/writes over the cache-first ESI client.
#[derive(Clone)]
pub struct ContactsClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl ContactsClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's contacts (paged).
    pub async fn contacts(&self, character_id: i64) -> Result<Vec<Contact>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/contacts/");
        self.esi.get_auth_json_paged::<Contact>(&path, &token).await
    }

    /// Add contacts at `standing`. Returns the created contact ids.
    pub async fn add(
        &self,
        character_id: i64,
        contact_ids: &[i64],
        standing: f64,
    ) -> Result<Vec<i64>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!(
            "/latest/characters/{character_id}/contacts/?standing={}",
            clamp_standing(standing)
        );
        self.esi.post_auth_json::<_, Vec<i64>>(&path, contact_ids, &token).await
    }

    /// Change existing contacts to `standing`.
    pub async fn edit(
        &self,
        character_id: i64,
        contact_ids: &[i64],
        standing: f64,
    ) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!(
            "/latest/characters/{character_id}/contacts/?standing={}",
            clamp_standing(standing)
        );
        self.esi.put_auth_empty(&path, contact_ids, &token).await
    }

    /// Remove contacts.
    pub async fn delete(&self, character_id: i64, contact_ids: &[i64]) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let ids = contact_ids
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let path = format!("/latest/characters/{character_id}/contacts/?contact_ids={ids}");
        self.esi.delete_auth_empty(&path, &token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_clamps_to_esi_range() {
        assert_eq!(clamp_standing(15.0), 10.0);
        assert_eq!(clamp_standing(-11.0), -10.0);
        assert_eq!(clamp_standing(5.5), 5.5);
    }

    #[test]
    fn contact_deserializes_with_defaults() {
        let c: Contact =
            serde_json::from_str(r#"{"contact_id": 90000001, "contact_type": "character"}"#)
                .unwrap();
        assert_eq!(c.contact_id, 90000001);
        assert_eq!(c.standing, 0.0);
        assert!(c.is_blocked.is_none());
    }
}
