//! EVE Gamelog (combat) parsing → after-action / DPS summary.
//!
//! The game writes combat events to `~/Documents/EVE/logs/Gamelogs/`. Each line
//! is timestamped and wrapped in color/font markup, e.g.
//!
//! ```text
//! [ 2024.01.15 12:34:57 ] (combat) <color=0xff...><b>247</b><color=..> to <b>Guristas Wrangler</b> ... - Hits
//! ```
//!
//! We strip the markup, pull out the damage amount, direction (to = dealt, from
//! = received), the other entity, and the hit quality, then roll the events up
//! into a DPS / after-action summary. Reading game logs is an explicitly allowed
//! passive read (no memory access, no automation). Parsing is pure + tested.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::split_timestamp;

/// Whether a damage event was dealt by us or received by us.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    Dealt,
    Received,
}

/// One parsed combat damage event. (Internal — only the [`AarSummary`] crosses
/// the IPC boundary, so this carries an `OffsetDateTime` rather than a string.)
#[derive(Debug, Clone, PartialEq)]
pub struct DamageEvent {
    /// Event time (the log's local clock, read as UTC — only deltas matter).
    pub at: OffsetDateTime,
    pub amount: i64,
    pub direction: Direction,
    /// The other party (target when dealt, attacker when received).
    pub entity: String,
    /// Hit quality ("Hits", "Wrecks", "Grazes", …); empty if absent.
    pub quality: String,
}

/// Remove `<...>` markup tags from a log body.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0u32;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Parse one Gamelog line into a [`DamageEvent`], or `None` if it isn't a combat
/// damage line. Pure.
pub fn parse_combat_line(line: &str) -> Option<DamageEvent> {
    let (at, body) = split_timestamp(line)?;
    let body = body.strip_prefix("(combat)")?.trim();
    // After stripping markup: "247 to Guristas Wrangler ... - Hits".
    let text = strip_tags(body);
    let text = text.trim();

    let mut words = text.split_whitespace();
    let amount: i64 = words.next()?.parse().ok()?;
    let direction = match words.next()? {
        "to" => Direction::Dealt,
        "from" => Direction::Received,
        _ => return None,
    };

    // The remainder is "<entity ...> - <quality>"; entity is up to " - ".
    let remainder = text.split_once(if direction == Direction::Dealt { " to " } else { " from " })?.1;
    let (entity_part, quality) = match remainder.rsplit_once(" - ") {
        Some((e, q)) => (e, q.trim().to_string()),
        None => (remainder, String::new()),
    };
    let entity = clean_entity(entity_part);

    Some(DamageEvent { at, amount, direction, entity, quality })
}

/// Trim an entity blob down to a readable name: drop the bracketed corp/ship
/// suffix EVE appends, e.g. "Guristas Wrangler[GURI](Frigate)" → "Guristas
/// Wrangler".
fn clean_entity(s: &str) -> String {
    let s = s.trim();
    let cut = s.find('[').or_else(|| s.find('(')).unwrap_or(s.len());
    s[..cut].trim().to_string()
}

/// Rolled-up after-action / DPS summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AarSummary {
    pub damage_dealt: i64,
    pub damage_received: i64,
    pub duration_seconds: i64,
    pub dps_dealt: f64,
    pub dps_received: f64,
    pub event_count: usize,
    /// Entities you dealt the most damage to, highest first.
    pub top_targets: Vec<EntityDamage>,
    /// Entities that dealt the most damage to you, highest first.
    pub top_attackers: Vec<EntityDamage>,
}

/// Damage attributed to one entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityDamage {
    pub entity: String,
    pub damage: i64,
}

/// Aggregate damage events into an after-action summary. DPS uses the span from
/// the first to the last event (min 1s). Pure.
pub fn summarize_combat(events: &[DamageEvent]) -> AarSummary {
    use std::collections::HashMap;

    let mut damage_dealt = 0;
    let mut damage_received = 0;
    let mut targets: HashMap<String, i64> = HashMap::new();
    let mut attackers: HashMap<String, i64> = HashMap::new();
    let (mut first, mut last): (Option<OffsetDateTime>, Option<OffsetDateTime>) = (None, None);

    for e in events {
        match e.direction {
            Direction::Dealt => {
                damage_dealt += e.amount;
                *targets.entry(e.entity.clone()).or_insert(0) += e.amount;
            }
            Direction::Received => {
                damage_received += e.amount;
                *attackers.entry(e.entity.clone()).or_insert(0) += e.amount;
            }
        }
        first = Some(first.map_or(e.at, |f| f.min(e.at)));
        last = Some(last.map_or(e.at, |l| l.max(e.at)));
    }

    let duration_seconds = match (first, last) {
        (Some(f), Some(l)) => (l - f).whole_seconds().max(1),
        _ => 1,
    };

    let mut top_targets = to_sorted(targets);
    let mut top_attackers = to_sorted(attackers);
    top_targets.truncate(5);
    top_attackers.truncate(5);

    AarSummary {
        damage_dealt,
        damage_received,
        duration_seconds,
        dps_dealt: damage_dealt as f64 / duration_seconds as f64,
        dps_received: damage_received as f64 / duration_seconds as f64,
        event_count: events.len(),
        top_targets,
        top_attackers,
    }
}

