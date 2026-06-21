//! Calm notifications: a collecting notification center with severity tiers.
//!
//! The product principle (see ROADMAP "Feels-fast + calm notifications") is to
//! **collect rather than interrupt**: low-severity events accumulate in a center
//! the user can glance at, while only events at or above a configurable
//! threshold raise an OS toast. Events are de-duplicated by a stable `key`, so a
//! condition that persists across many poll ticks (low fuel, empty skill queue)
//! produces a single, updatable entry rather than a storm.
//!
//! This module is pure and unit-tested; the desktop shell renders the center in
//! the Alerts rail and dispatches OS toasts for the entries that ask to
//! interrupt.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Severity tiers, ordered `Info < Warning < Critical` (the derived `Ord`
/// follows declaration order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    /// Stable string form (for persistence / UI).
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "Info",
            Severity::Warning => "Warning",
            Severity::Critical => "Critical",
        }
    }

    /// Parse from [`as_str`](Self::as_str); unknown values fall back to Warning.
    pub fn parse(s: &str) -> Severity {
        match s {
            "Info" => Severity::Info,
            "Critical" => Severity::Critical,
            _ => Severity::Warning,
        }
    }
}

/// A single notification. De-duplication is by [`key`](Self::key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    /// Stable de-dup key, e.g. `"fuel:1029384756"` or `"skillqueue:90000001"`.
    pub key: String,
    pub title: String,
    pub body: String,
    pub severity: Severity,
    /// Coarse grouping for the UI (e.g. `"industry"`, `"intel"`, `"fuel"`).
    pub category: String,
    /// Unix epoch seconds when first raised (or last updated).
    pub created_at: u64,
    pub read: bool,
}

impl Notification {
    /// Build a notification stamped `now`.
    pub fn new(
        key: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
        severity: Severity,
        category: impl Into<String>,
        now: SystemTime,
    ) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            body: body.into(),
            severity,
            category: category.into(),
            created_at: to_epoch(now),
            read: false,
        }
    }
}

/// What happened when a notification was pushed into the center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushStatus {
    /// First time we've seen this key.
    Created,
    /// Same key, but the content changed (e.g. fuel went from low to empty).
    Updated,
    /// Same key, identical content — collapsed, nothing changed.
    Duplicate,
}

/// Result of a push: what changed and whether the shell should raise an OS toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushResult {
    pub status: PushStatus,
    /// True iff the entry is new-or-changed AND meets the interrupt threshold.
    pub interrupt: bool,
}

/// Collects notifications, de-duplicating by key, and decides which ones are
/// allowed to interrupt with an OS toast.
#[derive(Debug, Clone)]
pub struct NotificationCenter {
    items: Vec<Notification>,
    /// Minimum severity that may raise an OS toast; below this, entries are
    /// collected silently.
    min_interrupt: Severity,
}

impl NotificationCenter {
    /// Create a center that only toasts at or above `min_interrupt`.
    pub fn new(min_interrupt: Severity) -> Self {
        Self {
            items: Vec::new(),
            min_interrupt,
        }
    }

    /// Change the interrupt threshold (user setting).
    pub fn set_min_interrupt(&mut self, min_interrupt: Severity) {
        self.min_interrupt = min_interrupt;
    }

    /// Push a notification, collapsing by key. Returns what changed and whether
    /// the shell should raise an OS toast.
    pub fn push(&mut self, notification: Notification) -> PushResult {
        let meets_threshold = notification.severity >= self.min_interrupt;

        if let Some(existing) = self.items.iter_mut().find(|n| n.key == notification.key) {
            // Compare on the meaningful content, ignoring read-state/timestamp.
            let changed = existing.title != notification.title
                || existing.body != notification.body
                || existing.severity != notification.severity;
            if !changed {
                return PushResult {
                    status: PushStatus::Duplicate,
                    interrupt: false,
                };
            }
            existing.title = notification.title;
            existing.body = notification.body;
            existing.severity = notification.severity;
            existing.category = notification.category;
            existing.created_at = notification.created_at;
            existing.read = false; // a changed condition is unread again
            return PushResult {
                status: PushStatus::Updated,
                interrupt: meets_threshold,
            };
        }

        self.items.push(notification);
        PushResult {
            status: PushStatus::Created,
            interrupt: meets_threshold,
        }
    }

