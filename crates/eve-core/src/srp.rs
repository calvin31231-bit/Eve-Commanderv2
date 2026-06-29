//! Ship Replacement Program (SRP) workflow.
//!
//! Members submit a loss; a reviewer approves (with a payout) or rejects;
//! approved claims are later marked paid. This module holds the claim shape, a
//! pure board summary (counts + ISK owed/paid), and a payout suggestion from a
//! simple policy. Persistence lives in `db::srp`.

use serde::{Deserialize, Serialize};

/// Claim lifecycle states.
pub const PENDING: &str = "pending";
pub const APPROVED: &str = "approved";
pub const REJECTED: &str = "rejected";
pub const PAID: &str = "paid";

/// One SRP claim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SrpClaim {
    pub id: i64,
    pub submitted_at: i64,
    pub pilot: String,
    pub ship: String,
    /// Claimed loss value (ISK).
    pub loss_value: f64,
    pub location: String,
    pub killmail_url: String,
    /// Submitter's note.
    pub notes: String,
    /// One of PENDING / APPROVED / REJECTED / PAID.
    pub status: String,
    /// Approved payout (ISK); 0 until approved.
    pub payout: f64,
    pub reviewer_note: String,
    pub decided_at: Option<i64>,
}

/// Board-level rollup of a claim queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SrpSummary {
    pub total: usize,
    pub pending: usize,
    pub approved_unpaid: usize,
    pub paid: usize,
    pub rejected: usize,
    /// Claimed loss across non-rejected claims.
    pub total_loss: f64,
    /// Approved-but-unpaid payout (what the corp currently owes).
    pub outstanding: f64,
    /// Total already paid out.
    pub total_paid: f64,
}

/// Summarize a claim queue into board stats. Pure.
pub fn summarize(claims: &[SrpClaim]) -> SrpSummary {
    let mut s = SrpSummary {
        total: claims.len(),
        pending: 0,
        approved_unpaid: 0,
        paid: 0,
        rejected: 0,
        total_loss: 0.0,
        outstanding: 0.0,
        total_paid: 0.0,
    };
    for c in claims {
        match c.status.as_str() {
            APPROVED => {
                s.approved_unpaid += 1;
                s.outstanding += c.payout;
                s.total_loss += c.loss_value;
            }
            PAID => {
                s.paid += 1;
                s.total_paid += c.payout;
                s.total_loss += c.loss_value;
            }
            REJECTED => s.rejected += 1,
            _ => {
                s.pending += 1;
                s.total_loss += c.loss_value;
            }
        }
    }
    s
}

/// Suggest a payout from a simple policy: `percent` (0–1) of the loss, optionally
/// capped at `cap` (ignored when `cap <= 0`). Pure.
pub fn suggest_payout(loss_value: f64, percent: f64, cap: f64) -> f64 {
    let raw = (loss_value.max(0.0)) * percent.clamp(0.0, 1.0);
    if cap > 0.0 {
        raw.min(cap)
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(status: &str, loss: f64, payout: f64) -> SrpClaim {
        SrpClaim {
            id: 0,
            submitted_at: 0,
            pilot: "Pilot".into(),
            ship: "Drake".into(),
            loss_value: loss,
            location: "J123456".into(),
            killmail_url: String::new(),
            notes: String::new(),
            status: status.into(),
            payout,
            reviewer_note: String::new(),
            decided_at: None,
        }
    }

    #[test]
    fn summary_counts_and_isk() {
        let claims = vec![
            claim(PENDING, 100_000_000.0, 0.0),
            claim(APPROVED, 200_000_000.0, 150_000_000.0),
            claim(PAID, 300_000_000.0, 250_000_000.0),
            claim(REJECTED, 999_000_000.0, 0.0),
        ];
        let s = summarize(&claims);
        assert_eq!(s.total, 4);
        assert_eq!(s.pending, 1);
        assert_eq!(s.approved_unpaid, 1);
        assert_eq!(s.paid, 1);
        assert_eq!(s.rejected, 1);
        // Rejected loss is excluded.
        assert_eq!(s.total_loss, 600_000_000.0);
        assert_eq!(s.outstanding, 150_000_000.0);
        assert_eq!(s.total_paid, 250_000_000.0);
    }

    #[test]
    fn payout_policy_percent_and_cap() {
        assert_eq!(suggest_payout(1_000_000_000.0, 0.8, 0.0), 800_000_000.0);
        // Capped.
        assert_eq!(suggest_payout(1_000_000_000.0, 0.8, 500_000_000.0), 500_000_000.0);
        // Percent clamps to [0,1].
        assert_eq!(suggest_payout(100.0, 2.0, 0.0), 100.0);
    }
}
