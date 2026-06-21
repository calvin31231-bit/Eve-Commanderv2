//! Character data domain: typed ESI read models for the Character hub (skills,
//! skill queue, wallet) plus the derived computations the UI needs.
//!
//! The models mirror the ESI response shapes exactly so they deserialize
//! directly; the derived helpers (total SP, currently-training skill, queue
//! time remaining) are pure and unit-tested. The [`CharacterClient`] ties the
//! cache-first [`EsiClient`] and the [`TokenManager`] together into typed,
//! authenticated reads.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One trained skill (ESI `GET /characters/{id}/skills/` element).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub skill_id: i64,
    pub skillpoints_in_skill: i64,
    pub trained_skill_level: i64,
    pub active_skill_level: i64,
}

/// Character attributes (ESI `GET /characters/{id}/attributes/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterAttributes {
    pub intelligence: i64,
    pub memory: i64,
    pub perception: i64,
    pub willpower: i64,
    pub charisma: i64,
    #[serde(default)]
    pub bonus_remaps: Option<i64>,
    #[serde(default)]
    pub accrued_remap_cooldown_date: Option<String>,
    #[serde(default)]
    pub last_remap_date: Option<String>,
}

/// A queued skill flattened for the UI, with its client-side countdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedSkill {
    pub skill_id: i64,
    pub finished_level: i64,
    pub queue_position: i64,
    /// Seconds until this entry finishes (0 if unknown/passed).
    pub seconds_remaining: i64,
}

/// The character skill sheet (ESI `GET /characters/{id}/skills/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSheet {
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub total_sp: i64,
    #[serde(default)]
    pub unallocated_sp: Option<i64>,
}

impl SkillSheet {
    /// Number of distinct skills injected/trained.
    pub fn skill_count(&self) -> usize {
        self.skills.len()
    }

    /// How many skills are trained to level V.
    pub fn maxed_count(&self) -> usize {
        self.skills.iter().filter(|s| s.trained_skill_level >= 5).count()
    }
}

/// One entry in the skill queue (ESI `GET /characters/{id}/skillqueue/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillQueueEntry {
    pub skill_id: i64,
    pub finished_level: i64,
    pub queue_position: i64,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub finish_date: Option<String>,
    #[serde(default)]
    pub level_start_sp: Option<i64>,
    #[serde(default)]
    pub level_end_sp: Option<i64>,
    #[serde(default)]
    pub training_start_sp: Option<i64>,
}

impl SkillQueueEntry {
    /// Parsed `finish_date`, if present and valid (ESI emits RFC 3339).
    pub fn finish_at(&self) -> Option<OffsetDateTime> {
        parse_rfc3339(self.finish_date.as_deref())
    }
}

/// The skill queue as a whole, with the derived state the UI shows.
#[derive(Debug, Clone, Default)]
pub struct SkillQueue {
    pub entries: Vec<SkillQueueEntry>,
}

impl SkillQueue {
    pub fn new(mut entries: Vec<SkillQueueEntry>) -> Self {
        entries.sort_by_key(|e| e.queue_position);
        Self { entries }
    }

    /// Whether the queue is empty (nothing training) — drives the "queue empty"
    /// alert.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The skill currently training (queue position 0).
    pub fn active(&self) -> Option<&SkillQueueEntry> {
        self.entries.iter().find(|e| e.queue_position == 0)
    }

    /// When the whole queue finishes (latest `finish_date`).
    pub fn finishes_at(&self) -> Option<OffsetDateTime> {
        self.entries.iter().filter_map(|e| e.finish_at()).max()
    }

    /// Time until the whole queue empties, relative to `now`. `None` if unknown;
    /// `Duration::ZERO` if already finished.
    pub fn time_remaining(&self, now: OffsetDateTime) -> Option<Duration> {
        let end = self.finishes_at()?;
        Some((end - now).max(Duration::ZERO))
    }

