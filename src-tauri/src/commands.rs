//! Tauri IPC command handlers. These are intentionally **thin**: they validate
//! inputs and delegate to `eve-core`. The frontend calls them via `invoke()`.

use serde::Serialize;
use tauri::State;

use eve_core::assets::{resolve_names, NamedAssetGroup};
use eve_core::character::CharacterSheet;
use eve_core::clones::ClonesSummary;
use eve_core::industry::ActiveJob;
use eve_core::market::OrderView;
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
    "esi-clones.read_clones.v1",
    "esi-clones.read_implants.v1",
    "esi-location.read_location.v1",
    "esi-location.read_online.v1",
    "esi-industry.read_character_jobs.v1",
    "esi-markets.read_character_orders.v1",
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
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
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

/// Top `limit` asset holdings for a character, aggregated by type and resolved
/// to names via the SDE (falling back to `Type {id}` when the SDE isn't loaded).
#[tauri::command]
pub async fn get_top_holdings(
    state: State<'_, AppState>,
    character_id: i64,
    limit: usize,
) -> CmdResult<Vec<NamedAssetGroup>> {
    let groups = state
        .assets
        .top_holdings(character_id, limit)
        .await
        .map_err(|e| e.to_string())?;

    match &state.sde {
        Some(sde) => resolve_names(&groups, sde).await.map_err(|e| e.to_string()),
        None => Ok(groups
            .into_iter()
            .map(|g| NamedAssetGroup {
                type_id: g.type_id,
                name: format!("Type {}", g.type_id),
                quantity: g.quantity,
                locations: g.locations,
            })
            .collect()),
    }
}

/// The clone view: jump-clone count and the active clone's named implants.
#[derive(Debug, Serialize)]
pub struct ClonesView {
    pub jump_clone_count: usize,
    pub active_implant_count: usize,
    pub implants: Vec<NamedType>,
}

/// Jump clones + active implants (with SDE-resolved implant names) for a
/// character.
#[tauri::command]
pub async fn get_clones(state: State<'_, AppState>, character_id: i64) -> CmdResult<ClonesView> {
    let summary: ClonesSummary = state
        .clones
        .summary(character_id)
        .await
        .map_err(|e| e.to_string())?;

    let implants = match &state.sde {
        Some(sde) => sde
            .name_types(&summary.active_implants)
            .await
            .map_err(|e| e.to_string())?,
        None => summary
            .active_implants
            .iter()
            .map(|&type_id| NamedType {
                type_id,
                name: format!("Type {type_id}"),
            })
            .collect(),
    };

    Ok(ClonesView {
        jump_clone_count: summary.jump_clone_count,
        active_implant_count: summary.active_implants.len(),
        implants,
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

    let mut out = Vec::with_capacity(summary.jobs.len());
    for job in summary.jobs {
        out.push(view_for(&state, job).await);
    }
    Ok(out)
}

/// Enrich one [`ActiveJob`] with its display item name (falling back to the id).
async fn view_for(state: &AppState, job: ActiveJob) -> IndustryJobView {
    let item_name = match &state.sde {
        Some(sde) => sde
            .type_name(job.display_type_id)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| format!("Type {}", job.display_type_id)),
        None => format!("Type {}", job.display_type_id),
    };
    IndustryJobView {
        job_id: job.job_id,
        activity: job.activity,
        item_name,
        runs: job.runs,
        status: job.status,
        end_date: job.end_date,
        seconds_remaining: job.seconds_remaining,
    }
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

    let mut orders = Vec::with_capacity(summary.orders.len());
    for o in summary.orders {
        orders.push(order_view(&state, o).await);
    }
    Ok(MarketView {
        buy_count: summary.buy_count,
        sell_count: summary.sell_count,
        total_escrow: summary.total_escrow,
        sell_value: summary.sell_value,
        orders,
    })
}

/// Enrich one [`OrderView`] with its item name (falling back to the id).
async fn order_view(state: &AppState, o: OrderView) -> MarketOrderView {
    let item_name = match &state.sde {
        Some(sde) => sde
            .type_name(o.type_id)
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| format!("Type {}", o.type_id)),
        None => format!("Type {}", o.type_id),
    };
    MarketOrderView {
        order_id: o.order_id,
        item_name,
        is_buy_order: o.is_buy_order,
        price: o.price,
        volume_remain: o.volume_remain,
        volume_total: o.volume_total,
        seconds_remaining: o.seconds_remaining,
    }
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
