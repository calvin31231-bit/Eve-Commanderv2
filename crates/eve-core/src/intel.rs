//! Local intel: per-pilot threat scoring from killboard stats.
//!
//! The flagship safety feature (the PySpy / Pirate's Little Helper pattern):
//! paste the names in Local, and for each pilot fuse zKillboard activity with
//! ESI security status into a Safe / Neutral / Caution / Danger flag. The
//! **score is deterministic and computed here in Rust** ([`score_pilot`], pure +
//! unit-tested); the AI layer (later) only narrates it. The zKill fetch lives in
//! [`ZkillClient`] and is exercised against the live API on the user's machine.
//!
//! EULA note: this is read-only third-party + ESI data. There is no game-memory
//! reading and no "who is in Local" endpoint (none exists) — the pilot pastes
//! the names, exactly as PySpy does.

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Killboard + ESI inputs for one pilot. Defaults are "no data" so a pilot with
/// an empty killboard scores Safe rather than erroring.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PilotStats {
    /// zKill danger ratio 0–100 (share of involvements that were kills).
    pub danger_ratio: i64,
    /// zKill gang ratio 0–100 (high = fights in gangs, low = solo).
    pub gang_ratio: i64,
    pub ships_destroyed: i64,
    pub ships_lost: i64,
    /// Character security status (−10..+5).
    pub sec_status: f64,
}

/// Threat flag for a pilot, in ascending order of concern (the derived `Ord`
/// matches that order, so `level.max(..)` escalates).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ThreatLevel {
    Safe,
    Neutral,
    Caution,
    Danger,
}

impl ThreatLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            ThreatLevel::Safe => "Safe",
            ThreatLevel::Neutral => "Neutral",
            ThreatLevel::Caution => "Caution",
            ThreatLevel::Danger => "Danger",
        }
    }
}

/// A scored pilot: flag + the reasons behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PilotThreat {
    pub level: ThreatLevel,
    pub reasons: Vec<String>,
}

/// Score a pilot from their killboard stats + sec status. Deterministic and
/// pure — the heuristics are intentionally explained in `reasons`.
pub fn score_pilot(s: &PilotStats) -> PilotThreat {
    let mut reasons = Vec::new();

    // No kill history → not a threat (fresh char, hauler, carebear).
    if s.ships_destroyed == 0 {
        let mut why = vec!["No kills on record".to_string()];
        if s.sec_status <= -5.0 {
            why.push(format!("but flashy sec status {:.1}", s.sec_status));
            return PilotThreat { level: ThreatLevel::Caution, reasons: why };
        }
        return PilotThreat { level: ThreatLevel::Safe, reasons: why };
    }

    let mut level = ThreatLevel::Neutral;

    if s.danger_ratio >= 70 {
        reasons.push(format!("High danger ratio {}%", s.danger_ratio));
        level = level.max(ThreatLevel::Caution);
    } else if s.danger_ratio >= 50 {
        reasons.push(format!("Moderate danger ratio {}%", s.danger_ratio));
    }

    if s.ships_destroyed >= 1000 {
        reasons.push(format!("{} kills — very active", s.ships_destroyed));
        level = level.max(ThreatLevel::Caution);
    } else if s.ships_destroyed >= 100 {
        reasons.push(format!("{} kills", s.ships_destroyed));
    }

    // A prolific killer who also kills frequently → Danger.
    if s.danger_ratio >= 70 && s.ships_destroyed >= 100 {
        level = ThreatLevel::Danger;
    }

    // Solo hunters (low gang ratio + high danger) are an outsized 1v1 risk:
    // at least Caution, and Danger once they have a real kill count.
    if s.gang_ratio > 0 && s.gang_ratio <= 30 && s.danger_ratio >= 60 {
        reasons.push("Solo hunter profile".to_string());
        level = level.max(ThreatLevel::Caution);
        if s.ships_destroyed >= 100 {
            level = ThreatLevel::Danger;
        }
    }

    if s.sec_status <= -5.0 {
        reasons.push(format!("Flashy sec status {:.1}", s.sec_status));
        level = level.max(ThreatLevel::Caution);
    }

    if reasons.is_empty() {
        reasons.push("Some kill history, low recent threat".to_string());
    }
    PilotThreat { level, reasons }
}

