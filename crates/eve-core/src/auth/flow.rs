//! High-level login orchestration tying together PKCE, the SSO token exchange,
//! and JWT claim parsing.
//!
//! The desktop shell drives this:
//! 1. [`LoginManager::begin`] → open the returned URL in the system browser.
//! 2. The loopback redirect delivers `?code=...&state=...`.
//! 3. [`LoginManager::complete`] verifies `state` (CSRF), exchanges the code,
//!    decodes the character identity, and returns a [`CompletedLogin`].

use std::collections::HashMap;
use std::sync::Mutex;

use url::Url;

use crate::error::{Error, Result};
use crate::model::Character;

use super::pkce::PkcePair;
use super::sso::SsoClient;
use super::token::decode_claims;

/// The result of a successful login.
#[derive(Debug, Clone)]
pub struct CompletedLogin {
    pub character: Character,
    /// Freshly-issued access token, used to prime the in-memory token cache so
    /// the first poll doesn't trigger an immediate refresh. Short-lived; never
    /// persisted to disk.
    pub access_token: String,
    /// Long-lived secret — store ONLY in the OS keychain.
    pub refresh_token: String,
    /// Access-token lifetime in seconds.
    pub expires_in: i64,
}

/// Tracks in-flight authorization attempts keyed by their `state` value.
#[derive(Default)]
pub struct LoginManager {
    pending: Mutex<HashMap<String, PkcePair>>,
}

impl LoginManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a login: generate PKCE, remember it by `state`, and return the
    /// authorize URL to open in the browser.
    pub fn begin(&self, sso: &SsoClient, scopes: &[&str]) -> Result<Url> {
        let pkce = PkcePair::generate();
        let url = sso.authorize_url(&pkce, scopes)?;
        self.pending
            .lock()
            .map_err(|_| Error::other("login manager poisoned"))?
            .insert(pkce.state.clone(), pkce);
        Ok(url)
    }

    /// Number of in-flight logins (for diagnostics/tests).
    pub fn pending_count(&self) -> usize {
        self.pending.lock().map(|p| p.len()).unwrap_or(0)
    }

    /// Take the PKCE pair for a `state`, removing it. Errors if the state is
    /// unknown — this is the CSRF check.
    fn take_pending(&self, state: &str) -> Result<PkcePair> {
        self.pending
            .lock()
            .map_err(|_| Error::other("login manager poisoned"))?
            .remove(state)
            .ok_or_else(|| Error::Auth("unknown or replayed state (possible CSRF)".into()))
    }

    /// Complete a login from the redirect's `code` + `state`.
    pub async fn complete(&self, sso: &SsoClient, state: &str, code: &str) -> Result<CompletedLogin> {
        let pkce = self.take_pending(state)?;
        let tokens = sso.exchange_code(code, &pkce.verifier).await?;
        let claims = decode_claims(&tokens.access_token)?;

        let mut character = Character::new(claims.character_id()?, claims.name.clone());
        character.scopes = claims.scopes();
        character.active = false;

        Ok(CompletedLogin {
            character,
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_in: tokens.expires_in,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sso() -> SsoClient {
        SsoClient::new(reqwest::Client::new(), "abc123", "http://localhost:8787/callback")
    }

    #[test]
    fn begin_registers_state() {
        let mgr = LoginManager::new();
        let url = mgr.begin(&sso(), &["publicData"]).unwrap();
        // The state in the URL must be the one we remembered.
        let state = url
            .query_pairs()
            .find(|(k, _)| k == "state")
            .map(|(_, v)| v.into_owned())
            .unwrap();
        assert_eq!(mgr.pending_count(), 1);
        // Completing with a *different* state is rejected as CSRF.
        assert!(mgr.take_pending("not-the-state").is_err());
        // The real state resolves.
        assert!(mgr.take_pending(&state).is_ok());
        assert_eq!(mgr.pending_count(), 0);
    }

    #[tokio::test]
    async fn complete_with_unknown_state_is_csrf_error() {
        let mgr = LoginManager::new();
        let err = mgr.complete(&sso(), "bogus", "code123").await.unwrap_err();
        assert!(matches!(err, Error::Auth(_)));
    }
}
