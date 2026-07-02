//! The background poll worker.
//!
//! A single tokio task drives the [`Scheduler`] on a fixed tick. Each tick it:
//! 1. reloads the roster from `app.sqlite` and rebuilds the job set from the
//!    endpoint catalog (preserving each job's last-run time across rebuilds, so
//!    a roster change never forces a re-poll),
//! 2. marks the active/foreground character so it polls at full cadence,
//! 3. asks the scheduler for the due batch (capped at the in-flight budget),
//! 4. resolves a token (for authenticated routes) and warms the ESI cache.
//!
//! All the *decisions* (which jobs are due, how to resolve a fetch, token
//! refresh) live in `eve-core` and are unit-tested there; this is just the
//! tokio lifecycle wiring, which needs a running app to exercise.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

use eve_core::auth::TokenManager;
use eve_core::character::CharacterClient;
use eve_core::config::Intensity;
use eve_core::corp::CorpClient;
use eve_core::db::Database;
use eve_core::esi::{all_jobs, plan_fetches, EsiClient, Scheduler};
use eve_core::names::NameResolver;
use eve_core::notify::{fuel_alert, skill_queue_alert, Notification, Severity};

use crate::tray::{self, SharedCenter};

/// How often the alert rules are evaluated (their underlying ESI reads are
/// cache-served, so this is cheap; it just bounds redundant rule passes).
const ALERT_EVAL_INTERVAL: u64 = 60;

/// How far ahead a skill queue ending / structure fuel running out raises a
/// heads-up.
const SKILL_WARN: Duration = Duration::from_secs(24 * 3600);
const FUEL_WARN: Duration = Duration::from_secs(48 * 3600);

/// How often the scheduler is consulted. Individual endpoints still only fire at
/// their own (cache-timer-aligned) cadence; this is just the heartbeat.
const TICK: Duration = Duration::from_secs(5);

/// De-dup key for the "polling paused" notification.
const BACKOFF_KEY: &str = "esi:backoff";

/// Spawn the background poller. Cheap clones of the shared handles are moved into
/// the task; it lives for the duration of the app.
pub fn spawn(
    app: AppHandle,
    esi: EsiClient,
    tokens: TokenManager,
    db: Database,
    intensity: Arc<RwLock<Intensity>>,
    notifications: SharedCenter,
    names: NameResolver,
) {
    tauri::async_runtime::spawn(async move {
        let mut last_runs: HashMap<(i64, &'static str), u64> = HashMap::new();
        let mut last_alert_eval: u64 = 0;
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            // Read the live data-freshness setting each tick.
            let current = intensity.read().map(|g| *g).unwrap_or_default();
            if let Err(e) =
                run_tick(&app, &esi, &tokens, &db, current, &notifications, &mut last_runs).await
            {
                tracing::warn!("poll tick failed: {e}");
            }
            // Evaluate the alert rules on their own (slower) cadence.
            let now = now_epoch();
            if now.saturating_sub(last_alert_eval) >= ALERT_EVAL_INTERVAL {
                last_alert_eval = now;
                evaluate_alerts(&app, &esi, &tokens, &db, &notifications, &names).await;
            }
        }
    });
}

