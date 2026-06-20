//! Tauri IPC command handlers. These are intentionally **thin**: they validate
//! inputs and delegate to `eve-core`. The frontend calls them via `invoke()`.

use serde::Serialize;
use tauri::State;

use eve_core::assets::{resolve_names, NamedAssetGroup};
use eve_core::character::CharacterSheet;
use eve_core::model::Character;
use eve_core::notify::Notification;

use crate::AppState;

/// Result type surfaced to the frontend: errors become strings.
type CmdResult<T> = std::result::Result<T, String>;

/// Default scopes requested at first login. Additional scopes are requested
/// **incrementally per feature** as the user enables them.
const BASE_SCOPES: &[&str] = &["publicData"];

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
