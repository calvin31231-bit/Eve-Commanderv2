//! Tauri IPC command handlers. These are intentionally **thin**: they validate
//! inputs and delegate to `eve-core`. The frontend calls them via `invoke()`.

use serde::Serialize;
use tauri::State;

use eve_core::model::Character;

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

/// Begin the SSO login flow: returns the authorize URL the frontend should open
/// in the system browser. The loopback redirect then calls [`complete_login`].
#[tauri::command]
pub async fn begin_login(state: State<'_, AppState>) -> CmdResult<String> {
    if state.config.client_id.is_empty() {
        return Err("EVE_COMMANDER_CLIENT_ID is not set — register an ESI app first".into());
    }
    let url = state
        .login
        .begin(&state.sso, BASE_SCOPES)
        .map_err(|e| e.to_string())?;
    Ok(url.to_string())
}

/// Complete login from the redirect's `code` + `state`: exchanges the code,
/// persists the character, and stores the refresh token in the OS keychain.
#[tauri::command]
pub async fn complete_login(
    state: State<'_, AppState>,
    code: String,
    oauth_state: String,
) -> CmdResult<Character> {
    let completed = state
        .login
        .complete(&state.sso, &oauth_state, &code)
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

    Ok(completed.character)
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
