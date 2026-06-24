//! Tauri IPC command handlers. These are intentionally **thin**: they validate
//! inputs and delegate to `eve-core`. The frontend calls them via `invoke()`.

use serde::Serialize;
use tauri::State;

use eve_core::account::{aggregate, AccountOverview, CharacterWorth};
use eve_core::assets::value_holdings;
use eve_core::character::CharacterSheet;
use eve_core::mail::strip_markup;
use eve_core::mining::estimated_yield;
use eve_core::model::Character;
use eve_core::notify::Notification;
use eve_core::sde::NamedType;
use eve_core::wallet::CashflowSummary;

use crate::AppState;

/// Result type surfaced to the frontend: errors become strings.
type CmdResult<T> = std::result::Result<T, String>;

/// Scopes requested at login — the read scopes backing the Character hub, so one
/// sign-in enables the whole monitor. (Truly incremental, per-feature scope
/// requests are a later refinement.)
const BASE_SCOPES: &[&str] = &[
    "publicData",
    "esi-skills.read_skills.v1",
    "esi-skills.read_skillqueue.v1",
    "esi-wallet.read_character_wallet.v1",
    "esi-assets.read_assets.v1",
    "esi-industry.read_character_mining.v1",
    "esi-planets.manage_planets.v1",
    "esi-characters.read_fatigue.v1",
    "esi-characters.read_agents_research.v1",
    "esi-bookmarks.read_character_bookmarks.v1",
    "esi-calendar.read_calendar_events.v1",
    "esi-fleets.read_fleet.v1",
    "esi-mail.read_mail.v1",
    "esi-mail.organize_mail.v1",
    "esi-clones.read_clones.v1",
    "esi-clones.read_implants.v1",
    "esi-universe.read_structures.v1",
    "esi-ui.write_waypoint.v1",
    "esi-ui.open_window.v1",
    "esi-corporations.read_structures.v1",
    "esi-corporations.track_members.v1",
    "esi-location.read_location.v1",
    "esi-location.read_ship_type.v1",
    "esi-location.read_online.v1",
    "esi-industry.read_character_jobs.v1",
    "esi-markets.read_character_orders.v1",
    "esi-contracts.read_character_contracts.v1",
];

/// EVE server status (public ESI endpoint) — a good first end-to-end check.
#[derive(Debug, Serialize, serde::Deserialize)]
pub struct ServerStatus {
    pub players: i64,
    #[serde(default)]
    pub server_version: String,
    #[serde(default)]
    pub vip: bool,
}

#[tauri::command]
pub async fn server_status(state: State<'_, AppState>) -> CmdResult<ServerStatus> {
    state
        .esi
        .get_public_json::<ServerStatus>("/latest/status/")
        .await
        .map_err(|e| e.to_string())
}

/// List the characters the user has added (from `app.sqlite`).
#[tauri::command]
pub async fn list_characters(state: State<'_, AppState>) -> CmdResult<Vec<Character>> {
    state.db.list_characters().await.map_err(|e| e.to_string())
}

/// One point in a persisted metric history.
#[derive(Debug, Serialize)]
pub struct HistoryPoint {
    /// Unix epoch seconds.
    pub at: i64,
    pub value: f64,
}

/// Portfolio history: net-worth (and SP) time-series over the last `days`, built
/// from our persisted snapshots (ESI has no history). `characterId` omitted →
/// account-wide totals. Includes the change vs the earliest point in range.
#[derive(Debug, Serialize)]
pub struct PortfolioHistory {
    pub networth: Vec<HistoryPoint>,
    pub sp: Vec<HistoryPoint>,
    /// networth now − networth at the start of the window (0 if <2 points).
    pub networth_change: f64,
    pub networth_change_pct: f64,
}

/// Net-worth / SP history for a character (or account-wide when `character_id`
/// is null), over the last `days`.
#[tauri::command]
pub async fn get_portfolio_history(
    state: State<'_, AppState>,
    character_id: Option<i64>,
    days: i64,
) -> CmdResult<PortfolioHistory> {
    let since = now_epoch_secs() - days.max(1) * 86_400;
    let (networth_raw, sp_raw) = match character_id {
        Some(id) => (
            state.db.snapshots(id, "networth", since).await.map_err(|e| e.to_string())?,
            state.db.snapshots(id, "sp", since).await.map_err(|e| e.to_string())?,
        ),
        None => (
            state.db.snapshots_total("networth", since).await.map_err(|e| e.to_string())?,
            state.db.snapshots_total("sp", since).await.map_err(|e| e.to_string())?,
        ),
    };

    let to_points = |v: Vec<eve_core::db::snapshots::Snapshot>| {
        v.into_iter().map(|s| HistoryPoint { at: s.taken_at, value: s.value }).collect::<Vec<_>>()
    };
    let networth = to_points(networth_raw);
    let sp = to_points(sp_raw);

    let (networth_change, networth_change_pct) = match (networth.first(), networth.last()) {
        (Some(first), Some(last)) if networth.len() >= 2 => {
            let change = last.value - first.value;
            let pct = if first.value > 0.0 { change / first.value * 100.0 } else { 0.0 };
            (change, pct)
        }
        _ => (0.0, 0.0),
    };

    Ok(PortfolioHistory { networth, sp, networth_change, networth_change_pct })
}

/// Whole seconds since the Unix epoch.
fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Account-wide overview: net worth / SP / wallet aggregated across every added
/// character, with a per-character breakdown. Each character's figures are
/// best-effort — a character missing a scope or token contributes what it can
/// (zeros) rather than failing the whole view.
#[tauri::command]
pub async fn get_account_overview(state: State<'_, AppState>) -> CmdResult<AccountOverview> {
    let characters = state.db.list_characters().await.map_err(|e| e.to_string())?;
    // Shared price reference (cached); best-effort.
    let prices = state.prices.price_map().await.unwrap_or_default();

    let mut worths = Vec::with_capacity(characters.len());
    for c in characters {
        // The three reads are independent — fetch them concurrently per character.
        let (wallet, skills, holdings) = tokio::join!(
            state.character.wallet_balance(c.id),
            state.character.skills(c.id),
            state.assets.all_holdings(c.id),
        );
        let wallet_balance = wallet.unwrap_or(0.0);
        let total_sp = skills.map(|s| s.total_sp).unwrap_or(0);
        let asset_value = holdings
            .map(|groups| value_holdings(&groups, &prices, 0).total_value)
            .unwrap_or(0.0);
        worths.push(CharacterWorth::new(c.id, c.name, wallet_balance, asset_value, total_sp));
    }
    Ok(aggregate(worths))
}

/// How long we keep the loopback redirect server open waiting for the user to
/// finish authorizing in their browser before giving up.
const LOGIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Run the full SSO login on the backend: bind the loopback redirect server,
/// open the system browser at the authorize URL, capture the redirect, exchange
/// the code, persist the character, and store the refresh token in the keychain.
///
/// This is one round-trip from the frontend's perspective: it resolves with the
/// newly-added [`Character`] (or an error string).
#[tauri::command]
pub async fn login(state: State<'_, AppState>) -> CmdResult<Character> {
    if state.config.client_id.is_empty() {
        return Err("EVE_COMMANDER_CLIENT_ID is not set — register an ESI app first".into());
    }

    // Bind the loopback server on the registered redirect port *before* opening
    // the browser, so the redirect can never race ahead of us.
    let port = redirect_port(&state.config.redirect_uri)?;
    let server = eve_core::auth::LoopbackServer::bind(port)
        .map_err(|e| format!("could not bind loopback redirect on port {port}: {e}"))?;

    // Generate PKCE + state and get the authorize URL, then open the browser.
    let url = state
        .login
        .begin(&state.sso, BASE_SCOPES)
        .map_err(|e| e.to_string())?;
    open_in_browser(url.as_str()).map_err(|e| format!("could not open browser: {e}"))?;

    // Wait for the redirect off the async runtime (the server is blocking I/O).
    let callback = tokio::task::spawn_blocking(move || server.wait_for_callback_timeout(LOGIN_TIMEOUT))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    // Exchange the code (this also performs the authoritative CSRF state check),
    // then persist.
    let completed = state
        .login
        .complete(&state.sso, &callback.state, &callback.code)
        .await
        .map_err(|e| e.to_string())?;

    state
        .db
        .upsert_character(&completed.character)
        .await
        .map_err(|e| e.to_string())?;
    state
        .tokens
        .save_refresh_token(completed.character.id, &completed.refresh_token)
        .map_err(|e| e.to_string())?;

    // Prime the in-memory token cache with the freshly-issued access token so
    // the first poll for this character doesn't trigger an immediate refresh.
    state.token_manager.prime(
        completed.character.id,
        completed.access_token,
        completed.expires_in,
    );

    Ok(completed.character)
}

/// Parse the explicit port out of the registered redirect URI. EVE SSO requires
/// the redirect to match exactly, so the port is fixed by registration.
fn redirect_port(redirect_uri: &str) -> CmdResult<u16> {
    let url = reqwest::Url::parse(redirect_uri).map_err(|e| e.to_string())?;
    url.port()
        .ok_or_else(|| format!("redirect URI '{redirect_uri}' must include an explicit port"))
}

/// Open `url` in the user's default browser. Uses the platform opener directly
/// to avoid pulling in a plugin; the SSO page is always an external https URL.
fn open_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        // Open via the shell URL protocol handler. NOT `cmd /C start` (cmd splits
        // the OAuth URL on `&`, dropping client_id/scope/state) and NOT
        // `explorer.exe <url>` (which can open a folder window instead of the
        // browser). rundll32 isn't a shell, so the URL — ampersands and all —
        // reaches the default browser intact.
        let mut c = std::process::Command::new("rundll32.exe");
        c.arg("url.dll,FileProtocolHandler").arg(url);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    cmd.spawn().map(|_| ())
}

/// The live status strip: online, current system, ship, and skill in training.
#[derive(Debug, Serialize)]
pub struct CharacterStatusView {
    pub online: bool,
    pub system_name: String,
    /// Custom ship name (may be empty).
    pub ship_name: String,
    /// Ship hull type name.
    pub ship_type_name: String,
    /// e.g. "Gunnery V", or null if nothing is training.
    pub training: Option<String>,
    pub training_seconds_remaining: Option<i64>,
}

/// Resolve a skill level (1–5) to its Roman numeral.
fn roman(level: i64) -> &'static str {
    match level {
        1 => "I",
        2 => "II",
        3 => "III",
        4 => "IV",
        5 => "V",
        _ => "",
    }
}

/// Public character profile for the Character-hub header.
#[derive(Debug, Serialize)]
pub struct CharacterProfile {
    pub name: String,
    pub corporation: String,
    pub alliance: Option<String>,
    pub security_status: f64,
}

