//! Opt-in telemetry (Phase 7): anonymous, off by default, and structurally
//! incapable of carrying personal game data.
//!
//! The trust rule (see `docs/ROADMAP.md`, "all data local by default"): events
//! are **just a name plus optional small integer counts** — never ISK, names,
//! character/corp ids, system names, or free text. [`TelemetryEvent::new`]
//! enforces that shape; there is no field for prose, so a wallet balance or a
//! pilot name simply has nowhere to go. Nothing is recorded unless the player
//! turns it on. Pure + unit-tested; the (optional) network flush lives elsewhere.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One anonymous usage event: a stable name and bounded integer dimensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// A fixed, code-defined event name (e.g. `"feature_open"`), lowercased.
    pub name: String,
    /// Small non-negative counters only (e.g. `{"hub": 3}`). No identifiers.
    pub counts: BTreeMap<String, u32>,
}

impl TelemetryEvent {
    /// Build an event, sanitizing the name to `[a-z0-9_]` and clamping each
    /// count to a safe ceiling so a stray large number can't smuggle data out.
    pub fn new(name: &str, counts: &[(&str, u32)]) -> Self {
        let clean = |s: &str| -> String {
            s.trim()
                .to_lowercase()
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
                .collect()
        };
        const CEIL: u32 = 100_000;
        let counts = counts.iter().map(|(k, v)| (clean(k), (*v).min(CEIL))).collect();
        Self { name: clean(name), counts }
    }
}

/// Whether telemetry should record anything, given the player's consent. The
/// single gate every recording path checks. Pure.
pub fn should_record(consent: bool) -> bool {
    consent
}

/// A bounded batch of pending events: when over `cap`, the oldest are dropped so
/// the local buffer can never grow without limit. Pure — returns the events to
/// keep (most recent `cap`).
pub fn trim_batch(events: Vec<TelemetryEvent>, cap: usize) -> Vec<TelemetryEvent> {
    let len = events.len();
    if len <= cap {
        return events;
    }
    events.into_iter().skip(len - cap).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_sanitizes_name_and_clamps_counts() {
        let e = TelemetryEvent::new("Feature Open!", &[("Hub-Id", 3), ("huge", 9_999_999)]);
        assert_eq!(e.name, "feature_open_");
        assert_eq!(e.counts.get("hub_id"), Some(&3));
        // The oversized count is clamped, never passed through verbatim.
        assert_eq!(e.counts.get("huge"), Some(&100_000));
    }

    #[test]
    fn consent_gates_recording() {
        assert!(!should_record(false));
        assert!(should_record(true));
    }

    #[test]
    fn batch_trims_to_cap_keeping_recent() {
        let evs: Vec<TelemetryEvent> =
            (0..10).map(|i| TelemetryEvent::new(&format!("e{i}"), &[])).collect();
        let kept = trim_batch(evs, 3);
        assert_eq!(kept.len(), 3);
        assert_eq!(kept[0].name, "e7");
        assert_eq!(kept[2].name, "e9");
    }
}
