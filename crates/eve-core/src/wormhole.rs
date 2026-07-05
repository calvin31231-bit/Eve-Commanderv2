//! Wormhole rolling calculator.
//!
//! A wormhole has a total mass budget and a per-jump mass limit. Pushing heavy
//! ships back and forth ("rolling") collapses it. Mass shows as stable (>50% of
//! nominal remaining), reduced (10–50%), or critical (<10%), and the *nominal*
//! total carries a ±10% variance — so a hole can collapse anywhere between 90%
//! and 110% of nominal mass spent. This computes, for a given hole + rolling
//! ship, how many passes reach each stage and how many are guaranteed safe (no
//! collapse risk). Pure and unit-tested; all masses in kilograms.

use serde::{Deserialize, Serialize};

/// The result of a rolling plan for one hole + ship.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollPlan {
    /// Mass removed per pass (the ship's jump mass).
    pub per_pass_mass: f64,
    /// Passes until the hole shows "reduced" (≤50% nominal remaining).
    pub passes_to_reduced: i64,
    /// Passes until the hole shows "critical" (≤10% nominal remaining).
    pub passes_to_critical: i64,
    /// Passes you can make with no collapse risk (the hole cannot collapse
    /// before this many, even at the light end of the variance).
    pub safe_passes: i64,
    /// Earliest pass that could collapse the hole (light end, 90% of nominal).
    pub collapse_earliest: i64,
    /// Latest pass that will definitely collapse the hole (heavy end, 110%).
    pub collapse_latest: i64,
    /// Set when the ship is too heavy to use the hole at all.
    pub warning: Option<String>,
}

/// Smallest integer `k` with `k * pass >= target` (and at least 1).
fn passes_for(target: f64, pass: f64) -> i64 {
    if pass <= 0.0 {
        return 0;
    }
    (target / pass).ceil().max(1.0) as i64
}

/// Plan a roll: how many passes of a ship (`ship_pass_mass`) it takes to walk a
/// hole (`nominal_total_mass`, `max_jump_mass`) through its mass stages and
/// collapse it. Pure.
pub fn roll_plan(nominal_total_mass: f64, max_jump_mass: f64, ship_pass_mass: f64) -> RollPlan {
    let none = RollPlan {
        per_pass_mass: ship_pass_mass,
        passes_to_reduced: 0,
        passes_to_critical: 0,
        safe_passes: 0,
        collapse_earliest: 0,
        collapse_latest: 0,
        warning: None,
    };

    if ship_pass_mass <= 0.0 || nominal_total_mass <= 0.0 {
        return RollPlan { warning: Some("Enter the hole and ship masses.".into()), ..none };
    }
    if max_jump_mass > 0.0 && ship_pass_mass > max_jump_mass {
        return RollPlan {
            warning: Some("Ship exceeds the hole's per-jump mass limit — it can't pass.".into()),
            ..none
        };
    }

    let collapse_earliest = passes_for(0.9 * nominal_total_mass, ship_pass_mass);
    RollPlan {
        per_pass_mass: ship_pass_mass,
        passes_to_reduced: passes_for(0.5 * nominal_total_mass, ship_pass_mass),
        passes_to_critical: passes_for(0.9 * nominal_total_mass, ship_pass_mass),
        safe_passes: (collapse_earliest - 1).max(0),
        collapse_earliest,
        collapse_latest: passes_for(1.1 * nominal_total_mass, ship_pass_mass),
        warning: None,
    }
}

/// One annotated wormhole connection between two systems (undirected — a hole
/// is traversable both ways).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChainLink {
    pub from: String,
    pub to: String,
    /// "stable" | "destab" | "critical".
    pub mass_state: String,
    /// End-of-life (< ~4h remaining).
    pub eol: bool,
}

/// A system reachable from the chain root, with how far it is and whether the
/// shortest path to it crosses a risky (EOL or mass-critical) hole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChainNode {
    pub system: String,
    /// Wormhole jumps from the root (root itself = 0).
    pub hops: i64,
    /// The shortest path here crosses an end-of-life hole.
    pub via_eol: bool,
    /// The shortest path here crosses a mass-critical hole.
    pub via_critical: bool,
}

