//! Industry jobs: the character's running manufacturing / research / invention /
//! reaction jobs, with client-side completion countdowns.
//!
//! Job end times are deterministic once fetched, so rather than re-poll we
//! compute `seconds_remaining` locally from the `end_date` (the "derive live
//! state locally" principle — a dashboard of ticking timers costs no network).
//! The [`summarize_jobs`] derivation is pure and unit-tested.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One industry job (ESI `GET /characters/{id}/industry/jobs/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndustryJob {
    pub job_id: i64,
    pub activity_id: i64,
    pub blueprint_type_id: i64,
    #[serde(default)]
    pub product_type_id: Option<i64>,
    pub status: String,
    #[serde(default)]
    pub runs: i64,
    pub start_date: String,
    pub end_date: String,
    #[serde(default)]
    pub facility_id: Option<i64>,
}

/// Human name for an ESI industry `activity_id`.
pub fn activity_name(activity_id: i64) -> &'static str {
    match activity_id {
        1 => "Manufacturing",
        3 => "Time Efficiency Research",
        4 => "Material Efficiency Research",
        5 => "Copying",
        7 => "Reverse Engineering",
        8 => "Invention",
        9 | 11 => "Reactions",
        _ => "Industry",
    }
}

/// An in-progress job flattened for the UI, with a client-side countdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveJob {
    pub job_id: i64,
    pub activity: String,
    /// The item to resolve a name for: the product when known, else the
    /// blueprint.
    pub display_type_id: i64,
    pub runs: i64,
    pub status: String,
    pub end_date: String,
    /// Seconds until completion (0 once the end time has passed / job ready).
    pub seconds_remaining: i64,
}

/// Rollup of the in-progress jobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndustrySummary {
    pub active_count: usize,
    /// Active jobs, soonest completion first.
    pub jobs: Vec<ActiveJob>,
}

/// Whether a job status counts as "in progress" for the hub.
fn is_in_progress(status: &str) -> bool {
    matches!(status, "active" | "paused" | "ready")
}

/// Summarize jobs into the in-progress set with client-side countdowns,
/// soonest-completion first. Pure (time injected) → unit-tested.
pub fn summarize_jobs(jobs: &[IndustryJob], now: OffsetDateTime) -> IndustrySummary {
    let mut active: Vec<ActiveJob> = jobs
        .iter()
        .filter(|j| is_in_progress(&j.status))
        .map(|j| {
            let end = OffsetDateTime::parse(&j.end_date, &Rfc3339).ok();
            let seconds_remaining = end
                .map(|e| (e - now).whole_seconds().max(0))
                .unwrap_or(0);
            ActiveJob {
                job_id: j.job_id,
                activity: activity_name(j.activity_id).to_string(),
                display_type_id: j.product_type_id.unwrap_or(j.blueprint_type_id),
                runs: j.runs,
                status: j.status.clone(),
                end_date: j.end_date.clone(),
                seconds_remaining,
            }
        })
        .collect();

    active.sort_by_key(|j| j.seconds_remaining);

    IndustrySummary {
        active_count: active.len(),
        jobs: active,
    }
}

/// Typed, authenticated industry-job reads over the cache-first ESI client.
#[derive(Clone)]
pub struct IndustryClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl IndustryClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's industry jobs (single page; ESI returns all active jobs).
    pub async fn jobs(&self, character_id: i64) -> Result<Vec<IndustryJob>> {
        let ep = endpoint("industry_jobs")
            .ok_or_else(|| Error::other("unknown endpoint 'industry_jobs'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<Vec<IndustryJob>>(&ep.path_for(character_id), &token)
            .await
    }

    /// Fetch and summarize in-progress jobs with countdowns.
    pub async fn summary(&self, character_id: i64) -> Result<IndustrySummary> {
        let jobs = self.jobs(character_id).await?;
        Ok(summarize_jobs(&jobs, OffsetDateTime::now_utc()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_names_cover_common_ids() {
        assert_eq!(activity_name(1), "Manufacturing");
        assert_eq!(activity_name(8), "Invention");
        assert_eq!(activity_name(9), "Reactions");
        assert_eq!(activity_name(11), "Reactions");
        assert_eq!(activity_name(999), "Industry");
    }

    #[test]
    fn deserializes_industry_job() {
        let json = r#"{
            "job_id": 5,
            "activity_id": 1,
            "blueprint_type_id": 700,
            "product_type_id": 587,
            "status": "active",
            "runs": 10,
            "start_date": "2026-06-20T00:00:00Z",
            "end_date": "2026-06-20T06:00:00Z",
            "facility_id": 60003760
        }"#;
        let job: IndustryJob = serde_json::from_str(json).unwrap();
        assert_eq!(job.activity_id, 1);
        assert_eq!(job.product_type_id, Some(587));
        assert_eq!(job.runs, 10);
    }

    fn job(job_id: i64, status: &str, end: &str, product: Option<i64>) -> IndustryJob {
        IndustryJob {
            job_id,
            activity_id: 1,
            blueprint_type_id: 700,
            product_type_id: product,
            status: status.into(),
            runs: 1,
            start_date: "2026-06-20T00:00:00Z".into(),
            end_date: end.into(),
            facility_id: None,
        }
    }

    #[test]
    fn summary_filters_and_orders_by_completion() {
        let now = OffsetDateTime::parse("2026-06-20T00:00:00Z", &Rfc3339).unwrap();
        let jobs = vec![
            job(1, "active", "2026-06-20T06:00:00Z", Some(587)), // 6h out
            job(2, "active", "2026-06-20T02:00:00Z", Some(588)), // 2h out (sooner)
            job(3, "delivered", "2026-06-19T00:00:00Z", Some(589)), // excluded
        ];
        let summary = summarize_jobs(&jobs, now);
        assert_eq!(summary.active_count, 2);
        // Soonest first.
        assert_eq!(summary.jobs[0].job_id, 2);
        assert_eq!(summary.jobs[0].seconds_remaining, 2 * 3600);
        assert_eq!(summary.jobs[0].display_type_id, 588);
        assert_eq!(summary.jobs[1].job_id, 1);
    }

    #[test]
    fn passed_end_time_clamps_remaining_to_zero() {
        let now = OffsetDateTime::parse("2026-06-20T12:00:00Z", &Rfc3339).unwrap();
        let jobs = vec![job(1, "ready", "2026-06-20T06:00:00Z", None)];
        let summary = summarize_jobs(&jobs, now);
        assert_eq!(summary.jobs[0].seconds_remaining, 0);
        // Falls back to blueprint id when no product.
        assert_eq!(summary.jobs[0].display_type_id, 700);
    }
}
