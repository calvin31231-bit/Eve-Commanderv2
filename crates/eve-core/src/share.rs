//! Community sharing (Phase 7): compact, copy-pasteable codes for fits and skill
//! plans — sharing without a backend.
//!
//! A shared artifact (an EFT fit or a skill-plan body, both already plain text)
//! is wrapped with its kind + name and base64url-encoded behind a versioned
//! prefix so it round-trips through a Discord message or forum post and imports
//! straight into another player's local library. Pure + unit-tested; the EULA
//! line is unchanged (text in, text out — no automation, no service).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Prefix marking an EVE Commander share code (and its format version).
pub const SHARE_PREFIX: &str = "EVECMDR1:";

/// What a share code carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedArtifact {
    /// `"fit"` or `"plan"`.
    pub kind: String,
    pub name: String,
    pub body: String,
}

/// Encode an artifact into a single shareable code string. Pure.
pub fn encode(artifact: &SharedArtifact) -> String {
    let json = serde_json::to_vec(artifact).unwrap_or_default();
    format!("{SHARE_PREFIX}{}", URL_SAFE_NO_PAD.encode(json))
}

/// Decode a share code back into an artifact, validating the prefix. Pure.
pub fn decode(code: &str) -> Result<SharedArtifact> {
    let body = code
        .trim()
        .strip_prefix(SHARE_PREFIX)
        .ok_or_else(|| Error::other("not an EVE Commander share code"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(body.trim())
        .map_err(|e| Error::other(format!("bad share code: {e}")))?;
    let artifact: SharedArtifact =
        serde_json::from_slice(&bytes).map_err(|e| Error::other(format!("bad share payload: {e}")))?;
    if artifact.kind != "fit" && artifact.kind != "plan" {
        return Err(Error::other(format!("unknown share kind: {}", artifact.kind)));
    }
    Ok(artifact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_code_roundtrips() {
        let a = SharedArtifact {
            kind: "fit".into(),
            name: "Shield Rifter".into(),
            body: "[Rifter, Shield Rifter]\n200mm AutoCannon II\n".into(),
        };
        let code = encode(&a);
        assert!(code.starts_with(SHARE_PREFIX));
        assert_eq!(decode(&code).unwrap(), a);
    }

    #[test]
    fn plan_code_roundtrips_and_tolerates_whitespace() {
        let a = SharedArtifact { kind: "plan".into(), name: "Caps".into(), body: "Capital Ships 5".into() };
        let code = format!("  {}\n", encode(&a)); // pasted with stray whitespace
        assert_eq!(decode(&code).unwrap(), a);
    }

    #[test]
    fn rejects_foreign_or_corrupt_codes() {
        assert!(decode("just some text").is_err());
        assert!(decode("EVECMDR1:!!!notbase64!!!").is_err());
        // Valid base64 + prefix but an unknown kind is rejected.
        let bad = encode(&SharedArtifact { kind: "virus".into(), name: "x".into(), body: "y".into() });
        assert!(decode(&bad).is_err());
    }
}
