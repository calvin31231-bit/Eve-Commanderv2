//! Live killfeed via zKillboard's RedisQ (push, not poll).
//!
//! RedisQ (`https://redisq.zkillboard.com/listen.php`) is a long-poll queue:
//! each request returns one killmail package (or `{"package":null}` after the
//! wait window). One background task draining it gives the Situational
//! Awareness rail a near-real-time killfeed without hammering ESI — the plan's
//! "push, not poll, where a stream exists". Parsing is pure and unit-tested;
//! the long-poll loop lives in the shell.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::Result;

/// One live kill, reduced to what the rail shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveKill {
    pub killmail_id: i64,
    pub solar_system_id: i64,
    pub ship_type_id: i64,
    pub total_value: f64,
    /// Kill time as epoch seconds.
    pub time_epoch: i64,
}

/// Parse one RedisQ response body into a kill (None for the `package: null`
/// keep-alive or an unrecognized shape). Pure.
pub fn parse_package(body: &serde_json::Value) -> Option<LiveKill> {
    let pkg = body.get("package")?;
    if pkg.is_null() {
        return None;
    }
    let km = pkg.get("killmail")?;
    let time_epoch = km
        .get("killmail_time")
        .and_then(|t| t.as_str())
        .and_then(|t| OffsetDateTime::parse(t, &Rfc3339).ok())
        .map(|t| t.unix_timestamp())
        .unwrap_or(0);
    Some(LiveKill {
        killmail_id: km.get("killmail_id").and_then(|v| v.as_i64())?,
        solar_system_id: km.get("solar_system_id").and_then(|v| v.as_i64()).unwrap_or(0),
        ship_type_id: km
            .get("victim")
            .and_then(|v| v.get("ship_type_id"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        total_value: pkg
            .get("zkb")
            .and_then(|z| z.get("totalValue"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        time_epoch,
    })
}

/// One RedisQ long-poll: returns the next kill, or None on the keep-alive.
/// `queue_id` must be stable per install so zKill can track the cursor.
pub async fn poll_once(
    http: &reqwest::Client,
    user_agent: &str,
    queue_id: &str,
) -> Result<Option<LiveKill>> {
    let url = format!("https://redisq.zkillboard.com/listen.php?queueID={queue_id}&ttw=10");
    let resp = http
        .get(&url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| crate::error::Error::other(format!("redisq: {e}")))?;
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| crate::error::Error::other(format!("redisq parse: {e}")))?;
    Ok(parse_package(&body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_a_kill_package() {
        let body = json!({ "package": {
            "killID": 1,
            "killmail": {
                "killmail_id": 129000001,
                "killmail_time": "2026-07-02T12:00:00Z",
                "solar_system_id": 30000142,
                "victim": { "ship_type_id": 587 }
            },
            "zkb": { "totalValue": 12_345_678.9 }
        }});
        let k = parse_package(&body).unwrap();
        assert_eq!(k.killmail_id, 129000001);
        assert_eq!(k.solar_system_id, 30000142);
        assert_eq!(k.ship_type_id, 587);
        assert!(k.total_value > 12_000_000.0);
        assert!(k.time_epoch > 0);
    }

    #[test]
    fn keepalive_and_garbage_yield_none() {
        assert!(parse_package(&json!({ "package": null })).is_none());
        assert!(parse_package(&json!({})).is_none());
        assert!(parse_package(&json!({ "package": { "no": "killmail" } })).is_none());
    }
}
