//! EVE Commander desktop shell (Tauri 2).
//!
//! This crate is a thin shell: it owns the window, system tray, and IPC
//! surface, and delegates all real work to [`eve_core`]. It requires a system
//! WebView (WebView2 / WKWebView / WebKitGTK) to build and run, so it is built
//! on developer/CI machines with those libraries present — the headless
//! `eve-core` crate carries the logic that is unit-tested everywhere.

mod commands;
mod poller;
mod tray;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use eve_core::auth::{LoginManager, SsoClient, TokenManager};
use eve_core::auth::token_store::TokenStore;
use eve_core::config::Config;
use eve_core::db::Database;
use eve_core::esi::EsiClient;
use eve_core::notify::NotificationCenter;

/// Shared application state handed to every Tauri command.
pub struct AppState {
    pub config: Config,
    pub esi: EsiClient,
    pub sso: SsoClient,
    pub login: LoginManager,
    pub db: Database,
    pub tokens: Arc<dyn TokenStore>,
    /// Hands out valid access tokens (refreshing as needed) for ESI polling.
    pub token_manager: TokenManager,
    /// Collected notifications shown in the Alerts rail.
    pub notifications: tray::SharedCenter,
}

/// Build the app config from environment / defaults. The ESI `client_id` and
/// `redirect_uri` come from the registered ESI application.
fn load_config() -> Config {
    let data_dir = data_dir().join("eve-commander");
    let client_id = std::env::var("EVE_COMMANDER_CLIENT_ID").unwrap_or_default();
    let redirect = std::env::var("EVE_COMMANDER_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8787/callback".to_string());
    Config::new(client_id, redirect, data_dir)
}

/// Minimal data-dir resolver without pulling in an extra crate at this stage.
fn data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".local/share");
    }
    std::env::temp_dir()
}

/// Construct the shared application state (opens the database).
fn build_state() -> AppState {
    let config = load_config();
    std::fs::create_dir_all(&config.data_dir).ok();

    let http = reqwest::Client::new();
    // Persistent on-disk cache so ETags/bodies survive restarts (the first poll
    // after launch is usually a free 304). Fall back to in-memory if the cache
    // DB can't be opened — a degraded cache must never block startup.
    let cache: Arc<dyn eve_core::esi::client::CacheStore> =
        match eve_core::esi::SqliteCacheStore::open(config.cache_db_path()) {
            Ok(store) => Arc::new(store),
            Err(e) => {
                tracing::warn!("falling back to in-memory cache: {e}");
                Arc::new(eve_core::esi::client::MemoryCacheStore::default())
            }
        };
    let esi = EsiClient::new(config.user_agent.clone(), cache).expect("failed to build ESI client");
    let sso = SsoClient::new(http, config.client_id.clone(), config.redirect_uri.clone());

    // Database::open is async; block on it during startup.
    let db = tauri::async_runtime::block_on(Database::open(config.app_db_path()))
        .expect("failed to open app database");

    let tokens: Arc<dyn TokenStore> = Arc::from(eve_core::auth::token_store::default_store());
    let token_manager = TokenManager::new(sso.clone(), tokens.clone());
    let notifications = Arc::new(Mutex::new(NotificationCenter::default()));

    AppState {
        config,
        esi,
        sso,
        login: LoginManager::new(),
        db,
        tokens,
        token_manager,
        notifications,
    }
}

/// Entry point invoked by `main.rs`.
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = build_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .setup(|app| {
            // System tray with Show/Quit.
            tray::build(app.handle())?;

            // Start the background poll worker with cheap clones of the shared
            // handles. It reloads the roster from the DB each tick, so it picks
            // up characters added later in the session.
            let state = app.state::<AppState>();
            poller::spawn(
                app.handle().clone(),
                state.esi.clone(),
                state.token_manager.clone(),
                state.db.clone(),
                state.config.clone(),
                state.notifications.clone(),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::server_status,
            commands::list_characters,
            commands::login,
            commands::set_active_character,
            commands::remove_character,
            commands::list_notifications,
            commands::unread_notifications,
            commands::mark_notifications_read,
            commands::dismiss_notification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EVE Commander");
}
