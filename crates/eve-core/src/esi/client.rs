//! The cache-first ESI HTTP client.
//!
//! Request flow for every call:
//! 1. Consult the cache ([`cache::decide`]). If fresh, serve it — **no network**.
//! 2. If stale-with-ETag, send `If-None-Match`; a `304` refreshes expiry for
//!    free (no error-budget cost).
//! 3. Otherwise perform a full request.
//!
//! Every response updates the shared [`ErrorBudget`]; when it trips, callers are
//! asked to back off. A descriptive `User-Agent` is always sent (CCP requires
//! it).

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::de::DeserializeOwned;

use crate::config::ESI_BASE;
use crate::error::{Error, Result};

use super::cache::{self, CacheDecision, CacheEntry};
use super::ratelimit::ErrorBudget;

/// Pluggable cache backend. The Tauri shell backs this with `cache.sqlite`;
/// tests use an in-memory map.
pub trait CacheStore: Send + Sync {
    fn get(&self, key: &str) -> Option<CacheEntry>;
    fn put(&self, key: &str, entry: CacheEntry);
}

/// Simple in-memory cache store (default; also used in tests).
#[derive(Default)]
pub struct MemoryCacheStore {
    inner: Mutex<std::collections::HashMap<String, CacheEntry>>,
}

impl CacheStore for MemoryCacheStore {
    fn get(&self, key: &str) -> Option<CacheEntry> {
        self.inner.lock().ok()?.get(key).cloned()
    }
    fn put(&self, key: &str, entry: CacheEntry) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(key.to_string(), entry);
        }
    }
}

/// A cache-first ESI client.
#[derive(Clone)]
pub struct EsiClient {
    http: reqwest::Client,
    user_agent: String,
    budget: Arc<Mutex<ErrorBudget>>,
    cache: Arc<dyn CacheStore>,
}

