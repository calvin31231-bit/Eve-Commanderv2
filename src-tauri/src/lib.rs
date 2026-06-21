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

use eve_core::assets::AssetsClient;
use eve_core::auth::{LoginManager, SsoClient, TokenManager};
use eve_core::auth::token_store::TokenStore;
use eve_core::character::CharacterClient;
use eve_core::clones::ClonesClient;
use eve_core::config::Config;
use eve_core::db::Database;
use eve_core::esi::EsiClient;
use eve_core::industry::IndustryClient;
use eve_core::mail::MailClient;
use eve_core::market::MarketClient;
use eve_core::mining::MiningClient;
use eve_core::names::NameResolver;
use eve_core::notify::NotificationCenter;
use eve_core::prices::PricesClient;
use eve_core::sde::Sde;
use eve_core::wallet::WalletClient;

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
    /// Typed character reads (skills, queue, wallet) for the Character hub.
    pub character: CharacterClient,
    /// Paginated asset reads for the Character hub.
    pub assets: AssetsClient,
    /// Jump clone + implant reads for the Character hub.
    pub clones: ClonesClient,
    /// Wallet journal + cashflow analytics for the Character hub.
    pub wallet: WalletClient,
    /// Industry job reads (with client-side countdowns) for the Economy hub.
    pub industry: IndustryClient,
    /// Open market-order reads (escrow/value + expiry) for the Economy hub.
    pub market: MarketClient,
    /// Mining-ledger reads (aggregated by ore) for the Character hub.
    pub mining: MiningClient,
    /// EVEmail reads (headers + body) for the Character hub.
    pub mail: MailClient,
    /// Public market-price reference for asset/ore valuation.
    pub prices: PricesClient,
    /// Layered id→name resolver (cache → SDE → ESI) for the hubs. Owns the SDE
    /// (full prebuilt `sde.sqlite` if shipped, otherwise a common-items seed).
    pub names: NameResolver,
    /// Collected notifications shown in the Alerts rail.
    pub notifications: tray::SharedCenter,
    /// Live data-freshness setting; the poller reads it each tick.
    pub intensity: std::sync::Arc<std::sync::RwLock<eve_core::config::Intensity>>,
}

/// Build the app config from environment / defaults. The ESI `client_id` and
/// `redirect_uri` come from the registered ESI application.
fn load_config() -> Config {
    // Load a git-ignored `.env` (searching cwd upward) so the client id can be
    // supplied from a file without shell env-var wrangling. Real environment
    // variables still win — dotenvy never overrides what's already set.
    let loaded = dotenvy::dotenv();
    match &loaded {
        Ok(path) => tracing::info!("loaded env file: {}", path.display()),
        Err(_) => tracing::debug!("no .env file found (using process environment)"),
    }

    let data_dir = data_dir().join("eve-commander");
    let client_id = std::env::var("EVE_COMMANDER_CLIENT_ID").unwrap_or_default();
    if client_id.is_empty() {
        tracing::warn!(
            "EVE_COMMANDER_CLIENT_ID is empty — set it (env var or .env file) or login will fail"
        );
    }
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
    let character = CharacterClient::new(esi.clone(), token_manager.clone());
    let assets = AssetsClient::new(esi.clone(), token_manager.clone());
    let clones = ClonesClient::new(esi.clone(), token_manager.clone());
    let wallet = WalletClient::new(esi.clone(), token_manager.clone());
    let industry = IndustryClient::new(esi.clone(), token_manager.clone());
    let market = MarketClient::new(esi.clone(), token_manager.clone());
    let mining = MiningClient::new(esi.clone(), token_manager.clone());
    let mail = MailClient::new(esi.clone(), token_manager.clone());
    let prices = PricesClient::new(esi.clone());
    let notifications = Arc::new(Mutex::new(NotificationCenter::default()));

    // Use the full prebuilt SDE if shipped, else fall back to the common-items
    // seed so frequent ids (minerals, ores, hubs, iconic ships) still resolve.
    let sde = match tauri::async_runtime::block_on(Sde::open(config.sde_db_path())) {
        Ok(s) => {
            tracing::info!("loaded prebuilt SDE");
            s
        }
        Err(_) => {
            tracing::info!("no prebuilt SDE; using common-items seed");
            tauri::async_runtime::block_on(Sde::seeded()).expect("failed to build seed SDE")
        }
    };

    let names = NameResolver::new(esi.clone(), db.clone(), sde);

    // Load persisted settings (data-freshness intensity, notification threshold)
    // and apply them.
    let intensity_val = tauri::async_runtime::block_on(async {
        eve_core::config::Intensity::parse(
            &db.get_setting_or("intensity", config.intensity.as_str())
                .await
                .unwrap_or_else(|_| config.intensity.as_str().to_string()),
        )
    });
    let intensity = std::sync::Arc::new(std::sync::RwLock::new(intensity_val));
    if let Ok(notify_min) = tauri::async_runtime::block_on(db.get_setting_or("notify_min", "Warning"))
    {
        if let Ok(mut center) = notifications.lock() {
            center.set_min_interrupt(eve_core::notify::Severity::parse(&notify_min));
        }
    }

    AppState {
        config,
        esi,
        sso,
        login: LoginManager::new(),
        db,
        tokens,
        token_manager,
        character,
        assets,
        clones,
        wallet,
        industry,
        market,
        mining,
        mail,
        prices,
        names,
        notifications,
        intensity,
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
                state.intensity.clone(),
                state.notifications.clone(),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::server_status,
            commands::list_characters,
            commands::get_account_overview,
            commands::login,
            commands::set_active_character,
            commands::remove_character,
            commands::get_character_sheet,
            commands::get_character_status,
            commands::get_character_profile,
            commands::get_skill_queue,
            commands::get_attributes,
            commands::get_transactions,
            commands::get_top_holdings,
            commands::get_assets_by_location,
            commands::get_clones,
            commands::get_cashflow,
            commands::get_industry_jobs,
            commands::get_market_orders,
            commands::get_mining,
            commands::get_mail_headers,
            commands::get_mail,
            commands::mark_mail_read,
            commands::get_settings,
            commands::set_settings,
            commands::list_groups,
            commands::create_group,
            commands::delete_group,
            commands::add_group_member,
            commands::remove_group_member,
            commands::list_notifications,
            commands::unread_notifications,
            commands::mark_notifications_read,
            commands::dismiss_notification,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EVE Commander");
}
