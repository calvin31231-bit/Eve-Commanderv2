//! EVE-Scout: public Thera / Turnur wormhole connections.
//!
//! EVE-Scout maintains scanned wormhole connections from the Thera and Turnur
//! hubs into known space (`api.eve-scout.com`). This is a read-only third-party
//! enrichment — handy for explorers/haulers looking for a shortcut. Cached and
//! degrades gracefully (the app never hard-depends on it). Exercised live.

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// One scanned EVE-Scout signature (a Thera/Turnur ↔ k-space wormhole).
#[derive(Debug, Clone, Deserialize)]
pub struct TheraSignature {
    #[serde(default)]
    pub signature_type: String,
    #[serde(default)]
    pub wh_type: Option<String>,
    #[serde(default)]
    pub max_ship_size: Option<String>,
    #[serde(default)]
    pub remaining_hours: Option<i64>,
    /// The k-space side of the connection.
    #[serde(default)]
    pub in_system_name: Option<String>,
    #[serde(default)]
    pub in_region_name: Option<String>,
    /// The hub side (Thera / Turnur).
    #[serde(default)]
    pub out_system_name: Option<String>,
}

/// A wormhole connection flattened for the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TheraConnection {
    pub hub: String,
    pub destination: String,
    pub region: String,
    pub wh_type: String,
    pub max_ship_size: String,
    pub remaining_hours: i64,
}

/// Reduce raw signatures to wormhole connections with a k-space destination,
/// soonest-to-collapse first. Pure.
pub fn connections(sigs: Vec<TheraSignature>) -> Vec<TheraConnection> {
    let mut out: Vec<TheraConnection> = sigs
        .into_iter()
        .filter(|s| s.signature_type == "wormhole")
        .filter_map(|s| {
            let destination = s.in_system_name?;
            Some(TheraConnection {
                hub: s.out_system_name.unwrap_or_else(|| "Thera".to_string()),
                destination,
                region: s.in_region_name.unwrap_or_default(),
                wh_type: s.wh_type.unwrap_or_default(),
                max_ship_size: s.max_ship_size.unwrap_or_default(),
                remaining_hours: s.remaining_hours.unwrap_or(0),
            })
        })
        .collect();
    out.sort_by_key(|c| c.remaining_hours);
    out
}

/// Reads EVE-Scout public connections.
#[derive(Clone)]
pub struct EveScoutClient {
    http: reqwest::Client,
    user_agent: String,
}

impl EveScoutClient {
    pub fn new(user_agent: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), user_agent: user_agent.into() }
    }

    /// Current Thera/Turnur wormhole connections.
    pub async fn connections(&self) -> Result<Vec<TheraConnection>> {
        let resp = self
            .http
            .get("https://api.eve-scout.com/v2/public/signatures")
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| Error::other(format!("eve-scout request: {e}")))?;
        let sigs = resp
            .json::<Vec<TheraSignature>>()
            .await
            .map_err(|e| Error::other(format!("eve-scout decode: {e}")))?;
        Ok(connections(sigs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(ty: &str, dest: Option<&str>, hours: i64) -> TheraSignature {
        TheraSignature {
            signature_type: ty.to_string(),
            wh_type: Some("Q063".into()),
            max_ship_size: Some("medium".into()),
            remaining_hours: Some(hours),
            in_system_name: dest.map(str::to_string),
            in_region_name: Some("The Forge".into()),
            out_system_name: Some("Thera".into()),
        }
    }

    #[test]
    fn filters_wormholes_and_sorts_by_expiry() {
        let sigs = vec![
            sig("wormhole", Some("Jita"), 10),
            sig("wormhole", Some("Amarr"), 2),
            sig("data", Some("X"), 1),       // not a wormhole → dropped
            sig("wormhole", None, 5),         // no destination → dropped
        ];
        let conns = connections(sigs);
        assert_eq!(conns.len(), 2);
        assert_eq!(conns[0].destination, "Amarr"); // soonest first
        assert_eq!(conns[0].hub, "Thera");
    }
}
