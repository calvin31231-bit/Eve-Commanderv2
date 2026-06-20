//! The tiered background poll scheduler.
//!
//! Each pollable endpoint is placed in a [`PollClass`] chosen to match its ESI
//! cache timer — we never poll faster than CCP refreshes the data. On top of
//! that, background (non-active) characters are polled less often, scaled by the
//! user's [`Intensity`](crate::config::Intensity) profile, so a stable full of
//! alts doesn't hammer the machine.
//!
//! Many "live" values (skill-queue completion, structure fuel, job ETA) are
//! deterministic once fetched and are counted down **client-side** rather than
//! re-polled — see the project plan's "Data Freshness vs. Resource Budget".

use std::time::Duration;

use crate::config::Intensity;

/// Poll tiers, each with a base cadence aligned to the relevant ESI cache
/// timers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PollClass {
    /// Not background-polled at all — fetched only while a view needs it
    /// (location, ship, online).
    OnDemand,
    /// ~1–2 min: skills, wallet balance, industry jobs, mail headers.
    Fast,
    /// ~5–20 min: market orders, contracts.
    Medium,
    /// ~1 hr: assets, journal, transactions, clones, structures, sov.
    Slow,
    /// ~1 day: market history, SDE manifest check.
    Daily,
}

impl PollClass {
    /// Base cadence for the tier. `OnDemand` has no schedule.
    pub fn base_cadence(self) -> Option<Duration> {
        match self {
            PollClass::OnDemand => None,
            PollClass::Fast => Some(Duration::from_secs(90)),
            PollClass::Medium => Some(Duration::from_secs(600)),
            PollClass::Slow => Some(Duration::from_secs(3600)),
            PollClass::Daily => Some(Duration::from_secs(86_400)),
        }
    }

    /// Effective cadence for this tier given the intensity profile and whether
    /// the owning character is the active/foreground one. Background characters
    /// are polled less often. Returns `None` for `OnDemand`.
    pub fn effective_cadence(self, intensity: Intensity, active: bool) -> Option<Duration> {
        let base = self.base_cadence()?;
        if active {
            return Some(base);
        }
        let mult = intensity.background_cadence_multiplier();
        Some(Duration::from_secs_f64(base.as_secs_f64() * mult))
    }
}

/// A scheduled poll for one (character, endpoint) pair.
#[derive(Debug, Clone)]
pub struct PollJob {
    /// Owning character id (or 0 for shared/public data fetched once for all).
    pub character_id: i64,
    /// Stable key identifying the endpoint (e.g. `"skills"`, `"assets"`).
    pub endpoint: &'static str,
    pub class: PollClass,
    /// Seconds since the Unix epoch when this job last ran (0 = never).
    pub last_run_epoch: u64,
}

impl PollJob {
    pub fn new(character_id: i64, endpoint: &'static str, class: PollClass) -> Self {
        Self {
            character_id,
            endpoint,
            class,
            last_run_epoch: 0,
        }
    }

    /// Whether this job is due at `now_epoch`, given the profile and active
    /// state. `OnDemand` jobs are never due on a schedule.
    pub fn is_due(&self, now_epoch: u64, intensity: Intensity, active: bool) -> bool {
        match self.class.effective_cadence(intensity, active) {
            None => false,
            Some(cadence) => {
                if self.last_run_epoch == 0 {
                    return true; // never run yet
                }
                now_epoch.saturating_sub(self.last_run_epoch) >= cadence.as_secs()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_demand_has_no_cadence() {
        assert!(PollClass::OnDemand.base_cadence().is_none());
        let job = PollJob::new(1, "location", PollClass::OnDemand);
        assert!(!job.is_due(u64::MAX, Intensity::Aggressive, true));
    }

    #[test]
    fn background_characters_polled_less_often() {
        let active = PollClass::Fast
            .effective_cadence(Intensity::Balanced, true)
            .unwrap();
        let background = PollClass::Fast
            .effective_cadence(Intensity::Balanced, false)
            .unwrap();
        assert!(background > active);
        // Balanced multiplies background cadence by 3.
        assert_eq!(background.as_secs(), active.as_secs() * 3);
    }

    #[test]
    fn never_run_job_is_due() {
        let job = PollJob::new(1, "skills", PollClass::Fast);
        assert!(job.is_due(1000, Intensity::Balanced, true));
    }

    #[test]
    fn job_not_due_before_cadence_elapses() {
        let mut job = PollJob::new(1, "skills", PollClass::Fast);
        job.last_run_epoch = 1000;
        // Fast base cadence is 90s; 30s later it is not yet due.
        assert!(!job.is_due(1030, Intensity::Balanced, true));
        // 90s later it is due.
        assert!(job.is_due(1090, Intensity::Balanced, true));
    }
}
