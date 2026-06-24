//! Durable AI memory: the small, curated set of facts the assistant keeps about
//! *you* (goals, preferences, decisions, relationships) so it grows richer over
//! months without hoarding game data.
//!
//! Per the plan (Phase 6.5, "AI Memory, Compression & Recall"):
//! - **Retrieve, don't restate.** Bulk game state stays in the relational DB; the
//!   memory store holds only distilled, durable notes — so it stays tiny.
//! - **Salience on write.** Goals/decisions/corrections score high; chit-chat
//!   scores low and isn't persisted.
//! - **Bounded growth.** A storage cap evicts by importance × recency (LRU),
//!   except pinned notes, which never evict.
//!
//! The scoring and eviction policy here are pure and unit-tested; persistence
//! lives in `db::ai_memory`.

use serde::{Deserialize, Serialize};

/// What a memory note is about — drives its baseline salience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// A long-term objective ("training toward a carrier").
    Goal,
    /// A decision the player made or the assistant helped reach.
    Decision,
    /// A correction the player gave the assistant — high value, must stick.
    Correction,
    /// A stable preference ("avoids lowsec", "prefers buy orders").
    Preference,
    /// A relationship/standing note (a corp, ally, known hostile).
    Relationship,
    /// Low-value conversational context.
    Chitchat,
}

impl MemoryKind {
    /// Baseline salience for the kind, before length/keyword adjustments.
    fn base(self) -> f64 {
        match self {
            MemoryKind::Correction => 0.9,
            MemoryKind::Goal => 0.8,
            MemoryKind::Decision => 0.7,
            MemoryKind::Preference => 0.65,
            MemoryKind::Relationship => 0.6,
            MemoryKind::Chitchat => 0.1,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MemoryKind::Goal => "goal",
            MemoryKind::Decision => "decision",
            MemoryKind::Correction => "correction",
            MemoryKind::Preference => "preference",
            MemoryKind::Relationship => "relationship",
            MemoryKind::Chitchat => "chitchat",
        }
    }
}

/// The salience threshold a note must clear to be worth persisting.
pub const PERSIST_THRESHOLD: f64 = 0.4;

/// Score how worth-remembering a note is, in `0.0..=1.0`. Pure. The kind sets a
/// baseline; a substantive body nudges up, an empty one down.
pub fn salience_score(kind: MemoryKind, body: &str) -> f64 {
    let mut s = kind.base();
    let len = body.trim().len();
    if len == 0 {
        return 0.0;
    }
    if len < 8 {
        s -= 0.2;
    } else if len > 40 {
        s += 0.05;
    }
    s.clamp(0.0, 1.0)
}

/// Whether a note of this salience should be written to durable memory. Pure.
pub fn should_persist(salience: f64) -> bool {
    salience >= PERSIST_THRESHOLD
}

/// The minimal note shape the eviction policy needs. (The DB row carries more.)
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryRef {
    pub id: i64,
    pub salience: f64,
    /// Last-access/update time (epoch secs); higher = more recent.
    pub updated_at: i64,
    /// Pinned notes are never evicted.
    pub pinned: bool,
}

/// Combined importance × recency rank used for eviction. Higher is more
/// worth-keeping. Recency is a gentle log-style decay so a slightly-less-salient
/// but fresh note can outrank a stale important one. Pure.
fn keep_rank(n: &MemoryRef, now: i64) -> f64 {
    let age_days = ((now - n.updated_at).max(0) as f64) / 86_400.0;
    // Recency factor in (0,1]: 1.0 today, ~0.5 at ~30 days, decaying further.
    let recency = 1.0 / (1.0 + age_days / 30.0);
    n.salience * recency
}

/// Choose which notes to evict to bring the store down to `cap` notes. Pinned
/// notes never evict and don't count toward the cap pressure beyond occupying a
/// slot. Returns the ids to remove, lowest keep-rank first. Pure.
pub fn evict_candidates(notes: &[MemoryRef], cap: usize, now: i64) -> Vec<i64> {
    if notes.len() <= cap {
        return Vec::new();
    }
    // Only unpinned notes are eligible.
    let mut evictable: Vec<&MemoryRef> = notes.iter().filter(|n| !n.pinned).collect();
    let need = notes.len() - cap;
    // Sort by keep-rank ascending (worst first).
    evictable.sort_by(|a, b| {
        keep_rank(a, now)
            .partial_cmp(&keep_rank(b, now))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    evictable.into_iter().take(need).map(|n| n.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections_and_goals_outscore_chitchat() {
        let goal = salience_score(MemoryKind::Goal, "Training toward a Nyx over the next year");
        let chat = salience_score(MemoryKind::Chitchat, "lol nice");
        assert!(goal > chat);
        assert!(should_persist(goal));
        assert!(!should_persist(chat));
    }

    #[test]
    fn empty_body_scores_zero() {
        assert_eq!(salience_score(MemoryKind::Goal, "   "), 0.0);
        assert!(!should_persist(0.0));
    }

    #[test]
    fn no_eviction_under_cap() {
        let notes = vec![
            MemoryRef { id: 1, salience: 0.9, updated_at: 100, pinned: false },
            MemoryRef { id: 2, salience: 0.5, updated_at: 100, pinned: false },
        ];
        assert!(evict_candidates(&notes, 5, 1_000).is_empty());
    }

    #[test]
    fn evicts_lowest_rank_first_and_spares_pinned() {
        let now = 100 * 86_400;
        let notes = vec![
            // Low salience + old → worst.
            MemoryRef { id: 1, salience: 0.3, updated_at: 0, pinned: false },
            // High salience, old, but pinned → must survive.
            MemoryRef { id: 2, salience: 0.95, updated_at: 0, pinned: true },
            // Mid salience, fresh → keep.
            MemoryRef { id: 3, salience: 0.6, updated_at: now, pinned: false },
            // Low-ish, old → second worst.
            MemoryRef { id: 4, salience: 0.45, updated_at: 0, pinned: false },
        ];
        // Cap of 2 over 4 notes → evict 2, but pinned never goes.
        let evicted = evict_candidates(&notes, 2, now);
        assert_eq!(evicted.len(), 2);
        assert!(evicted.contains(&1));
        assert!(!evicted.contains(&2)); // pinned spared
        assert!(!evicted.contains(&3)); // freshest kept
    }
}
