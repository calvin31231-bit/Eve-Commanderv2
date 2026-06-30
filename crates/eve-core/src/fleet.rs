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

/// A squad within a wing (ESI `GET /fleets/{id}/wings/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetSquad {
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

/// A wing with its squads (ESI `GET /fleets/{id}/wings/`) — the move targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetWing {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub squads: Vec<FleetSquad>,
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

    /// The fleet's wings + squads (the move-target hierarchy). Boss/member read.
    pub async fn wings(&self, character_id: i64, fleet_id: i64) -> Result<Vec<FleetWing>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/fleets/{fleet_id}/wings/");
        self.esi.get_auth_json::<Vec<FleetWing>>(&path, &token).await
    }

    /// Update the fleet's MOTD and free-move flag (the requesting character must
    /// be the fleet boss). The one ESI-sanctioned fleet write — not input
    /// automation. Requires the `esi-fleets.write_fleet.v1` scope.
    pub async fn set_settings(
        &self,
        character_id: i64,
        fleet_id: i64,
        motd: &str,
        is_free_move: bool,
    ) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/fleets/{fleet_id}/");
        let body = serde_json::json!({ "motd": motd, "is_free_move": is_free_move });
        self.esi.put_auth_empty(&path, &body, &token).await
    }

    /// Kick a member from the fleet (the requesting character must be the boss).
    /// `DELETE /fleets/{id}/members/{member_id}/`. EULA-sanctioned fleet write.
    pub async fn kick_member(
        &self,
        character_id: i64,
        fleet_id: i64,
        member_id: i64,
    ) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/fleets/{fleet_id}/members/{member_id}/");
        self.esi.delete_auth_empty(&path, &token).await
    }

    /// Move a member to a new role/position in the fleet (boss only).
    /// `PUT /fleets/{id}/members/{member_id}/`. `role` is one of
    /// `fleet_commander`, `wing_commander`, `squad_commander`, `squad_member`;
    /// `wing_id`/`squad_id` target the destination (required for squad roles).
    pub async fn move_member(
        &self,
        character_id: i64,
        fleet_id: i64,
        member_id: i64,
        role: &str,
        wing_id: Option<i64>,
        squad_id: Option<i64>,
    ) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/fleets/{fleet_id}/members/{member_id}/");
        let mut body = serde_json::json!({ "role": role });
        if let Some(w) = wing_id {
            body["wing_id"] = serde_json::json!(w);
        }
        if let Some(s) = squad_id {
            body["squad_id"] = serde_json::json!(s);
        }
        self.esi.put_auth_empty(&path, &body, &token).await
    }
}