/// Corp/alliance/sec-status header for a character (portrait is built on the
/// frontend from the public image server).
#[tauri::command]
pub async fn get_character_profile(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<CharacterProfile> {
    let info = state
        .character
        .public_info(character_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut ids = vec![info.corporation_id];
    if let Some(a) = info.alliance_id {
        ids.push(a);
    }
    let names = names_for(&state, &ids).await;

    Ok(CharacterProfile {
        name: info.name,
        corporation: named(&names, info.corporation_id),
        alliance: info.alliance_id.map(|a| named(&names, a)),
        security_status: info.security_status,
    })
}

/// Live character status for the always-on context bar.
#[tauri::command]
pub async fn get_character_status(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<CharacterStatusView> {
    let raw = state
        .character
        .status(character_id)
        .await
        .map_err(|e| e.to_string())?;

    // Resolve system / ship / skill ids in one batch (0 = unknown, skipped).
    let mut ids = Vec::new();
    if raw.system_id != 0 {
        ids.push(raw.system_id);
    }
    if raw.ship_type_id != 0 {
        ids.push(raw.ship_type_id);
    }
    if let Some(s) = raw.training_skill_id {
        ids.push(s);
    }
    let names = names_for(&state, &ids).await;

    let training = raw.training_skill_id.map(|sid| {
        let level = raw.training_level.unwrap_or(0);
        format!("{} {}", named(&names, sid), roman(level)).trim().to_string()
    });

    Ok(CharacterStatusView {
        online: raw.online,
        system_name: if raw.system_id != 0 {
            named(&names, raw.system_id)
        } else {
            "—".into()
        },
        ship_name: raw.ship_name,
        ship_type_name: if raw.ship_type_id != 0 {
            named(&names, raw.ship_type_id)
        } else {
            String::new()
        },
        training,
        training_seconds_remaining: raw.training_seconds_remaining,
    })
}

/// A queued skill with its resolved name.
#[derive(Debug, Serialize)]
pub struct QueuedSkillView {
    pub name: String,
    pub finished_level: i64,
    pub queue_position: i64,
    pub seconds_remaining: i64,
}

/// The character's skill queue contents (named, with per-skill countdowns).
#[tauri::command]
pub async fn get_skill_queue(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<QueuedSkillView>> {
    let upcoming = state
        .character
        .skill_queue_upcoming(character_id)
        .await
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = upcoming.iter().map(|s| s.skill_id).collect();
    let names = names_for(&state, &ids).await;
    Ok(upcoming
        .into_iter()
        .map(|s| QueuedSkillView {
            name: format!("{} {}", named(&names, s.skill_id), roman(s.finished_level)),
            finished_level: s.finished_level,
            queue_position: s.queue_position,
            seconds_remaining: s.seconds_remaining,
        })
        .collect())
}

/// The character's attributes (+ remap info).
#[tauri::command]
pub async fn get_attributes(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<eve_core::character::CharacterAttributes> {
    state
        .character
        .attributes(character_id)
        .await
        .map_err(|e| e.to_string())
}

/// Fetch the Character-hub summary (skills + queue + wallet) for a character.
#[tauri::command]
pub async fn get_character_sheet(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<CharacterSheet> {
    state
        .character
        .sheet(character_id)
        .await
        .map_err(|e| e.to_string())
}

/// A named, valued asset holding.
#[derive(Debug, Serialize)]
pub struct ValuedAssetGroup {
    pub type_id: i64,
    pub name: String,
    pub quantity: i64,
    pub locations: usize,
    pub value: f64,
}

/// A character's holdings: total estimated value plus the top valued groups.
#[derive(Debug, Serialize)]
pub struct HoldingsView {
    pub total_value: f64,
    pub groups: Vec<ValuedAssetGroup>,
}

/// Top `limit` holdings for a character, ranked by ISK value (priced via the
/// public market reference) and named via the SDE. `total_value` spans all
/// holdings, not just the returned rows.
#[tauri::command]
pub async fn get_top_holdings(
    state: State<'_, AppState>,
    character_id: i64,
    limit: usize,
) -> CmdResult<HoldingsView> {
    let groups = state
        .assets
        .all_holdings(character_id)
        .await
        .map_err(|e| e.to_string())?;
    // Prices are best-effort: if the fetch fails, values fall back to 0.
    let prices = state.prices.price_map().await.unwrap_or_default();

    let valued = value_holdings(&groups, &prices, limit);
    let ids: Vec<i64> = valued.groups.iter().map(|g| g.type_id).collect();
    let names = names_for(&state, &ids).await;
    let out = valued
        .groups
        .into_iter()
        .map(|g| ValuedAssetGroup {
            type_id: g.type_id,
            name: named(&names, g.type_id),
            quantity: g.quantity,
            locations: g.locations,
            value: g.value,
        })
        .collect();
    Ok(HoldingsView {
        total_value: valued.total_value,
        groups: out,
    })
}

/// One jump clone with resolved location + implant names.
#[derive(Debug, Serialize)]
pub struct JumpCloneView {
    pub jump_clone_id: i64,
    pub name: Option<String>,
    pub location_name: String,
    pub implants: Vec<NamedType>,
}

/// Value at one named location.
#[derive(Debug, Serialize)]
pub struct LocationValueView {
    pub location_name: String,
    pub value: f64,
    pub item_count: usize,
}

/// Hangar asset value grouped by station/location (top `limit` by value).
#[tauri::command]
pub async fn get_assets_by_location(
    state: State<'_, AppState>,
    character_id: i64,
    limit: usize,
) -> CmdResult<Vec<LocationValueView>> {
    let prices = state.prices.price_map().await.unwrap_or_default();
    let mut locations = state
        .assets
        .by_location(character_id, &prices)
        .await
        .map_err(|e| e.to_string())?;
    locations.truncate(limit);

    let ids: Vec<i64> = locations.iter().map(|l| l.location_id).collect();
    let names = names_with_structures(&state, character_id, &ids).await;
    Ok(locations
        .into_iter()
        .map(|l| LocationValueView {
            location_name: names
                .get(&l.location_id)
                .cloned()
                .unwrap_or_else(|| format!("Location {}", l.location_id)),
            value: l.value,
            item_count: l.item_count,
        })
        .collect())
}

/// The clone view: active implants plus a per-jump-clone breakdown.
#[derive(Debug, Serialize)]
pub struct ClonesView {
    pub jump_clone_count: usize,
    pub active_implant_count: usize,
    pub implants: Vec<NamedType>,
    pub home_location_name: Option<String>,
    pub jump_clones: Vec<JumpCloneView>,
    pub last_jump_date: Option<String>,
}

/// Jump clones (location + implants each) + active implants, all named.
#[tauri::command]
pub async fn get_clones(state: State<'_, AppState>, character_id: i64) -> CmdResult<ClonesView> {
    let (clones, active) = tokio::join!(
        state.clones.clones(character_id),
        state.clones.active_implants(character_id),
    );
    let clones = clones.map_err(|e| e.to_string())?;
    let active = active.unwrap_or_default();

    // One batch resolve for every id across home, clones (location + implants),
    // and the active set.
    let mut ids: Vec<i64> = active.clone();
    if let Some(h) = &clones.home_location {
        ids.push(h.location_id);
    }
    for jc in &clones.jump_clones {
        ids.push(jc.location_id);
        ids.extend(&jc.implants);
    }
    // Clone locations are commonly citadels, which need authenticated structure
    // resolution on top of the public name endpoint.
    let names = names_with_structures(&state, character_id, &ids).await;
    let named_types = |type_ids: &[i64]| -> Vec<NamedType> {
        type_ids
            .iter()
            .map(|&type_id| NamedType { type_id, name: named(&names, type_id) })
            .collect()
    };

    Ok(ClonesView {
        jump_clone_count: clones.jump_clones.len(),
        active_implant_count: active.len(),
        implants: named_types(&active),
        home_location_name: clones.home_location.as_ref().map(|h| named(&names, h.location_id)),
        jump_clones: clones
            .jump_clones
            .iter()
            .map(|jc| JumpCloneView {
                jump_clone_id: jc.jump_clone_id,
                name: jc.name.clone().filter(|n| !n.is_empty()),
                location_name: named(&names, jc.location_id),
                implants: named_types(&jc.implants),
            })
            .collect(),
        last_jump_date: clones.last_clone_jump_date.clone(),
    })
}

/// Wallet cashflow summary (income/expenses/net + top categories) for a
/// character, from its wallet journal.
#[tauri::command]
pub async fn get_cashflow(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<CashflowSummary> {
    state
        .wallet
        .cashflow(character_id, 6)
        .await
        .map_err(|e| e.to_string())
}

/// An active industry job with its SDE-resolved item name.
#[derive(Debug, Serialize)]
pub struct IndustryJobView {
    pub job_id: i64,
    pub activity: String,
    pub item_name: String,
    pub runs: i64,
    pub status: String,
    pub end_date: String,
    pub seconds_remaining: i64,
}

/// In-progress industry jobs (with client-side countdowns) for a character,
/// items resolved to names via the SDE.
#[tauri::command]
pub async fn get_industry_jobs(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<IndustryJobView>> {
    let summary = state
        .industry
        .summary(character_id)
        .await
        .map_err(|e| e.to_string())?;

    let ids: Vec<i64> = summary.jobs.iter().map(|j| j.display_type_id).collect();
    let names = names_for(&state, &ids).await;
    let out = summary
        .jobs
        .into_iter()
        .map(|job| IndustryJobView {
            job_id: job.job_id,
            activity: job.activity,
            item_name: named(&names, job.display_type_id),
            runs: job.runs,
            status: job.status,
            end_date: job.end_date,
            seconds_remaining: job.seconds_remaining,
        })
        .collect();
    Ok(out)
}

/// One open order with its SDE-resolved item name.
#[derive(Debug, Serialize)]
pub struct MarketOrderView {
    pub order_id: i64,
    pub item_name: String,
    pub is_buy_order: bool,
    pub price: f64,
    pub volume_remain: i64,
    pub volume_total: i64,
    pub seconds_remaining: i64,
}

/// A character's open market orders: buy/sell rollup plus named per-order rows.
#[derive(Debug, Serialize)]
pub struct MarketView {
    pub buy_count: usize,
    pub sell_count: usize,
    pub total_escrow: f64,
    pub sell_value: f64,
    pub orders: Vec<MarketOrderView>,
}

#[tauri::command]
pub async fn get_market_orders(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<MarketView> {
    let summary = state
        .market
        .summary(character_id)
        .await
        .map_err(|e| e.to_string())?;

    let ids: Vec<i64> = summary.orders.iter().map(|o| o.type_id).collect();
    let names = names_for(&state, &ids).await;
    let orders = summary
        .orders
        .into_iter()
        .map(|o| MarketOrderView {
            order_id: o.order_id,
            item_name: named(&names, o.type_id),
            is_buy_order: o.is_buy_order,
            price: o.price,
            volume_remain: o.volume_remain,
            volume_total: o.volume_total,
            seconds_remaining: o.seconds_remaining,
        })
        .collect();
    Ok(MarketView {
        buy_count: summary.buy_count,
        sell_count: summary.sell_count,
        total_escrow: summary.total_escrow,
        sell_value: summary.sell_value,
        orders,
    })
}

/// One ore total with its SDE name and estimated ISK value.
#[derive(Debug, Serialize)]
pub struct NamedOre {
    pub type_id: i64,
    pub name: String,
    pub quantity: i64,
    pub value: f64,
}

/// A character's mining ledger rollup: unit/value totals + named top ores.
#[derive(Debug, Serialize)]
pub struct MiningView {
    pub total_units: i64,
    pub day_count: usize,
    pub total_value: f64,
    pub ores: Vec<NamedOre>,
}

#[tauri::command]
pub async fn get_mining(state: State<'_, AppState>, character_id: i64) -> CmdResult<MiningView> {
    let summary = state
        .mining
        .summary(character_id)
        .await
        .map_err(|e| e.to_string())?;
    let prices = state.prices.price_map().await.unwrap_or_default();

    // Total yield spans all ore; only the display rows are truncated.
    let total_value = estimated_yield(&summary.by_ore, &prices);

    let top: Vec<_> = summary.by_ore.into_iter().take(8).collect();
    let ids: Vec<i64> = top.iter().map(|o| o.type_id).collect();
    let names = names_for(&state, &ids).await;
    let ores = top
        .into_iter()
        .map(|ore| NamedOre {
            type_id: ore.type_id,
            name: named(&names, ore.type_id),
            quantity: ore.quantity,
            value: prices.value(ore.type_id, ore.quantity),
        })
        .collect();
    Ok(MiningView {
        total_units: summary.total_units,
        day_count: summary.day_count,
        total_value,
        ores,
    })
}

/// A PI colony rolled up for the UI, with the system + extracted products named.
#[derive(Debug, Serialize)]
pub struct ColonyView {
    pub planet_id: i64,
    pub system_name: String,
    pub planet_type: String,
    pub upgrade_level: i64,
    pub num_pins: i64,
    pub extractor_count: i64,
    pub products: Vec<String>,
    pub soonest_expiry: Option<String>,
    pub seconds_remaining: i64,
}

/// The character's planetary-industry colonies with extractor-cycle countdowns,
/// soonest expiry first. Empty when the character runs no PI (or lacks the
/// planets scope).
#[tauri::command]
pub async fn get_planets(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<ColonyView>> {
    let colonies = state
        .planets
        .summary(character_id)
        .await
        .map_err(|e| e.to_string())?;

    // Resolve systems + extracted product types in one batch.
    let mut ids: Vec<i64> = Vec::new();
    for c in &colonies {
        ids.push(c.solar_system_id);
        ids.extend(&c.products);
    }
    let names = names_for(&state, &ids).await;

    Ok(colonies
        .into_iter()
        .map(|c| ColonyView {
            planet_id: c.planet_id,
            system_name: named(&names, c.solar_system_id),
            planet_type: title_case(&c.planet_type),
            upgrade_level: c.upgrade_level,
            num_pins: c.num_pins,
            extractor_count: c.extractor_count as i64,
            products: c.products.iter().map(|&p| named(&names, p)).collect(),
            soonest_expiry: c.soonest_expiry,
            seconds_remaining: c.seconds_remaining,
        })
        .collect())
}

/// Capitalize the first letter (ESI planet types come lowercase: "barren").
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// Batch-resolve ids to names via the layered resolver (cache → SDE → ESI).
/// Returns the resolved map; ids that didn't resolve are simply absent.
async fn names_for(
    state: &AppState,
    ids: &[i64],
) -> std::collections::HashMap<i64, String> {
    state.names.resolve(ids).await.unwrap_or_default()
}

/// Look up a name in a resolved map, falling back to `Type {id}`.
fn named(names: &std::collections::HashMap<i64, String>, id: i64) -> String {
    names.get(&id).cloned().unwrap_or_else(|| format!("Type {id}"))
}

/// Resolve a mix of ids that may include player-owned structure ids (citadels),
/// which the public name endpoint can't resolve. Falls back to the character's
/// token + the authenticated structures endpoint for any unresolved id in the
/// structure range, so hangars/clones docked in a citadel show its real name.
async fn names_with_structures(
    state: &AppState,
    character_id: i64,
    ids: &[i64],
) -> std::collections::HashMap<i64, String> {
    let mut names = names_for(state, ids).await;
    let structure_ids: Vec<i64> = ids
        .iter()
        .copied()
        .filter(|&id| id >= eve_core::names::STRUCTURE_ID_MIN && !names.contains_key(&id))
        .collect();
    if !structure_ids.is_empty() {
        if let Ok(token) = state.token_manager.access_token(character_id).await {
            names.extend(state.names.resolve_structures(&structure_ids, &token).await);
        }
    }
    names
}

/// A mail header with its sender resolved to a name.
#[derive(Debug, Serialize)]
pub struct MailHeaderView {
    pub mail_id: i64,
    pub subject: String,
    pub from_name: String,
    pub timestamp: String,
    pub is_read: bool,
}

/// The latest page of mail headers, with sender names resolved.
#[tauri::command]
pub async fn get_mail_headers(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<MailHeaderView>> {
    let headers = state
        .mail
        .headers(character_id)
        .await
        .map_err(|e| e.to_string())?;

    let sender_ids: Vec<i64> = headers.iter().map(|h| h.from).collect();
    let names = names_for(&state, &sender_ids).await;

    Ok(headers
        .into_iter()
        .map(|h| MailHeaderView {
            from_name: if h.from != 0 { named(&names, h.from) } else { "—".into() },
            mail_id: h.mail_id,
            subject: h.subject,
            timestamp: h.timestamp,
            is_read: h.is_read,
        })
        .collect())
}

/// A single mail, body reduced to plain text.
#[derive(Debug, Serialize)]
pub struct MailView {
    pub subject: String,
    pub from: i64,
    pub body: String,
    pub timestamp: String,
    pub read: bool,
}

/// Read one mail's body (EVE markup stripped to readable text).
#[tauri::command]
pub async fn get_mail(
    state: State<'_, AppState>,
    character_id: i64,
    mail_id: i64,
) -> CmdResult<MailView> {
    let mail = state
        .mail
        .body(character_id, mail_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(MailView {
        subject: mail.subject,
        from: mail.from,
        body: strip_markup(&mail.body),
        timestamp: mail.timestamp,
        read: mail.read,
    })
}

/// A market transaction with its item name resolved.
#[derive(Debug, Serialize)]
pub struct TransactionView {
    pub item_name: String,
    pub is_buy: bool,
    pub quantity: i64,
    pub unit_price: f64,
    pub total: f64,
    pub date: String,
}

/// The character's most recent market transactions (item names resolved).
#[tauri::command]
pub async fn get_transactions(
    state: State<'_, AppState>,
    character_id: i64,
    limit: usize,
) -> CmdResult<Vec<TransactionView>> {
    let mut txns = state
        .wallet
        .transactions(character_id)
        .await
        .map_err(|e| e.to_string())?;
    txns.truncate(limit);

    let ids: Vec<i64> = txns.iter().map(|t| t.type_id).collect();
    let names = names_for(&state, &ids).await;
    Ok(txns
        .into_iter()
        .map(|t| TransactionView {
            item_name: named(&names, t.type_id),
            is_buy: t.is_buy,
            quantity: t.quantity,
            unit_price: t.unit_price,
            total: t.unit_price * t.quantity as f64,
            date: t.date,
        })
        .collect())
}

/// Mark a mail as read (requires the organize-mail scope).
#[tauri::command]
pub async fn mark_mail_read(
    state: State<'_, AppState>,
    character_id: i64,
    mail_id: i64,
) -> CmdResult<()> {
    state
        .mail
        .mark_read(character_id, mail_id)
        .await
        .map_err(|e| e.to_string())
}

/// List character groups with their members.
#[tauri::command]
pub async fn list_groups(
    state: State<'_, AppState>,
) -> CmdResult<Vec<eve_core::model::CharacterGroup>> {
    state.db.list_groups().await.map_err(|e| e.to_string())
}

/// Create a new (empty) group.
#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<eve_core::model::CharacterGroup> {
    state.db.create_group(&name).await.map_err(|e| e.to_string())
}

/// Delete a group.
#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, group_id: i64) -> CmdResult<()> {
    state.db.delete_group(group_id).await.map_err(|e| e.to_string())
}

/// Add a character to a group.
#[tauri::command]
pub async fn add_group_member(
    state: State<'_, AppState>,
    group_id: i64,
    character_id: i64,
) -> CmdResult<()> {
    state
        .db
        .add_group_member(group_id, character_id)
        .await
        .map_err(|e| e.to_string())
}

/// Remove a character from a group.
#[tauri::command]
pub async fn remove_group_member(
    state: State<'_, AppState>,
    group_id: i64,
    character_id: i64,
) -> CmdResult<()> {
    state
        .db
        .remove_group_member(group_id, character_id)
        .await
        .map_err(|e| e.to_string())
}

/// User-editable application settings.
#[derive(Debug, Serialize)]
pub struct AppSettings {
    /// Data-freshness profile: "Light" | "Balanced" | "Aggressive".
    pub intensity: String,
    /// Minimum notification severity that interrupts: "Info" | "Warning" | "Critical".
    pub notify_min: String,
    /// Discord webhook URL for mirroring interrupting alerts ("" = off).
    pub discord_webhook: String,
}

/// Read the current settings.
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CmdResult<AppSettings> {
    let intensity = state
        .db
        .get_setting_or("intensity", state.config.intensity.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let notify_min = state
        .db
        .get_setting_or("notify_min", "Warning")
        .await
        .map_err(|e| e.to_string())?;
    let discord_webhook = state
        .db
        .get_setting_or("discord_webhook", "")
        .await
        .map_err(|e| e.to_string())?;
    Ok(AppSettings { intensity, notify_min, discord_webhook })
}

/// Persist and apply settings live (poll intensity + notification threshold +
/// Discord webhook).
#[tauri::command]
pub async fn set_settings(
    state: State<'_, AppState>,
    intensity: String,
    notify_min: String,
    discord_webhook: Option<String>,
) -> CmdResult<()> {
    let parsed_intensity = eve_core::config::Intensity::parse(&intensity);
    let parsed_sev = eve_core::notify::Severity::parse(&notify_min);
    let webhook = discord_webhook.unwrap_or_default();
    let webhook = webhook.trim().to_string();

    state
        .db
        .set_setting("intensity", parsed_intensity.as_str())
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .set_setting("notify_min", parsed_sev.as_str())
        .await
        .map_err(|e| e.to_string())?;
    state
        .db
        .set_setting("discord_webhook", &webhook)
        .await
        .map_err(|e| e.to_string())?;

    // Apply live: the poller reads intensity each tick; the center is shared.
    if let Ok(mut g) = state.intensity.write() {
        *g = parsed_intensity;
    }
    if let Ok(mut center) = state.notifications.lock() {
        center.set_min_interrupt(parsed_sev);
    }
    if let Ok(mut g) = state.discord_webhook.write() {
        *g = if webhook.is_empty() { None } else { Some(webhook) };
    }
    Ok(())
}

/// The character's contracts (active first), capped to a recent window.
#[tauri::command]
pub async fn get_contracts(
    state: State<'_, AppState>,
    character_id: i64,
    limit: usize,
) -> CmdResult<Vec<eve_core::contracts::Contract>> {
    let mut cs = state
        .contracts
        .contracts(character_id)
        .await
        .map_err(|e| e.to_string())?;
    cs.truncate(limit);
    Ok(cs)
}

/// A market-search result.
#[derive(Debug, Serialize)]
pub struct ItemHit {
    pub type_id: i64,
    pub name: String,
}

/// Prefix-search item types by name (SDE; full coverage needs the prebuilt SDE).
#[tauri::command]
pub async fn search_items(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> CmdResult<Vec<ItemHit>> {
    let hits = state
        .names
        .search_types(&query, limit)
        .await
        .map_err(|e| e.to_string())?;
    Ok(hits
        .into_iter()
        .map(|t| ItemHit { type_id: t.type_id, name: t.name })
        .collect())
}

/// Regional market view for an item: best quote + history stats (The Forge),
/// plus ship insurance tiers when applicable.
#[derive(Debug, Serialize)]
pub struct MarketBrowse {
    pub quote: eve_core::marketdata::MarketQuote,
    pub history: eve_core::marketdata::HistoryStats,
    pub insurance: Option<Vec<eve_core::insurance::InsuranceLevel>>,
    /// Best buy/sell across the major trade hubs for cross-hub comparison.
    pub hubs: Vec<eve_core::marketdata::HubQuote>,
}

#[tauri::command]
pub async fn get_market_browse(
    state: State<'_, AppState>,
    type_id: i64,
) -> CmdResult<MarketBrowse> {
    let region = eve_core::marketdata::THE_FORGE;
    let (quote, history, insurance, hubs) = tokio::join!(
        state.marketdata.quote(region, type_id),
        state.marketdata.history(region, type_id),
        state.insurance.for_type(type_id),
        state.marketdata.compare(type_id),
    );
    Ok(MarketBrowse {
        quote: quote.map_err(|e| e.to_string())?,
        history: history.unwrap_or_else(|_| eve_core::marketdata::history_stats(&[])),
        insurance: insurance.ok().flatten(),
        hubs: hubs.unwrap_or_default(),
    })
}

/// One station-trade candidate with its item name resolved.
#[derive(Debug, Serialize)]
pub struct TradeOpportunityView {
    pub type_id: i64,
    pub name: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub margin_pct: f64,
    pub profit_per_unit: f64,
    pub daily_volume: i64,
    pub daily_potential: f64,
}

/// Scan a list of item types at Jita for station-trade opportunities, ranked by
/// daily profit potential. When `typeIds` is omitted, a curated default set of
/// liquid items is scanned. `brokerFee`/`salesTax` are fractions (0.03 = 3%).
#[tauri::command]
pub async fn scan_station_trades(
    state: State<'_, AppState>,
    type_ids: Option<Vec<i64>>,
    broker_fee: Option<f64>,
    sales_tax: Option<f64>,
) -> CmdResult<Vec<TradeOpportunityView>> {
    let region = eve_core::marketdata::THE_FORGE;
    let ids = type_ids.unwrap_or_else(default_scan_types);
    let mut fees = eve_core::marketdata::TradeFees::default();
    if let Some(b) = broker_fee {
        fees.broker_fee = b;
    }
    if let Some(t) = sales_tax {
        fees.sales_tax = t;
    }
    let opps = state
        .marketdata
        .scan(region, &ids, fees)
        .await
        .map_err(|e| e.to_string())?;
    let names = names_for(&state, &opps.iter().map(|o| o.type_id).collect::<Vec<_>>()).await;
    Ok(opps
        .into_iter()
        .map(|o| TradeOpportunityView {
            type_id: o.type_id,
            name: named(&names, o.type_id),
            buy_price: o.metrics.buy_price,
            sell_price: o.metrics.sell_price,
            margin_pct: o.metrics.margin_pct,
            profit_per_unit: o.metrics.profit_per_unit,
            daily_volume: o.metrics.daily_volume,
            daily_potential: o.metrics.daily_potential,
        })
        .collect())
}

/// One cross-hub haul candidate with its item name resolved.
#[derive(Debug, Serialize)]
pub struct ArbitrageView {
    pub type_id: i64,
    pub name: String,
    pub buy_hub: String,
    pub sell_hub: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub profit_per_unit: f64,
    pub margin_pct: f64,
}

/// Scan a list of item types for the best cross-hub flip per item (buy at the
/// cheapest hub, sell at the richest), ranked by per-unit profit after sales
/// tax. When `typeIds` is omitted, a curated default set is scanned.
/// `salesTax` is a fraction (0.045 = 4.5%).
#[tauri::command]
pub async fn scan_arbitrage(
    state: State<'_, AppState>,
    type_ids: Option<Vec<i64>>,
    sales_tax: Option<f64>,
) -> CmdResult<Vec<ArbitrageView>> {
    let ids = type_ids.unwrap_or_else(default_scan_types);
    let mut fees = eve_core::marketdata::TradeFees::default();
    if let Some(t) = sales_tax {
        fees.sales_tax = t;
    }
    let opps = state
        .marketdata
        .arbitrage(&ids, fees)
        .await
        .map_err(|e| e.to_string())?;
    let names = names_for(&state, &opps.iter().map(|o| o.type_id).collect::<Vec<_>>()).await;
    Ok(opps
        .into_iter()
        .map(|o| ArbitrageView {
            type_id: o.type_id,
            name: named(&names, o.type_id),
            buy_hub: o.flip.buy_hub,
            sell_hub: o.flip.sell_hub,
            buy_price: o.flip.buy_price,
            sell_price: o.flip.sell_price,
            profit_per_unit: o.flip.profit_per_unit,
            margin_pct: o.flip.margin_pct,
        })
        .collect())
}

/// A small curated set of liquid, commonly-flipped items for the default scan
/// when the caller supplies no list (minerals, salvage, common modules/ships).
fn default_scan_types() -> Vec<i64> {
    vec![
        34, 35, 36, 37, 38, 39, 40, 11399, // minerals
        16240, 587, 597, 603, 593, 24698, 24702, // common hulls
    ]
}

/// A refined-material line with its name resolved.
#[derive(Debug, Serialize)]
pub struct RefineYieldView {
    pub type_id: i64,
    pub name: String,
    pub quantity: i64,
    pub value: f64,
}

/// Reprocessing result: refined yields + refine-vs-sell verdict, or `null` when
/// the SDE has no reprocessing data for the item (seed-only without full SDE).
#[derive(Debug, Serialize)]
pub struct ReprocessView {
    pub portions: i64,
    pub leftover_units: i64,
    pub yields: Vec<RefineYieldView>,
    pub refined_value: f64,
    pub sell_value: f64,
    pub advantage: f64,
}

/// Refine `units` of an item at a given efficiency (0..1, default 0.5), valuing
/// the output against the public market price reference.
#[tauri::command]
pub async fn reprocess_item(
    state: State<'_, AppState>,
    type_id: i64,
    units: i64,
    efficiency: Option<f64>,
) -> CmdResult<Option<ReprocessView>> {
    let prices = state.prices.price_map().await.map_err(|e| e.to_string())?;
    let eff = efficiency.unwrap_or(0.5);
    let Some(result) = state
        .reprocess
        .refine(type_id, units, eff, &prices)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let ids: Vec<i64> = result.yields.iter().map(|y| y.type_id).collect();
    let names = names_for(&state, &ids).await;
    let vs = eve_core::reprocess::refine_vs_sell(
        units,
        result.refined_value,
        prices.price(type_id).unwrap_or(0.0),
    );
    Ok(Some(ReprocessView {
        portions: result.portions,
        leftover_units: result.leftover_units,
        yields: result
            .yields
            .into_iter()
            .map(|y| RefineYieldView {
                type_id: y.type_id,
                name: named(&names, y.type_id),
                quantity: y.quantity,
                value: y.value,
            })
            .collect(),
        refined_value: result.refined_value,
        sell_value: vs.sell_value,
        advantage: vs.advantage,
    }))
}

/// A build-plan material line with its name resolved.
#[derive(Debug, Serialize)]
pub struct PlanLineView {
    pub type_id: i64,
    pub name: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub value: f64,
}

/// A priced bill-of-materials for a manufacturing/reaction job, or `null` when
/// the SDE has no blueprint for the product (seed-only without full SDE).
#[derive(Debug, Serialize)]
pub struct BuildPlanView {
    pub product_type_id: i64,
    pub product_name: String,
    pub runs: i64,
    pub me: i64,
    pub output_units: i64,
    pub materials: Vec<PlanLineView>,
    pub material_cost: f64,
    pub product_value: f64,
    pub profit: f64,
    pub margin_pct: f64,
    /// Base invention success chance (null for manufacturing/reaction).
    pub probability: Option<f64>,
}

/// Plan a manufacturing (default) or reaction job: bill-of-materials after ME,
/// priced against the market, with build-vs-buy profit.
#[tauri::command]
pub async fn plan_build(
    state: State<'_, AppState>,
    product_type_id: i64,
    runs: i64,
    me: i64,
    activity: Option<String>,
) -> CmdResult<Option<BuildPlanView>> {
    let prices = state.prices.price_map().await.map_err(|e| e.to_string())?;
    let activity = activity.unwrap_or_else(|| "manufacturing".to_string());
    let Some(plan) = state
        .industry_plan
        .plan(product_type_id, runs.max(1), me, &activity, &prices)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(None);
    };
    let mut ids: Vec<i64> = plan.materials.iter().map(|m| m.type_id).collect();
    ids.push(product_type_id);
    let names = names_for(&state, &ids).await;
    Ok(Some(BuildPlanView {
        product_type_id: plan.product_type_id,
        product_name: named(&names, product_type_id),
        runs: plan.runs,
        me: plan.me,
        output_units: plan.output_units,
        materials: plan
            .materials
            .into_iter()
            .map(|m| PlanLineView {
                type_id: m.type_id,
                name: named(&names, m.type_id),
                quantity: m.quantity,
                unit_price: m.unit_price,
                value: m.value,
            })
            .collect(),
        material_cost: plan.material_cost,
        product_value: plan.product_value,
        profit: plan.profit,
        margin_pct: plan.margin_pct,
        probability: plan.probability,
    }))
}

/// One target row of a skill plan from the UI.
#[derive(Debug, serde::Deserialize)]
pub struct SkillTarget {
    pub skill_type_id: i64,
    pub target_level: i64,
}

/// A costed plan step with the skill name + current level filled in.
#[derive(Debug, Serialize)]
pub struct SkillStepView {
    pub skill_type_id: i64,
    pub name: String,
    pub current_level: i64,
    pub target_level: i64,
    pub sp: i64,
    pub seconds: i64,
    /// False when the SDE has no rank/attributes for the skill (seed-only).
    pub known: bool,
}

/// A costed skill plan for a character.
#[derive(Debug, Serialize)]
pub struct SkillPlanView {
    pub steps: Vec<SkillStepView>,
    pub total_sp: i64,
    pub total_seconds: i64,
}

/// Map a dogma attribute type id to the character's attribute value.
fn attr_value(a: &eve_core::character::CharacterAttributes, attr_id: i64) -> f64 {
    let v = match attr_id {
        165 => a.intelligence,
        166 => a.memory,
        167 => a.perception,
        168 => a.willpower,
        164 => a.charisma,
        _ => 0,
    };
    v as f64
}

/// Cost a skill plan for a character: per-skill SP and training time (using the
/// character's current levels + attributes and SDE skill ranks) plus totals.
/// Skills the SDE doesn't know are returned with `known = false` and zero cost.
#[tauri::command]
pub async fn cost_skill_plan(
    state: State<'_, AppState>,
    character_id: i64,
    targets: Vec<SkillTarget>,
) -> CmdResult<SkillPlanView> {
    use eve_core::skillplan::{sp_for_level, training_seconds};

    let sheet = state
        .character
        .skills(character_id)
        .await
        .map_err(|e| e.to_string())?;
    let attrs = state
        .character
        .attributes(character_id)
        .await
        .map_err(|e| e.to_string())?;
    let current: std::collections::HashMap<i64, i64> = sheet
        .skills
        .iter()
        .map(|s| (s.skill_id, s.trained_skill_level))
        .collect();

    let ids: Vec<i64> = targets.iter().map(|t| t.skill_type_id).collect();
    let names = names_for(&state, &ids).await;

    let mut steps = Vec::with_capacity(targets.len());
    let mut total_sp = 0;
    let mut total_seconds = 0;
    for t in &targets {
        let current_level = current.get(&t.skill_type_id).copied().unwrap_or(0);
        let meta = state
            .names
            .sde()
            .skill_meta(t.skill_type_id)
            .await
            .map_err(|e| e.to_string())?;
        let (sp, seconds, known) = match meta {
            Some(m) => {
                let from = sp_for_level(m.rank, current_level);
                let to = sp_for_level(m.rank, t.target_level);
                let sp = (to - from).max(0);
                let seconds = training_seconds(
                    sp,
                    attr_value(&attrs, m.primary_attr),
                    attr_value(&attrs, m.secondary_attr),
                );
                (sp, seconds, true)
            }
            None => (0, 0, false),
        };
        total_sp += sp;
        total_seconds += seconds;
        steps.push(SkillStepView {
            skill_type_id: t.skill_type_id,
            name: named(&names, t.skill_type_id),
            current_level,
            target_level: t.target_level,
            sp,
            seconds,
            known,
        });
    }
    Ok(SkillPlanView { steps, total_sp, total_seconds })
}

/// Rank income activities by risk-adjusted ISK return over the time available.
/// Each activity is described in ISK terms (gross ISK/hr, risk, setup cost,
/// eligibility); ranking lives in `eve_core::income::rank_income`.
#[tauri::command]
pub fn rank_income(
    activities: Vec<eve_core::income::IncomeActivity>,
    hours: f64,
) -> CmdResult<Vec<eve_core::income::IncomeRanking>> {
    Ok(eve_core::income::rank_income(&activities, hours))
}

/// Rank candidate skill plans by ISK return on training time. Each plan is
/// described in ISK terms (income unlocked, time to train, optional upfront
/// cost); the deterministic ranking lives in `eve_core::skillplan::rank_roi`.
#[tauri::command]
pub fn rank_skill_roi(
    plans: Vec<eve_core::skillplan::RoiPlan>,
) -> CmdResult<Vec<eve_core::skillplan::RoiResult>> {
    Ok(eve_core::skillplan::rank_roi(&plans))
}

/// One active incursion with its staging system + faction named.
#[derive(Debug, Serialize)]
pub struct IncursionView {
    pub staging_system: String,
    pub faction: String,
    pub state: String,
    /// 0–100; lower means more farmed.
    pub influence_pct: f64,
    pub has_boss: bool,
    pub system_count: i64,
}

/// Active incursions (public), freshest first, with staging-system + faction
/// names resolved.
#[tauri::command]
pub async fn get_incursions(state: State<'_, AppState>) -> CmdResult<Vec<IncursionView>> {
    let incursions = state.pve.incursions().await.map_err(|e| e.to_string())?;
    let mut ids: Vec<i64> = Vec::new();
    for i in &incursions {
        ids.push(i.staging_solar_system_id);
        ids.push(i.faction_id);
    }
    let names = names_for(&state, &ids).await;
    Ok(incursions
        .into_iter()
        .map(|i| IncursionView {
            staging_system: named(&names, i.staging_solar_system_id),
            faction: named(&names, i.faction_id),
            state: i.state.replace('_', " "),
            influence_pct: i.influence * 100.0,
            has_boss: i.has_boss,
            system_count: i.infested_solar_systems.len() as i64,
        })
        .collect())
}

/// One fleet member, named.
#[derive(Debug, Serialize)]
pub struct FleetMemberView {
    pub name: String,
    pub ship: String,
    pub system: String,
    pub role: String,
}

/// Live fleet composition for the active character.
#[derive(Debug, Serialize)]
pub struct FleetView {
    pub in_fleet: bool,
    pub member_count: i64,
    pub members: Vec<FleetMemberView>,
}

/// The character's current fleet composition (members + ships + systems). When
/// the character isn't in a fleet (or lacks the scope), `in_fleet` is false.
#[tauri::command]
pub async fn get_fleet(state: State<'_, AppState>, character_id: i64) -> CmdResult<FleetView> {
    let Ok(info) = state.fleet.current(character_id).await else {
        return Ok(FleetView { in_fleet: false, member_count: 0, members: Vec::new() });
    };
    let members = state
        .fleet
        .members(character_id, info.fleet_id)
        .await
        .unwrap_or_default();

    let mut ids: Vec<i64> = Vec::new();
    for m in &members {
        ids.push(m.character_id);
        ids.push(m.ship_type_id);
        ids.push(m.solar_system_id);
    }
    let names = names_for(&state, &ids).await;

    Ok(FleetView {
        in_fleet: true,
        member_count: members.len() as i64,
        members: members
            .into_iter()
            .map(|m| FleetMemberView {
                name: named(&names, m.character_id),
                ship: named(&names, m.ship_type_id),
                system: named(&names, m.solar_system_id),
                role: m.role_name.replace('_', " "),
            })
            .collect(),
    })
}

/// Current EVE-Scout Thera/Turnur wormhole connections (public 3P), soonest to
/// collapse first. Empty/best-effort if EVE-Scout is unreachable.
#[tauri::command]
pub async fn get_thera_connections(
    state: State<'_, AppState>,
) -> CmdResult<Vec<eve_core::eve_scout::TheraConnection>> {
    Ok(state.eve_scout.connections().await.unwrap_or_default())
}

/// The character's upcoming calendar events (frontend renders the countdown).
/// Empty when the scope isn't granted.
#[tauri::command]
pub async fn get_calendar(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<eve_core::calendar::CalendarEvent>> {
    Ok(state.calendar.events(character_id).await.unwrap_or_default())
}

/// A personal bookmark with its location named.
#[derive(Debug, Serialize)]
pub struct BookmarkView {
    pub bookmark_id: i64,
    pub label: String,
    pub notes: String,
    pub location_name: String,
    pub created: String,
}

/// The character's personal bookmarks (newest first), location names resolved
/// (citadels included). Empty when the scope isn't granted.
#[tauri::command]
pub async fn get_bookmarks(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<BookmarkView>> {
    let bookmarks = match state.bookmarks.bookmarks(character_id).await {
        Ok(b) => b,
        Err(_) => return Ok(Vec::new()),
    };
    let loc_ids: Vec<i64> = bookmarks.iter().map(|b| b.location_id).collect();
    let names = names_with_structures(&state, character_id, &loc_ids).await;
    Ok(bookmarks
        .into_iter()
        .map(|b| BookmarkView {
            location_name: names
                .get(&b.location_id)
                .cloned()
                .unwrap_or_else(|| format!("Location {}", b.location_id)),
            bookmark_id: b.bookmark_id,
            label: b.label,
            notes: b.notes,
            created: b.created,
        })
        .collect())
}

/// One R&D agent with its datacore type named (frontend computes accrued RP).
#[derive(Debug, Serialize)]
pub struct ResearchAgentView {
    pub agent_name: String,
    pub datacore_name: String,
    pub points_per_day: f64,
    pub remainder_points: f64,
    pub started_at: String,
}

/// The character's running R&D agents (passive datacore income). Empty when the
/// scope isn't granted. The frontend renders accrued RP from `started_at`.
#[tauri::command]
pub async fn get_research_agents(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<ResearchAgentView>> {
    let agents = match state.research.agents(character_id).await {
        Ok(a) => a,
        Err(_) => return Ok(Vec::new()),
    };
    let mut ids: Vec<i64> = Vec::new();
    for a in &agents {
        ids.push(a.agent_id);
        ids.push(a.skill_type_id);
    }
    let names = names_for(&state, &ids).await;
    Ok(agents
        .into_iter()
        .map(|a| ResearchAgentView {
            agent_name: named(&names, a.agent_id),
            datacore_name: named(&names, a.skill_type_id),
            points_per_day: a.points_per_day,
            remainder_points: a.remainder_points,
            started_at: a.started_at,
        })
        .collect())
}

/// One contested faction-warfare system, named.
#[derive(Debug, Serialize)]
pub struct FwSystemView {
    pub system_name: String,
    pub owner: String,
    pub occupier: String,
    pub contested: String,
    /// Contest progress toward a flip, 0–100.
    pub progress_pct: f64,
}

/// Contested faction-warfare systems (public), most-contested first, with
/// system + faction names resolved. Capped to the hottest 40.
#[tauri::command]
pub async fn get_fw_systems(state: State<'_, AppState>) -> CmdResult<Vec<FwSystemView>> {
    let mut systems = state.pve.contested_fw_systems().await.map_err(|e| e.to_string())?;
    systems.truncate(40);
    let mut ids: Vec<i64> = Vec::new();
    for s in &systems {
        ids.push(s.solar_system_id);
        ids.push(s.owner_faction_id);
        ids.push(s.occupier_faction_id);
    }
    let names = names_for(&state, &ids).await;
    Ok(systems
        .into_iter()
        .map(|s| FwSystemView {
            system_name: named(&names, s.solar_system_id),
            owner: named(&names, s.owner_faction_id),
            occupier: named(&names, s.occupier_faction_id),
            progress_pct: s.progress() * 100.0,
            contested: s.contested.replace('_', " "),
        })
        .collect())
}

/// A valued LP-store offer with names resolved.
#[derive(Debug, Serialize)]
pub struct LpOfferView {
    pub offer_id: i64,
    pub name: String,
    pub quantity: i64,
    pub lp_cost: i64,
    pub total_isk_cost: f64,
    pub output_value: f64,
    pub profit: f64,
    pub isk_per_lp: f64,
}

/// LP-store result for a corporation.
#[derive(Debug, Serialize)]
pub struct LpStoreView {
    pub found: bool,
    pub corporation: String,
    pub offers: Vec<LpOfferView>,
    pub message: String,
}

/// Rank a corporation's LP-store offers by ISK-per-LP (output market value minus
/// ISK + required-item cost, over LP cost). `corporation` is resolved by name.
#[tauri::command]
pub async fn lp_store(state: State<'_, AppState>, corporation: String) -> CmdResult<LpStoreView> {
    let corporation = corporation.trim().to_string();
    let Some(corp_id) = state
        .names
        .corporation_id(&corporation)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(LpStoreView {
            found: false,
            corporation,
            offers: Vec::new(),
            message: "Corporation not found.".into(),
        });
    };

    let offers = match state.lp.offers(corp_id).await {
        Ok(o) => o,
        Err(_) => {
            return Ok(LpStoreView {
                found: false,
                corporation,
                offers: Vec::new(),
                message: "No LP store for that corporation.".into(),
            })
        }
    };
    let prices = state.prices.price_map().await.unwrap_or_default();
    let ranked = eve_core::lp::rank_offers(&offers, &prices);

    let ids: Vec<i64> = ranked.iter().map(|o| o.type_id).collect();
    let names = names_for(&state, &ids).await;

    Ok(LpStoreView {
        found: true,
        corporation,
        offers: ranked
            .into_iter()
            .map(|o| LpOfferView {
                offer_id: o.offer_id,
                name: named(&names, o.type_id),
                quantity: o.quantity,
                lp_cost: o.lp_cost,
                total_isk_cost: o.total_isk_cost,
                output_value: o.output_value,
                profit: o.profit,
                isk_per_lp: o.isk_per_lp,
            })
            .collect(),
        message: String::new(),
    })
}

/// A corp structure rolled up for the UI (names resolved + fuel countdown).
#[derive(Debug, Serialize)]
pub struct CorpStructureView {
    pub structure_id: i64,
    pub name: String,
    pub type_name: String,
    pub system_name: String,
    pub state: String,
    pub fuel_seconds_remaining: i64,
    pub has_fuel_timer: bool,
}

/// The active (or given) character's corp structures with fuel-expiry
/// countdowns, soonest first. Empty when the character lacks the role/scope
/// (the endpoint 403s) — surfaced as an empty list, not an error.
#[tauri::command]
pub async fn get_corp_structures(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<CorpStructureView>> {
    let Ok(public) = state.character.public_info(character_id).await else {
        return Ok(Vec::new());
    };
    let summary = match state.corp.structure_status(character_id, public.corporation_id).await {
        Ok(s) => s,
        // No director/station-manager role, or scope not granted → no access.
        Err(_) => return Ok(Vec::new()),
    };

    // Resolve type + system names in one batch; structure names need the
    // authenticated structures endpoint.
    let mut ids: Vec<i64> = Vec::new();
    for s in &summary {
        ids.push(s.type_id);
        ids.push(s.system_id);
    }
    let names = names_for(&state, &ids).await;
    let struct_ids: Vec<i64> = summary.iter().map(|s| s.structure_id).collect();
    let struct_names = names_with_structures(&state, character_id, &struct_ids).await;

    Ok(summary
        .into_iter()
        .map(|s| CorpStructureView {
            name: struct_names
                .get(&s.structure_id)
                .cloned()
                .unwrap_or_else(|| format!("Structure {}", s.structure_id)),
            type_name: named(&names, s.type_id),
            system_name: named(&names, s.system_id),
            state: s.state.replace('_', " "),
            fuel_seconds_remaining: s.fuel_seconds_remaining,
            has_fuel_timer: s.fuel_expires.is_some(),
            structure_id: s.structure_id,
        })
        .collect())
}

/// One corp member row (names resolved) for the vetting view.
#[derive(Debug, Serialize)]
pub struct CorpMemberView {
    pub character_id: i64,
    pub name: String,
    pub ship_name: String,
    pub location_name: String,
    pub logon_date: Option<String>,
    pub logoff_date: Option<String>,
}

/// Corp member tracking (last logon/logoff, current location + ship) for the
/// active character's corp, most-recently-active first. Empty when the
/// character lacks the director role/scope.
#[tauri::command]
pub async fn get_corp_members(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<CorpMemberView>> {
    let Ok(public) = state.character.public_info(character_id).await else {
        return Ok(Vec::new());
    };
    let members = match state.corp.member_tracking(character_id, public.corporation_id).await {
        Ok(m) => m,
        Err(_) => return Ok(Vec::new()),
    };

    // Resolve character + ship names in one batch; locations may be structures.
    let mut ids: Vec<i64> = Vec::new();
    let mut loc_ids: Vec<i64> = Vec::new();
    for m in &members {
        ids.push(m.character_id);
        if let Some(s) = m.ship_type_id {
            ids.push(s);
        }
        if let Some(l) = m.location_id {
            ids.push(l);
            loc_ids.push(l);
        }
    }
    let names = names_for(&state, &ids).await;
    let struct_names = names_with_structures(&state, character_id, &loc_ids).await;
    let loc_name = |id: Option<i64>| match id {
        Some(id) => struct_names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| named(&names, id)),
        None => String::new(),
    };

    Ok(members
        .into_iter()
        .map(|m| CorpMemberView {
            name: named(&names, m.character_id),
            ship_name: m.ship_type_id.map(|s| named(&names, s)).unwrap_or_default(),
            location_name: loc_name(m.location_id),
            logon_date: m.logon_date,
            logoff_date: m.logoff_date,
            character_id: m.character_id,
        })
        .collect())
}

/// Parse a pasted EFT fit and resolve its ship + modules to type ids via the
/// SDE. Returns `null` when the EFT header is malformed. Unresolved names (those
/// the current SDE doesn't know) are listed so the UI can flag them.
#[tauri::command]
pub async fn parse_fit(
    state: State<'_, AppState>,
    eft: String,
) -> CmdResult<Option<eve_core::fitting::ResolvedFit>> {
    state.fitting.resolve_eft(&eft).await.map_err(|e| e.to_string())
}

/// Parse pasted D-scan clipboard text into a grouped readout with danger
/// callouts (combat probes, tackle hulls). Pure — needs no character or network.
#[tauri::command]
pub fn parse_dscan(text: String) -> eve_core::dscan::DscanResult {
    eve_core::dscan::parse_dscan(&text)
}

/// After-action / DPS summary of the latest combat log, plus a flag for whether
/// any log was found (so the UI can distinguish "no fights" from "no logs dir").
#[derive(Debug, Serialize)]
pub struct CombatLogView {
    pub found: bool,
    pub summary: Option<eve_core::logs::gamelog::AarSummary>,
}

/// Read the most recent Gamelog and summarize it into an after-action report
/// (damage dealt/received, DPS, top targets/attackers). Passive local-file read.
#[tauri::command]
pub fn get_combat_summary() -> CombatLogView {
    let path = eve_core::logs::gamelogs_dir().and_then(|d| eve_core::logs::latest_log(&d, ""));
    let Some(path) = path else {
        return CombatLogView { found: false, summary: None };
    };
    let Some(text) = eve_core::logs::read_log(&path) else {
        return CombatLogView { found: false, summary: None };
    };
    let events = eve_core::logs::gamelog::parse_gamelog(&text);
    CombatLogView { found: true, summary: Some(eve_core::logs::gamelog::summarize_combat(&events)) }
}

/// A combined multi-box fleet after-action report, plus a found flag.
#[derive(Debug, Serialize)]
pub struct FleetAarView {
    pub found: bool,
    pub fleet: Option<eve_core::logs::gamelog::FleetAar>,
}

/// Read the most recent Gamelogs (one per boxed character) and merge them into a
/// combined fleet after-action report: per-pilot DPS contributions plus fleet
/// totals and combined target/attacker tables. Passive local-file reads.
#[tauri::command]
pub fn get_fleet_aar(max_pilots: Option<usize>) -> FleetAarView {
    let Some(dir) = eve_core::logs::gamelogs_dir() else {
        return FleetAarView { found: false, fleet: None };
    };
    let max = max_pilots.unwrap_or(8).clamp(1, 32);
    let paths = eve_core::logs::recent_logs(&dir, "", max);
    if paths.is_empty() {
        return FleetAarView { found: false, fleet: None };
    }
    let mut pilots = Vec::new();
    for path in paths {
        let Some(text) = eve_core::logs::read_log(&path) else { continue };
        let events = eve_core::logs::gamelog::parse_gamelog(&text);
        if events.is_empty() {
            continue;
        }
        // Label by the log's file stem (timestamp_listenerId) — the pilot is not
        // in the file body, but this disambiguates the streams.
        let label = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("pilot")
            .to_string();
        pilots.push((label, events));
    }
    if pilots.is_empty() {
        return FleetAarView { found: false, fleet: None };
    }
    FleetAarView {
        found: true,
        fleet: Some(eve_core::logs::gamelog::merge_fleet_aar(pilots)),
    }
}

/// Read the most recent Local chatlog and summarize who has spoken + the current
/// system. Passive local-file read; the EULA-safe Local intel signal.
#[tauri::command]
pub fn get_local_intel() -> Option<eve_core::logs::chatlog::LocalIntel> {
    let path = eve_core::logs::chatlogs_dir().and_then(|d| eve_core::logs::latest_log(&d, "Local"))?;
    let text = eve_core::logs::read_log(&path)?;
    Some(eve_core::logs::chatlog::summarize_local(&text))
}

/// One system in the safety readout.
#[derive(Debug, Serialize)]
pub struct SafetySystemView {
    pub system_id: i64,
    pub name: String,
    pub security: f64,
    pub kills_last_hour: i64,
    /// Jumps from the active character (0 = current system).
    pub jumps: i64,
}

/// The active character's neighbourhood safety: kills in the current system and
/// each adjacent system over the last hour, with an overall flag.
#[derive(Debug, Serialize)]
pub struct SystemSafetyView {
    /// False when there's no active character or its location is unavailable.
    pub found: bool,
    pub current: Option<SafetySystemView>,
    pub neighbors: Vec<SafetySystemView>,
    pub total_kills: i64,
    pub level: String,
    pub message: String,
}

/// System-safety readout for the active character: its current system + the
/// systems one jump away, each with recent kill volume (zKillboard). Topology is
/// cached, so the cost is mostly the per-system kill counts.
#[tauri::command]
pub async fn get_system_safety(state: State<'_, AppState>) -> CmdResult<SystemSafetyView> {
    let empty = |found: bool| SystemSafetyView {
        found,
        current: None,
        neighbors: Vec::new(),
        total_kills: 0,
        level: "Safe".into(),
        message: if found { "Quiet.".into() } else { "No active character.".into() },
    };

    let characters = state.db.list_characters().await.map_err(|e| e.to_string())?;
    let Some(active) = characters.iter().find(|c| c.active) else {
        return Ok(empty(false));
    };
    let Ok(loc) = state.character.location(active.id).await else {
        return Ok(empty(false));
    };
    let current_id = loc.solar_system_id;
    let info = state.universe.system_info(current_id).await.map_err(|e| e.to_string())?;
    let neighbor_ids: Vec<i64> = state
        .universe
        .neighbors(current_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .take(12)
        .collect();

    let current_kills = state.zkill.system_kill_count(current_id, 3600).await.unwrap_or(0);
    let mut total = current_kills;
    let mut neighbors = Vec::new();
    for nid in neighbor_ids {
        let ninfo = state.universe.system_info(nid).await.ok();
        let kills = state.zkill.system_kill_count(nid, 3600).await.unwrap_or(0);
        total += kills;
        neighbors.push(SafetySystemView {
            system_id: nid,
            name: ninfo.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| format!("System {nid}")),
            security: ninfo.as_ref().map(|i| i.security_status).unwrap_or(0.0),
            kills_last_hour: kills,
            jumps: 1,
        });
    }
    neighbors.sort_by(|a, b| b.kills_last_hour.cmp(&a.kills_last_hour));

    let a = eve_core::intel::assess_safety(total);
    Ok(SystemSafetyView {
        found: true,
        current: Some(SafetySystemView {
            system_id: current_id,
            name: info.name,
            security: info.security_status,
            kills_last_hour: current_kills,
            jumps: 0,
        }),
        neighbors,
        total_kills: total,
        level: a.level.as_str().to_string(),
        message: a.message,
    })
}

/// One unified risk read for the active character's current system.
#[derive(Debug, Serialize)]
pub struct SystemRiskView {
    pub found: bool,
    pub system_id: i64,
    pub system_name: String,
    /// 0 (clear) … 100 (extreme).
    pub score: i64,
    pub level: String,
    pub reasons: Vec<String>,
}

/// One unified threat number for the active character's current system, fusing
/// in-system + neighbour kills (zKill), security band, and a gate-camp flag from
/// recent kill volume into a single 0–100 score. The deterministic scoring lives
/// in `eve_core::intel::score_system_risk`.
#[tauri::command]
pub async fn get_system_risk(state: State<'_, AppState>) -> CmdResult<SystemRiskView> {
    let empty = |found: bool| SystemRiskView {
        found,
        system_id: 0,
        system_name: String::new(),
        score: 0,
        level: "Safe".into(),
        reasons: vec![if found { "Quiet.".into() } else { "No active character.".into() }],
    };

    let characters = state.db.list_characters().await.map_err(|e| e.to_string())?;
    let Some(active) = characters.iter().find(|c| c.active) else {
        return Ok(empty(false));
    };
    let Ok(loc) = state.character.location(active.id).await else {
        return Ok(empty(false));
    };
    let current_id = loc.solar_system_id;
    let info = state.universe.system_info(current_id).await.map_err(|e| e.to_string())?;
    let neighbor_ids: Vec<i64> = state
        .universe
        .neighbors(current_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .take(12)
        .collect();

    let system_kills = state.zkill.system_kill_count(current_id, 3600).await.unwrap_or(0);
    let mut neighbour_kills = 0;
    for nid in neighbor_ids {
        neighbour_kills += state.zkill.system_kill_count(nid, 3600).await.unwrap_or(0);
    }

    let inputs = eve_core::intel::RiskInputs {
        system_kills,
        neighbour_kills,
        danger_pilots: 0,
        caution_pilots: 0,
        security: info.security_status,
        // Heavy recent in-system kill volume is the gate-camp signal.
        gate_camp: system_kills > 3,
    };
    let risk = eve_core::intel::score_system_risk(&inputs);
    Ok(SystemRiskView {
        found: true,
        system_id: current_id,
        system_name: info.name,
        score: risk.score,
        level: risk.level.as_str().to_string(),
        reasons: risk.reasons,
    })
}

/// A system positioned on the region map, with its recent ship-kill count and
/// sovereignty owner (alliance) when claimed.
#[derive(Debug, Serialize)]
pub struct MapNode {
    pub system_id: i64,
    pub name: String,
    pub security: f64,
    pub x: f64,
    pub z: f64,
    pub kills: i64,
    /// Sovereignty-holding alliance id (0 = unclaimed/high-sec).
    pub sov_alliance_id: i64,
    pub sov_owner: String,
}

/// A Dotlan-style region map: positioned systems + intra-region jumps + a kill
/// heatmap (ESI hourly).
#[derive(Debug, Serialize)]
pub struct RegionMapView {
    pub found: bool,
    pub region_id: i64,
    pub region_name: String,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<(i64, i64)>,
    pub message: String,
}

/// Region map for `region` (by name), or the active character's current region
/// when omitted. Needs the universe data in the SDE (build it with
/// `--systems-csv/--jumps-csv/--regions-csv`); otherwise `found` is false.
#[tauri::command]
pub async fn get_region_map(
    state: State<'_, AppState>,
    region: Option<String>,
) -> CmdResult<RegionMapView> {
    let sde = state.names.sde();
    let empty = |msg: &str| RegionMapView {
        found: false,
        region_id: 0,
        region_name: String::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        message: msg.to_string(),
    };

    // Resolve the region: explicit name, else the active character's location.
    let region_id = if let Some(name) = region.as_ref().filter(|s| !s.trim().is_empty()) {
        match sde.region_id_by_name(name.trim()).await.map_err(|e| e.to_string())? {
            Some(id) => id,
            None => return Ok(empty("Region not found (rebuild the SDE with universe data?).")),
        }
    } else {
        let characters = state.db.list_characters().await.map_err(|e| e.to_string())?;
        let Some(active) = characters.iter().find(|c| c.active) else {
            return Ok(empty("No active character; pick a region by name."));
        };
        let Ok(loc) = state.character.location(active.id).await else {
            return Ok(empty("Character location unavailable; pick a region by name."));
        };
        match sde.system_region(loc.solar_system_id).await.map_err(|e| e.to_string())? {
            Some(id) => id,
            None => return Ok(empty("Universe data missing — rebuild the SDE (systems CSV).")),
        }
    };

    let systems = sde.systems_in_region(region_id).await.map_err(|e| e.to_string())?;
    if systems.is_empty() {
        return Ok(empty("No systems for this region — rebuild the SDE with universe data."));
    }
    let edges = sde.jumps_in_region(region_id).await.map_err(|e| e.to_string())?;
    let region_name = sde
        .list_regions()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|(id, _)| *id == region_id)
        .map(|(_, n)| n)
        .unwrap_or_else(|| format!("Region {region_id}"));

    // Kill heatmap + sovereignty overlay (one ESI call each, best-effort).
    let kills: std::collections::HashMap<i64, i64> = state
        .universe
        .system_kills()
        .await
        .map(|v| v.into_iter().map(|k| (k.system_id, k.ship_kills + k.pod_kills)).collect())
        .unwrap_or_default();
    let sov: std::collections::HashMap<i64, i64> = state
        .universe
        .sovereignty()
        .await
        .map(|v| {
            v.into_iter()
                .filter_map(|s| s.alliance_id.map(|a| (s.system_id, a)))
                .collect()
        })
        .unwrap_or_default();

    // Resolve sov-holding alliance names for systems in this region.
    let alliance_ids: Vec<i64> = systems
        .iter()
        .filter_map(|s| sov.get(&s.system_id).copied())
        .collect();
    let alliance_names = names_for(&state, &alliance_ids).await;

    let nodes = systems
        .into_iter()
        .map(|s| {
            let sov_alliance_id = sov.get(&s.system_id).copied().unwrap_or(0);
            MapNode {
                kills: kills.get(&s.system_id).copied().unwrap_or(0),
                sov_owner: if sov_alliance_id != 0 {
                    named(&alliance_names, sov_alliance_id)
                } else {
                    String::new()
                },
                sov_alliance_id,
                system_id: s.system_id,
                name: s.name,
                security: s.security,
                x: s.x,
                z: s.z,
            }
        })
        .collect();

    Ok(RegionMapView { found: true, region_id, region_name, nodes, edges, message: String::new() })
}

/// The list of regions (id + name) for the map picker. Empty until the SDE has
/// universe data.
#[tauri::command]
pub async fn list_map_regions(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let regions = state.names.sde().list_regions().await.map_err(|e| e.to_string())?;
    Ok(regions.into_iter().map(|(_, name)| name).collect())
}

/// The active character's jump fatigue (the frontend renders the countdown from
/// the expiry date). `null` when the scope isn't granted.
#[tauri::command]
pub async fn get_jump_fatigue(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Option<eve_core::character::JumpFatigue>> {
    Ok(state.character.fatigue(character_id).await.ok())
}

/// One hop in a planned route.
#[derive(Debug, Serialize)]
pub struct RouteHop {
    pub system_id: i64,
    pub name: String,
    pub security: f64,
}

/// A planned route between two systems.
#[derive(Debug, Serialize)]
pub struct RouteView {
    pub found: bool,
    /// Jumps = hops - 1 (0 when origin == destination or not found).
    pub jumps: i64,
    pub hops: Vec<RouteHop>,
    pub message: String,
}

/// Plan a route between two systems by name, using ESI's solver. `flag` is
/// "shortest" | "secure" | "insecure". Each hop is resolved to a name +
/// security (cached topology), so lowsec/null hops are visible.
#[tauri::command]
pub async fn plan_route(
    state: State<'_, AppState>,
    origin: String,
    destination: String,
    flag: Option<String>,
) -> CmdResult<RouteView> {
    let flag = eve_core::navigation::RouteFlag::parse(&flag.unwrap_or_default());
    let not_found = |message: &str| RouteView {
        found: false,
        jumps: 0,
        hops: Vec::new(),
        message: message.to_string(),
    };

    let (origin_id, dest_id) = tokio::join!(
        state.names.system_id(origin.trim()),
        state.names.system_id(destination.trim()),
    );
    let Some(origin_id) = origin_id.map_err(|e| e.to_string())? else {
        return Ok(not_found("Origin system not found."));
    };
    let Some(dest_id) = dest_id.map_err(|e| e.to_string())? else {
        return Ok(not_found("Destination system not found."));
    };

    let ids = state
        .navigation
        .route(origin_id, dest_id, flag)
        .await
        .map_err(|e| e.to_string())?;
    if ids.is_empty() {
        return Ok(not_found("No route found."));
    }

    let mut hops = Vec::with_capacity(ids.len());
    for id in &ids {
        let info = state.universe.system_info(*id).await.ok();
        hops.push(RouteHop {
            system_id: *id,
            name: info.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| format!("System {id}")),
            security: info.as_ref().map(|i| i.security_status).unwrap_or(0.0),
        });
    }
    let jumps = (hops.len() as i64 - 1).max(0);
    Ok(RouteView { found: true, jumps, hops, message: format!("{jumps} jumps") })
}

/// A hauling estimate: economics + route risk.
#[derive(Debug, Serialize)]
pub struct CourierView {
    pub found: bool,
    pub jumps: i64,
    pub reward_per_jump: f64,
    pub reward_per_m3: f64,
    pub collateral_ratio: f64,
    pub lowsec_hops: i64,
    pub kills_on_route: i64,
    pub verdict: String,
    pub hops: Vec<RouteHop>,
    pub message: String,
}

/// Estimate a courier/hauling job: solve the route, overlay the kill heatmap,
/// and compute reward/collateral economics + a risk verdict.
#[tauri::command]
pub async fn courier_estimate(
    state: State<'_, AppState>,
    origin: String,
    destination: String,
    volume: f64,
    collateral: f64,
    reward: f64,
    flag: Option<String>,
) -> CmdResult<CourierView> {
    let flag = eve_core::navigation::RouteFlag::parse(&flag.unwrap_or_default());
    let not_found = |message: &str| CourierView {
        found: false,
        jumps: 0,
        reward_per_jump: 0.0,
        reward_per_m3: 0.0,
        collateral_ratio: 0.0,
        lowsec_hops: 0,
        kills_on_route: 0,
        verdict: String::new(),
        hops: Vec::new(),
        message: message.to_string(),
    };

    let (origin_id, dest_id) = tokio::join!(
        state.names.system_id(origin.trim()),
        state.names.system_id(destination.trim()),
    );
    let Some(origin_id) = origin_id.map_err(|e| e.to_string())? else {
        return Ok(not_found("Origin system not found."));
    };
    let Some(dest_id) = dest_id.map_err(|e| e.to_string())? else {
        return Ok(not_found("Destination system not found."));
    };

    let ids = state
        .navigation
        .route(origin_id, dest_id, flag)
        .await
        .map_err(|e| e.to_string())?;
    if ids.is_empty() {
        return Ok(not_found("No route found."));
    }

    // Kill heatmap (one ESI call), then resolve each hop.
    let kills: std::collections::HashMap<i64, i64> = state
        .universe
        .system_kills()
        .await
        .map(|v| v.into_iter().map(|k| (k.system_id, k.ship_kills + k.pod_kills)).collect())
        .unwrap_or_default();

    let mut hops = Vec::with_capacity(ids.len());
    let mut lowsec_hops = 0;
    let mut kills_on_route = 0;
    for id in &ids {
        let info = state.universe.system_info(*id).await.ok();
        let security = info.as_ref().map(|i| i.security_status).unwrap_or(0.0);
        if security < 0.45 {
            lowsec_hops += 1;
        }
        kills_on_route += kills.get(id).copied().unwrap_or(0);
        hops.push(RouteHop {
            system_id: *id,
            name: info.as_ref().map(|i| i.name.clone()).unwrap_or_else(|| format!("System {id}")),
            security,
        });
    }

    let jumps = (hops.len() as i64 - 1).max(0);
    let est = eve_core::courier::estimate(volume, collateral, reward, jumps);
    let verdict = eve_core::courier::verdict(&est, lowsec_hops, kills_on_route);
    Ok(CourierView {
        found: true,
        jumps,
        reward_per_jump: est.reward_per_jump,
        reward_per_m3: est.reward_per_m3,
        collateral_ratio: est.collateral_ratio,
        lowsec_hops,
        kills_on_route,
        verdict,
        hops,
        message: String::new(),
    })
}

/// Set the active character's in-game autopilot waypoint to a system by name
/// (clears existing waypoints). EULA-sanctioned ESI write.
#[tauri::command]
pub async fn set_route_waypoint(
    state: State<'_, AppState>,
    character_id: i64,
    system: String,
) -> CmdResult<()> {
    let Some(system_id) = state.names.system_id(system.trim()).await.map_err(|e| e.to_string())?
    else {
        return Err("System not found.".to_string());
    };
    state
        .navigation
        .set_waypoint(character_id, system_id, false, true)
        .await
        .map_err(|e| e.to_string())
}

/// Open the in-game market window for a type (in-game UI bridge).
#[tauri::command]
pub async fn open_market_window(
    state: State<'_, AppState>,
    character_id: i64,
    type_id: i64,
) -> CmdResult<()> {
    state
        .navigation
        .open_market(character_id, type_id)
        .await
        .map_err(|e| e.to_string())
}

/// Gate-camp assessment for a named system.
#[derive(Debug, Serialize)]
pub struct GateCampView {
    pub system: String,
    pub found: bool,
    pub kills_last_hour: i64,
    pub level: String,
    pub message: String,
}

/// Assess gate-camp risk for a system by name: resolve it, count zKillboard
/// kills there in the last hour, and flag the likelihood. `found` is false when
/// the system name doesn't resolve.
#[tauri::command]
pub async fn gate_camp_check(
    state: State<'_, AppState>,
    system: String,
) -> CmdResult<GateCampView> {
    let system = system.trim().to_string();
    let Some(system_id) = state.names.system_id(&system).await.map_err(|e| e.to_string())? else {
        return Ok(GateCampView {
            system,
            found: false,
            kills_last_hour: 0,
            level: "Safe".into(),
            message: "System not found.".into(),
        });
    };
    let kills = state
        .zkill
        .system_kill_count(system_id, 3600)
        .await
        .unwrap_or(0);
    let a = eve_core::intel::assess_gatecamp(kills);
    Ok(GateCampView {
        system,
        found: true,
        kills_last_hour: a.kills_last_hour,
        level: a.level.as_str().to_string(),
        message: a.message,
    })
}

/// A single-pilot background check (affiliation + age + threat).
#[derive(Debug, Serialize)]
pub struct PilotBackgroundView {
    pub found: bool,
    pub name: String,
    pub corporation: String,
    pub alliance: Option<String>,
    pub security_status: f64,
    /// ISO birthday (the frontend renders character age from it).
    pub birthday: Option<String>,
    pub level: String,
    pub reasons: Vec<String>,
    pub danger_ratio: i64,
    pub ships_destroyed: i64,
    pub ships_lost: i64,
}

/// Background-check one pilot by name: resolve, pull ESI public info
/// (corp/alliance/sec/age) + zKillboard stats, and score the threat. `found` is
/// false when the name doesn't resolve to a character.
#[tauri::command]
pub async fn pilot_background(
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<PilotBackgroundView> {
    use eve_core::intel::score_pilot;

    let name = name.trim().to_string();
    let empty = PilotBackgroundView {
        found: false,
        name: name.clone(),
        corporation: String::new(),
        alliance: None,
        security_status: 0.0,
        birthday: None,
        level: "Safe".into(),
        reasons: Vec::new(),
        danger_ratio: 0,
        ships_destroyed: 0,
        ships_lost: 0,
    };
    let id_map = state
        .names
        .character_ids(std::slice::from_ref(&name))
        .await
        .map_err(|e| e.to_string())?;
    let Some(&id) = id_map.get(&name.to_lowercase()) else {
        return Ok(empty);
    };

    let public = state.character.public_info(id).await.ok();
    let stats = state.zkill.character_stats(id).await.unwrap_or_default();
    let ps = stats.to_pilot_stats();
    let threat = score_pilot(&ps);

    let mut ids = Vec::new();
    if let Some(p) = &public {
        ids.push(p.corporation_id);
        if let Some(a) = p.alliance_id {
            ids.push(a);
        }
    }
    let names = names_for(&state, &ids).await;

    Ok(PilotBackgroundView {
        found: true,
        name: public.as_ref().map(|p| p.name.clone()).unwrap_or(name),
        corporation: public
            .as_ref()
            .map(|p| named(&names, p.corporation_id))
            .unwrap_or_default(),
        alliance: public
            .as_ref()
            .and_then(|p| p.alliance_id)
            .map(|a| named(&names, a)),
        security_status: public.as_ref().map(|p| p.security_status).unwrap_or(0.0),
        birthday: public.as_ref().and_then(|p| p.birthday.clone()),
        level: threat.level.as_str().to_string(),
        reasons: threat.reasons,
        danger_ratio: ps.danger_ratio,
        ships_destroyed: ps.ships_destroyed,
        ships_lost: ps.ships_lost,
    })
}

/// One scored pilot in a Local threat scan.
#[derive(Debug, Serialize)]
pub struct PilotThreatView {
    pub name: String,
    pub level: String,
    pub reasons: Vec<String>,
    pub danger_ratio: i64,
    pub ships_destroyed: i64,
    pub sec_status: f64,
}

/// The result of scanning a list of pasted pilot names.
#[derive(Debug, Serialize)]
pub struct ThreatScanView {
    pub pilots: Vec<PilotThreatView>,
    pub summary: String,
    /// Pasted names that didn't resolve to a character.
    pub unresolved: Vec<String>,
}

/// Local threat scanner: resolve pasted pilot names to characters, pull each
/// one's zKillboard stats, and score them Safe/Neutral/Caution/Danger. The
/// deterministic score is computed in `eve_core::intel`; this just orchestrates
/// the name→id and killboard fetches and sorts the most dangerous first.
#[tauri::command]
pub async fn scan_pilots(
    state: State<'_, AppState>,
    names: Vec<String>,
) -> CmdResult<ThreatScanView> {
    use eve_core::intel::{score_pilot, summarize, ThreatLevel};

    let names: Vec<String> = names
        .into_iter()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    let id_map = state
        .names
        .character_ids(&names)
        .await
        .map_err(|e| e.to_string())?;

    // (name, level, threat, stats) so we can sort by level before serializing.
    let mut scored: Vec<(String, ThreatLevel, eve_core::intel::PilotThreat, eve_core::intel::PilotStats)> =
        Vec::new();
    let mut unresolved = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        let key = name.to_lowercase();
        if !seen.insert(key.clone()) {
            continue; // de-dup repeated names in the paste
        }
        match id_map.get(&key) {
            Some(&id) => {
                let stats = state.zkill.character_stats(id).await.unwrap_or_default();
                let ps = stats.to_pilot_stats();
                let threat = score_pilot(&ps);
                scored.push((name.clone(), threat.level, threat, ps));
            }
            None => unresolved.push(name.clone()),
        }
    }

    // Most dangerous first.
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let summary = summarize(&scored.iter().map(|s| s.1).collect::<Vec<_>>());
    let pilots = scored
        .into_iter()
        .map(|(name, _, threat, ps)| PilotThreatView {
            name,
            level: threat.level.as_str().to_string(),
            reasons: threat.reasons,
            danger_ratio: ps.danger_ratio,
            ships_destroyed: ps.ships_destroyed,
            sec_status: ps.sec_status,
        })
        .collect();

    Ok(ThreatScanView { pilots, summary, unresolved })
}

/// A single skill requirement the character hasn't met for a fit.
#[derive(Debug, Serialize)]
pub struct MissingSkillView {
    pub skill_type_id: i64,
    pub name: String,
    pub required_level: i64,
    pub current_level: i64,
    /// Training time from current to required level (0 if rank unknown).
    pub seconds: i64,
}

/// "Can I fly this fit?" verdict for the active character.
#[derive(Debug, Serialize)]
pub struct CanFlyView {
    pub ship: String,
    pub can_fly: bool,
    pub missing: Vec<MissingSkillView>,
    pub total_seconds: i64,
    /// Whether the fit parsed at all (false ⇒ malformed EFT).
    pub parsed: bool,
    /// Fit item names with no SDE type id — their skill needs can't be checked.
    pub unresolved: Vec<String>,
}

/// Check whether a character can fly a pasted EFT fit: collects the required
/// skills of the ship + every module/charge/drone from the SDE, compares to the
/// character's trained levels, and costs the training time for any gaps.
#[tauri::command]
pub async fn can_fly_fit(
    state: State<'_, AppState>,
    character_id: i64,
    eft: String,
) -> CmdResult<CanFlyView> {
    let Some(fit) = state.fitting.resolve_eft(&eft).await.map_err(|e| e.to_string())? else {
        return Ok(CanFlyView {
            ship: String::new(),
            can_fly: false,
            missing: Vec::new(),
            total_seconds: 0,
            parsed: false,
            unresolved: Vec::new(),
        });
    };

    let required = required_skills_for_fit(&state, &fit).await.map_err(|e| e.to_string())?;
    let skill_ids: Vec<i64> = required.keys().copied().collect();
    let names = names_for(&state, &skill_ids).await;
    let (missing, total_seconds) =
        evaluate_character(&state, character_id, &required, &names).await.map_err(|e| e.to_string())?;

    Ok(CanFlyView {
        ship: fit.ship,
        can_fly: missing.is_empty(),
        missing,
        total_seconds,
        parsed: true,
        unresolved: fit.unresolved,
    })
}

/// One line item in the fit gatekeeper (ownership + cost for a fit component).
#[derive(Debug, Serialize)]
pub struct GatekeeperItem {
    pub type_id: i64,
    pub name: String,
    pub needed: i64,
    pub owned: i64,
    pub missing: i64,
    pub unit_price: f64,
    /// Market cost of the units you still need to buy.
    pub missing_cost: f64,
}

/// The full "can-I / should-I" verdict for a fit: can you fly it, do you own the
/// parts, and what's the acquisition cost + total value.
#[derive(Debug, Serialize)]
pub struct FitGatekeeperView {
    pub parsed: bool,
    pub ship: String,
    pub can_fly: bool,
    pub missing_skills: Vec<MissingSkillView>,
    pub train_seconds: i64,
    pub items: Vec<GatekeeperItem>,
    /// Market value of the whole fit (all components at reference price).
    pub total_value: f64,
    /// Cost to acquire everything you don't already own.
    pub acquisition_cost: f64,
    /// Fraction of components (by count) you already own.
    pub owned_fraction: f64,
    pub unresolved: Vec<String>,
}

/// Fit gatekeeper: parse a fit and answer can-I-fly (skills) **and** should-I —
/// what you own vs. need (assets), and the ISK to acquire the rest (market).
#[tauri::command]
pub async fn fit_gatekeeper(
    state: State<'_, AppState>,
    character_id: i64,
    eft: String,
) -> CmdResult<FitGatekeeperView> {
    let empty = FitGatekeeperView {
        parsed: false,
        ship: String::new(),
        can_fly: false,
        missing_skills: Vec::new(),
        train_seconds: 0,
        items: Vec::new(),
        total_value: 0.0,
        acquisition_cost: 0.0,
        owned_fraction: 0.0,
        unresolved: Vec::new(),
    };
    let Some(fit) = state.fitting.resolve_eft(&eft).await.map_err(|e| e.to_string())? else {
        return Ok(empty);
    };

    // Can-fly (skills).
    let required = required_skills_for_fit(&state, &fit).await.map_err(|e| e.to_string())?;
    let skill_ids: Vec<i64> = required.keys().copied().collect();
    let skill_names = names_for(&state, &skill_ids).await;
    let (missing_skills, train_seconds) =
        evaluate_character(&state, character_id, &required, &skill_names).await.map_err(|e| e.to_string())?;

    // Ownership (assets) + pricing (market).
    let owned: std::collections::HashMap<i64, i64> = state
        .assets
        .all_holdings(character_id)
        .await
        .map(|groups| groups.into_iter().map(|g| (g.type_id, g.quantity)).collect())
        .unwrap_or_default();
    let prices = state.prices.price_map().await.unwrap_or_default();

    // Assemble the component list: hull + each resolved item (by quantity).
    let mut components: Vec<(i64, i64)> = Vec::new(); // (type_id, needed)
    if let Some(id) = fit.ship_type_id {
        components.push((id, 1));
    }
    for it in &fit.items {
        if let Some(id) = it.type_id {
            components.push((id, it.quantity.max(1)));
        }
    }
    // Collapse duplicate type ids (e.g. two of the same module line).
    let mut needed_by_type: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut order: Vec<i64> = Vec::new();
    for (id, n) in components {
        if !needed_by_type.contains_key(&id) {
            order.push(id);
        }
        *needed_by_type.entry(id).or_insert(0) += n;
    }

    let names = names_for(&state, &order).await;
    let mut items = Vec::new();
    let mut total_value = 0.0;
    let mut acquisition_cost = 0.0;
    let mut owned_units = 0i64;
    let mut total_units = 0i64;
    for id in order {
        let needed = needed_by_type[&id];
        let owned_qty = owned.get(&id).copied().unwrap_or(0).min(needed);
        let missing = (needed - owned_qty).max(0);
        let unit_price = prices.price(id).unwrap_or(0.0);
        let missing_cost = unit_price * missing as f64;
        total_value += unit_price * needed as f64;
        acquisition_cost += missing_cost;
        owned_units += owned_qty;
        total_units += needed;
        items.push(GatekeeperItem {
            type_id: id,
            name: named(&names, id),
            needed,
            owned: owned_qty,
            missing,
            unit_price,
            missing_cost,
        });
    }

    Ok(FitGatekeeperView {
        parsed: true,
        ship: fit.ship,
        can_fly: missing_skills.is_empty(),
        missing_skills,
        train_seconds,
        items,
        total_value,
        acquisition_cost,
        owned_fraction: if total_units > 0 { owned_units as f64 / total_units as f64 } else { 0.0 },
        unresolved: fit.unresolved,
    })
}

/// The union of skills a resolved fit requires (ship + every module/charge/drone
/// it could resolve), keeping the highest level demanded for each skill.
async fn required_skills_for_fit(
    state: &AppState,
    fit: &eve_core::fitting::ResolvedFit,
) -> eve_core::Result<std::collections::HashMap<i64, i64>> {
    let mut type_ids: Vec<i64> = Vec::new();
    if let Some(id) = fit.ship_type_id {
        type_ids.push(id);
    }
    for it in &fit.items {
        if let Some(id) = it.type_id {
            type_ids.push(id);
        }
    }
    let sde = state.names.sde();
    let mut required: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    for &tid in &type_ids {
        for req in sde.required_skills(tid).await? {
            let e = required.entry(req.type_id).or_insert(0);
            *e = (*e).max(req.quantity);
        }
    }
    Ok(required)
}

/// Compare a character's trained skills to a required-skills map, returning the
/// unmet skills (with training time, longest-first) and the total time to clear
/// them.
async fn evaluate_character(
    state: &AppState,
    character_id: i64,
    required: &std::collections::HashMap<i64, i64>,
    names: &std::collections::HashMap<i64, String>,
) -> eve_core::Result<(Vec<MissingSkillView>, i64)> {
    use eve_core::skillplan::{sp_for_level, training_seconds};

    let sheet = state.character.skills(character_id).await?;
    let attrs = state.character.attributes(character_id).await?;
    let current: std::collections::HashMap<i64, i64> = sheet
        .skills
        .iter()
        .map(|s| (s.skill_id, s.trained_skill_level))
        .collect();

    let sde = state.names.sde();
    let mut missing = Vec::new();
    let mut total_seconds = 0;
    for (&skill_id, &required_level) in required {
        let current_level = current.get(&skill_id).copied().unwrap_or(0);
        if current_level >= required_level {
            continue;
        }
        let seconds = match sde.skill_meta(skill_id).await? {
            Some(m) => {
                let sp = (sp_for_level(m.rank, required_level) - sp_for_level(m.rank, current_level))
                    .max(0);
                training_seconds(
                    sp,
                    attr_value(&attrs, m.primary_attr),
                    attr_value(&attrs, m.secondary_attr),
                )
            }
            None => 0,
        };
        total_seconds += seconds;
        missing.push(MissingSkillView {
            skill_type_id: skill_id,
            name: named(names, skill_id),
            required_level,
            current_level,
            seconds,
        });
    }
    missing.sort_by(|a, b| b.seconds.cmp(&a.seconds));
    Ok((missing, total_seconds))
}

/// One character's verdict against a doctrine fit.
#[derive(Debug, Serialize)]
pub struct DoctrinePilotView {
    pub character_id: i64,
    pub name: String,
    pub can_fly: bool,
    /// Number of skills still to train.
    pub missing_count: i64,
    /// Time to train every gap (0 when can_fly).
    pub total_seconds: i64,
}

/// Doctrine-compliance result across the whole roster.
#[derive(Debug, Serialize)]
pub struct DoctrineView {
    pub ship: String,
    pub parsed: bool,
    /// Pilots who can fly it now, then the rest sorted by least training time.
    pub pilots: Vec<DoctrinePilotView>,
    pub can_fly_count: i64,
    pub unresolved: Vec<String>,
}

/// Check a doctrine fit against every added character: who can fly it now, and
/// how long the rest need to train. The fit is parsed and its required skills
/// resolved once, then each pilot is evaluated.
#[tauri::command]
pub async fn doctrine_check(state: State<'_, AppState>, eft: String) -> CmdResult<DoctrineView> {
    let Some(fit) = state.fitting.resolve_eft(&eft).await.map_err(|e| e.to_string())? else {
        return Ok(DoctrineView {
            ship: String::new(),
            parsed: false,
            pilots: Vec::new(),
            can_fly_count: 0,
            unresolved: Vec::new(),
        });
    };

    let required = required_skills_for_fit(&state, &fit).await.map_err(|e| e.to_string())?;
    let skill_ids: Vec<i64> = required.keys().copied().collect();
    let names = names_for(&state, &skill_ids).await;

    let characters = state.db.list_characters().await.map_err(|e| e.to_string())?;
    let mut pilots = Vec::with_capacity(characters.len());
    for c in characters {
        // Best-effort per pilot: a missing scope/token contributes a skip, not a
        // whole-roster failure.
        let (missing, total_seconds) = evaluate_character(&state, c.id, &required, &names)
            .await
            .unwrap_or_else(|_| (Vec::new(), 0));
        pilots.push(DoctrinePilotView {
            character_id: c.id,
            name: c.name,
            can_fly: missing.is_empty(),
            missing_count: missing.len() as i64,
            total_seconds,
        });
    }
    // Can-fly first, then ascending by training time.
    pilots.sort_by(|a, b| {
        b.can_fly
            .cmp(&a.can_fly)
            .then(a.total_seconds.cmp(&b.total_seconds))
    });
    let can_fly_count = pilots.iter().filter(|p| p.can_fly).count() as i64;

    Ok(DoctrineView {
        ship: fit.ship,
        parsed: true,
        pilots,
        can_fly_count,
        unresolved: fit.unresolved,
    })
}

/// All collected notifications, most recent first (drives the Alerts rail).
#[tauri::command]
pub fn list_notifications(state: State<'_, AppState>) -> CmdResult<Vec<Notification>> {
    let center = state.notifications.lock().map_err(|_| "notification center poisoned")?;
    Ok(center.list())
}

/// Count of unread notifications (drives the Alerts badge).
#[tauri::command]
pub fn unread_notifications(state: State<'_, AppState>) -> CmdResult<usize> {
    let center = state.notifications.lock().map_err(|_| "notification center poisoned")?;
    Ok(center.unread_count())
}

/// Mark every notification read.
#[tauri::command]
pub fn mark_notifications_read(state: State<'_, AppState>) -> CmdResult<()> {
    let mut center = state.notifications.lock().map_err(|_| "notification center poisoned")?;
    center.mark_all_read();
    Ok(())
}

/// Dismiss a single notification by key.
#[tauri::command]
pub fn dismiss_notification(state: State<'_, AppState>, key: String) -> CmdResult<()> {
    let mut center = state.notifications.lock().map_err(|_| "notification center poisoned")?;
    center.dismiss(&key);
    Ok(())
}

/// Make a character the active/foreground one (polled at full cadence).
#[tauri::command]
pub async fn set_active_character(state: State<'_, AppState>, character_id: i64) -> CmdResult<()> {
    state
        .db
        .set_active_character(character_id)
        .await
        .map_err(|e| e.to_string())
}

/// Remove a character and forget its refresh token.
#[tauri::command]
pub async fn remove_character(state: State<'_, AppState>, character_id: i64) -> CmdResult<()> {
    state.db.delete_character(character_id).await.map_err(|e| e.to_string())?;
    state
        .tokens
        .delete_refresh_token(character_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}
