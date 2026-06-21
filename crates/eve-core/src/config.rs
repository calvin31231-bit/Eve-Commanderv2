//! Application configuration: ESI app credentials, data-freshness intensity,
//! and on-disk paths for the three logical SQLite stores + the AI memory vault.

use std::path::{Path, PathBuf};

/// The descriptive `User-Agent` CCP requires every ESI client to send.
/// Includes a contact so CCP can reach the maintainers if a tool misbehaves.
pub const DEFAULT_USER_AGENT: &str =
    "EVE-Commander/0.1 (+https://github.com/calvin31231-bit/Eve-Commanderv2; contact via GitHub issues)";

/// Base URL of the ESI API.
pub const ESI_BASE: &str = "https://esi.evetech.net";

/// EVE SSO endpoints.
pub const SSO_AUTHORIZE_URL: &str = "https://login.eveonline.com/v2/oauth/authorize";
pub const SSO_TOKEN_URL: &str = "https://login.eveonline.com/v2/oauth/token";

/// How aggressively the background scheduler refreshes data. This never beats
/// an endpoint's published cache timer — it only scales *which* characters and
/// *which* tiers get polled, to keep the resource footprint low.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Intensity {
    /// Foreground essentials only; lean on on-demand fetches. Ideal on a laptop
    /// / on battery.
    Light,
    /// Sensible default: foreground at full cadence, background alts slower.
    #[default]
    Balanced,
    /// Keep everything near its cache cadence. Highest freshness, highest cost.
    Aggressive,
}

impl Intensity {
    /// Multiplier applied to a *background* (non-active) character's poll
    /// cadence. Higher = polled less often.
    pub fn background_cadence_multiplier(self) -> f64 {
        match self {
            Intensity::Light => 6.0,
            Intensity::Balanced => 3.0,
            Intensity::Aggressive => 1.5,
        }
    }

    /// Stable string form (for persistence / UI).
    pub fn as_str(self) -> &'static str {
        match self {
            Intensity::Light => "Light",
            Intensity::Balanced => "Balanced",
            Intensity::Aggressive => "Aggressive",
        }
    }

    /// Parse from [`as_str`](Self::as_str); unknown values fall back to Balanced.
    pub fn parse(s: &str) -> Intensity {
        match s {
            "Light" => Intensity::Light,
            "Aggressive" => Intensity::Aggressive,
            _ => Intensity::Balanced,
        }
    }

    /// Maximum number of concurrent in-flight ESI requests the scheduler will
    /// allow. Kept modest so we never spike the player's connection.
    pub fn max_in_flight(self) -> usize {
        match self {
            Intensity::Light => 6,
            Intensity::Balanced => 12,
            Intensity::Aggressive => 16,
        }
    }
}

/// Application configuration. `data_dir` is the root for all local state.
#[derive(Debug, Clone)]
pub struct Config {
    /// ESI application client id (public PKCE client; registered at
    /// developers.eveonline.com).
    pub client_id: String,
    /// Loopback redirect URI registered for the ESI application.
    pub redirect_uri: String,
    /// `User-Agent` sent with every request.
    pub user_agent: String,
    /// Root directory for all local data.
    pub data_dir: PathBuf,
    /// Data-freshness intensity profile.
    pub intensity: Intensity,
}

impl Config {
    /// Construct a config rooted at `data_dir`.
    pub fn new(client_id: impl Into<String>, redirect_uri: impl Into<String>, data_dir: impl AsRef<Path>) -> Self {
        Self {
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
            user_agent: DEFAULT_USER_AGENT.to_string(),
            data_dir: data_dir.as_ref().to_path_buf(),
            intensity: Intensity::default(),
        }
    }

    /// Static game data (read-mostly, shipped prebuilt, hot-swappable).
    pub fn sde_db_path(&self) -> PathBuf {
        self.data_dir.join("sde.sqlite")
    }

    /// ESI response cache + ETag/expiry metadata (disposable).
    pub fn cache_db_path(&self) -> PathBuf {
        self.data_dir.join("cache.sqlite")
    }

    /// Durable user data: characters, settings, watchlists, fits, plans,
    /// historical snapshots.
    pub fn app_db_path(&self) -> PathBuf {
        self.data_dir.join("app.sqlite")
    }

    /// Obsidian-compatible Markdown vault holding the AI's durable memory.
    pub fn memory_vault_dir(&self) -> PathBuf {
        self.data_dir.join("memory-vault")
    }
}