/// One scheduler tick: rebuild the job set, select the due batch, and execute it.
#[allow(clippy::too_many_arguments)]
async fn run_tick(
    app: &AppHandle,
    esi: &EsiClient,
    tokens: &TokenManager,
    db: &Database,
    intensity: Intensity,
    notifications: &SharedCenter,
    last_runs: &mut HashMap<(i64, &'static str), u64>,
) -> Result<(), String> {
    // If the error-budget breaker is backing off, raise a calm (Info) notice
    // that polling is paused and skip this tick's network work.
    let backoff = esi.backoff();
    if backoff > Duration::ZERO {
        tray::dispatch(
            app,
            notifications,
            Notification::new(
                BACKOFF_KEY,
                "ESI polling paused",
                "Backing off briefly to respect EVE's rate limits.",
                Severity::Info,
                "system",
                SystemTime::now(),
            ),
        );
        return Ok(());
    }
    // Budget healthy again — clear any stale backoff notice.
    if let Ok(mut center) = notifications.lock() {
        center.dismiss(BACKOFF_KEY);
    }

    let characters = db.list_characters().await.map_err(|e| e.to_string())?;

    // Rebuild the scheduler from the current roster, restoring last-run times so
    // cadence survives the rebuild.
    let mut scheduler = Scheduler::new(intensity);
    scheduler.add_jobs(all_jobs(&characters));
    for (&(character_id, endpoint), &last) in last_runs.iter() {
        scheduler.mark_ran(character_id, endpoint, last);
    }
    scheduler.set_active(characters.iter().filter(|c| c.active).map(|c| c.id));

    let now = now_epoch();
    let batch = scheduler.next_batch(now);

    for fetch in plan_fetches(&batch) {
        let token = if fetch.needs_auth {
            match tokens.access_token(fetch.character_id).await {
                Ok(t) => Some(t),
                // No token yet (e.g. character added but scope/refresh missing):
                // skip quietly; we'll retry next tick.
                Err(e) => {
                    tracing::debug!("skip {} for {}: {e}", fetch.endpoint, fetch.character_id);
                    continue;
                }
            }
        } else {
            None
        };

        match esi.get_raw(&fetch.path, token.as_deref()).await {
            Ok(_) => {
                last_runs.insert((fetch.character_id, fetch.endpoint), now);
            }
            Err(e) => {
                // Cache-warming failures (rate-limit backoff, transient ESI
                // errors) are expected; the scheduler will retry on cadence.
                tracing::debug!("poll {} failed: {e}", fetch.path);
            }
        }
    }

    Ok(())
}

/// Evaluate the notification rules against fresh (cache-served) ESI reads and
/// dispatch any alerts to the center + tray. Best-effort per character: a
/// missing token/scope or a 403 (e.g. no corp director role) is skipped quietly
/// so one character never blocks the others. This is what makes the Alerts rail
/// populate — skill-queue and structure-fuel rules already live in `eve-core`.
async fn evaluate_alerts(
    app: &AppHandle,
    esi: &EsiClient,
    tokens: &TokenManager,
    db: &Database,
    notifications: &SharedCenter,
    names: &NameResolver,
) {
    let Ok(characters) = db.list_characters().await else { return };
    let character = CharacterClient::new(esi.clone(), tokens.clone());
    let corp = CorpClient::new(esi.clone(), tokens.clone());
    let now = SystemTime::now();

    for c in &characters {
        // Skill queue: empty → Warning, ending within a day → Info.
        if let Ok(queue) = character.skill_queue(c.id).await {
            let finish = queue.finishes_at().map(|t| {
                UNIX_EPOCH + Duration::from_secs(t.unix_timestamp().max(0) as u64)
            });
            if let Some(note) = skill_queue_alert(c.id, &c.name, finish, now, SKILL_WARN) {
                tray::dispatch(app, notifications, note);
            }
        }

        // Structure fuel for the active character's corp (needs a director role +
        // scope; 403s for everyone else and is skipped).
        if c.active {
            if let Ok(public) = character.public_info(c.id).await {
                if let Ok(structures) = corp.structure_status(c.id, public.corporation_id).await {
                    // Resolve structure names in one pass (auth'd endpoint,
                    // best-effort); fall back to the raw id.
                    let ids: Vec<i64> = structures.iter().map(|s| s.structure_id).collect();
                    let resolved = match tokens.access_token(c.id).await {
                        Ok(token) => names.resolve_structures(&ids, &token).await,
                        Err(_) => HashMap::new(),
                    };
                    for s in structures {
                        if s.fuel_expires.is_none() {
                            continue;
                        }
                        let expires =
                            now + Duration::from_secs(s.fuel_seconds_remaining.max(0) as u64);
                        let name = resolved
                            .get(&s.structure_id)
                            .cloned()
                            .unwrap_or_else(|| format!("Structure {}", s.structure_id));
                        if let Some(note) = fuel_alert(s.structure_id, &name, expires, now, FUEL_WARN)
                        {
                            tray::dispatch(app, notifications, note);
                        }
                    }
                }
            }
        }
    }
}

/// Whole seconds since the Unix epoch (the scheduler's time base).
fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