impl EsiClient {
    /// Build a client with the given `User-Agent` and cache backend.
    pub fn new(user_agent: impl Into<String>, cache: Arc<dyn CacheStore>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            user_agent: user_agent.into(),
            budget: Arc::new(Mutex::new(ErrorBudget::default())),
            cache,
        })
    }

    /// Current backoff requested by the error-budget breaker.
    pub fn backoff(&self) -> Duration {
        self.budget.lock().map(|b| b.backoff()).unwrap_or(Duration::ZERO)
    }

    /// GET a public (unauthenticated) ESI path (e.g. `/latest/status/`) and
    /// deserialize the JSON body, honoring the cache.
    pub async fn get_public_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let body = self.get_cached(path, None).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    /// GET an authenticated ESI path with a bearer token.
    pub async fn get_auth_json<T: DeserializeOwned>(&self, path: &str, access_token: &str) -> Result<T> {
        let body = self.get_cached(path, Some(access_token)).await?;
        Ok(serde_json::from_slice(&body)?)
    }

    /// POST a JSON body to a public ESI path and deserialize the JSON response.
    /// Not cached (POSTs aren't); used for bulk id→name resolution. Still
    /// respects the error-budget breaker and updates it from response headers.
    pub async fn post_public_json<B, T>(&self, path: &str, body: &B) -> Result<T>
    where
        B: serde::Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let backoff = self.backoff();
        if backoff > Duration::ZERO {
            return Err(Error::RateLimited(backoff.as_secs()));
        }

        let url = format!("{ESI_BASE}{path}");
        let resp = self
            .http
            .post(&url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .json(body)
            .send()
            .await?;

        if let Ok(mut budget) = self.budget.lock() {
            let headers = resp.headers().clone();
            budget.observe_headers(|k| headers.get(k).and_then(|v| v.to_str().ok()));
        }

        if !resp.status().is_success() {
            return Err(Error::other(format!("ESI POST {} returned {}", path, resp.status())));
        }
        Ok(resp.json::<T>().await?)
    }

    /// PUT a JSON body to an authenticated ESI path (write). ESI returns 204 on
    /// success; no response body is parsed. Breaker-aware.
    pub async fn put_auth_empty<B>(&self, path: &str, body: &B, access_token: &str) -> Result<()>
    where
        B: serde::Serialize + ?Sized,
    {
        let backoff = self.backoff();
        if backoff > Duration::ZERO {
            return Err(Error::RateLimited(backoff.as_secs()));
        }
        let url = format!("{ESI_BASE}{path}");
        let resp = self
            .http
            .put(&url)
            .header(reqwest::header::USER_AGENT, &self.user_agent)
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await?;
        if let Ok(mut budget) = self.budget.lock() {
            let headers = resp.headers().clone();
            budget.observe_headers(|k| headers.get(k).and_then(|v| v.to_str().ok()));
        }
        if !resp.status().is_success() {
            return Err(Error::other(format!("ESI PUT {} returned {}", path, resp.status())));
        }
        Ok(())
    }

    /// Fetch a path through the full cache-first pipeline and return the raw
    /// body, without deserializing. The background poller uses this to **warm
    /// the cache** (and update the error budget) so later typed reads are served
    /// locally. `access_token` is `None` for public routes.
    pub async fn get_raw(&self, path: &str, access_token: Option<&str>) -> Result<Vec<u8>> {
        self.get_cached(path, access_token).await
    }

    /// GET every page of a paginated authenticated route, concatenating the JSON
    /// arrays. The page count comes from the `X-Pages` header (cached alongside
    /// the body), and each page is itself cache-first.
    pub async fn get_auth_json_paged<T: DeserializeOwned>(
        &self,
        path: &str,
        access_token: &str,
    ) -> Result<Vec<T>> {
        self.get_json_paged(path, Some(access_token)).await
    }

    /// Like [`get_auth_json_paged`](Self::get_auth_json_paged) but for a public
    /// (unauthenticated) paginated route (e.g. region market orders).
    pub async fn get_public_json_paged<T: DeserializeOwned>(&self, path: &str) -> Result<Vec<T>> {
        self.get_json_paged(path, None).await
    }

    /// Core paginated fetch shared by the public/auth variants.
    async fn get_json_paged<T: DeserializeOwned>(
        &self,
        path: &str,
        token: Option<&str>,
    ) -> Result<Vec<T>> {
        let (first, pages) = self.get_cached_meta(&page_url(path, 1), token).await?;
        let mut items: Vec<T> = serde_json::from_slice(&first)?;
        for page in 2..=pages {
            let (body, _) = self.get_cached_meta(&page_url(path, page), token).await?;
            items.extend(serde_json::from_slice::<Vec<T>>(&body)?);
        }
        Ok(items)
    }

    /// Core cache-first GET. Returns the (possibly cached) response body bytes.
    async fn get_cached(&self, path: &str, token: Option<&str>) -> Result<Vec<u8>> {
        Ok(self.get_cached_meta(path, token).await?.0)
    }

    /// Cache-first GET returning the body plus the route's page count (1 when not
    /// paginated).
    async fn get_cached_meta(&self, path: &str, token: Option<&str>) -> Result<(Vec<u8>, u32)> {
        // Respect the breaker before touching the network.
        let backoff = self.backoff();
        if backoff > Duration::ZERO {
            return Err(Error::RateLimited(backoff.as_secs()));
        }

        let now = SystemTime::now();
        let cached = self.cache.get(path);
        match cache::decide(cached.as_ref(), now) {
            CacheDecision::ServeFresh => {
                // Safe to unwrap: ServeFresh implies a cache entry exists.
                let entry = cached.unwrap();
                Ok((entry.body, entry.pages.unwrap_or(1)))
            }
            CacheDecision::Revalidate(etag) => {
                self.fetch(path, token, Some(etag), cached).await
            }
            CacheDecision::Fetch => self.fetch(path, token, None, None).await,
        }
    }

    async fn fetch(
        &self,
        path: &str,
        token: Option<&str>,
        if_none_match: Option<String>,
        previous: Option<CacheEntry>,
    ) -> Result<(Vec<u8>, u32)> {
        let url = format!("{ESI_BASE}{path}");
        let mut req = self
            .http
            .get(&url)
            .header(reqwest::header::USER_AGENT, &self.user_agent);
        if let Some(tok) = token {
            req = req.bearer_auth(tok);
        }
        if let Some(tag) = &if_none_match {
            req = req.header(reqwest::header::IF_NONE_MATCH, tag);
        }

        let resp = req.send().await?;

        // Update the error budget from headers on every response.
        if let Ok(mut budget) = self.budget.lock() {
            let headers = resp.headers().clone();
            budget.observe_headers(|k| headers.get(k).and_then(|v| v.to_str().ok()));
        }

        // 304: our cached body is still valid; refresh its expiry cheaply.
        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(mut prev) = previous {
                prev.expires_at = parse_expires(&resp).unwrap_or(prev.expires_at);
                let pages = parse_pages(&resp).or(prev.pages);
                prev.pages = pages;
                let body = prev.body.clone();
                self.cache.put(path, prev);
                return Ok((body, pages.unwrap_or(1)));
            }
        }

        if !resp.status().is_success() {
            return Err(Error::other(format!(
                "ESI {} returned {}",
                path,
                resp.status()
            )));
        }

        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);
        let expires_at = parse_expires(&resp).unwrap_or_else(|| SystemTime::now() + Duration::from_secs(60));
        let pages = parse_pages(&resp);
        let body = resp.bytes().await?.to_vec();

        self.cache.put(
            path,
            CacheEntry {
                body: body.clone(),
                etag,
                expires_at,
                pages,
            },
        );
        Ok((body, pages.unwrap_or(1)))
    }
}

