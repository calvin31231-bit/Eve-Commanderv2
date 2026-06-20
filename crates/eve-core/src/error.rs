//! Crate-wide error type.

use thiserror::Error;

/// Convenience alias used throughout `eve-core`.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors surfaced by `eve-core`.
#[derive(Debug, Error)]
pub enum Error {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON (de)serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    Url(#[from] url::ParseError),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),

    #[error("cache database error: {0}")]
    CacheDb(#[from] rusqlite::Error),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// The ESI error budget is exhausted; callers should back off until the
    /// contained number of seconds has elapsed.
    #[error("ESI error budget exhausted; retry after {0}s")]
    RateLimited(u64),

    /// The OAuth2 / SSO flow failed (bad state, exchange failure, etc.).
    #[error("authentication error: {0}")]
    Auth(String),

    /// A token was requested for a character that is not authenticated.
    #[error("no valid token for character {0}")]
    MissingToken(i64),

    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Build a generic error from anything string-like.
    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}
