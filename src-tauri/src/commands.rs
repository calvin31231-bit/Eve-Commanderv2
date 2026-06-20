//! Tauri IPC command handlers. These are intentionally **thin**: they validate
//! inputs and delegate to `eve-core`. The frontend calls them via `invoke()`.

use serde::Serialize;
use tauri::State;

use eve_core::assets::value_holdings;
use eve_core::character::CharacterSheet;
use eve_core::clones::ClonesSummary;
use eve_core::industry::ActiveJob;
use eve_core::mail::{strip_markup, MailHeader};
use eve_core::market::OrderView;
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
    "esi-mail.read_mail.v1",
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
    let mut out = Vec::with_capacity(valued.groups.len());
    for g in valued.groups {
        out.push(ValuedAssetGroup {
            type_id: g.type_id,
            name: resolve_type_name(&state, g.type_id).await,
            quantity: g.quantity,
            locations: g.locations,
            value: g.value,
        });
    }
    Ok(HoldingsView {
        total_value: valued.total_value,
        groups: out,
    })
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

    let implants = state
        .sde
        .name_types(&summary.active_implants)
        .await
        .map_err(|e| e.to_string())?;

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
    let item_name = resolve_type_name(state, job.display_type_id).await;
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
    let item_name = resolve_type_name(state, o.type_id).await;
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

    let mut ores = Vec::new();
    for ore in summary.by_ore.into_iter().take(8) {
        ores.push(NamedOre {
            type_id: ore.type_id,
            name: resolve_type_name(&state, ore.type_id).await,
            quantity: ore.quantity,
            value: prices.value(ore.type_id, ore.quantity),
        });
    }
    Ok(MiningView {
        total_units: summary.total_units,
        day_count: summary.day_count,
        total_value,
        ores,
    })
}

/// Resolve a single type id to a name via the SDE, falling back to `Type {id}`.
async fn resolve_type_name(state: &AppState, type_id: i64) -> String {
    state
        .sde
        .type_name(type_id)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("Type {type_id}"))
}

/// The latest page of mail headers for a character.
#[tauri::command]
pub async fn get_mail_headers(
    state: State<'_, AppState>,
    character_id: i64,
) -> CmdResult<Vec<MailHeader>> {
    state
        .mail
        .headers(character_id)
        .await
        .map_err(|e| e.to_string())
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
