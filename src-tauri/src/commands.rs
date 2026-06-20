//! Tauri IPC command handlers. These are intentionally **thin**: they validate
//! inputs and delegate to `eve-core`. The frontend calls them via `invoke()`.

use serde::Serialize;
use tauri::State;

use crate::AppState;

/// Result type surfaced to the frontend: errors become strings.
type CmdResult<T> = std::result::Result<T, String>;

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

/// List the characters the user has added. (Phase 0: backed by `app.sqlite`;
/// stubbed empty until the DB layer lands.)
#[tauri::command]
pub async fn list_characters(_state: State<'_, AppState>) -> CmdResult<Vec<eve_core::model::Character>> {
    Ok(Vec::new())
}

/// Begin the SSO login flow: returns the authorize URL the frontend should open
/// in the system browser. (Phase 0 scaffold.)
#[tauri::command]
pub async fn begin_login(_state: State<'_, AppState>) -> CmdResult<String> {
    Ok("login-not-yet-wired".to_string())
}