    /// Number of unread notifications (drives the rail badge).
    pub fn unread_count(&self) -> usize {
        self.items.iter().filter(|n| !n.read).count()
    }

    /// Total collected notifications.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// All notifications, most recent first.
    pub fn list(&self) -> Vec<Notification> {
        let mut out = self.items.clone();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        out
    }

    /// Mark every notification read.
    pub fn mark_all_read(&mut self) {
        for n in &mut self.items {
            n.read = true;
        }
    }

    /// Mark one notification read by key; returns whether it was found.
    pub fn mark_read(&mut self, key: &str) -> bool {
        if let Some(n) = self.items.iter_mut().find(|n| n.key == key) {
            n.read = true;
            true
        } else {
            false
        }
    }

    /// Remove a notification by key (e.g. the condition cleared).
    pub fn dismiss(&mut self, key: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|n| n.key != key);
        self.items.len() != before
    }

    /// Drop every notification.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

impl Default for NotificationCenter {
    fn default() -> Self {
        // Collect everything; interrupt on Warning and above.
        Self::new(Severity::Warning)
    }
}

/// Rule: structure fuel. Critical once empty, Warning within `warn_within` of
/// expiry, otherwise nothing. The key collapses repeated ticks for one
/// structure into a single entry.
pub fn fuel_alert(
    structure_id: i64,
    structure_name: &str,
    fuel_expires_at: SystemTime,
    now: SystemTime,
    warn_within: Duration,
) -> Option<Notification> {
    let key = format!("fuel:{structure_id}");
    if fuel_expires_at <= now {
        Some(Notification::new(
            key,
            "Structure out of fuel",
            format!("{structure_name} has run out of fuel."),
            Severity::Critical,
            "fuel",
            now,
        ))
    } else if fuel_expires_at.duration_since(now).unwrap_or_default() <= warn_within {
        Some(Notification::new(
            key,
            "Structure fuel low",
            format!("{structure_name} is low on fuel."),
            Severity::Warning,
            "fuel",
            now,
        ))
    } else {
        None
    }
}

/// Rule: skill queue. Warning if empty, Info if the last skill finishes within
/// `warn_within`, otherwise nothing.
pub fn skill_queue_alert(
    character_id: i64,
    character_name: &str,
    queue_finish: Option<SystemTime>,
    now: SystemTime,
    warn_within: Duration,
) -> Option<Notification> {
    let key = format!("skillqueue:{character_id}");
    match queue_finish {
        None => Some(Notification::new(
            key,
            "Skill queue empty",
            format!("{character_name} has no skills training."),
            Severity::Warning,
            "skills",
            now,
        )),
        Some(finish) if finish.duration_since(now).unwrap_or_default() <= warn_within => {
            Some(Notification::new(
                key,
                "Skill queue ending soon",
                format!("{character_name}'s skill queue is almost empty."),
                Severity::Info,
                "skills",
                now,
            ))
        }
        Some(_) => None,
    }
}

