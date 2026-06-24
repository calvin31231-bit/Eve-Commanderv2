//! Calendar: upcoming in-game events.
//!
//! A thin authenticated read of `GET /characters/{id}/calendar/` (the next ~50
//! events from now). Mostly passthrough; the value is the upcoming list with
//! client-side countdowns. The fetch is exercised live.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One calendar event summary (ESI `GET /characters/{id}/calendar/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub event_id: i64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub event_date: String,
    /// "accepted" | "declined" | "tentative" | "not_responded".
    #[serde(default)]
    pub event_response: String,
    #[serde(default)]
    pub importance: i64,
}

/// Authenticated calendar reads over the cache-first ESI client.
#[derive(Clone)]
pub struct CalendarClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl CalendarClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's upcoming calendar events, soonest first.
    pub async fn events(&self, character_id: i64) -> Result<Vec<CalendarEvent>> {
        let ep = endpoint("calendar").ok_or_else(|| Error::other("unknown endpoint 'calendar'"))?;
        let token = self.tokens.access_token(character_id).await?;
        let mut events = self
            .esi
            .get_auth_json::<Vec<CalendarEvent>>(&ep.path_for(character_id), &token)
            .await?;
        events.sort_by(|a, b| a.event_date.cmp(&b.event_date));
        Ok(events)
    }
}