    /// The queue as flattened [`QueuedSkill`]s with per-entry countdowns,
    /// ordered by position. Names are resolved by the caller.
    pub fn upcoming(&self, now: OffsetDateTime) -> Vec<QueuedSkill> {
        self.entries
            .iter()
            .map(|e| QueuedSkill {
                skill_id: e.skill_id,
                finished_level: e.finished_level,
                queue_position: e.queue_position,
                seconds_remaining: e
                    .finish_at()
                    .map(|f| (f - now).whole_seconds().max(0))
                    .unwrap_or(0),
            })
            .collect()
    }
}

/// A flattened, serializable summary of a character that populates the
/// Character hub in a single payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterSheet {
    pub wallet_balance: f64,
    pub total_sp: i64,
    pub unallocated_sp: Option<i64>,
    pub skill_count: usize,
    pub maxed_count: usize,
    pub queue_len: usize,
    pub active_skill_id: Option<i64>,
    /// RFC 3339 timestamp the whole queue finishes, if known.
    pub queue_finishes_at: Option<String>,
    /// Whole seconds until the queue empties, if known.
    pub queue_seconds_remaining: Option<i64>,
}

impl CharacterSheet {
    /// Assemble the hub summary from the three source reads. Pure (time is
    /// injected) so the derivation is unit-tested without network.
    pub fn assemble(
        skills: &SkillSheet,
        queue: &SkillQueue,
        wallet_balance: f64,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            wallet_balance,
            total_sp: skills.total_sp,
            unallocated_sp: skills.unallocated_sp,
            skill_count: skills.skill_count(),
            maxed_count: skills.maxed_count(),
            queue_len: queue.entries.len(),
            active_skill_id: queue.active().map(|e| e.skill_id),
            queue_finishes_at: queue.finishes_at().and_then(|t| t.format(&Rfc3339).ok()),
            queue_seconds_remaining: queue.time_remaining(now).map(|d| d.whole_seconds()),
        }
    }
}

/// Public character info (ESI `GET /characters/{id}/` — no auth needed).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterPublic {
    pub name: String,
    pub corporation_id: i64,
    #[serde(default)]
    pub alliance_id: Option<i64>,
    #[serde(default)]
    pub security_status: f64,
    #[serde(default)]
    pub birthday: Option<String>,
}

/// Current location (ESI `GET /characters/{id}/location/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterLocation {
    pub solar_system_id: i64,
    #[serde(default)]
    pub station_id: Option<i64>,
    #[serde(default)]
    pub structure_id: Option<i64>,
}

/// Current ship (ESI `GET /characters/{id}/ship/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterShip {
    pub ship_type_id: i64,
    #[serde(default)]
    pub ship_name: String,
    #[serde(default)]
    pub ship_item_id: i64,
}

/// Online status (ESI `GET /characters/{id}/online/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterOnline {
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub last_login: Option<String>,
    #[serde(default)]
    pub last_logout: Option<String>,
    #[serde(default)]
    pub logins: Option<i64>,
}

/// The live status strip's raw ids (names resolved by the caller).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterStatusRaw {
    pub online: bool,
    /// 0 when unknown (e.g. location scope not granted).
    pub system_id: i64,
    pub ship_type_id: i64,
    pub ship_name: String,
    pub training_skill_id: Option<i64>,
    pub training_level: Option<i64>,
    pub training_seconds_remaining: Option<i64>,
}