fn to_epoch(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> SystemTime {
        SystemTime::now()
    }

    fn note(key: &str, sev: Severity) -> Notification {
        Notification::new(key, "T", "B", sev, "test", now())
    }

    #[test]
    fn severity_orders_info_warning_critical() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Critical);
    }

    #[test]
    fn new_above_threshold_interrupts() {
        let mut c = NotificationCenter::new(Severity::Warning);
        let r = c.push(note("a", Severity::Critical));
        assert_eq!(r.status, PushStatus::Created);
        assert!(r.interrupt);
    }

    #[test]
    fn info_below_threshold_is_collected_not_interrupting() {
        let mut c = NotificationCenter::new(Severity::Warning);
        let r = c.push(note("a", Severity::Info));
        assert_eq!(r.status, PushStatus::Created);
        assert!(!r.interrupt); // collected, but does not interrupt
        assert_eq!(c.len(), 1);
        assert_eq!(c.unread_count(), 1);
    }

    #[test]
    fn duplicate_is_collapsed() {
        let mut c = NotificationCenter::new(Severity::Warning);
        c.push(note("a", Severity::Critical));
        let r = c.push(note("a", Severity::Critical));
        assert_eq!(r.status, PushStatus::Duplicate);
        assert!(!r.interrupt);
        assert_eq!(c.len(), 1); // not duplicated
    }

    #[test]
    fn changed_content_updates_and_reinterrupts() {
        let mut c = NotificationCenter::new(Severity::Warning);
        c.push(Notification::new("a", "Low fuel", "low", Severity::Warning, "fuel", now()));
        c.mark_all_read();
        assert_eq!(c.unread_count(), 0);
        // Same key, escalated → Updated, unread again, interrupts.
        let r = c.push(Notification::new("a", "Out of fuel", "empty", Severity::Critical, "fuel", now()));
        assert_eq!(r.status, PushStatus::Updated);
        assert!(r.interrupt);
        assert_eq!(c.len(), 1);
        assert_eq!(c.unread_count(), 1);
    }

    #[test]
    fn read_and_dismiss_tracking() {
        let mut c = NotificationCenter::new(Severity::Info);
        c.push(note("a", Severity::Info));
        c.push(note("b", Severity::Warning));
        assert_eq!(c.unread_count(), 2);
        assert!(c.mark_read("a"));
        assert_eq!(c.unread_count(), 1);
        assert!(c.dismiss("b"));
        assert_eq!(c.len(), 1);
        assert!(!c.dismiss("nope"));
    }

    #[test]
    fn fuel_rule_escalates_with_time() {
        let now = now();
        // Plenty of fuel → nothing.
        assert!(fuel_alert(1, "Astrahus", now + Duration::from_secs(86_400), now, Duration::from_secs(3600)).is_none());
        // Within the warning window → Warning.
        let warn = fuel_alert(1, "Astrahus", now + Duration::from_secs(600), now, Duration::from_secs(3600)).unwrap();
        assert_eq!(warn.severity, Severity::Warning);
        assert_eq!(warn.key, "fuel:1");
        // Already expired → Critical.
        let crit = fuel_alert(1, "Astrahus", now - Duration::from_secs(1), now, Duration::from_secs(3600)).unwrap();
        assert_eq!(crit.severity, Severity::Critical);
    }

    #[test]
    fn skill_queue_rule() {
        let now = now();
        let empty = skill_queue_alert(9, "Pilot", None, now, Duration::from_secs(3600)).unwrap();
        assert_eq!(empty.severity, Severity::Warning);
        let soon = skill_queue_alert(9, "Pilot", Some(now + Duration::from_secs(60)), now, Duration::from_secs(3600)).unwrap();
        assert_eq!(soon.severity, Severity::Info);
        assert!(skill_queue_alert(9, "Pilot", Some(now + Duration::from_secs(999_999)), now, Duration::from_secs(3600)).is_none());
    }

    #[test]
    fn list_is_most_recent_first() {
        let mut c = NotificationCenter::new(Severity::Info);
        c.push(Notification::new("old", "T", "B", Severity::Info, "t", UNIX_EPOCH + Duration::from_secs(100)));
        c.push(Notification::new("new", "T", "B", Severity::Info, "t", UNIX_EPOCH + Duration::from_secs(200)));
        let list = c.list();
        assert_eq!(list[0].key, "new");
        assert_eq!(list[1].key, "old");
    }
}
