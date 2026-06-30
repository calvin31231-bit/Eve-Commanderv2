//! Configurable home dashboard (Phase 7): the player's saved widget layout,
//! reconciled against the widgets the build actually ships.
//!
//! The home is a grid of toggleable, reorderable widget cards. The saved layout
//! is just an ordered list of widget ids with a visible flag; this module keeps
//! it honest across versions: ids the build no longer ships are dropped, and new
//! widgets appear (visible, at the end) so an upgrade never hides a feature or
//! references a dead one. Pure + unit-tested; persistence is a JSON setting.

use serde::{Deserialize, Serialize};

/// One widget's placement in the saved layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WidgetSlot {
    pub id: String,
    pub visible: bool,
}

/// Reconcile a `saved` layout against the `available` widget ids the build
/// ships: keep saved slots whose id still exists (preserving order + visibility),
/// drop unknown ids, and append any new widget ids as visible. Pure.
pub fn reconcile(saved: &[WidgetSlot], available: &[&str]) -> Vec<WidgetSlot> {
    let mut out: Vec<WidgetSlot> = saved
        .iter()
        .filter(|s| available.contains(&s.id.as_str()))
        .cloned()
        .collect();
    for &id in available {
        if !out.iter().any(|s| s.id == id) {
            out.push(WidgetSlot { id: id.to_string(), visible: true });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str, visible: bool) -> WidgetSlot {
        WidgetSlot { id: id.into(), visible }
    }

    #[test]
    fn reconcile_preserves_order_drops_dead_and_appends_new() {
        let saved = vec![
            slot("characters", true),
            slot("networth", false), // hidden by the user — stays hidden
            slot("ancient_widget", true), // no longer shipped — dropped
        ];
        let available = ["characters", "networth", "status", "training"];
        let out = reconcile(&saved, &available);
        // Saved order preserved for surviving ids, with visibility intact.
        assert_eq!(out[0], slot("characters", true));
        assert_eq!(out[1], slot("networth", false));
        // New widgets appended, visible; the dead one is gone.
        assert!(out.iter().all(|s| s.id != "ancient_widget"));
        assert!(out.iter().any(|s| s.id == "status" && s.visible));
        assert!(out.iter().any(|s| s.id == "training" && s.visible));
        assert_eq!(out.len(), 4);
    }

    #[test]
    fn empty_saved_yields_all_visible() {
        let out = reconcile(&[], &["a", "b"]);
        assert_eq!(out, vec![slot("a", true), slot("b", true)]);
    }
}
