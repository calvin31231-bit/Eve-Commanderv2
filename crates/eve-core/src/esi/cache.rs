//! Cache freshness for ESI responses.
//!
//! Every ESI route publishes a cache timer via the `Expires` header. The
//! cardinal rule of being a good API citizen — and of keeping the player's
//! machine quiet — is **never request again before `Expires`**: the data is
//! byte-for-byte identical until then. When an entry is stale but carries an
//! `ETag`, we revalidate with `If-None-Match`, and a `304 Not Modified` simply
//! refreshes the expiry at no error-budget cost.

use std::time::{Duration, SystemTime};

/// A cached ESI response plus the metadata needed to serve and revalidate it.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Raw response body.
    pub body: Vec<u8>,
    /// `ETag` header, if the route provided one.
    pub etag: Option<String>,
    /// Absolute time the entry becomes stale (from the `Expires` header).
    pub expires_at: SystemTime,
}

impl CacheEntry {
    /// Whether the entry is still fresh at `now` and may be served without any
    /// network request.
    pub fn is_fresh(&self, now: SystemTime) -> bool {
        now < self.expires_at
    }

    /// Whether we can cheaply revalidate this entry with `If-None-Match`.
    pub fn can_revalidate(&self) -> bool {
        self.etag.is_some()
    }

    /// Remaining freshness; `Duration::ZERO` once stale.
    pub fn ttl(&self, now: SystemTime) -> Duration {
        self.expires_at.duration_since(now).unwrap_or(Duration::ZERO)
    }
}

/// The action the client should take for a given cache entry.
#[derive(Debug, PartialEq, Eq)]
pub enum CacheDecision {
    /// Serve the cached body directly; no network needed.
    ServeFresh,
    /// Stale but revalidatable — send a conditional request with this ETag.
    Revalidate(String),
    /// No usable cache — perform a full request.
    Fetch,
}

/// Decide what to do with an optional cache entry at time `now`.
pub fn decide(entry: Option<&CacheEntry>, now: SystemTime) -> CacheDecision {
    match entry {
        Some(e) if e.is_fresh(now) => CacheDecision::ServeFresh,
        Some(e) => match &e.etag {
            Some(tag) => CacheDecision::Revalidate(tag.clone()),
            None => CacheDecision::Fetch,
        },
        None => CacheDecision::Fetch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(fresh_for: i64, etag: Option<&str>) -> CacheEntry {
        let expires_at = if fresh_for >= 0 {
            SystemTime::now() + Duration::from_secs(fresh_for as u64)
        } else {
            SystemTime::now() - Duration::from_secs((-fresh_for) as u64)
        };
        CacheEntry {
            body: b"{}".to_vec(),
            etag: etag.map(String::from),
            expires_at,
        }
    }

    #[test]
    fn fresh_entry_is_served() {
        let e = entry(300, Some("abc"));
        assert_eq!(decide(Some(&e), SystemTime::now()), CacheDecision::ServeFresh);
    }

    #[test]
    fn stale_with_etag_revalidates() {
        let e = entry(-1, Some("abc"));
        assert_eq!(
            decide(Some(&e), SystemTime::now()),
            CacheDecision::Revalidate("abc".to_string())
        );
    }

    #[test]
    fn stale_without_etag_fetches() {
        let e = entry(-1, None);
        assert_eq!(decide(Some(&e), SystemTime::now()), CacheDecision::Fetch);
    }

    #[test]
    fn missing_entry_fetches() {
        assert_eq!(decide(None, SystemTime::now()), CacheDecision::Fetch);
    }
}
