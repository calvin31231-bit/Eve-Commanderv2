//! Navigation: route planning + the in-game UI bridge.
//!
//! Routing is computed by ESI's own solver (`GET /route/{origin}/{destination}/`),
//! so we don't need the full universe graph — we ask for shortest / secure /
//! insecure and resolve the hops. The **UI bridge** is the one EULA-sanctioned
//! write path to the client: set an autopilot waypoint and open in-game windows
//! (`POST /ui/...`). No input automation — these are first-party ESI actions the
//! player explicitly triggers.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::Result;
use crate::esi::EsiClient;

/// Routing preference passed to ESI's route solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteFlag {
    Shortest,
    Secure,
    Insecure,
}

impl RouteFlag {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteFlag::Shortest => "shortest",
            RouteFlag::Secure => "secure",
            RouteFlag::Insecure => "insecure",
        }
    }

    pub fn parse(s: &str) -> RouteFlag {
        match s {
            "secure" => RouteFlag::Secure,
            "insecure" => RouteFlag::Insecure,
            _ => RouteFlag::Shortest,
        }
    }
}

/// Public route solving + autopilot/UI writes over the ESI client.
#[derive(Clone)]
pub struct NavigationClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl NavigationClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// Solve a route between two systems (by id) with a preference flag,
    /// returning the ordered system ids (origin → destination, inclusive).
    /// Server-computed, public, cache-friendly.
    pub async fn route(&self, origin: i64, destination: i64, flag: RouteFlag) -> Result<Vec<i64>> {
        let path = format!(
            "/latest/route/{origin}/{destination}/?flag={}",
            flag.as_str()
        );
        self.esi.get_public_json::<Vec<i64>>(&path).await
    }

    /// Set the character's autopilot waypoint to `destination_id` (a system,
    /// station, or structure). Clears existing waypoints by default. Requires
    /// `esi-ui.write_waypoint.v1`.
    pub async fn set_waypoint(
        &self,
        character_id: i64,
        destination_id: i64,
        add_to_beginning: bool,
        clear_other: bool,
    ) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!(
            "/latest/ui/autopilot/waypoint/?add_to_beginning={}&clear_other_waypoints={}&destination_id={}",
            add_to_beginning, clear_other, destination_id
        );
        self.esi.post_auth_empty(&path, &token).await
    }

    /// Open the in-game market details window for a type. Requires
    /// `esi-ui.open_window.v1`.
    pub async fn open_market(&self, character_id: i64, type_id: i64) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/ui/openwindow/marketdetails/?type_id={type_id}");
        self.esi.post_auth_empty(&path, &token).await
    }

    /// Open the in-game "show info" window for any entity (type/character/corp/…).
    pub async fn open_info(&self, character_id: i64, target_id: i64) -> Result<()> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/ui/openwindow/information/?target_id={target_id}");
        self.esi.post_auth_empty(&path, &token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_round_trips() {
        for f in [RouteFlag::Shortest, RouteFlag::Secure, RouteFlag::Insecure] {
            assert_eq!(RouteFlag::parse(f.as_str()), f);
        }
        // Unknown falls back to shortest.
        assert_eq!(RouteFlag::parse("nonsense"), RouteFlag::Shortest);
    }
}
