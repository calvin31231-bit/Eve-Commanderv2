//! Recruitment / HR pipeline.
//!
//! Tracks applicants through a hiring funnel — applied → interview → trial →
//! accepted / rejected — with a pure board summary (stage counts + acceptance
//! rate). Persistence lives in `db::recruit`.

use serde::{Deserialize, Serialize};

/// Pipeline stages.
pub const APPLIED: &str = "applied";
pub const INTERVIEW: &str = "interview";
pub const TRIAL: &str = "trial";
pub const ACCEPTED: &str = "accepted";
pub const REJECTED: &str = "rejected";

/// All stages a recruit may be set to, in funnel order.
pub const STAGES: &[&str] = &[APPLIED, INTERVIEW, TRIAL, ACCEPTED, REJECTED];

/// One applicant in the pipeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recruit {
    pub id: i64,
    pub applied_at: i64,
    pub name: String,
    /// Where they came from (forum, in-game, referral, …).
    pub source: String,
    pub notes: String,
    /// One of the STAGES.
    pub status: String,
    /// Who's handling the application.
    pub recruiter: String,
    pub reviewer_note: String,
    pub decided_at: Option<i64>,
}

/// Pipeline rollup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecruitSummary {
    pub total: usize,
    pub applied: usize,
    pub interview: usize,
    pub trial: usize,
    pub accepted: usize,
    pub rejected: usize,
    /// In-pipeline (applied + interview + trial).
    pub active: usize,
    /// accepted / (accepted + rejected); 0 when none decided.
    pub acceptance_rate: f64,
}

/// Summarize the recruit pipeline. Pure.
pub fn summarize(recruits: &[Recruit]) -> RecruitSummary {
    let mut s = RecruitSummary {
        total: recruits.len(),
        applied: 0,
        interview: 0,
        trial: 0,
        accepted: 0,
        rejected: 0,
        active: 0,
        acceptance_rate: 0.0,
    };
    for r in recruits {
        match r.status.as_str() {
            INTERVIEW => {
                s.interview += 1;
                s.active += 1;
            }
            TRIAL => {
                s.trial += 1;
                s.active += 1;
            }
            ACCEPTED => s.accepted += 1,
            REJECTED => s.rejected += 1,
            _ => {
                s.applied += 1;
                s.active += 1;
            }
        }
    }
    let decided = s.accepted + s.rejected;
    if decided > 0 {
        s.acceptance_rate = s.accepted as f64 / decided as f64;
    }
    s
}

/// Whether `status` is a valid pipeline stage. Pure.
pub fn is_stage(status: &str) -> bool {
    STAGES.contains(&status)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(status: &str) -> Recruit {
        Recruit {
            id: 0,
            applied_at: 0,
            name: "Applicant".into(),
            source: "forum".into(),
            notes: String::new(),
            status: status.into(),
            recruiter: String::new(),
            reviewer_note: String::new(),
            decided_at: None,
        }
    }

    #[test]
    fn pipeline_counts_and_acceptance_rate() {
        let recruits = vec![
            r(APPLIED),
            r(INTERVIEW),
            r(TRIAL),
            r(ACCEPTED),
            r(ACCEPTED),
            r(REJECTED),
        ];
        let s = summarize(&recruits);
        assert_eq!(s.total, 6);
        assert_eq!(s.active, 3); // applied + interview + trial
        assert_eq!(s.accepted, 2);
        assert_eq!(s.rejected, 1);
        // 2 accepted of 3 decided.
        assert!((s.acceptance_rate - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn no_decisions_means_zero_rate() {
        let s = summarize(&[r(APPLIED), r(INTERVIEW)]);
        assert_eq!(s.acceptance_rate, 0.0);
    }

    #[test]
    fn stage_validation() {
        assert!(is_stage("trial"));
        assert!(!is_stage("banished"));
    }
}
