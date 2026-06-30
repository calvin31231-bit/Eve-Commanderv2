//! EVE Commander desktop shell (Tauri 2).
//!
//! This crate is a thin shell: it owns the window, system tray, and IPC
//! surface, and delegates all real work to [`eve_core`]. It requires a system
//! WebView (WebView2 / WKWebView / WebKitGTK) to build and run, so it is built
//! on developer/CI machines with those libraries present — the headless
//! `eve-core` crate carries the logic that is unit-tested everywhere.

mod ai_tools;
mod commands;
mod poller;
mod snapshot;
mod tray;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use eve_core::assets::AssetsClient;
use eve_core::auth::{LoginManager, SsoClient, TokenManager};
use eve_core::auth::token_store::TokenStore;
use eve_core::calendar::CalendarClient;
use eve_core::character::CharacterClient;
use eve_core::clones::ClonesClient;
use eve_core::config::Config;
use eve_core::contracts::ContractsClient;
use eve_core::corp::CorpClient;
use eve_core::db::Database;
use eve_core::esi::EsiClient;
use eve_core::eve_scout::EveScoutClient;
use eve_core::fitting::FittingClient;
use eve_core::fleet::FleetClient;
use eve_core::industry::IndustryClient;
use eve_core::industry_plan::IndustryPlanClient;
use eve_core::insurance::InsuranceClient;
use eve_core::intel::ZkillClient;
use eve_core::lp::LpClient;
use eve_core::navigation::NavigationClient;
use eve_core::universe::UniverseClient;
use eve_core::mail::MailClient;
use eve_core::market::MarketClient;
use eve_core::marketdata::MarketDataClient;
use eve_core::mining::MiningClient;
use eve_core::names::NameResolver;
use eve_core::notify::NotificationCenter;
use eve_core::planets::PlanetsClient;
use eve_core::prices::PricesClient;
use eve_core::pve::PveClient;
use eve_core::research::ResearchClient;
use eve_core::reprocess::ReprocessClient;
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
    /// Public regional market data (browser) for the Economy hub.
    pub marketdata: MarketDataClient,
    /// Public ship insurance prices.
    pub insurance: InsuranceClient,
    /// Character contract reads for the Economy hub.
    pub contracts: ContractsClient,
    /// Mining-ledger reads (aggregated by ore) for the Character hub.
    pub mining: MiningClient,
    /// Planetary-industry colony reads (extractor countdowns) for the Economy hub.
    pub planets: PlanetsClient,
    /// EVEmail reads (headers + body) for the Character hub.
    pub mail: MailClient,
    /// Public market-price reference for asset/ore valuation.
    pub prices: PricesClient,
    /// SDE-backed reprocessing/refining calculator (Economy hub).
    pub reprocess: ReprocessClient,
    /// SDE-backed industry build planner (BOM/ME/invention) for the Economy hub.
    pub industry_plan: IndustryPlanClient,
    /// SDE-backed EFT fit parser/resolver for the Combat & Intel hub.
    pub fitting: FittingClient,
    /// zKillboard reads for the Local threat scanner (Combat & Intel hub).
    pub zkill: ZkillClient,
    /// Universe topology (system neighbours) for the System Safety surface.
    pub universe: UniverseClient,
    /// Route solving + in-game UI bridge (waypoint/open-window) for Navigation.
    pub navigation: NavigationClient,
    /// Corporation structure reads (fuel timers) for the Corp & Fleet hub.
    pub corp: CorpClient,
    /// Public LP-store offers for the LP optimizer (Tools hub).
    pub lp: LpClient,
    /// Public PvE content (incursions, faction warfare) for the Combat hub.
    pub pve: PveClient,
    /// R&D agent / datacore reads (passive income) for the Economy hub.
    pub research: ResearchClient,
    /// Upcoming calendar-event reads for the Character hub.
    pub calendar: CalendarClient,
    /// EVE-Scout Thera/Turnur connections for the Navigation hub.
    pub eve_scout: EveScoutClient,
    /// Live fleet composition reads for the Corp & Fleet hub.
    pub fleet: FleetClient,
    /// Layered id→name resolver (cache → SDE → ESI) for the hubs. Owns the SDE
    /// (full prebuilt `sde.sqlite` if shipped, otherwise a common-items seed).
    pub names: NameResolver,
    /// Collected notifications shown in the Alerts rail.
    pub notifications: tray::SharedCenter,
    /// Live data-freshness setting; the poller reads it each tick.
    pub intensity: std::sync::Arc<std::sync::RwLock<eve_core::config::Intensity>>,
    /// Optional Discord webhook URL; interrupting notifications mirror here.
    pub discord_webhook: std::sync::Arc<std::sync::RwLock<Option<String>>>,
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
///
/// Order: `XDG_DATA_HOME` (Linux override) → `APPDATA` (Windows roaming) →
/// `HOME/.local/share` (Unix) → temp. Picking `APPDATA` on Windows gives a
/// stable, user-visible location (`%APPDATA%\eve-commander`) instead of the
/// volatile temp dir, so the prebuilt SDE and durable DB survive reboots.
fn data_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return std::path::PathBuf::from(appdata);
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
    let marketdata = MarketDataClient::new(esi.clone());
    let insurance = InsuranceClient::new(esi.clone());
    let contracts = ContractsClient::new(esi.clone(), token_manager.clone());
    let mining = MiningClient::new(esi.clone(), token_manager.clone());
    let planets = PlanetsClient::new(esi.clone(), token_manager.clone());
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

    let reprocess = ReprocessClient::new(sde.clone());
    let industry_plan = IndustryPlanClient::new(sde.clone());
    let fitting = FittingClient::new(sde.clone());
    let zkill = ZkillClient::new(config.user_agent.clone());
    let universe = UniverseClient::new(esi.clone());
    let navigation = NavigationClient::new(esi.clone(), token_manager.clone());
    let corp = CorpClient::new(esi.clone(), token_manager.clone());
    let lp = LpClient::new(esi.clone());
    let pve = PveClient::new(esi.clone());
    let research = ResearchClient::new(esi.clone(), token_manager.clone());
    let calendar = CalendarClient::new(esi.clone(), token_manager.clone());
    let eve_scout = EveScoutClient::new(config.user_agent.clone());
    let fleet = FleetClient::new(esi.clone(), token_manager.clone());
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
    let webhook = tauri::async_runtime::block_on(db.get_setting_or("discord_webhook", ""))
        .ok()
        .filter(|s| !s.is_empty());
    let discord_webhook = std::sync::Arc::new(std::sync::RwLock::new(webhook));

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
        marketdata,
        insurance,
        contracts,
        mining,
        planets,
        mail,
        prices,
        reprocess,
        industry_plan,
        fitting,
        zkill,
        universe,
        navigation,
        corp,
        lp,
        pve,
        research,
        calendar,
        eve_scout,
        fleet,
        names,
        notifications,
        intensity,
        discord_webhook,
    }
}

