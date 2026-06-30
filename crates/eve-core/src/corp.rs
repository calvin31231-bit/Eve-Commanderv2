//! Corporation reads: structures (with fuel-expiry countdowns) for the Corp hub.
//!
//! Structure fuel runs out at a known time, so — like industry jobs and PI
//! extractors — we fetch the expiry once and count down locally (no re-poll).
//! The [`summarize_structures`] reduction is pure and unit-tested; the corp
//! endpoint needs a director/station-manager role, so the caller treats a 403 as
//! "no access" rather than an error.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::EsiClient;

/// One corp structure (ESI `GET /corporations/{id}/structures/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpStructure {
    pub structure_id: i64,
    #[serde(default)]
    pub type_id: i64,
    pub system_id: i64,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub fuel_expires: Option<String>,
}

/// A structure with its fuel countdown derived client-side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureStatus {
    pub structure_id: i64,
    pub type_id: i64,
    pub system_id: i64,
    pub state: String,
    pub fuel_expires: Option<String>,
    /// Seconds until fuel runs out (0 if expired/unknown).
    pub fuel_seconds_remaining: i64,
}

/// Reduce corp structures to fuel countdowns, soonest-to-expire first. Pure
/// (time injected) → unit-tested.
pub fn summarize_structures(structures: &[CorpStructure], now: OffsetDateTime) -> Vec<StructureStatus> {
    let mut out: Vec<StructureStatus> = structures
        .iter()
        .map(|s| {
            let fuel_seconds_remaining = s
                .fuel_expires
                .as_deref()
                .and_then(|t| OffsetDateTime::parse(t, &Rfc3339).ok())
                .map(|t| (t - now).whole_seconds().max(0))
                .unwrap_or(0);
            StructureStatus {
                structure_id: s.structure_id,
                type_id: s.type_id,
                system_id: s.system_id,
                state: s.state.clone(),
                fuel_expires: s.fuel_expires.clone(),
                fuel_seconds_remaining,
            }
        })
        .collect();
    // Structures with a fuel timer first (soonest expiry), then the rest.
    out.sort_by_key(|s| {
        if s.fuel_expires.is_some() {
            (0, s.fuel_seconds_remaining)
        } else {
            (1, 0)
        }
    });
    out
}

/// One moon-mining extraction (ESI
/// `GET /corporation/{id}/mining/extractions/`). The chunk arrives at a known
/// time, so — like structure fuel — we count down locally from one fetch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoonExtraction {
    pub structure_id: i64,
    #[serde(default)]
    pub moon_id: i64,
    #[serde(default)]
    pub extraction_start_time: Option<String>,
    #[serde(default)]
    pub chunk_arrival_time: Option<String>,
    #[serde(default)]
    pub natural_decay_time: Option<String>,
}

/// An extraction with its chunk-arrival countdown derived client-side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionStatus {
    pub structure_id: i64,
    pub moon_id: i64,
    pub chunk_arrival_time: Option<String>,
    pub natural_decay_time: Option<String>,
    /// Seconds until the chunk arrives (0 if already arrived/unknown).
    pub arrival_seconds_remaining: i64,
    /// True once the chunk has arrived (ready to fracture).
    pub ready: bool,
}

/// Reduce moon extractions to chunk-arrival countdowns, soonest first. Pure
/// (time injected) → unit-tested.
pub fn summarize_extractions(
    extractions: &[MoonExtraction],
    now: OffsetDateTime,
) -> Vec<ExtractionStatus> {
    let mut out: Vec<ExtractionStatus> = extractions
        .iter()
        .map(|e| {
            let secs = e
                .chunk_arrival_time
                .as_deref()
                .and_then(|t| OffsetDateTime::parse(t, &Rfc3339).ok())
                .map(|t| (t - now).whole_seconds())
                .unwrap_or(0);
            ExtractionStatus {
                structure_id: e.structure_id,
                moon_id: e.moon_id,
                chunk_arrival_time: e.chunk_arrival_time.clone(),
                natural_decay_time: e.natural_decay_time.clone(),
                arrival_seconds_remaining: secs.max(0),
                ready: e.chunk_arrival_time.is_some() && secs <= 0,
            }
        })
        .collect();
    out.sort_by_key(|e| e.arrival_seconds_remaining);
    out
}

/// One member-tracking row (ESI `GET /corporations/{id}/membertracking/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberTrack {
    pub character_id: i64,
    #[serde(default)]
    pub logon_date: Option<String>,
    #[serde(default)]
    pub logoff_date: Option<String>,
    #[serde(default)]
    pub location_id: Option<i64>,
    #[serde(default)]
    pub ship_type_id: Option<i64>,
    #[serde(default)]
    pub start_date: Option<String>,
}

