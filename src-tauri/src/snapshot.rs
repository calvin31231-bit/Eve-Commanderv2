//! Background net-worth snapshotter — the durable history behind portfolio
//! analytics (ESI keeps no history, so we persist our own).
//!
//! A light tokio task wakes on a slow cadence, computes each character's net
//! worth (wallet + valued assets) and total SP, and appends a point per metric —
//! but only when the latest stored point is old enough, so the series stays one
//! point per ~hour regardless of how often the task ticks or the app restarts.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use eve_core::assets::{value_holdings, AssetsClient};
use eve_core::character::CharacterClient;
use eve_core::db::Database;
use eve_core::prices::PricesClient;

/// How often the task wakes.
const TICK: Duration = Duration::from_secs(30 * 60);
/// Minimum spacing between recorded points per character (≈ hourly).
const MIN_SPACING_SECS: i64 = 55 * 60;

/// Spawn the snapshotter. Cheap clones of the shared handles are moved in.
pub fn spawn(
    db: Database,
    character: CharacterClient,
    assets: AssetsClient,
    prices: PricesClient,
) {
    tauri::async_runtime::spawn(async move {
        // Record once shortly after launch, then on the slow cadence.
        tokio::time::sleep(Duration::from_secs(20)).await;
        loop {
            if let Err(e) = run_once(&db, &character, &assets, &prices).await {
                tracing::debug!("snapshot tick failed: {e}");
            }
            tokio::time::sleep(TICK).await;
        }
    });
}

async fn run_once(
    db: &Database,
    character: &CharacterClient,
    assets: &AssetsClient,
    prices: &PricesClient,
) -> Result<(), String> {
    let now = now_epoch();
    let characters = db.list_characters().await.map_err(|e| e.to_string())?;
    if characters.is_empty() {
        return Ok(());
    }
    // One shared, day-cached price map for valuing every character's assets.
    let price_map = prices.price_map().await.unwrap_or_default();

    for c in characters {
        // Skip if we already have a recent point (keeps the series ~hourly and
        // survives restarts without piling up dense points).
        if let Ok(Some(last)) = db.latest_snapshot(c.id, "networth").await {
            if now - last.taken_at < MIN_SPACING_SECS {
                continue;
            }
        }

        let (wallet, skills, holdings) = tokio::join!(
            character.wallet_balance(c.id),
            character.skills(c.id),
            assets.all_holdings(c.id),
        );
        // Only record when the authenticated reads actually succeeded — a
        // scope/token gap must not write a misleading zero into the history.
        let Ok(wallet_balance) = wallet else { continue };
        let asset_value = holdings
            .map(|groups| value_holdings(&groups, &price_map, 0).total_value)
            .unwrap_or(0.0);

        db.record_snapshot(c.id, "networth", wallet_balance + asset_value, now)
            .await
            .map_err(|e| e.to_string())?;
        if let Ok(s) = skills {
            db.record_snapshot(c.id, "sp", s.total_sp as f64, now)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