/// Typed, authenticated character reads over the cache-first ESI client.
#[derive(Clone)]
pub struct CharacterClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl CharacterClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// Fetch + deserialize an authenticated catalog endpoint for a character.
    async fn auth_get<T: serde::de::DeserializeOwned>(
        &self,
        character_id: i64,
        endpoint_key: &str,
    ) -> Result<T> {
        let ep = endpoint(endpoint_key)
            .ok_or_else(|| Error::other(format!("unknown endpoint '{endpoint_key}'")))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<T>(&ep.path_for(character_id), &token)
            .await
    }

    /// The character's skill sheet.
    pub async fn skills(&self, character_id: i64) -> Result<SkillSheet> {
        self.auth_get(character_id, "skills").await
    }

    /// The character's skill queue (sorted by position).
    pub async fn skill_queue(&self, character_id: i64) -> Result<SkillQueue> {
        let entries: Vec<SkillQueueEntry> = self.auth_get(character_id, "skillqueue").await?;
        Ok(SkillQueue::new(entries))
    }

    /// The character's wallet balance in ISK. ESI returns a bare JSON number.
    pub async fn wallet_balance(&self, character_id: i64) -> Result<f64> {
        self.auth_get(character_id, "wallet_balance").await
    }

    /// The character's current location (solar system, station/structure).
    pub async fn location(&self, character_id: i64) -> Result<CharacterLocation> {
        self.auth_get(character_id, "location").await
    }

    /// The character's current ship (hull type + custom name).
    pub async fn ship(&self, character_id: i64) -> Result<CharacterShip> {
        self.auth_get(character_id, "ship").await
    }

    /// The character's online status.
    pub async fn online(&self, character_id: i64) -> Result<CharacterOnline> {
        self.auth_get(character_id, "online").await
    }

    /// The character's attributes (+ remap info).
    pub async fn attributes(&self, character_id: i64) -> Result<CharacterAttributes> {
        self.auth_get(character_id, "attributes").await
    }

    /// The skill queue as flattened entries with per-skill countdowns (names
    /// resolved by the caller).
    pub async fn skill_queue_upcoming(&self, character_id: i64) -> Result<Vec<QueuedSkill>> {
        let queue = self.skill_queue(character_id).await?;
        Ok(queue.upcoming(OffsetDateTime::now_utc()))
    }

    /// Public character info (corp/alliance/sec status). Unauthenticated.
    pub async fn public_info(&self, character_id: i64) -> Result<CharacterPublic> {
        let ep = endpoint("character_public")
            .ok_or_else(|| Error::other("unknown endpoint 'character_public'"))?;
        self.esi
            .get_public_json::<CharacterPublic>(&ep.path_for(character_id))
            .await
    }

    /// Assemble the live status strip (online, location, ship, current training)
    /// from four concurrent reads. Each read is best-effort — a missing scope
    /// leaves that field at its default rather than failing the whole status.
    /// Name resolution (system/ship/skill ids) is left to the caller.
    pub async fn status(&self, character_id: i64) -> Result<CharacterStatusRaw> {
        let (location, ship, online, queue) = tokio::join!(
            self.location(character_id),
            self.ship(character_id),
            self.online(character_id),
            self.skill_queue(character_id),
        );

        let now = OffsetDateTime::now_utc();
        let mut training_skill_id = None;
        let mut training_level = None;
        let mut training_seconds_remaining = None;
        if let Ok(q) = &queue {
            if let Some(active) = q.active() {
                training_skill_id = Some(active.skill_id);
                training_level = Some(active.finished_level);
                training_seconds_remaining =
                    active.finish_at().map(|f| (f - now).whole_seconds().max(0));
            }
        }

        Ok(CharacterStatusRaw {
            online: online.map(|o| o.online).unwrap_or(false),
            system_id: location.as_ref().map(|l| l.solar_system_id).unwrap_or(0),
            ship_type_id: ship.as_ref().map(|s| s.ship_type_id).unwrap_or(0),
            ship_name: ship.map(|s| s.ship_name).unwrap_or_default(),
            training_skill_id,
            training_level,
            training_seconds_remaining,
        })
    }

    /// Fetch skills, skill queue, and wallet concurrently and assemble the hub
    /// summary.
    pub async fn sheet(&self, character_id: i64) -> Result<CharacterSheet> {
        let (skills, queue, wallet) = tokio::join!(
            self.skills(character_id),
            self.skill_queue(character_id),
            self.wallet_balance(character_id),
        );
        Ok(CharacterSheet::assemble(
            &skills?,
            &queue?,
            wallet?,
            OffsetDateTime::now_utc(),
        ))
    }
}