fn to_sorted(map: std::collections::HashMap<String, i64>) -> Vec<EntityDamage> {
    let mut v: Vec<EntityDamage> = map
        .into_iter()
        .map(|(entity, damage)| EntityDamage { entity, damage })
        .collect();
    v.sort_by(|a, b| b.damage.cmp(&a.damage).then(a.entity.cmp(&b.entity)));
    v
}

/// Parse a whole Gamelog into its damage events (skipping non-combat lines).
pub fn parse_gamelog(text: &str) -> Vec<DamageEvent> {
    text.lines().filter_map(parse_combat_line).collect()
}

/// An incoming EWAR effect applied to you (tackle, web, jam, …) — the "you're
/// caught" signal read from the Gamelog.
#[derive(Debug, Clone, PartialEq)]
pub struct EwarEvent {
    pub at: OffsetDateTime,
    /// The effect phrase as EVE logs it ("Warp scramble", "Stasis Webification").
    pub effect: String,
    /// The pilot applying it.
    pub source: String,
}

/// Parse one Gamelog EWAR line — EVE's `<Effect> attempt from <Source> to you!`
/// family (warp scramble/disruption, stasis web, sensor damp, …), i.e. effects
/// applied *to you*. `None` for any other line. Pure.
pub fn parse_ewar_line(line: &str) -> Option<EwarEvent> {
    let (at, body) = split_timestamp(line)?;
    let body = body.strip_prefix("(combat)")?.trim();
    let text = strip_tags(body);
    let text = text.trim();
    let lower = text.to_lowercase();
    if !lower.contains(" attempt from ") || !lower.contains("to you") {
        return None;
    }
    let effect = text.split(" attempt").next()?.trim().to_string();
    let source = text.split(" attempt from ").nth(1)?.split(" to ").next()?.trim().to_string();
    if effect.is_empty() || source.is_empty() {
        return None;
    }
    Some(EwarEvent { at, effect: clean_entity(&effect), source: clean_entity(&source) })
}

/// All incoming-EWAR events in a Gamelog, oldest first. Pure.
pub fn parse_ewar(text: &str) -> Vec<EwarEvent> {
    text.lines().filter_map(parse_ewar_line).collect()
}

/// Which effects are hard tackle (stop you warping) vs soft. Pure.
pub fn is_hard_tackle(effect: &str) -> bool {
    let e = effect.to_lowercase();
    e.contains("scramble") || e.contains("disruption")
}

/// One pilot's contribution within a fleet after-action report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetPilot {
    pub name: String,
    pub summary: AarSummary,
}

/// A combined, multi-box fleet after-action report: each pilot's own AAR plus
/// fleet totals and combined target/attacker tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FleetAar {
    pub pilots: Vec<FleetPilot>,
    pub damage_dealt: i64,
    pub damage_received: i64,
    /// Wall-clock span of the whole engagement across all pilots' logs (min 1s).
    pub duration_seconds: i64,
    pub dps_dealt: f64,
    pub dps_received: f64,
    pub top_targets: Vec<EntityDamage>,
    pub top_attackers: Vec<EntityDamage>,
}