/// Summarize a set of scored pilots into a one-line situational callout.
pub fn summarize(levels: &[ThreatLevel]) -> String {
    let danger = levels.iter().filter(|l| **l == ThreatLevel::Danger).count();
    let caution = levels.iter().filter(|l| **l == ThreatLevel::Caution).count();
    if danger > 0 {
        format!("{danger} known hunter(s), {caution} to watch — be careful undocking.")
    } else if caution > 0 {
        format!("{caution} pilot(s) worth watching, no clear hunters.")
    } else {
        "No notable threats among these pilots.".to_string()
    }
}

/// A gate-camp likelihood read for a system, from recent kill volume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GateCampAssessment {
    pub kills_last_hour: i64,
    pub level: ThreatLevel,
    pub message: String,
}

/// Assess gate-camp risk from the number of kills in a system in the last hour.
/// Heuristic and deliberately conservative — recent kills mean *activity*, which
/// near a chokepoint usually means a camp. Pure.
pub fn assess_gatecamp(kills_last_hour: i64) -> GateCampAssessment {
    let (level, message) = if kills_last_hour == 0 {
        (ThreatLevel::Safe, "No kills in the last hour — clear for now.".to_string())
    } else if kills_last_hour <= 3 {
        (
            ThreatLevel::Caution,
            format!("{kills_last_hour} kill(s) in the last hour — some activity, stay alert."),
        )
    } else {
        (
            ThreatLevel::Danger,
            format!("{kills_last_hour} kills in the last hour — likely an active camp."),
        )
    };
    GateCampAssessment { kills_last_hour, level, message }
}

/// A neighbourhood-safety read: total recent kills across a system and its
/// neighbours mapped to a threat flag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyAssessment {
    pub total_kills: i64,
    pub level: ThreatLevel,
    pub message: String,
}

/// Assess how hot a neighbourhood is from the combined kills in a system + its
/// neighbours over the last hour. Pure.
pub fn assess_safety(total_kills: i64) -> SafetyAssessment {
    let (level, message) = if total_kills == 0 {
        (ThreatLevel::Safe, "Quiet — no recent kills nearby.".to_string())
    } else if total_kills <= 5 {
        (
            ThreatLevel::Caution,
            format!("{total_kills} kill(s) in/around your system this hour."),
        )
    } else {
        (
            ThreatLevel::Danger,
            format!("{total_kills} kills in/around your system this hour — hot."),
        )
    };
    SafetyAssessment { total_kills, level, message }
}

// ---- zKillboard client (live; exercised on the user's machine) -------------

/// The subset of zKill's `stats` we score on. zKill nests sec status under
/// `info`; everything defaults to zero/empty so a pilot with no board still
/// deserializes.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ZkillStats {
    #[serde(rename = "dangerRatio", default)]
    pub danger_ratio: i64,
    #[serde(rename = "gangRatio", default)]
    pub gang_ratio: i64,
    #[serde(rename = "shipsDestroyed", default)]
    pub ships_destroyed: i64,
    #[serde(rename = "shipsLost", default)]
    pub ships_lost: i64,
    #[serde(default)]
    pub info: ZkillInfo,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ZkillInfo {
    #[serde(rename = "secStatus", default)]
    pub sec_status: f64,
}

impl ZkillStats {
    /// Project the raw zKill payload onto the pure scorer's inputs.
    pub fn to_pilot_stats(&self) -> PilotStats {
        PilotStats {
            danger_ratio: self.danger_ratio,
            gang_ratio: self.gang_ratio,
            ships_destroyed: self.ships_destroyed,
            ships_lost: self.ships_lost,
            sec_status: self.info.sec_status,
        }
    }
}

/// Reads per-character stats from the public zKillboard API.
#[derive(Clone)]
pub struct ZkillClient {
    http: reqwest::Client,
    user_agent: String,
}

