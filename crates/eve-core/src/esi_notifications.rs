//! ESI character notifications → alerts + auto-timers.
//!
//! `GET /characters/{id}/notifications/` (scope
//! `esi-characters.read_notifications.v1`) delivers the game's own pings:
//! structure attacks, reinforcements, fuel warnings, war declarations. We
//! classify the types we understand into alert severities, and parse the
//! `timeLeft` field (Windows 100-ns ticks) out of reinforcement notifications
//! so the timerboard can auto-populate. Classification/parsing are pure and
//! unit-tested; the fetch rides the cache-first ESI client.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::auth::TokenManager;
use crate::error::Result;
use crate::esi::EsiClient;
use crate::notify::Severity;

/// One raw ESI notification (only the fields we read).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EsiNotification {
    pub notification_id: i64,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub timestamp: String,
    /// YAML-ish body ("key: value" lines) — where `timeLeft` etc. live.
    #[serde(default)]
    pub text: String,
}

/// How an understood notification type maps into the alert rail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Classified {
    pub severity: Severity,
    pub title: &'static str,
    /// True when the body carries a `timeLeft` and should create a timer.
    pub makes_timer: bool,
}

/// Classify a notification type. `None` = a type we don't surface. Pure.
pub fn classify(kind: &str) -> Option<Classified> {
    let c = |severity, title, makes_timer| Some(Classified { severity, title, makes_timer });
    match kind {
        "StructureUnderAttack" => c(Severity::Critical, "Structure under attack", false),
        "StructureLostShields" => c(Severity::Critical, "Structure shields lost — reinforced", true),
        "StructureLostArmor" => c(Severity::Critical, "Structure armor lost — reinforced", true),
        "SovStructureReinforced" => c(Severity::Warning, "Sov structure reinforced", false),
        "StructureFuelAlert" => c(Severity::Warning, "Structure fuel low (in-game alert)", false),
        "CorpWarDeclaredMsg" | "AllWarDeclaredMsg" => {
            c(Severity::Warning, "War declared", false)
        }
        "StructureDestroyed" => c(Severity::Critical, "Structure destroyed", false),
        "TowerAlertMsg" => c(Severity::Warning, "POS under attack", false),
        _ => None,
    }
}

/// Parse the `timeLeft` line (Windows 100-ns ticks) from a notification body
/// into whole seconds. Pure.
pub fn parse_time_left_secs(text: &str) -> Option<i64> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("timeLeft:") {
            let ticks: i64 = v.trim().parse().ok()?;
            return Some(ticks / 10_000_000);
        }
    }
    None
}

/// When a reinforcement exits: notification timestamp + `timeLeft`. Pure.
pub fn reinforcement_exit(timestamp: &str, text: &str) -> Option<OffsetDateTime> {
    let ts = OffsetDateTime::parse(timestamp, &Rfc3339).ok()?;
    let secs = parse_time_left_secs(text)?;
    Some(ts + time::Duration::seconds(secs))
}

/// A notification's timestamp as epoch seconds (None if unparseable). Pure.
pub fn timestamp_epoch(timestamp: &str) -> Option<i64> {
    OffsetDateTime::parse(timestamp, &Rfc3339).ok().map(|t| t.unix_timestamp())
}

/// Authenticated notification reads over the cache-first ESI client.
#[derive(Clone)]
pub struct NotificationsClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl NotificationsClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's recent in-game notifications (ESI keeps ~a few months).
    pub async fn notifications(&self, character_id: i64) -> Result<Vec<EsiNotification>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/notifications/");
        self.esi.get_auth_json::<Vec<EsiNotification>>(&path, &token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_types_and_skips_noise() {
        let attack = classify("StructureUnderAttack").unwrap();
        assert_eq!(attack.severity, Severity::Critical);
        assert!(!attack.makes_timer);

        let shields = classify("StructureLostShields").unwrap();
        assert!(shields.makes_timer);

        assert!(classify("InsurancePayoutMsg").is_none());
    }

    #[test]
    fn parses_time_left_ticks_and_exit_time() {
        // 1_080_000_000_000 ticks = 108_000 s = 30 h.
        let text = "solarsystemID: 30000142\ntimeLeft: 1080000000000\nvulnerableTime: 9000000000\n";
        assert_eq!(parse_time_left_secs(text), Some(108_000));

        let exit = reinforcement_exit("2026-07-01T12:00:00Z", text).unwrap();
        let expect = OffsetDateTime::parse("2026-07-02T18:00:00Z", &Rfc3339).unwrap();
        assert_eq!(exit, expect);

        // Missing field / bad timestamp → None, never a panic.
        assert_eq!(parse_time_left_secs("foo: bar"), None);
        assert!(reinforcement_exit("not a time", text).is_none());
    }
}
