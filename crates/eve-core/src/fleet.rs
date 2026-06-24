//! Fleet: live fleet composition (read side of `esi-fleets`).
//!
//! `GET /characters/{id}/fleet/` returns the fleet the character is in (404 when
//! not in one); `GET /fleets/{fleet_id}/members/` lists the members with their
//! ships and locations. Read-only here — the EULA-sanctioned write side (move
//! members, broadcasts) can layer on later. Exercised live.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::Result;
use crate::esi::EsiClient;

/// The character's current fleet (ESI `GET /characters/{id}/fleet/`).
#[derive(Debug, Clone, Deserialize)]
pub struct FleetInfo {
    pub fleet_id: i64,
    #[serde(default)]
    pub role: String,
}

/// One fleet member (ESI `GET /fleets/{id}/members/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetMember {
    pub character_id: i64,
    #[serde(default)]
    pub ship_type_id: i64,
    #[serde(default)]
    pub solar_system_id: i64,
    #[serde(default)]
    pub role_name: String,
    #[serde(default)]
    pub takes_fleet_warp: bool,
}

/// Authenticated fleet reads over the cache-first ESI client.
#[derive(Clone)]
pub struct FleetClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl FleetClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The fleet the character is currently in (errors with 404 if not in one).
    pub async fn current(&self, character_id: i64) -> Result<FleetInfo> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/fleet/");
        self.esi.get_auth_json::<FleetInfo>(&path, &token).await
    }

    /// The members of a fleet (the requesting character must be in it).
    pub async fn members(&self, character_id: i64, fleet_id: i64) -> Result<Vec<FleetMember>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/fleets/{fleet_id}/members/");
        self.esi.get_auth_json::<Vec<FleetMember>>(&path, &token).await
    }
}
