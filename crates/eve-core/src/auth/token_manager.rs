//! Access-token management: a per-character in-memory cache that hands out a
//! valid bearer token on demand, transparently refreshing via the refresh-token
//! grant when one is missing or about to expire.
//!
//! EVE SSO **rotates** refresh tokens: every successful refresh returns a new
//! refresh token and invalidates the old one, so we must persist the rotated
//! value back to the [`TokenStore`] or the next launch can't log in. The
//! caching and rotation *decisions* are factored into pure functions
//! ([`TokenCache`] and [`plan_refresh`]) that are fully unit-tested; the async
//! method is a thin orchestration over the network call.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use crate::error::{Error, Result};

use super::sso::{SsoClient, TokenResponse};
use super::token_store::TokenStore;

/// Refresh this long *before* the real expiry so an in-flight request never
/// races a token blinking out from under it.
const SAFETY_MARGIN: Duration = Duration::from_secs(60);

/// A cached access token and the instant it stops being usable.
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: SystemTime,
}

/// Thread-safe in-memory cache of access tokens keyed by character id. Cloneable
/// (shares one backing map) so it can be handed to background tasks.
#[derive(Clone, Default)]
pub struct TokenCache {
    inner: Arc<Mutex<HashMap<i64, CachedToken>>>,
}

impl TokenCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the cached access token for `character_id` iff it is still valid at
    /// `now` with the safety margin applied.
    pub fn get_valid(&self, character_id: i64, now: SystemTime) -> Option<String> {
        let guard = self.inner.lock().ok()?;
        let entry = guard.get(&character_id)?;
        // Valid only if it survives the margin window.
        if now + SAFETY_MARGIN < entry.expires_at {
            Some(entry.access_token.clone())
        } else {
            None
        }
    }

    /// Insert / replace the token for `character_id`.
    pub fn insert(&self, character_id: i64, access_token: String, expires_at: SystemTime) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(
                character_id,
                CachedToken {
                    access_token,
                    expires_at,
                },
            );
        }
    }

    /// Drop the cached token (e.g. after a `401`, forcing a refresh next time).
    pub fn invalidate(&self, character_id: i64) {
        if let Ok(mut g) = self.inner.lock() {
            g.remove(&character_id);
        }
    }
}

/// The outcome of interpreting a refresh response: the new access token, when it
/// expires, and — if SSO rotated it — the refresh token to persist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshPlan {
    pub access_token: String,
    pub expires_at: SystemTime,
    /// `Some(new_refresh)` when EVE rotated the refresh token; it MUST be
    /// persisted, replacing the old one.
    pub rotated_refresh: Option<String>,
}

/// Pure interpretation of a token response against the refresh token we sent.
/// Computes expiry and detects refresh-token rotation.
pub fn plan_refresh(old_refresh: &str, resp: &TokenResponse, now: SystemTime) -> RefreshPlan {
    let expires_at = now + Duration::from_secs(resp.expires_in.max(0) as u64);
    let rotated_refresh = if !resp.refresh_token.is_empty() && resp.refresh_token != old_refresh {
        Some(resp.refresh_token.clone())
    } else {
        None
    };
    RefreshPlan {
        access_token: resp.access_token.clone(),
        expires_at,
        rotated_refresh,
    }
}

/// Hands out valid access tokens for characters, refreshing as needed and
/// persisting rotated refresh tokens. Cloneable for use across tasks.
#[derive(Clone)]
pub struct TokenManager {
    sso: SsoClient,
    store: Arc<dyn TokenStore>,
    cache: TokenCache,
}

impl TokenManager {
    pub fn new(sso: SsoClient, store: Arc<dyn TokenStore>) -> Self {
        Self {
            sso,
            store,
            cache: TokenCache::new(),
        }
    }

    /// Prime the cache with a freshly-issued access token (e.g. straight from
    /// login) so the first poll doesn't trigger an immediate refresh.
    pub fn prime(&self, character_id: i64, access_token: String, expires_in: i64) {
        let expires_at = SystemTime::now() + Duration::from_secs(expires_in.max(0) as u64);
        self.cache.insert(character_id, access_token, expires_at);
    }

    /// Forget a character's cached access token.
    pub fn invalidate(&self, character_id: i64) {
        self.cache.invalidate(character_id);
    }

