//! Neural remap optimizer.
//!
//! Given a skill plan, find the attribute allocation that trains it fastest. EVE
//! gives every attribute a base of 17 and 14 points to distribute, each attribute
//! capped at 27 (base + 10). Training rate for a skill is
//! `primary + secondary / 2` SP per minute, so the best remap depends on how much
//! of the plan's SP leans on each attribute. The search is exact (a bounded
//! brute force over integer allocations) and pure.

use serde::{Deserialize, Serialize};

/// A character training attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attr {
    Intelligence,
    Memory,
    Perception,
    Willpower,
    Charisma,
}

impl Attr {
    fn index(self) -> usize {
        match self {
            Attr::Intelligence => 0,
            Attr::Memory => 1,
            Attr::Perception => 2,
            Attr::Willpower => 3,
            Attr::Charisma => 4,
        }
    }

    /// Map an SDE character-attribute dogma id to an [`Attr`].
    pub fn from_attribute_id(id: i64) -> Option<Attr> {
        match id {
            165 => Some(Attr::Intelligence),
            166 => Some(Attr::Memory),
            167 => Some(Attr::Perception),
            168 => Some(Attr::Willpower),
            164 => Some(Attr::Charisma),
            _ => None,
        }
    }
}

/// One skill in the plan for remap purposes: SP to train and the attributes it
/// trains on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RemapSkill {
    pub sp: i64,
    pub primary: Attr,
    pub secondary: Attr,
}

/// A recommended remap and the resulting plan training time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Remap {
    pub intelligence: i64,
    pub memory: i64,
    pub perception: i64,
    pub willpower: i64,
    pub charisma: i64,
    pub train_seconds: i64,
}

const BASE: i64 = 17;
const MAX: i64 = 27;
/// Points to distribute on top of the five 17-point bases.
const POOL: i64 = 14;

/// Total training time (seconds) for a plan under an attribute allocation
/// `[int, mem, per, wil, cha]`. Pure.
pub fn plan_train_seconds(skills: &[RemapSkill], alloc: [i64; 5]) -> i64 {
    let mut total = 0.0;
    for s in skills {
        let p = alloc[s.primary.index()] as f64;
        let sec = alloc[s.secondary.index()] as f64;
        let per_min = p + sec / 2.0;
        if per_min > 0.0 {
            total += s.sp as f64 / per_min * 60.0;
        }
    }
    total.round() as i64
}

/// Find the attribute allocation that trains `skills` fastest, respecting the
/// 17–27 per-attribute bounds and the 14-point pool. Exact brute force over the
/// (bounded) integer allocations. Pure.
pub fn optimal_remap(skills: &[RemapSkill]) -> Remap {
    let mut best: Option<([i64; 5], i64)> = None;
    // Distribute POOL points across the first four attributes (0..=10 each); the
    // fifth takes the remainder if it lands in range.
    for i in 0..=POOL.min(10) {
        for m in 0..=(POOL - i).min(10) {
            for p in 0..=(POOL - i - m).min(10) {
                for w in 0..=(POOL - i - m - p).min(10) {
                    let c = POOL - i - m - p - w;
                    if !(0..=10).contains(&c) {
                        continue;
                    }
                    let alloc = [BASE + i, BASE + m, BASE + p, BASE + w, BASE + c];
                    debug_assert!(alloc.iter().all(|&v| (BASE..=MAX).contains(&v)));
                    let secs = plan_train_seconds(skills, alloc);
                    if best.map_or(true, |(_, b)| secs < b) {
                        best = Some((alloc, secs));
                    }
                }
            }
        }
    }
    let (a, secs) = best.unwrap_or(([BASE + 3, BASE + 3, BASE + 3, BASE + 3, BASE + 2], 0));
    Remap {
        intelligence: a[0],
        memory: a[1],
        perception: a[2],
        willpower: a[3],
        charisma: a[4],
        train_seconds: secs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_ids_map() {
        assert_eq!(Attr::from_attribute_id(165), Some(Attr::Intelligence));
        assert_eq!(Attr::from_attribute_id(164), Some(Attr::Charisma));
        assert_eq!(Attr::from_attribute_id(999), None);
    }

    #[test]
    fn remap_favours_the_dominant_attributes() {
        // A plan that's entirely Intelligence-primary / Memory-secondary should
        // push the pool into Int (and then Mem), not Charisma.
        let skills = vec![
            RemapSkill { sp: 1_000_000, primary: Attr::Intelligence, secondary: Attr::Memory },
            RemapSkill { sp: 1_000_000, primary: Attr::Intelligence, secondary: Attr::Memory },
        ];
        let r = optimal_remap(&skills);
        assert_eq!(r.intelligence, 27, "int should be maxed");
        assert!(r.memory > r.charisma, "mem should outrank cha");
        // Allocation always spends the whole pool.
        assert_eq!(
            r.intelligence + r.memory + r.perception + r.willpower + r.charisma,
            5 * BASE + POOL
        );
    }

    #[test]
    fn optimal_is_no_slower_than_balanced() {
        let skills = vec![
            RemapSkill { sp: 500_000, primary: Attr::Perception, secondary: Attr::Willpower },
            RemapSkill { sp: 2_000_000, primary: Attr::Intelligence, secondary: Attr::Memory },
        ];
        let balanced = plan_train_seconds(&skills, [20, 20, 20, 20, 19]);
        let r = optimal_remap(&skills);
        assert!(r.train_seconds <= balanced);
    }
}
