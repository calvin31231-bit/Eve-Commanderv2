//! Alliance-tournament / competitive team-comp validator.
//!
//! AT formats cap a team by pilot count and a **ship-points budget** (each hull
//! costs points from a published list; the whole team must fit under the cap).
//! Captains track this in spreadsheets today; this replaces that with an exact,
//! instant legality check. Points per hull change yearly and are supplied by the
//! caller (from the current rules), so this stays correct across formats. The
//! reduction is pure and unit-tested.

use serde::{Deserialize, Serialize};

/// One pilot's pick in a proposed team composition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TeamPick {
    pub pilot: String,
    pub ship: String,
    /// Point cost of the hull under the active rules.
    pub points: i64,
}

/// The active format's limits.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TeamRules {
    /// Maximum total ship points for the team.
    pub point_budget: i64,
    /// Maximum number of pilots fielded.
    pub max_pilots: i64,
    /// Maximum copies of any one hull (0 = unlimited).
    pub max_per_hull: i64,
}

/// The legality verdict for a proposed comp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompReport {
    pub total_points: i64,
    pub pilot_count: i64,
    /// Points still available (negative when over budget).
    pub remaining_points: i64,
    pub over_budget: bool,
    pub over_size: bool,
    /// Pilots listed more than once (a pilot can't fly two ships).
    pub duplicate_pilots: Vec<String>,
    /// Hulls fielded more than `max_per_hull` times.
    pub over_stacked_hulls: Vec<String>,
    /// True only when nothing is violated.
    pub legal: bool,
}

/// Check a proposed comp against the format rules. Pure.
pub fn validate_comp(picks: &[TeamPick], rules: TeamRules) -> CompReport {
    use std::collections::HashMap;

    let total_points: i64 = picks.iter().map(|p| p.points.max(0)).sum();
    let pilot_count = picks.len() as i64;

    // Duplicate pilots (case-insensitive), preserving first-seen order.
    let mut pilot_seen: HashMap<String, i64> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in picks {
        let key = p.pilot.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        let e = pilot_seen.entry(key.clone()).or_insert(0);
        if *e == 0 {
            order.push(p.pilot.trim().to_string());
        }
        *e += 1;
    }
    let duplicate_pilots: Vec<String> = order
        .into_iter()
        .filter(|name| pilot_seen[&name.to_lowercase()] > 1)
        .collect();

    // Over-stacked hulls.
    let mut hull_count: HashMap<String, i64> = HashMap::new();
    let mut hull_order: Vec<String> = Vec::new();
    for p in picks {
        let key = p.ship.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        let e = hull_count.entry(key.clone()).or_insert(0);
        if *e == 0 {
            hull_order.push(p.ship.trim().to_string());
        }
        *e += 1;
    }
    let over_stacked_hulls: Vec<String> = if rules.max_per_hull > 0 {
        hull_order
            .into_iter()
            .filter(|name| hull_count[&name.to_lowercase()] > rules.max_per_hull)
            .collect()
    } else {
        Vec::new()
    };

    let over_budget = total_points > rules.point_budget;
    let over_size = pilot_count > rules.max_pilots;
    let legal = !over_budget
        && !over_size
        && duplicate_pilots.is_empty()
        && over_stacked_hulls.is_empty();

    CompReport {
        total_points,
        pilot_count,
        remaining_points: rules.point_budget - total_points,
        over_budget,
        over_size,
        duplicate_pilots,
        over_stacked_hulls,
        legal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(pilot: &str, ship: &str, points: i64) -> TeamPick {
        TeamPick { pilot: pilot.into(), ship: ship.into(), points }
    }

    fn rules() -> TeamRules {
        TeamRules { point_budget: 100, max_pilots: 12, max_per_hull: 2 }
    }

    #[test]
    fn legal_comp_passes() {
        let picks = vec![
            pick("Alice", "Rook", 19),
            pick("Bob", "Guardian", 17),
            pick("Carol", "Vindicator", 18),
        ];
        let r = validate_comp(&picks, rules());
        assert_eq!(r.total_points, 54);
        assert_eq!(r.remaining_points, 46);
        assert_eq!(r.pilot_count, 3);
        assert!(r.legal);
    }

    #[test]
    fn over_budget_is_flagged() {
        let picks = vec![pick("Alice", "Nightmare", 60), pick("Bob", "Nightmare", 60)];
        let r = validate_comp(&picks, rules());
        assert!(r.over_budget);
        assert_eq!(r.remaining_points, -20);
        assert!(!r.legal);
    }

    #[test]
    fn duplicate_pilot_and_over_stacked_hull() {
        let picks = vec![
            pick("Alice", "Ishtar", 15),
            pick("alice", "Guardian", 17), // same pilot, different case
            pick("Bob", "Ishtar", 15),
            pick("Carol", "Ishtar", 15), // three Ishtars, max 2
        ];
        let r = validate_comp(&picks, rules());
        assert_eq!(r.duplicate_pilots, vec!["Alice"]);
        assert_eq!(r.over_stacked_hulls, vec!["Ishtar"]);
        assert!(!r.legal);
    }

    #[test]
    fn over_size_is_flagged() {
        let rules = TeamRules { point_budget: 1000, max_pilots: 2, max_per_hull: 0 };
        let picks = vec![
            pick("A", "Rifter", 5),
            pick("B", "Rifter", 5),
            pick("C", "Rifter", 5),
        ];
        let r = validate_comp(&picks, rules);
        assert!(r.over_size);
        assert!(!r.legal);
        // max_per_hull 0 = unlimited, so three Rifters is fine on that axis.
        assert!(r.over_stacked_hulls.is_empty());
    }
}