impl ZkillClient {
    pub fn new(user_agent: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), user_agent: user_agent.into() }
    }

    /// Fetch a character's killboard stats. zKill requires a descriptive
    /// User-Agent. A pilot with no killboard yields a zeroed [`ZkillStats`].
    pub async fn character_stats(&self, character_id: i64) -> Result<ZkillStats> {
        let url = format!("https://zkillboard.com/api/stats/characterID/{character_id}/");
        let resp = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error::other(format!("zkill request: {e}")))?;
        // zKill returns `{}` (empty object) for an unknown character → defaults.
        let stats = resp
            .json::<ZkillStats>()
            .await
            .unwrap_or_default();
        Ok(stats)
    }

    /// Count killmails in a system over the last `past_seconds` (zKill caps at
    /// 3600s for the `pastSeconds` filter). Used for gate-camp assessment.
    pub async fn system_kill_count(&self, system_id: i64, past_seconds: i64) -> Result<i64> {
        let url = format!(
            "https://zkillboard.com/api/systemID/{system_id}/pastSeconds/{past_seconds}/"
        );
        let resp = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| crate::error::Error::other(format!("zkill request: {e}")))?;
        // The response is a JSON array of recent killmails; its length is the
        // kill count. An empty/odd body counts as zero.
        let kills = resp
            .json::<Vec<serde_json::Value>>()
            .await
            .map(|v| v.len() as i64)
            .unwrap_or(0);
        Ok(kills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_kills_is_safe() {
        let t = score_pilot(&PilotStats::default());
        assert_eq!(t.level, ThreatLevel::Safe);
    }

    #[test]
    fn flashy_no_kills_is_caution() {
        let s = PilotStats { sec_status: -7.5, ..Default::default() };
        assert_eq!(score_pilot(&s).level, ThreatLevel::Caution);
    }

    #[test]
    fn prolific_killer_is_danger() {
        let s = PilotStats {
            danger_ratio: 85,
            gang_ratio: 50,
            ships_destroyed: 4000,
            ships_lost: 300,
            sec_status: -2.0,
        };
        assert_eq!(score_pilot(&s).level, ThreatLevel::Danger);
    }

    #[test]
    fn solo_hunter_escalates_to_danger() {
        let s = PilotStats {
            danger_ratio: 65,
            gang_ratio: 10,
            ships_destroyed: 120,
            ships_lost: 40,
            sec_status: -1.0,
        };
        assert_eq!(score_pilot(&s).level, ThreatLevel::Danger);
    }

    #[test]
    fn modest_board_is_neutral() {
        let s = PilotStats {
            danger_ratio: 40,
            gang_ratio: 80,
            ships_destroyed: 20,
            ships_lost: 50,
            sec_status: 0.5,
        };
        assert_eq!(score_pilot(&s).level, ThreatLevel::Neutral);
    }

    #[test]
    fn summary_calls_out_hunters() {
        let levels = [ThreatLevel::Danger, ThreatLevel::Caution, ThreatLevel::Safe];
        assert!(summarize(&levels).contains("hunter"));
    }

    #[test]
    fn safety_levels_by_kill_volume() {
        assert_eq!(assess_safety(0).level, ThreatLevel::Safe);
        assert_eq!(assess_safety(3).level, ThreatLevel::Caution);
        assert_eq!(assess_safety(20).level, ThreatLevel::Danger);
    }

    #[test]
    fn gatecamp_levels_by_kill_volume() {
        assert_eq!(assess_gatecamp(0).level, ThreatLevel::Safe);
        assert_eq!(assess_gatecamp(2).level, ThreatLevel::Caution);
        assert_eq!(assess_gatecamp(9).level, ThreatLevel::Danger);
    }

    #[test]
    fn zkill_projects_onto_scorer() {
        let z = ZkillStats {
            danger_ratio: 90,
            gang_ratio: 20,
            ships_destroyed: 500,
            ships_lost: 10,
            info: ZkillInfo { sec_status: -9.0 },
        };
        let s = z.to_pilot_stats();
        assert_eq!(s.danger_ratio, 90);
        assert_eq!(s.sec_status, -9.0);
        assert_eq!(score_pilot(&s).level, ThreatLevel::Danger);
    }
}
