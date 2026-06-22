//! Corporation reads: structures (with fuel-expiry countdowns) for the Corp hub.
//!
//! Structure fuel runs out at a known time, so — like industry jobs and PI
//! extractors — we fetch the expiry once and count down locally (no re-poll).
//! The [`summarize_structures`] reduction is pure and unit-tested; the corp
//! endpoint needs a director/station-manager role, so the caller treats a 403 as
//! "no access" rather than an error.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::EsiClient;

/// One corp structure (ESI `GET /corporations/{id}/structures/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpStructure {
    pub structure_id: i64,
    #[serde(default)]
    pub type_id: i64,
    pub system_id: i64,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub fuel_expires: Option<String>,
}

/// A structure with its fuel countdown derived client-side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureStatus {
    pub structure_id: i64,
    pub type_id: i64,
    pub system_id: i64,
    pub state: String,
    pub fuel_expires: Option<String>,
    /// Seconds until fuel runs out (0 if expired/unknown).
    pub fuel_seconds_remaining: i64,
}

/// Reduce corp structures to fuel countdowns, soonest-to-expire first. Pure
/// (time injected) → unit-tested.
pub fn summarize_structures(structures: &[CorpStructure], now: OffsetDateTime) -> Vec<StructureStatus> {
    let mut out: Vec<StructureStatus> = structures
        .iter()
        .map(|s| {
            let fuel_seconds_remaining = s
                .fuel_expires
                .as_deref()
                .and_then(|t| OffsetDateTime::parse(t, &Rfc3339).ok())
                .map(|t| (t - now).whole_seconds().max(0))
                .unwrap_or(0);
            StructureStatus {
                structure_id: s.structure_id,
                type_id: s.type_id,
                system_id: s.system_id,
                state: s.state.clone(),
                fuel_expires: s.fuel_expires.clone(),
                fuel_seconds_remaining,
            }
        })
        .collect();
    // Structures with a fuel timer first (soonest expiry), then the rest.
    out.sort_by_key(|s| {
        if s.fuel_expires.is_some() {
            (0, s.fuel_seconds_remaining)
        } else {
            (1, 0)
        }
    });
    out
}

/// Authenticated corporation reads over the cache-first ESI client.
#[derive(Clone)]
pub struct CorpClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl CorpClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The corporation's structures. Requires a director / station-manager role
    /// plus `esi-corporations.read_structures.v1`; ESI 403s otherwise (the caller
    /// treats an error as "no access").
    pub async fn structures(&self, character_id: i64, corp_id: i64) -> Result<Vec<CorpStructure>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/corporations/{corp_id}/structures/");
        self.esi
            .get_auth_json_paged::<CorpStructure>(&path, &token)
            .await
            .map_err(|e| Error::other(format!("corp structures: {e}")))
    }

    /// Fetch + summarize structures with fuel countdowns (now injected here so
    /// the shell needn't depend on `time`).
    pub async fn structure_status(
        &self,
        character_id: i64,
        corp_id: i64,
    ) -> Result<Vec<StructureStatus>> {
        let structures = self.structures(character_id, corp_id).await?;
        Ok(summarize_structures(&structures, OffsetDateTime::now_utc()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }

    fn st(id: i64, fuel: Option<&str>) -> CorpStructure {
        CorpStructure {
            structure_id: id,
            type_id: 35832,
            system_id: 30000142,
            state: "shield_vulnerable".into(),
            fuel_expires: fuel.map(str::to_string),
        }
    }

    #[test]
    fn fuel_countdown_and_ordering() {
        let now = ts("2026-06-22T00:00:00Z");
        let structures = vec![
            st(1, Some("2026-06-24T00:00:00Z")), // 2 days
            st(2, Some("2026-06-22T01:00:00Z")), // 1 hour (soonest)
            st(3, None),                          // no timer → last
        ];
        let out = summarize_structures(&structures, now);
        assert_eq!(out[0].structure_id, 2);
        assert_eq!(out[0].fuel_seconds_remaining, 3600);
        assert_eq!(out[2].structure_id, 3);
        assert_eq!(out[2].fuel_seconds_remaining, 0);
    }

    #[test]
    fn expired_fuel_clamps_to_zero() {
        let now = ts("2026-06-22T00:00:00Z");
        let out = summarize_structures(&[st(1, Some("2026-06-21T00:00:00Z"))], now);
        assert_eq!(out[0].fuel_seconds_remaining, 0);
    }
}
