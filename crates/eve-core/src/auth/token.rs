//! Parsing of the EVE SSO access token (a JWT) to extract the character
//! identity and granted scopes.
//!
//! NOTE: Phase 0 decodes the claims without verifying the RS256 signature.
//! Production must verify against the SSO JWKS
//! (`https://login.eveonline.com/oauth/jwks`) and validate `iss`/`aud`/`exp`.
//! That requires a network fetch of the keyset and is tracked as follow-up.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::Deserialize;

use crate::error::{Error, Result};

/// The subset of SSO JWT claims we use.
#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    /// e.g. `"CHARACTER:EVE:2112000000"`.
    pub sub: String,
    /// Character name.
    #[serde(default)]
    pub name: String,
    /// Granted scopes — the SSO encodes this as a string for a single scope or
    /// an array for many. Normalized via [`Claims::scopes`].
    #[serde(default)]
    scp: ScopeField,
    /// Expiry (seconds since epoch).
    #[serde(default)]
    pub exp: i64,
}

/// `scp` is either a single string or an array of strings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(untagged)]
enum ScopeField {
    One(String),
    Many(Vec<String>),
    #[default]
    None,
}

impl Claims {
    /// The numeric character id parsed from `sub` (`CHARACTER:EVE:<id>`).
    pub fn character_id(&self) -> Result<i64> {
        self.sub
            .rsplit(':')
            .next()
            .and_then(|s| s.parse::<i64>().ok())
            .ok_or_else(|| Error::Auth(format!("unexpected sub claim: {}", self.sub)))
    }

    /// Granted scopes as a vec.
    pub fn scopes(&self) -> Vec<String> {
        match &self.scp {
            ScopeField::One(s) => vec![s.clone()],
            ScopeField::Many(v) => v.clone(),
            ScopeField::None => Vec::new(),
        }
    }
}

/// Decode (without verifying) the claims of a JWT access token.
pub fn decode_claims(jwt: &str) -> Result<Claims> {
    let mut parts = jwt.split('.');
    let _header = parts.next().ok_or_else(|| Error::Auth("malformed JWT".into()))?;
    let payload = parts.next().ok_or_else(|| Error::Auth("malformed JWT".into()))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| Error::Auth(format!("bad JWT payload base64: {e}")))?;
    Ok(serde_json::from_slice::<Claims>(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    /// Build a fake unsigned JWT with the given JSON payload.
    fn fake_jwt(payload: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#);
        let body = URL_SAFE_NO_PAD.encode(payload.as_bytes());
        format!("{header}.{body}.sig")
    }

    #[test]
    fn parses_character_and_array_scopes() {
        let jwt = fake_jwt(
            r#"{"sub":"CHARACTER:EVE:2112000000","name":"Test Pilot","scp":["publicData","esi-skills.read_skills.v1"],"exp":1893456000}"#,
        );
        let claims = decode_claims(&jwt).unwrap();
        assert_eq!(claims.character_id().unwrap(), 2112000000);
        assert_eq!(claims.name, "Test Pilot");
        assert_eq!(claims.scopes().len(), 2);
        assert_eq!(claims.exp, 1893456000);
    }

    #[test]
    fn parses_single_string_scope() {
        let jwt = fake_jwt(r#"{"sub":"CHARACTER:EVE:42","scp":"publicData"}"#);
        let claims = decode_claims(&jwt).unwrap();
        assert_eq!(claims.character_id().unwrap(), 42);
        assert_eq!(claims.scopes(), vec!["publicData".to_string()]);
    }

    #[test]
    fn rejects_malformed() {
        assert!(decode_claims("not-a-jwt").is_err());
    }
}
