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

/// Relevance of a memory note to a query, for retrieval-augmented recall. Blends
/// keyword overlap with the query, the note's salience, and its recency — the
/// structured + recency×importance half of the plan's hybrid retrieval (the
/// vector-similarity half needs an embeddings model). Range ~0..=1. Pure.
pub fn relevance(query: &str, haystack: &str, salience: f64, updated_at: i64, now: i64) -> f64 {
    let q = query.to_lowercase();
    let tokens: Vec<&str> = q.split(|c: char| !c.is_alphanumeric()).filter(|t| t.len() >= 3).collect();
    let hay = haystack.to_lowercase();
    let overlap = if tokens.is_empty() {
        0.0
    } else {
        let hits = tokens.iter().filter(|t| hay.contains(**t)).count();
        hits as f64 / tokens.len() as f64
    };
    let age_days = ((now - updated_at).max(0) as f64) / 86_400.0;
    let recency = 1.0 / (1.0 + age_days / 30.0);
    0.5 * overlap + 0.3 * salience.clamp(0.0, 1.0) + 0.2 * recency
}

/// Cosine similarity of two equal-length vectors, in `-1..=1` (0 when either is
/// empty, length-mismatched, or zero-norm). Pure — the vector half of recall.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += (*x as f64) * (*y as f64);
        na += (*x as f64) * (*x as f64);
        nb += (*y as f64) * (*y as f64);
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Blend the keyword/recency [`relevance`] score with vector cosine similarity
/// into one hybrid recall score — the plan's "vector similarity + structured +
/// recency×importance" retrieval. `cosine` is expected in `0..=1` (negative
/// similarities are clamped to 0). When no embedding is available pass
/// `cosine = None` and the score falls back to the keyword half. Pure.
pub fn hybrid_relevance(keyword: f64, cosine: Option<f64>) -> f64 {
    match cosine {
        Some(c) => 0.5 * keyword + 0.5 * c.clamp(0.0, 1.0),
        None => keyword,
    }
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
    fn relevance_rewards_query_overlap() {
        let now = 100 * 86_400;
        // A note matching the query outranks an unrelated one of equal salience.
        let hit = relevance("training toward a carrier", "Goal: train toward a Nyx carrier", 0.8, now, now);
        let miss = relevance("training toward a carrier", "Prefers buy orders in Jita", 0.8, now, now);
        assert!(hit > miss);
        // Empty query falls back to salience + recency (no panic, finite).
        let base = relevance("", "anything", 0.5, now, now);
        assert!(base > 0.0 && base.is_finite());
    }

    #[test]
    fn cosine_and_hybrid_blend() {
        // Identical direction → 1.0; orthogonal → 0.0; mismatched/empty → 0.0.
        assert!((cosine_similarity(&[1.0, 0.0], &[2.0, 0.0]) - 1.0).abs() < 1e-9);
        assert!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-9);
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
        assert_eq!(cosine_similarity(&[], &[]), 0.0);

        // Hybrid: with a strong cosine the blended score beats keyword alone;
        // without an embedding it falls back to the keyword score exactly.
        assert!(hybrid_relevance(0.2, Some(1.0)) > 0.2);
        assert_eq!(hybrid_relevance(0.42, None), 0.42);
        // Negative similarity is clamped, never dragging the score below half-keyword.
        assert!((hybrid_relevance(0.4, Some(-0.9)) - 0.2).abs() < 1e-9);
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