/// Parse an optional RFC 3339 timestamp (ESI's format) into an `OffsetDateTime`.
fn parse_rfc3339(s: Option<&str>) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(s?, &Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_skill_sheet() {
        // Shape from ESI GET /characters/{id}/skills/.
        let json = r#"{
            "skills": [
                {"skill_id": 3300, "skillpoints_in_skill": 256000, "trained_skill_level": 5, "active_skill_level": 5},
                {"skill_id": 3301, "skillpoints_in_skill": 8000, "trained_skill_level": 2, "active_skill_level": 2}
            ],
            "total_sp": 264000,
            "unallocated_sp": 12000
        }"#;
        let sheet: SkillSheet = serde_json::from_str(json).unwrap();
        assert_eq!(sheet.total_sp, 264_000);
        assert_eq!(sheet.unallocated_sp, Some(12_000));
        assert_eq!(sheet.skill_count(), 2);
        assert_eq!(sheet.maxed_count(), 1);
    }

    #[test]
    fn skill_sheet_tolerates_missing_optionals() {
        // Older/sparse responses may omit unallocated_sp.
        let json = r#"{"skills": [], "total_sp": 0}"#;
        let sheet: SkillSheet = serde_json::from_str(json).unwrap();
        assert!(sheet.skills.is_empty());
        assert_eq!(sheet.unallocated_sp, None);
    }

    #[test]
    fn deserializes_and_orders_skill_queue() {
        // Deliberately out of order to prove sorting.
        let json = r#"[
            {"skill_id": 2, "finished_level": 4, "queue_position": 1, "finish_date": "2026-06-21T12:00:00Z"},
            {"skill_id": 1, "finished_level": 3, "queue_position": 0, "start_date": "2026-06-20T10:00:00Z", "finish_date": "2026-06-20T16:00:00Z"}
        ]"#;
        let entries: Vec<SkillQueueEntry> = serde_json::from_str(json).unwrap();
        let queue = SkillQueue::new(entries);
        assert!(!queue.is_empty());
        // Active = position 0.
        assert_eq!(queue.active().unwrap().skill_id, 1);
        // Sorted by position.
        assert_eq!(queue.entries[0].queue_position, 0);
        assert_eq!(queue.entries[1].queue_position, 1);
    }

    #[test]
    fn queue_finish_and_remaining() {
        let json = r#"[
            {"skill_id": 1, "finished_level": 3, "queue_position": 0, "finish_date": "2026-06-20T16:00:00Z"},
            {"skill_id": 2, "finished_level": 4, "queue_position": 1, "finish_date": "2026-06-21T12:00:00Z"}
        ]"#;
        let queue = SkillQueue::new(serde_json::from_str(json).unwrap());
        // Whole-queue finish is the latest entry.
        let finish = queue.finishes_at().unwrap();
        assert_eq!(finish, OffsetDateTime::parse("2026-06-21T12:00:00Z", &Rfc3339).unwrap());

        // Remaining from a point before the end is positive.
        let now = OffsetDateTime::parse("2026-06-20T12:00:00Z", &Rfc3339).unwrap();
        let remaining = queue.time_remaining(now).unwrap();
        assert!(remaining > Duration::ZERO);

        // Remaining from after the end clamps to zero.
        let later = OffsetDateTime::parse("2026-06-22T00:00:00Z", &Rfc3339).unwrap();
        assert_eq!(queue.time_remaining(later).unwrap(), Duration::ZERO);
    }

    #[test]
    fn empty_queue_has_no_active_or_finish() {
        let queue = SkillQueue::new(vec![]);
        assert!(queue.is_empty());
        assert!(queue.active().is_none());
        assert!(queue.finishes_at().is_none());
    }

    #[test]
    fn deserializes_attributes() {
        let a: CharacterAttributes = serde_json::from_str(
            r#"{"intelligence": 20, "memory": 21, "perception": 22, "willpower": 23, "charisma": 19, "bonus_remaps": 2}"#,
        )
        .unwrap();
        assert_eq!(a.perception, 22);
        assert_eq!(a.bonus_remaps, Some(2));
    }

    #[test]
    fn skill_queue_upcoming_has_per_entry_countdowns() {
        let json = r#"[
            {"skill_id": 1, "finished_level": 3, "queue_position": 0, "finish_date": "2026-06-20T12:00:00Z"},
            {"skill_id": 2, "finished_level": 4, "queue_position": 1, "finish_date": "2026-06-21T12:00:00Z"}
        ]"#;
        let queue = SkillQueue::new(serde_json::from_str(json).unwrap());
        let now = OffsetDateTime::parse("2026-06-20T00:00:00Z", &Rfc3339).unwrap();
        let up = queue.upcoming(now);
        assert_eq!(up.len(), 2);
        assert_eq!(up[0].skill_id, 1);
        assert_eq!(up[0].seconds_remaining, 12 * 3600);
        assert_eq!(up[1].seconds_remaining, 36 * 3600);
    }

    #[test]
    fn deserializes_public_info() {
        let p: CharacterPublic = serde_json::from_str(
            r#"{"name": "Capsuleer", "corporation_id": 98000001, "alliance_id": 99000001, "security_status": 4.7, "birthday": "2018-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert_eq!(p.name, "Capsuleer");
        assert_eq!(p.corporation_id, 98000001);
        assert_eq!(p.alliance_id, Some(99000001));
        assert!((p.security_status - 4.7).abs() < 1e-9);
    }

    #[test]
    fn deserializes_status_reads() {
        let loc: CharacterLocation =
            serde_json::from_str(r#"{"solar_system_id": 30000142, "station_id": 60003760}"#).unwrap();
        assert_eq!(loc.solar_system_id, 30000142);
        assert_eq!(loc.station_id, Some(60003760));

        let ship: CharacterShip = serde_json::from_str(
            r#"{"ship_type_id": 587, "ship_name": "Pewpew", "ship_item_id": 1000000016991}"#,
        )
        .unwrap();
        assert_eq!(ship.ship_type_id, 587);
        assert_eq!(ship.ship_name, "Pewpew");

        let online: CharacterOnline =
            serde_json::from_str(r#"{"online": true, "last_login": "2026-06-21T00:00:00Z"}"#).unwrap();
        assert!(online.online);
    }

    #[test]
    fn wallet_balance_is_a_bare_number() {
        // ESI GET /characters/{id}/wallet/ returns a bare JSON number.
        let balance: f64 = serde_json::from_str("1500000.42").unwrap();
        assert!((balance - 1_500_000.42).abs() < 1e-6);
    }

    #[test]
    fn assembles_character_sheet_summary() {
        let sheet = SkillSheet {
            skills: vec![
                Skill { skill_id: 1, skillpoints_in_skill: 256_000, trained_skill_level: 5, active_skill_level: 5 },
                Skill { skill_id: 2, skillpoints_in_skill: 8_000, trained_skill_level: 3, active_skill_level: 3 },
            ],
            total_sp: 264_000,
            unallocated_sp: Some(5_000),
        };
        let queue = SkillQueue::new(vec![
            SkillQueueEntry { skill_id: 2, finished_level: 4, queue_position: 0, start_date: None, finish_date: Some("2026-06-21T00:00:00Z".into()), level_start_sp: None, level_end_sp: None, training_start_sp: None },
        ]);
        let now = OffsetDateTime::parse("2026-06-20T00:00:00Z", &Rfc3339).unwrap();

        let summary = CharacterSheet::assemble(&sheet, &queue, 1_000_000.0, now);
        assert_eq!(summary.total_sp, 264_000);
        assert_eq!(summary.skill_count, 2);
        assert_eq!(summary.maxed_count, 1);
        assert_eq!(summary.queue_len, 1);
        assert_eq!(summary.active_skill_id, Some(2));
        assert_eq!(summary.queue_finishes_at.as_deref(), Some("2026-06-21T00:00:00Z"));
        // 24h between now and finish.
        assert_eq!(summary.queue_seconds_remaining, Some(86_400));
    }
}
