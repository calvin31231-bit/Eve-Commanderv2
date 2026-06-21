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
    "esi-mail.read_mail.v1",
    "esi-mail.organize_mail.v1",
    "esi-clones.read_clones.v1",
    "esi-clones.read_implants.v1",
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
    let names = names_for(&state, &ids).await;
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
    let names = names_for(&state, &ids).await;
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
    Ok(AppSettings { intensity, notify_min })
}

/// Persist and apply settings live (poll intensity + notification threshold).
#[tauri::command]
pub async fn set_settings(
    state: State<'_, AppState>,
    intensity: String,
    notify_min: String,
) -> CmdResult<()> {
    let parsed_intensity = eve_core::config::Intensity::parse(&intensity);
    let parsed_sev = eve_core::notify::Severity::parse(&notify_min);

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

    // Apply live: the poller reads intensity each tick; the center is shared.
    if let Ok(mut g) = state.intensity.write() {
        *g = parsed_intensity;
    }
    if let Ok(mut center) = state.notifications.lock() {
        center.set_min_interrupt(parsed_sev);
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
