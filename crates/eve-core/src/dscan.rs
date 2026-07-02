//! Directional-scan (D-scan) clipboard parsing.
//!
//! In the game, the directional scanner's "Copy to clipboard" yields one
//! tab-separated row per contact: `id \t name \t typeName \t distance`. Pasting
//! that here turns a wall of rows into a glanceable readout — contacts grouped
//! by type with counts, plus the danger signals a pilot cares about (combat
//! scanner probes = you may be hunted; tackle/bomber/recon hulls on grid).
//!
//! Parsing is pure and unit-tested; it needs no SDE (it reads the type names the
//! client already provides) so it works offline and instantly.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One parsed D-scan contact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DscanContact {
    pub type_name: String,
    /// Distance text as the client formats it ("14 km", "1.4 AU", "-").
    pub distance: String,
}

/// A type grouped with how many appeared on scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DscanGroup {
    pub type_name: String,
    pub count: i64,
}

/// The reduced D-scan readout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DscanResult {
    pub total: i64,
    /// Distinct types, most numerous first.
    pub groups: Vec<DscanGroup>,
    /// Danger callouts derived from the contact types.
    pub warnings: Vec<String>,
}

/// Type-name fragments that warrant a warning, with the message to show. Matched
/// case-insensitively as substrings so faction/variant prefixes still trigger.
const DANGER_SIGNS: &[(&str, &str)] = &[
    ("Combat Scanner Probe", "Combat scanner probes on scan — you may be getting hunted."),
    ("Mobile Cynosural", "Cyno on scan — possible hot-drop."),
    ("Mobile Warp Disruptor", "Warp-disruption bubble on scan."),
];

/// Hull-name fragments that signal a specific threat, grouped under one warning.
const THREAT_HULLS: &[(&str, &str)] = &[
    ("Sabre", "interdictor"),
    ("Flycatcher", "interdictor"),
    ("Heretic", "interdictor"),
    ("Eris", "interdictor"),
    ("Onyx", "heavy interdictor"),
    ("Broadsword", "heavy interdictor"),
    ("Phobos", "heavy interdictor"),
    ("Devoter", "heavy interdictor"),
];

/// Parse D-scan clipboard text into a grouped readout with danger callouts.
/// Tolerates blank lines and rows with missing columns. Pure.
pub fn parse_dscan(text: &str) -> DscanResult {
    let mut counts: HashMap<String, i64> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut total = 0;

    for line in text.lines() {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            continue;
        }
        // Columns are tab-separated: id, name, typeName, distance. The type name
        // is the 3rd column; fall back to the last non-empty column if the row
        // is shaped unexpectedly.
        let cols: Vec<&str> = line.split('\t').collect();
        let type_name = match cols.as_slice() {
            [_, _, ty, ..] if !ty.trim().is_empty() => ty.trim().to_string(),
            _ => match cols.iter().rev().find(|c| !c.trim().is_empty()) {
                Some(c) => c.trim().to_string(),
                None => continue,
            },
        };
        total += 1;
        if !counts.contains_key(&type_name) {
            order.push(type_name.clone());
        }
        *counts.entry(type_name).or_insert(0) += 1;
    }

    let mut groups: Vec<DscanGroup> = order
        .into_iter()
        .map(|type_name| {
            let count = counts[&type_name];
            DscanGroup { type_name, count }
        })
        .collect();
    // Most numerous first; stable by name for ties.
    groups.sort_by(|a, b| b.count.cmp(&a.count).then(a.type_name.cmp(&b.type_name)));

    let warnings = derive_warnings(&groups);
    DscanResult { total, groups, warnings }
}

/// Build the danger callouts from grouped contacts.
fn derive_warnings(groups: &[DscanGroup]) -> Vec<String> {
    let mut warnings = Vec::new();

    for (fragment, message) in DANGER_SIGNS {
        let n: i64 = groups
            .iter()
            .filter(|g| contains_ci(&g.type_name, fragment))
            .map(|g| g.count)
            .sum();
        if n > 0 {
            warnings.push(message.to_string());
        }
    }

    // Collect any threat hulls present into a single line.
    let mut hull_hits: Vec<String> = Vec::new();
    for (hull, role) in THREAT_HULLS {
        let n: i64 = groups
            .iter()
            .filter(|g| contains_ci(&g.type_name, hull))
            .map(|g| g.count)
            .sum();
        if n > 0 {
            hull_hits.push(format!("{n}× {hull} ({role})"));
        }
    }
    if !hull_hits.is_empty() {
        warnings.push(format!("Tackle on scan: {}", hull_hits.join(", ")));
    }

    warnings
}