/// Merge several pilots' Gamelog event streams into one fleet AAR. Each pilot
/// keeps an individual summary; fleet DPS uses the engagement's overall span
/// (earliest to latest event across all pilots) so combined DPS is honest rather
/// than a sum of per-pilot rates. Pure.
pub fn merge_fleet_aar(pilots: Vec<(String, Vec<DamageEvent>)>) -> FleetAar {
    use std::collections::HashMap;

    let mut out_pilots = Vec::with_capacity(pilots.len());
    let mut damage_dealt = 0;
    let mut damage_received = 0;
    let mut targets: HashMap<String, i64> = HashMap::new();
    let mut attackers: HashMap<String, i64> = HashMap::new();
    let (mut first, mut last): (Option<OffsetDateTime>, Option<OffsetDateTime>) = (None, None);

    for (name, events) in pilots {
        for e in &events {
            match e.direction {
                Direction::Dealt => {
                    damage_dealt += e.amount;
                    *targets.entry(e.entity.clone()).or_insert(0) += e.amount;
                }
                Direction::Received => {
                    damage_received += e.amount;
                    *attackers.entry(e.entity.clone()).or_insert(0) += e.amount;
                }
            }
            first = Some(first.map_or(e.at, |f| f.min(e.at)));
            last = Some(last.map_or(e.at, |l| l.max(e.at)));
        }
        out_pilots.push(FleetPilot { name, summary: summarize_combat(&events) });
    }

    let duration_seconds = match (first, last) {
        (Some(f), Some(l)) => (l - f).whole_seconds().max(1),
        _ => 1,
    };
    let mut top_targets = to_sorted(targets);
    let mut top_attackers = to_sorted(attackers);
    top_targets.truncate(5);
    top_attackers.truncate(5);
    // Strongest contributors first.
    out_pilots.sort_by(|a, b| b.summary.damage_dealt.cmp(&a.summary.damage_dealt));

    FleetAar {
        pilots: out_pilots,
        damage_dealt,
        damage_received,
        duration_seconds,
        dps_dealt: damage_dealt as f64 / duration_seconds as f64,
        dps_received: damage_received as f64 / duration_seconds as f64,
        top_targets,
        top_attackers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEALT: &str = "[ 2024.01.15 12:34:57 ] (combat) <color=0xff...><b>247</b><color=0x77ffffff><font size=10> to </font><b><color=0xffffffff>Guristas Wrangler[GURI](Frigate)</b><font size=10><color=0x77ffffff> - Hits</font>";
    const RECV: &str = "[ 2024.01.15 12:35:07 ] (combat) <color=0xffcc0000><b>88</b><color=0x77ffffff><font size=10> from </font><b><color=0xffffffff>Guristas Wrangler</b><font size=10> - Penetrates</font>";

    const SCRAM: &str = "[ 2024.01.15 12:35:10 ] (combat) <color=0xffe57f7f><b>Warp scramble attempt</b> <color=0xFFFFFFFF>from <b><color=0xffffffff>Enemy Hunter[BAD](Sabre)</color></b> <color=0x77ffffff>to<b><color=0xffffffff> you!</color></b>";
    const WEB: &str = "[ 2024.01.15 12:35:11 ] (combat) <b>Stasis Webification attempt</b> from <b>Slower Pilot</b> to you!";

    #[test]
    fn parses_incoming_ewar_and_classifies_tackle() {
        let s = parse_ewar_line(SCRAM).unwrap();
        assert_eq!(s.effect, "Warp scramble");
        assert_eq!(s.source, "Enemy Hunter");
        assert!(is_hard_tackle(&s.effect));

        let w = parse_ewar_line(WEB).unwrap();
        assert_eq!(w.effect, "Stasis Webification");
        assert_eq!(w.source, "Slower Pilot");
        assert!(!is_hard_tackle(&w.effect)); // a web doesn't stop warp

        // A damage line is not EWAR; an EWAR line is not damage.
        assert!(parse_ewar_line(DEALT).is_none());
        assert!(parse_combat_line(SCRAM).is_none());
        assert_eq!(parse_ewar(&format!("{SCRAM}\n{WEB}\n{DEALT}")).len(), 2);
    }

    #[test]
    fn parses_dealt_and_received() {
        let d = parse_combat_line(DEALT).unwrap();
        assert_eq!(d.amount, 247);
        assert_eq!(d.direction, Direction::Dealt);
        assert_eq!(d.entity, "Guristas Wrangler");
        assert_eq!(d.quality, "Hits");

        let r = parse_combat_line(RECV).unwrap();
        assert_eq!(r.amount, 88);
        assert_eq!(r.direction, Direction::Received);
        assert_eq!(r.quality, "Penetrates");
    }

    #[test]
    fn ignores_non_combat_lines() {
        assert!(parse_combat_line("[ 2024.01.15 12:34:57 ] (notify) Some notification").is_none());
        assert!(parse_combat_line("garbage").is_none());
    }

    #[test]
    fn summarize_computes_dps_and_tops() {
        let events = parse_gamelog(&format!("{DEALT}\n{RECV}\n{DEALT}"));
        let s = summarize_combat(&events);
        assert_eq!(s.event_count, 3);
        assert_eq!(s.damage_dealt, 494); // 247 * 2
        assert_eq!(s.damage_received, 88);
        // 10s span between first and last.
        assert_eq!(s.duration_seconds, 10);
        assert!((s.dps_dealt - 49.4).abs() < 1e-9);
        assert_eq!(s.top_targets[0].entity, "Guristas Wrangler");
        assert_eq!(s.top_targets[0].damage, 494);
    }

    #[test]
    fn empty_log_is_safe() {
        let s = summarize_combat(&[]);
        assert_eq!(s.event_count, 0);
        assert_eq!(s.duration_seconds, 1);
        assert_eq!(s.dps_dealt, 0.0);
    }

    #[test]
    fn fleet_aar_combines_pilots_and_keeps_individuals() {
        let a = parse_gamelog(&format!("{DEALT}\n{RECV}"));
        let b = parse_gamelog(&format!("{DEALT}\n{DEALT}"));
        let fleet = merge_fleet_aar(vec![("Alpha".into(), a), ("Bravo".into(), b)]);
        // Combined dealt = 247 (alpha) + 494 (bravo) = 741.
        assert_eq!(fleet.damage_dealt, 741);
        assert_eq!(fleet.damage_received, 88);
        assert_eq!(fleet.pilots.len(), 2);
        // Sorted by damage dealt: Bravo (494) before Alpha (247).
        assert_eq!(fleet.pilots[0].name, "Bravo");
        assert_eq!(fleet.top_targets[0].damage, 741);
    }
}