/// Entry point invoked by `main.rs`.
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let state = build_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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

            // Periodically persist net-worth / SP snapshots for portfolio history.
            snapshot::spawn(
                state.db.clone(),
                state.character.clone(),
                state.assets.clone(),
                state.prices.clone(),
            );
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::server_status,
            commands::list_characters,
            commands::get_account_overview,
            commands::get_portfolio_history,
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
            commands::get_realized_income,
            commands::get_industry_jobs,
            commands::get_market_orders,
            commands::search_items,
            commands::get_market_browse,
            commands::scan_station_trades,
            commands::scan_arbitrage,
            commands::reprocess_item,
            commands::plan_build,
            commands::parse_fit,
            commands::fit_stats,
            commands::list_implants,
            commands::value_implants,
            commands::get_pod_risk,
            commands::can_fly_fit,
            commands::fit_gatekeeper,
            commands::doctrine_check,
            commands::parse_dscan,
            commands::scan_pilots,
            commands::pilot_background,
            commands::gate_camp_check,
            commands::get_system_safety,
            commands::get_system_risk,
            commands::get_region_map,
            commands::list_map_regions,
            commands::plan_route,
            commands::courier_estimate,
            commands::roll_wormhole,
            commands::get_jump_fatigue,
            commands::set_route_waypoint,
            commands::open_market_window,
            commands::get_corp_structures,
            commands::get_corp_members,
            commands::lp_store,
            commands::get_incursions,
            commands::get_fw_systems,
            commands::get_research_agents,
            commands::get_calendar,
            commands::get_thera_connections,
            commands::get_fleet,
            commands::get_combat_summary,
            commands::get_fleet_aar,
            commands::get_local_intel,
            commands::cost_skill_plan,
            commands::optimize_remap,
            commands::import_skill_plan,
            commands::rank_skill_roi,
            commands::rank_income,
            commands::get_contracts,
            commands::get_mining,
            commands::get_planets,
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
            commands::get_ai_settings,
            commands::set_ai_settings,
            commands::ai_detect_endpoints,
            commands::ai_chat,
            commands::ai_briefing,
            commands::add_memory,
            commands::list_memory,
            commands::forget_memory,
            commands::pin_memory,
            commands::export_data,
            commands::wipe_data,
            commands::save_skill_plan,
            commands::list_skill_plans,
            commands::delete_skill_plan,
            commands::save_fit,
            commands::list_fits,
            commands::delete_fit,
            commands::save_implant_loadout,
            commands::list_implant_loadouts,
            commands::delete_implant_loadout,
            commands::log_abyss_run,
            commands::get_abyss_tracker,
            commands::delete_abyss_run,
            commands::value_loot,
            commands::submit_srp_claim,
            commands::get_srp_board,
            commands::decide_srp_claim,
            commands::mark_srp_paid,
            commands::delete_srp_claim,
            commands::submit_recruit,
            commands::get_recruit_board,
            commands::set_recruit_status,
            commands::delete_recruit,
            commands::paste_signatures,
            commands::list_signatures,
            commands::annotate_signature,
            commands::delete_signature,
            commands::clear_signatures,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EVE Commander");
}
