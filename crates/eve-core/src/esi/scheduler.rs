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

    /// Drain priority: lower is drained first when the in-flight budget forces a
    /// choice. Fast (freshest, most user-visible) wins over a Daily housekeeping
    /// poll. `OnDemand` is never scheduled and sorts last.
    pub fn priority(self) -> u8 {
        match self {
            PollClass::Fast => 0,
            PollClass::Medium => 1,
            PollClass::Slow => 2,
            PollClass::Daily => 3,
            PollClass::OnDemand => u8::MAX,
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

    /// Epoch second at which this job next becomes due, or `None` if it is not
    /// schedulable (`OnDemand`). A never-run job is due immediately (`0`).
    fn due_at(&self, intensity: Intensity, active: bool) -> Option<u64> {
        let cadence = self.class.effective_cadence(intensity, active)?.as_secs();
        if self.last_run_epoch == 0 {
            return Some(0); // never run — due now
        }
        Some(self.last_run_epoch.saturating_add(cadence))
    }

    /// Whether this job is due at `now_epoch`, given the profile and active
    /// state. `OnDemand` jobs are never due on a schedule. Due exactly when the
    /// cadence has elapsed (inclusive boundary).
    pub fn is_due(&self, now_epoch: u64, intensity: Intensity, active: bool) -> bool {
        match self.due_at(intensity, active) {
            None => false,
            Some(due) => now_epoch >= due,
        }
    }

    /// How long (seconds) past its due time this job is at `now_epoch`; the sort
    /// key the [`Scheduler`] uses so the most stale data refreshes first. A
    /// never-run job is reported maximally overdue so first-fetches lead.
    /// Returns `0` for not-yet-due or non-schedulable jobs.
    pub fn overdue_secs(&self, now_epoch: u64, intensity: Intensity, active: bool) -> u64 {
        match self.due_at(intensity, active) {
            None => 0,
            Some(0) => u64::MAX, // never run
            Some(due) => now_epoch.saturating_sub(due),
        }
    }
}

/// Owns the full set of poll jobs and turns the resource-budget design into a
/// concrete, smoothed work queue.
///
/// Every tick the scheduler does **not** fire every due job at once — that would
/// spike the connection and the error budget. Instead it selects at most
/// [`Intensity::max_in_flight`] jobs, choosing the most urgent first (freshest
/// tier, then most overdue). Whatever doesn't fit waits for the next tick, which
/// naturally staggers load over time. Foreground (`active`) characters and
/// shared/public jobs (`character_id == 0`) poll at full cadence; background alts
/// are stretched by the intensity multiplier.
pub struct Scheduler {
    jobs: Vec<PollJob>,
    intensity: Intensity,
    active: std::collections::HashSet<i64>,
}

impl Scheduler {
    pub fn new(intensity: Intensity) -> Self {
        Self {
            jobs: Vec::new(),
            intensity,
            active: std::collections::HashSet::new(),
        }
    }

    /// Register a job to be scheduled.
    pub fn add_job(&mut self, job: PollJob) {
        self.jobs.push(job);
    }

    /// Register many jobs at once (e.g. from
    /// [`endpoints::all_jobs`](super::endpoints::all_jobs)).
    pub fn add_jobs(&mut self, jobs: impl IntoIterator<Item = PollJob>) {
        self.jobs.extend(jobs);
    }

    pub fn set_intensity(&mut self, intensity: Intensity) {
        self.intensity = intensity;
    }

    /// Replace the set of foreground/active character ids.
    pub fn set_active(&mut self, active: impl IntoIterator<Item = i64>) {
        self.active = active.into_iter().collect();
    }

    /// Whether a job's owning character should be polled at active cadence.
    /// Shared/public jobs (`character_id == 0`) always count as active.
    fn is_active(&self, character_id: i64) -> bool {
        character_id == 0 || self.active.contains(&character_id)
    }

    /// Number of registered jobs (test/inspection helper).
    pub fn job_count(&self) -> usize {
        self.jobs.len()
    }

    /// Select the next batch to run at `now_epoch`: the due jobs, ordered by
    /// urgency (tier priority, then most overdue), capped at the intensity's
    /// in-flight budget. Returns owned `(character_id, endpoint)` keys; call
    /// [`mark_ran`](Self::mark_ran) for each once dispatched.
    pub fn next_batch(&self, now_epoch: u64) -> Vec<(i64, &'static str)> {
        let mut due: Vec<&PollJob> = self
            .jobs
            .iter()
            .filter(|j| j.is_due(now_epoch, self.intensity, self.is_active(j.character_id)))
            .collect();

        // Most urgent first: lower tier-priority wins; ties broken by who is
        // more overdue (descending).
        due.sort_by(|a, b| {
            a.class.priority().cmp(&b.class.priority()).then_with(|| {
                let oa = a.overdue_secs(now_epoch, self.intensity, self.is_active(a.character_id));
                let ob = b.overdue_secs(now_epoch, self.intensity, self.is_active(b.character_id));
                ob.cmp(&oa)
            })
        });

        due.into_iter()
            .take(self.intensity.max_in_flight())
            .map(|j| (j.character_id, j.endpoint))
            .collect()
    }

    /// Record that a (character, endpoint) job ran at `now_epoch`, resetting its
    /// cadence countdown.
    pub fn mark_ran(&mut self, character_id: i64, endpoint: &str, now_epoch: u64) {
        for job in &mut self.jobs {
            if job.character_id == character_id && job.endpoint == endpoint {
                job.last_run_epoch = now_epoch;
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

    #[test]
    fn overdue_grows_past_due_time() {
        let mut job = PollJob::new(1, "skills", PollClass::Fast);
        job.last_run_epoch = 1000; // due at 1090
        assert_eq!(job.overdue_secs(1090, Intensity::Balanced, true), 0);
        assert_eq!(job.overdue_secs(1150, Intensity::Balanced, true), 60);
        // Never-run jobs are maximally urgent.
        let fresh = PollJob::new(1, "wallet", PollClass::Fast);
        assert_eq!(fresh.overdue_secs(1150, Intensity::Balanced, true), u64::MAX);
    }

    #[test]
    fn batch_is_capped_at_in_flight_budget() {
        // Light allows 6 in flight; queue 10 never-run jobs.
        let mut sched = Scheduler::new(Intensity::Light);
        let endpoints = [
            "a", "b", "c", "d", "e", "f", "g", "h", "i", "j",
        ];
        for (n, ep) in endpoints.iter().enumerate() {
            sched.add_job(PollJob::new(n as i64 + 1, ep, PollClass::Fast));
        }
        let batch = sched.next_batch(10_000);
        assert_eq!(batch.len(), Intensity::Light.max_in_flight());
    }

    #[test]
    fn faster_tier_drains_first() {
        let mut sched = Scheduler::new(Intensity::Light); // budget 6
        // 6 Daily jobs plus 1 Fast job, all never-run/due. The Fast one must be
        // in the batch despite being added last.
        for n in 0..6 {
            sched.add_job(PollJob::new(n + 1, "daily", PollClass::Daily));
        }
        sched.add_job(PollJob::new(100, "skills", PollClass::Fast));
        let batch = sched.next_batch(10_000);
        assert_eq!(batch.len(), 6);
        assert!(batch.iter().any(|(_, ep)| *ep == "skills"));
    }

    #[test]
    fn mark_ran_clears_due_then_next_batch_skips_it() {
        let mut sched = Scheduler::new(Intensity::Balanced);
        sched.add_job(PollJob::new(1, "skills", PollClass::Fast));
        sched.set_active([1]); // active char → full 90s Fast cadence
        assert_eq!(sched.next_batch(10_000).len(), 1);
        sched.mark_ran(1, "skills", 10_000);
        // Immediately after running, it is no longer due.
        assert!(sched.next_batch(10_000).is_empty());
        // After the Fast cadence (90s) elapses, it is due again.
        assert_eq!(sched.next_batch(10_090).len(), 1);
    }

    #[test]
    fn shared_jobs_poll_at_active_cadence() {
        // character_id 0 is shared/public and must always count as active even
        // with no active set configured.
        let mut sched = Scheduler::new(Intensity::Light);
        sched.add_job(PollJob::new(0, "server_status", PollClass::Fast));
        assert!(sched.is_active(0));
        assert_eq!(sched.next_batch(10_000).len(), 1);
    }

    #[test]
    fn background_char_waits_longer_than_active() {
        let mut sched = Scheduler::new(Intensity::Balanced);
        // Two characters, both ran at t=1000. char 1 is active, char 2 is not.
        let mut a = PollJob::new(1, "skills", PollClass::Fast);
        a.last_run_epoch = 1000;
        let mut b = PollJob::new(2, "skills", PollClass::Fast);
        b.last_run_epoch = 1000;
        sched.add_job(a);
        sched.add_job(b);
        sched.set_active([1]);
        // At t=1100: active char (90s cadence) is due; background (270s) is not.
        let batch = sched.next_batch(1100);
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].0, 1);
    }
}
