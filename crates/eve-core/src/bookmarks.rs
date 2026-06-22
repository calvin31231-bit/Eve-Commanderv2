//! Personal bookmarks (nav / exploration / safespots).
//!
//! A thin authenticated read of `GET /characters/{id}/bookmarks/` (paginated).
//! Mostly passthrough — the value is surfacing the player's bookmarks with their
//! locations resolved. The fetch is exercised live.

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::Result;
use crate::esi::EsiClient;

/// One personal bookmark (ESI `GET /characters/{id}/bookmarks/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub bookmark_id: i64,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub location_id: i64,
    #[serde(default)]
    pub created: String,
}

/// Authenticated bookmark reads over the cache-first ESI client.
#[derive(Clone)]
pub struct BookmarksClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl BookmarksClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's personal bookmarks (all pages), newest first.
    pub async fn bookmarks(&self, character_id: i64) -> Result<Vec<Bookmark>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/bookmarks/");
        let mut out = self
            .esi
            .get_auth_json_paged::<Bookmark>(&path, &token)
            .await?;
        out.sort_by(|a, b| b.created.cmp(&a.created));
        Ok(out)
    }
}