/// One corp container audit-log entry (ESI
/// `GET /corporations/{id}/containers/logs/`). The action plus the actor and
/// quantity is what theft detection keys on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerLog {
    #[serde(default)]
    pub action: String,
    pub character_id: i64,
    #[serde(default)]
    pub container_id: i64,
    #[serde(default)]
    pub container_type_id: i64,
    #[serde(default)]
    pub location_id: i64,
    #[serde(default)]
    pub logged_at: Option<String>,
    #[serde(default)]
    pub quantity: Option<i64>,
    #[serde(default)]
    pub type_id: Option<i64>,
    #[serde(default)]
    pub password_type: Option<String>,
}

/// A container-log entry the vetting heuristic flagged as theft-suspicious, with
/// a severity (0..=1) and a human reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerLogFlag {
    pub character_id: i64,
    pub container_id: i64,
    pub action: String,
    pub quantity: i64,
    pub logged_at: Option<String>,
    pub type_id: i64,
    /// 0.0..=1.0 — how suspicious this looks.
    pub severity: f64,
    pub reason: String,
}

/// Score container-log entries for likely theft and return only the suspicious
/// ones, most-severe first. Pure → unit-tested.
///
/// Heuristic: password tampering (`set_password`/`enter_password`) and `unlock`
/// are the classic theft vectors (someone breaking into a secured can), and a
/// large-`quantity` `move`/`add` out of a can is the act itself. Benign,
/// high-volume operational actions (`configure`, `assemble`, `repackage`,
/// `set_name`, `lock`) are ignored.
pub fn flag_container_thefts(logs: &[ContainerLog], large_quantity: i64) -> Vec<ContainerLogFlag> {
    let mut out: Vec<ContainerLogFlag> = Vec::new();
    for l in logs {
        let qty = l.quantity.unwrap_or(0);
        let (severity, reason) = match l.action.as_str() {
            "set_password" | "enter_password" => {
                (0.9, "container password changed/entered — classic theft vector".to_string())
            }
            "unlock" => (0.7, "secured container unlocked".to_string()),
            "move" | "add" if qty >= large_quantity => {
                (0.6, format!("large {} of {} units", l.action, qty))
            }
            _ => continue,
        };
        out.push(ContainerLogFlag {
            character_id: l.character_id,
            container_id: l.container_id,
            action: l.action.clone(),
            quantity: qty,
            logged_at: l.logged_at.clone(),
            type_id: l.type_id.unwrap_or(0),
            severity,
            reason,
        });
    }
    out.sort_by(|a, b| b.severity.partial_cmp(&a.severity).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Authenticated corporation reads over the cache-first ESI client.
#[derive(Clone)]
pub struct CorpClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl CorpClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The corporation's structures. Requires a director / station-manager role
    /// plus `esi-corporations.read_structures.v1`; ESI 403s otherwise (the caller
    /// treats an error as "no access").
    pub async fn structures(&self, character_id: i64, corp_id: i64) -> Result<Vec<CorpStructure>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/corporations/{corp_id}/structures/");
        self.esi
            .get_auth_json_paged::<CorpStructure>(&path, &token)
            .await
            .map_err(|e| Error::other(format!("corp structures: {e}")))
    }

    /// Fetch + summarize structures with fuel countdowns (now injected here so
    /// the shell needn't depend on `time`).
    pub async fn structure_status(
        &self,
        character_id: i64,
        corp_id: i64,
    ) -> Result<Vec<StructureStatus>> {
        let structures = self.structures(character_id, corp_id).await?;
        Ok(summarize_structures(&structures, OffsetDateTime::now_utc()))
    }

    /// Corp member tracking (last logon/logoff, location, ship). Requires a
    /// director role + `esi-corporations.track_members.v1`; 403s otherwise. The
    /// rows come back most-recently-active first.
    pub async fn member_tracking(
        &self,
        character_id: i64,
        corp_id: i64,
    ) -> Result<Vec<MemberTrack>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/corporations/{corp_id}/membertracking/");
        let mut members = self
            .esi
            .get_auth_json::<Vec<MemberTrack>>(&path, &token)
            .await
            .map_err(|e| Error::other(format!("member tracking: {e}")))?;
        // Most recently logged on first; never-seen members sink to the bottom.
        members.sort_by(|a, b| b.logon_date.cmp(&a.logon_date));
        Ok(members)
    }

    /// Corp moon-mining extractions. Requires a Structure_Manager role +
    /// `esi-industry.read_corporation_mining.v1`; 403s otherwise.
    pub async fn extractions(
        &self,
        character_id: i64,
        corp_id: i64,
    ) -> Result<Vec<MoonExtraction>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/corporation/{corp_id}/mining/extractions/");
        self.esi
            .get_auth_json_paged::<MoonExtraction>(&path, &token)
            .await
            .map_err(|e| Error::other(format!("moon extractions: {e}")))
    }

    /// Fetch + summarize extractions with chunk-arrival countdowns (now injected
    /// here so the shell needn't depend on `time`).
    pub async fn extraction_status(
        &self,
        character_id: i64,
        corp_id: i64,
    ) -> Result<Vec<ExtractionStatus>> {
        let extractions = self.extractions(character_id, corp_id).await?;
        Ok(summarize_extractions(&extractions, OffsetDateTime::now_utc()))
    }

    /// Corp container audit logs (theft-detection source). Requires a director
    /// role + `esi-corporations.read_container_logs.v1`; 403s otherwise.
    pub async fn container_logs(
        &self,
        character_id: i64,
        corp_id: i64,
    ) -> Result<Vec<ContainerLog>> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/corporations/{corp_id}/containers/logs/");
        self.esi
            .get_auth_json_paged::<ContainerLog>(&path, &token)
            .await
            .map_err(|e| Error::other(format!("container logs: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }

    fn st(id: i64, fuel: Option<&str>) -> CorpStructure {
        CorpStructure {
            structure_id: id,
            type_id: 35832,
            system_id: 30000142,
            state: "shield_vulnerable".into(),
            fuel_expires: fuel.map(str::to_string),
        }
    }

    #[test]
    fn fuel_countdown_and_ordering() {
        let now = ts("2026-06-22T00:00:00Z");
        let structures = vec![
            st(1, Some("2026-06-24T00:00:00Z")), // 2 days
            st(2, Some("2026-06-22T01:00:00Z")), // 1 hour (soonest)
            st(3, None),                          // no timer → last
        ];
        let out = summarize_structures(&structures, now);
        assert_eq!(out[0].structure_id, 2);
        assert_eq!(out[0].fuel_seconds_remaining, 3600);
        assert_eq!(out[2].structure_id, 3);
        assert_eq!(out[2].fuel_seconds_remaining, 0);
    }

    #[test]
    fn expired_fuel_clamps_to_zero() {
        let now = ts("2026-06-22T00:00:00Z");
        let out = summarize_structures(&[st(1, Some("2026-06-21T00:00:00Z"))], now);
        assert_eq!(out[0].fuel_seconds_remaining, 0);
    }

    #[test]
    fn extraction_countdown_orders_soonest_first_and_marks_ready() {
        let now = ts("2026-06-22T00:00:00Z");
        let ex = |sid: i64, arrival: &str| MoonExtraction {
            structure_id: sid,
            moon_id: 40000001,
            extraction_start_time: Some("2026-06-20T00:00:00Z".into()),
            chunk_arrival_time: Some(arrival.into()),
            natural_decay_time: Some("2026-06-25T00:00:00Z".into()),
        };
        let out = summarize_extractions(
            &[
                ex(1, "2026-06-24T00:00:00Z"), // 2 days out
                ex(2, "2026-06-21T00:00:00Z"), // already arrived → ready
            ],
            now,
        );
        assert_eq!(out[0].structure_id, 2);
        assert!(out[0].ready);
        assert_eq!(out[0].arrival_seconds_remaining, 0);
        assert!(!out[1].ready);
        assert_eq!(out[1].arrival_seconds_remaining, 2 * 86_400);
    }

    fn log(action: &str, qty: Option<i64>) -> ContainerLog {
        ContainerLog {
            action: action.into(),
            character_id: 90000001,
            container_id: 1000000001,
            container_type_id: 17363,
            location_id: 60003760,
            logged_at: Some("2026-06-22T00:00:00Z".into()),
            quantity: qty,
            type_id: Some(34),
            password_type: None,
        }
    }

    #[test]
    fn theft_flags_password_unlock_and_large_moves() {
        let logs = vec![
            log("configure", None),          // benign → ignored
            log("set_name", None),           // benign → ignored
            log("move", Some(5)),            // small move → ignored
            log("unlock", None),             // suspicious
            log("move", Some(10_000)),       // large move → suspicious
            log("set_password", None),       // most suspicious
        ];
        let flags = flag_container_thefts(&logs, 1000);
        assert_eq!(flags.len(), 3);
        // Most-severe first: password tampering tops the list.
        assert_eq!(flags[0].action, "set_password");
        assert!(flags[0].severity > flags[1].severity);
        assert!(flags.iter().all(|f| f.action != "move" || f.quantity >= 1000));
    }
}