    /// Return a valid access token for `character_id`, refreshing via the stored
    /// refresh token when the cache is cold or stale. Persists a rotated refresh
    /// token if EVE issues one.
    pub async fn access_token(&self, character_id: i64) -> Result<String> {
        if let Some(tok) = self.cache.get_valid(character_id, SystemTime::now()) {
            return Ok(tok);
        }

        let refresh = self
            .store
            .load_refresh_token(character_id)?
            .ok_or(Error::MissingToken(character_id))?;

        let resp = self.sso.refresh(&refresh).await?;
        let plan = plan_refresh(&refresh, &resp, SystemTime::now());

        if let Some(new_refresh) = &plan.rotated_refresh {
            self.store.save_refresh_token(character_id, new_refresh)?;
        }
        self.cache
            .insert(character_id, plan.access_token.clone(), plan.expires_at);
        Ok(plan.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::token_store::MemoryTokenStore;

    fn at(secs_from_now: i64) -> SystemTime {
        if secs_from_now >= 0 {
            SystemTime::now() + Duration::from_secs(secs_from_now as u64)
        } else {
            SystemTime::now() - Duration::from_secs((-secs_from_now) as u64)
        }
    }

    #[test]
    fn cache_returns_token_while_valid() {
        let cache = TokenCache::new();
        cache.insert(1, "tok".into(), at(3600));
        assert_eq!(cache.get_valid(1, SystemTime::now()).as_deref(), Some("tok"));
    }

    #[test]
    fn cache_misses_within_safety_margin() {
        let cache = TokenCache::new();
        // Expires in 30s, but the margin is 60s → treated as already stale.
        cache.insert(1, "tok".into(), at(30));
        assert!(cache.get_valid(1, SystemTime::now()).is_none());
    }

    #[test]
    fn cache_misses_when_expired_or_absent() {
        let cache = TokenCache::new();
        assert!(cache.get_valid(99, SystemTime::now()).is_none());
        cache.insert(1, "tok".into(), at(-10));
        assert!(cache.get_valid(1, SystemTime::now()).is_none());
    }

    #[test]
    fn cache_invalidate_clears() {
        let cache = TokenCache::new();
        cache.insert(1, "tok".into(), at(3600));
        cache.invalidate(1);
        assert!(cache.get_valid(1, SystemTime::now()).is_none());
    }

    fn resp(access: &str, refresh: &str, expires_in: i64) -> TokenResponse {
        TokenResponse {
            access_token: access.into(),
            refresh_token: refresh.into(),
            expires_in,
            token_type: "Bearer".into(),
        }
    }

    #[test]
    fn plan_detects_rotated_refresh_token() {
        let plan = plan_refresh("old-rt", &resp("AT", "new-rt", 1200), SystemTime::now());
        assert_eq!(plan.access_token, "AT");
        assert_eq!(plan.rotated_refresh.as_deref(), Some("new-rt"));
    }

    #[test]
    fn plan_ignores_unchanged_or_empty_refresh_token() {
        // Same value back → not a rotation.
        let same = plan_refresh("rt", &resp("AT", "rt", 1200), SystemTime::now());
        assert_eq!(same.rotated_refresh, None);
        // Empty refresh in response → nothing to persist.
        let empty = plan_refresh("rt", &resp("AT", "", 1200), SystemTime::now());
        assert_eq!(empty.rotated_refresh, None);
    }

    #[test]
    fn plan_computes_future_expiry() {
        let now = SystemTime::now();
        let plan = plan_refresh("rt", &resp("AT", "rt", 1200), now);
        assert!(plan.expires_at > now);
        assert!(plan.expires_at <= now + Duration::from_secs(1200));
    }

    fn sso() -> SsoClient {
        SsoClient::new(reqwest::Client::new(), "abc123", "http://localhost:8787/callback")
    }

    #[tokio::test]
    async fn access_token_serves_primed_value_without_network() {
        let mgr = TokenManager::new(sso(), Arc::new(MemoryTokenStore::default()));
        mgr.prime(1, "primed-AT".into(), 1200);
        // Cache hit → returns immediately, no refresh/network involved.
        assert_eq!(mgr.access_token(1).await.unwrap(), "primed-AT");
    }

    #[tokio::test]
    async fn access_token_without_stored_refresh_is_missing_token_error() {
        let mgr = TokenManager::new(sso(), Arc::new(MemoryTokenStore::default()));
        let err = mgr.access_token(7).await.unwrap_err();
        assert!(matches!(err, Error::MissingToken(7)));
    }
}