/// Walk the mapped wormhole connections outward from `root`, breadth-first, to
/// list every reachable system with its hop distance and whether the path to it
/// is compromised by an EOL or mass-critical hole along the way. The risk flags
/// propagate: once a path crosses a bad hole, everything beyond it inherits the
/// flag. Deterministic (systems visited in sorted order) and pure.
pub fn build_chain(root: &str, links: &[ChainLink]) -> Vec<ChainNode> {
    use std::collections::{BTreeMap, HashMap, VecDeque};

    // Undirected adjacency: system → [(neighbour, mass_state, eol)].
    let mut adj: HashMap<&str, Vec<(&str, &str, bool)>> = HashMap::new();
    for l in links {
        if l.from.is_empty() || l.to.is_empty() {
            continue;
        }
        adj.entry(&l.from).or_default().push((&l.to, &l.mass_state, l.eol));
        adj.entry(&l.to).or_default().push((&l.from, &l.mass_state, l.eol));
    }

    let mut best: BTreeMap<String, ChainNode> = BTreeMap::new();
    best.insert(
        root.to_string(),
        ChainNode { system: root.to_string(), hops: 0, via_eol: false, via_critical: false },
    );
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(root.to_string());

    while let Some(sys) = queue.pop_front() {
        let cur = best[&sys].clone();
        let mut neighbours: Vec<(&str, &str, bool)> =
            adj.get(sys.as_str()).cloned().unwrap_or_default();
        // Sorted traversal keeps the output deterministic.
        neighbours.sort_by(|a, b| a.0.cmp(b.0));
        for (to, mass_state, eol) in neighbours {
            let via_eol = cur.via_eol || eol;
            let via_critical = cur.via_critical || mass_state == "critical";
            let hops = cur.hops + 1;
            let improved = match best.get(to) {
                // First time seen, or a strictly shorter path.
                None => true,
                Some(existing) => hops < existing.hops,
                // (equal-length ties keep the first/sorted path)
            };
            if improved {
                best.insert(
                    to.to_string(),
                    ChainNode { system: to.to_string(), hops, via_eol, via_critical },
                );
                queue.push_back(to.to_string());
            }
        }
    }

    let mut out: Vec<ChainNode> = best.into_values().collect();
    out.sort_by(|a, b| a.hops.cmp(&b.hops).then(a.system.cmp(&b.system)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(from: &str, to: &str, mass: &str, eol: bool) -> ChainLink {
        ChainLink { from: from.into(), to: to.into(), mass_state: mass.into(), eol }
    }

    #[test]
    fn chain_bfs_distances_and_risk_propagation() {
        // Home — A (stable) — B (eol) — C; and Home — D (critical).
        let links = vec![
            link("Home", "A", "stable", false),
            link("A", "B", "stable", true), // EOL hole into B
            link("B", "C", "stable", false),
            link("Home", "D", "critical", false),
        ];
        let chain = build_chain("Home", &links);
        let node = |s: &str| chain.iter().find(|n| n.system == s).unwrap();

        assert_eq!(node("Home").hops, 0);
        assert_eq!(node("A").hops, 1);
        assert!(!node("A").via_eol);
        // B is reached across the EOL hole → flagged, and C inherits it.
        assert_eq!(node("B").hops, 2);
        assert!(node("B").via_eol);
        assert!(node("C").via_eol);
        assert_eq!(node("C").hops, 3);
        // D is one hop across a critical hole.
        assert!(node("D").via_critical);
        assert!(!node("D").via_eol);
        // Root first, then by hop distance.
        assert_eq!(chain[0].system, "Home");
    }

    #[test]
    fn chain_takes_shortest_path_flags() {
        // Two ways to X: Home—X direct (stable), and Home—Y(eol)—X. Shortest is
        // the direct hop, so X should not inherit the EOL flag.
        let links = vec![
            link("Home", "X", "stable", false),
            link("Home", "Y", "stable", true),
            link("Y", "X", "stable", false),
        ];
        let chain = build_chain("Home", &links);
        let x = chain.iter().find(|n| n.system == "X").unwrap();
        assert_eq!(x.hops, 1);
        assert!(!x.via_eol);
    }

    #[test]
    fn empty_chain_is_just_root() {
        let chain = build_chain("Home", &[]);
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].system, "Home");
        assert_eq!(chain[0].hops, 0);
    }

    #[test]
    fn standard_roll_stages() {
        // 2.0 Gg hole, 300 Mkg jump limit, 200 Mkg/pass battleship.
        let p = roll_plan(2_000_000_000.0, 300_000_000.0, 200_000_000.0);
        assert!(p.warning.is_none());
        assert_eq!(p.passes_to_reduced, 5); // 1.0Gg / 200Mkg
        assert_eq!(p.passes_to_critical, 9); // 1.8Gg / 200Mkg
        assert_eq!(p.collapse_earliest, 9); // light hole (90%)
        assert_eq!(p.collapse_latest, 11); // heavy hole (110%): 2.2Gg / 200Mkg
        assert_eq!(p.safe_passes, 8);
    }

    #[test]
    fn ship_too_heavy_warns() {
        let p = roll_plan(2_000_000_000.0, 300_000_000.0, 400_000_000.0);
        assert!(p.warning.is_some());
        assert_eq!(p.safe_passes, 0);
    }

    #[test]
    fn zero_inputs_are_safe() {
        let p = roll_plan(0.0, 0.0, 0.0);
        assert!(p.warning.is_some());
    }
}
