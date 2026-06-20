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
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eve_core::auth::TokenManager;
use eve_core::config::Config;
use eve_core::db::Database;
use eve_core::esi::{all_jobs, plan_fetches, EsiClient, Scheduler};

/// How often the scheduler is consulted. Individual endpoints still only fire at
/// their own (cache-timer-aligned) cadence; this is just the heartbeat.
const TICK: Duration = Duration::from_secs(5);

/// Spawn the background poller. Cheap clones of the shared handles are moved into
/// the task; it lives for the duration of the app.
pub fn spawn(esi: EsiClient, tokens: TokenManager, db: Database, config: Config) {
    tauri::async_runtime::spawn(async move {
        let mut last_runs: HashMap<(i64, &'static str), u64> = HashMap::new();
        let mut ticker = tokio::time::interval(TICK);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            if let Err(e) = run_tick(&esi, &tokens, &db, &config, &mut last_runs).await {
                tracing::warn!("poll tick failed: {e}");
            }
        }
    });
}

/// One scheduler tick: rebuild the job set, select the due batch, and execute it.
async fn run_tick(
    esi: &EsiClient,
    tokens: &TokenManager,
    db: &Database,
    config: &Config,
    last_runs: &mut HashMap<(i64, &'static str), u64>,
) -> Result<(), String> {
    let characters = db.list_characters().await.map_err(|e| e.to_string())?;

    // Rebuild the scheduler from the current roster, restoring last-run times so
    // cadence survives the rebuild.
    let mut scheduler = Scheduler::new(config.intensity);
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

/// Whole seconds since the Unix epoch (the scheduler's time base).
fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
