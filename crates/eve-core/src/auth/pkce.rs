//! PKCE (RFC 7636) helpers for the EVE SSO authorization-code flow.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// A PKCE verifier/challenge pair plus a CSRF `state` value.
#[derive(Debug, Clone)]
pub struct PkcePair {
    /// High-entropy secret kept by the client and sent at token exchange.
    pub verifier: String,
    /// `BASE64URL(SHA256(verifier))`, sent on the authorize request.
    pub challenge: String,
    /// Opaque CSRF token echoed back on the redirect and verified.
    pub state: String,
}

/// Generate `n` random bytes and return them base64url-encoded (no padding).
fn random_b64url(n: usize) -> String {
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    URL_SAFE_NO_PAD.encode(buf)
}

impl PkcePair {
    /// Generate a fresh PKCE pair using the S256 challenge method.
    pub fn generate() -> Self {
        // 32 random bytes -> 43-char verifier, comfortably within the
        // RFC 7636 43..=128 character range.
        let verifier = random_b64url(32);
        let challenge = Self::challenge_for(&verifier);
        let state = random_b64url(24);
        Self {
            verifier,
            challenge,
            state,
        }
    }

    /// Compute the S256 code challenge for a given verifier.
    pub fn challenge_for(verifier: &str) -> String {
        let digest = Sha256::digest(verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_matches_spec() {
        // Test vector from RFC 7636, Appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = PkcePair::challenge_for(verifier);
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn generated_pairs_are_unique_and_valid_length() {
        let a = PkcePair::generate();
        let b = PkcePair::generate();
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.state, b.state);
        // verifier must be 43..=128 chars per RFC 7636.
        assert!((43..=128).contains(&a.verifier.len()));
        // S256 challenge is always 43 chars (256-bit digest, base64url no pad).
        assert_eq!(a.challenge.len(), 43);
    }
}