/// Case-insensitive substring test without allocating for the common path.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// What changed between two scans of the same spot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DscanDiff {
    /// Types with more contacts than before (count = how many more).
    pub appeared: Vec<DscanGroup>,
    /// Types with fewer contacts than before (count = how many fewer).
    pub disappeared: Vec<DscanGroup>,
    /// Danger callouts for what just arrived.
    pub warnings: Vec<String>,
}

/// Diff two D-scan pastes: which types appeared or disappeared (per-type count
/// deltas), with danger callouts derived from the newcomers — the hunter's
/// "what just landed on grid" readout. Pure.
pub fn diff_scans(previous: &str, current: &str) -> DscanDiff {
    let old = parse_dscan(previous);
    let new = parse_dscan(current);
    let count = |groups: &[DscanGroup], name: &str| -> i64 {
        groups.iter().find(|g| g.type_name == name).map(|g| g.count).unwrap_or(0)
    };

    let mut appeared: Vec<DscanGroup> = Vec::new();
    for g in &new.groups {
        let delta = g.count - count(&old.groups, &g.type_name);
        if delta > 0 {
            appeared.push(DscanGroup { type_name: g.type_name.clone(), count: delta });
        }
    }
    let mut disappeared: Vec<DscanGroup> = Vec::new();
    for g in &old.groups {
        let delta = g.count - count(&new.groups, &g.type_name);
        if delta > 0 {
            disappeared.push(DscanGroup { type_name: g.type_name.clone(), count: delta });
        }
    }
    appeared.sort_by(|a, b| b.count.cmp(&a.count).then(a.type_name.cmp(&b.type_name)));
    disappeared.sort_by(|a, b| b.count.cmp(&a.count).then(a.type_name.cmp(&b.type_name)));

    let warnings = derive_warnings(&appeared);
    DscanDiff { appeared, disappeared, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real D-scan rows are tab-separated id/name/type/distance.
    const SCAN: &str = "\
1001\tTackle Alt\tSabre\t14 km
1002\tHunter\tLoki\t1.2 AU
1003\tScout\tLoki\t1.2 AU
1004\tProbe\tSisters Combat Scanner Probe\t-
1005\t\tShuttle\t500 km";

    #[test]
    fn groups_by_type_and_counts() {
        let r = parse_dscan(SCAN);
        assert_eq!(r.total, 5);
        // Loki ×2 is the largest group and sorts first.
        assert_eq!(r.groups[0], DscanGroup { type_name: "Loki".into(), count: 2 });
        assert!(r.groups.iter().any(|g| g.type_name == "Sabre" && g.count == 1));
    }

    #[test]
    fn flags_combat_probes_and_tackle() {
        let r = parse_dscan(SCAN);
        assert!(r.warnings.iter().any(|w| w.contains("hunted")));
        assert!(r.warnings.iter().any(|w| w.contains("Sabre") && w.contains("interdictor")));
    }

    #[test]
    fn empty_or_blank_is_zeroed() {
        let r = parse_dscan("\n  \n");
        assert_eq!(r.total, 0);
        assert!(r.groups.is_empty());
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn tolerates_short_rows() {
        // A row with only a type name still counts.
        let r = parse_dscan("Capsule");
        assert_eq!(r.total, 1);
        assert_eq!(r.groups[0].type_name, "Capsule");
    }

    #[test]
    fn diff_reports_arrivals_departures_and_new_threats() {
        let before = "1\tA\tLoki\t1 AU\n2\tB\tLoki\t1 AU\n3\tC\tShuttle\t- ";
        let after = "1\tA\tLoki\t1 AU\n4\tD\tSabre\t14 km\n5\tE\tSabre\t14 km";
        let d = diff_scans(before, after);
        // Two Sabres arrived; one Loki and the Shuttle left.
        assert_eq!(d.appeared, vec![DscanGroup { type_name: "Sabre".into(), count: 2 }]);
        assert!(d.disappeared.iter().any(|g| g.type_name == "Loki" && g.count == 1));
        assert!(d.disappeared.iter().any(|g| g.type_name == "Shuttle" && g.count == 1));
        // The arrivals drive the warnings (interdictors on grid).
        assert!(d.warnings.iter().any(|w| w.contains("Sabre")));
        // Identical scans → clean diff.
        let clean = diff_scans(after, after);
        assert!(clean.appeared.is_empty() && clean.disappeared.is_empty());
    }
}
