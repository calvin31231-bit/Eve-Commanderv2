//! EVE SSO authorize-URL construction and token exchange/refresh.

use serde::Deserialize;
use url::Url;

use crate::config::{SSO_AUTHORIZE_URL, SSO_TOKEN_URL};
use crate::error::{Error, Result};

use super::pkce::PkcePair;

/// Tokens returned by the SSO token endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: i64,
    #[serde(default)]
    pub token_type: String,
}

/// A thin SSO client bound to one ESI application (client id + redirect).
#[derive(Debug, Clone)]
pub struct SsoClient {
    http: reqwest::Client,
    client_id: String,
    redirect_uri: String,
}

impl SsoClient {
    pub fn new(http: reqwest::Client, client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self {
            http,
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
        }
    }

    /// Build the authorize URL to open in the system browser.
    ///
    /// `scopes` is the set of ESI scopes being requested (space-delimited per
    /// the spec). We request scopes **incrementally per feature**, never all at
    /// once.
    pub fn authorize_url(&self, pkce: &PkcePair, scopes: &[&str]) -> Result<Url> {
        let mut url = Url::parse(SSO_AUTHORIZE_URL)?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("client_id", &self.client_id)
            .append_pair("scope", &scopes.join(" "))
            .append_pair("state", &pkce.state)
            .append_pair("code_challenge", &pkce.challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url)
    }

    /// Exchange an authorization `code` (+ PKCE verifier) for tokens.
    pub async fn exchange_code(&self, code: &str, verifier: &str) -> Result<TokenResponse> {
        let params = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &self.client_id),
            ("code_verifier", verifier),
        ];
        self.post_token(&params).await
    }

    /// Use a refresh token to obtain a fresh access token.
    pub async fn refresh(&self, refresh_token: &str) -> Result<TokenResponse> {
        let params = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", &self.client_id),
        ];
        self.post_token(&params).await
    }

    async fn post_token(&self, params: &[(&str, &str)]) -> Result<TokenResponse> {
        let resp = self
            .http
            .post(SSO_TOKEN_URL)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(params)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Auth(format!("token endpoint returned {status}: {body}")));
        }
        Ok(resp.json::<TokenResponse>().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> SsoClient {
        SsoClient::new(reqwest::Client::new(), "abc123", "http://localhost:8787/callback")
    }

    #[test]
    fn authorize_url_has_pkce_and_scopes() {
        let pkce = PkcePair::generate();
        let url = client()
            .authorize_url(&pkce, &["publicData", "esi-skills.read_skills.v1"])
            .unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q.get("response_type").unwrap(), "code");
        assert_eq!(q.get("code_challenge_method").unwrap(), "S256");
        assert_eq!(q.get("client_id").unwrap(), "abc123");
        assert_eq!(q.get("code_challenge").unwrap(), &pkce.challenge);
        assert_eq!(q.get("state").unwrap(), &pkce.state);
        assert_eq!(q.get("scope").unwrap(), "publicData esi-skills.read_skills.v1");
    }
}
