//! EVE Commander desktop shell (Tauri 2).
//!
//! This crate is a thin shell: it owns the window, system tray, and IPC
//! surface, and delegates all real work to [`eve_core`]. It requires a system
//! WebView (WebView2 / WKWebView / WebKitGTK) to build and run, so it is built
//! on developer/CI machines with those libraries present — the headless
//! `eve-core` crate carries the logic that is unit-tested everywhere.

mod commands;

use std::sync::Arc;

use eve_core::config::Config;

/// Shared application state handed to every Tauri command.
pub struct AppState {
    pub config: Config,
    pub esi: eve_core::esi::EsiClient,
    pub tokens: Arc<dyn eve_core::auth::token_store::TokenStore>,
}

/// Build the app config from environment / defaults. The ESI `client_id` and
/// `redirect_uri` come from the registered ESI application.
fn load_config() -> Config {
    let data_dir = dirs_next_data_dir().join("eve-commander");
    let client_id = std::env::var("EVE_COMMANDER_CLIENT_ID").unwrap_or_default();
    let redirect = std::env::var("EVE_COMMANDER_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8787/callback".to_string());
    Config::new(client_id, redirect, data_dir)
}

/// Minimal data-dir resolver without pulling in an extra crate at this stage.
fn dirs_next_data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".local/share");
    }
    std::env::temp_dir()
}

/// Entry point invoked by `main.rs`.
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let config = load_config();
    let cache: Arc<dyn eve_core::esi::client::CacheStore> =
        Arc::new(eve_core::esi::client::MemoryCacheStore::default());
    let esi = eve_core::esi::EsiClient::new(config.user_agent.clone(), cache)
        .expect("failed to build ESI client");
    let tokens = Arc::from(eve_core::auth::token_store::default_store());

    let state = AppState { config, esi, tokens };

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::server_status,
            commands::list_characters,
            commands::begin_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EVE Commander");
}