/// Append `?page=N` to a path, preserving any existing query string.
fn page_url(path: &str, page: u32) -> String {
    let sep = if path.contains('?') { '&' } else { '?' };
    format!("{path}{sep}page={page}")
}

/// Parse the `X-Pages` header into a page count.
fn parse_pages(resp: &reqwest::Response) -> Option<u32> {
    resp.headers()
        .get("x-pages")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u32>().ok())
}

/// Parse the `Expires` header (RFC 7231 IMF-fixdate) into a `SystemTime`.
fn parse_expires(resp: &reqwest::Response) -> Option<SystemTime> {
    let raw = resp
        .headers()
        .get(reqwest::header::EXPIRES)?
        .to_str()
        .ok()?;
    httpdate_parse(raw)
}

/// Minimal IMF-fixdate parser (e.g. "Wed, 21 Oct 2026 07:28:00 GMT") to avoid an
/// extra dependency. Returns `None` on any parse failure.
fn httpdate_parse(s: &str) -> Option<SystemTime> {
    // Format: "Day, DD Mon YYYY HH:MM:SS GMT"
    let s = s.trim();
    let comma = s.find(',')?;
    let rest = s[comma + 1..].trim();
    let mut parts = rest.split_whitespace();
    let day: i64 = parts.next()?.parse().ok()?;
    let mon = match parts.next()? {
        "Jan" => 1, "Feb" => 2, "Mar" => 3, "Apr" => 4, "May" => 5, "Jun" => 6,
        "Jul" => 7, "Aug" => 8, "Sep" => 9, "Oct" => 10, "Nov" => 11, "Dec" => 12,
        _ => return None,
    };
    let year: i64 = parts.next()?.parse().ok()?;
    let mut hms = parts.next()?.split(':');
    let h: i64 = hms.next()?.parse().ok()?;
    let mi: i64 = hms.next()?.parse().ok()?;
    let se: i64 = hms.next()?.parse().ok()?;

    let secs = days_from_civil(year, mon, day) * 86_400 + h * 3600 + mi * 60 + se;
    if secs < 0 {
        return None;
    }
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs as u64))
}

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_imf_fixdate() {
        let t = httpdate_parse("Wed, 21 Oct 2026 07:28:00 GMT").unwrap();
        let secs = t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        // 2026-10-21T07:28:00Z == 1792567680
        assert_eq!(secs, 1_792_567_680);
    }

    #[test]
    fn page_url_appends_query() {
        assert_eq!(page_url("/latest/characters/1/assets/", 1), "/latest/characters/1/assets/?page=1");
        assert_eq!(page_url("/x/?foo=bar", 3), "/x/?foo=bar&page=3");
    }

    #[test]
    fn epoch_is_zero_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn client_constructs() {
        let cache: Arc<dyn CacheStore> = Arc::new(MemoryCacheStore::default());
        let c = EsiClient::new("test-agent/1.0", cache);
        assert!(c.is_ok());
    }
}
