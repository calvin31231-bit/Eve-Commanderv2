//! Persistent ESI response cache backed by `cache.sqlite`.
//!
//! This warms across restarts: ETags and bodies survive a relaunch, so the
//! first poll after startup is usually a free `304 Not Modified` rather than a
//! full download. The store is disposable — deleting `cache.sqlite` only costs
//! one round of full fetches.
//!
//! It implements the synchronous [`CacheStore`](super::client::CacheStore) trait
//! the client calls on its hot path, so we use **rusqlite** (synchronous) rather
//! than the async `sqlx` pool used for durable user data.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use rusqlite::{Connection, OptionalExtension};

use super::cache::CacheEntry;
use super::client::CacheStore;
use crate::error::Result;

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS cache (
    key        TEXT    PRIMARY KEY,
    body       BLOB    NOT NULL,
    etag       TEXT,
    expires_at INTEGER NOT NULL,  -- unix epoch seconds
    pages      INTEGER            -- X-Pages count for paginated routes
);
CREATE INDEX IF NOT EXISTS idx_cache_expires ON cache(expires_at);
"#;

/// A `cache.sqlite`-backed [`CacheStore`].
pub struct SqliteCacheStore {
    conn: Mutex<Connection>,
}

impl SqliteCacheStore {
    /// Open (creating if needed) the cache database at `path`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory cache store — used in tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Delete entries that are stale and not revalidatable (no ETag), reclaiming
    /// space. Stale-but-revalidatable rows are kept — their ETag still buys a
    /// free `304`. Returns the number of rows removed.
    pub fn purge_unrevalidatable(&self, now: SystemTime) -> Result<usize> {
        let now_secs = to_epoch(now);
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let n = conn.execute(
            "DELETE FROM cache WHERE expires_at < ?1 AND etag IS NULL",
            [now_secs],
        )?;
        Ok(n)
    }

    /// Number of cached rows (test/inspection helper).
    pub fn count(&self) -> Result<i64> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM cache", [], |r| r.get(0))?;
        Ok(n)
    }

    /// Fallible read used internally by the trait impl.
    fn try_get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        let row = conn
            .query_row(
                "SELECT body, etag, expires_at, pages FROM cache WHERE key = ?1",
                [key],
                |r| {
                    let body: Vec<u8> = r.get(0)?;
                    let etag: Option<String> = r.get(1)?;
                    let expires: i64 = r.get(2)?;
                    let pages: Option<u32> = r.get(3)?;
                    Ok((body, etag, expires, pages))
                },
            )
            .optional()?;
        Ok(row.map(|(body, etag, expires, pages)| CacheEntry {
            body,
            etag,
            expires_at: from_epoch(expires),
            pages,
        }))
    }

    /// Fallible write used internally by the trait impl.
    fn try_put(&self, key: &str, entry: &CacheEntry) -> Result<()> {
        let conn = self.conn.lock().expect("cache mutex poisoned");
        conn.execute(
            "INSERT INTO cache (key, body, etag, expires_at, pages) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(key) DO UPDATE SET
                 body = excluded.body,
                 etag = excluded.etag,
                 expires_at = excluded.expires_at,
                 pages = excluded.pages",
            rusqlite::params![key, entry.body, entry.etag, to_epoch(entry.expires_at), entry.pages],
        )?;
        Ok(())
    }
}

impl CacheStore for SqliteCacheStore {
    fn get(&self, key: &str) -> Option<CacheEntry> {
        // A cache miss is normal; swallow read errors into a miss (the client
        // will just perform a full fetch) but trace them for diagnosis.
        match self.try_get(key) {
            Ok(entry) => entry,
            Err(e) => {
                tracing::warn!("cache get failed for {key}: {e}");
                None
            }
        }
    }

    fn put(&self, key: &str, entry: CacheEntry) {
        if let Err(e) = self.try_put(key, &entry) {
            tracing::warn!("cache put failed for {key}: {e}");
        }
    }
}

/// Whole seconds since the Unix epoch; clamps pre-epoch times to 0.
fn to_epoch(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn from_epoch(secs: i64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs.max(0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esi::cache::{decide, CacheDecision};

    fn entry(fresh_for_secs: i64, etag: Option<&str>) -> CacheEntry {
        CacheEntry {
            body: b"{\"ok\":true}".to_vec(),
            etag: etag.map(String::from),
            expires_at: SystemTime::now() + Duration::from_secs(fresh_for_secs.max(0) as u64),
            pages: None,
        }
    }

    #[test]
    fn put_then_get_roundtrips() {
        let store = SqliteCacheStore::open_in_memory().unwrap();
        let mut e = entry(300, Some("etag-1"));
        e.pages = Some(7);
        store.put("/latest/status/", e);
        let got = store.get("/latest/status/").expect("entry present");
        assert_eq!(got.body, b"{\"ok\":true}");
        assert_eq!(got.etag.as_deref(), Some("etag-1"));
        assert_eq!(got.pages, Some(7)); // X-Pages survives the round-trip
        assert!(got.is_fresh(SystemTime::now()));
    }

    #[test]
    fn missing_key_is_none() {
        let store = SqliteCacheStore::open_in_memory().unwrap();
        assert!(store.get("/nope/").is_none());
    }

    #[test]
    fn put_upserts_existing_key() {
        let store = SqliteCacheStore::open_in_memory().unwrap();
        store.put("/k/", entry(300, Some("old")));
        store.put("/k/", entry(300, Some("new")));
        assert_eq!(store.count().unwrap(), 1);
        assert_eq!(store.get("/k/").unwrap().etag.as_deref(), Some("new"));
    }

    #[test]
    fn persisted_entry_drives_revalidation_decision() {
        // A stale-but-etagged entry survives in the store and yields Revalidate,
        // proving the warm-cache → free-304 path works end to end.
        let store = SqliteCacheStore::open_in_memory().unwrap();
        let mut e = entry(0, Some("keep-me"));
        e.expires_at = SystemTime::now() - Duration::from_secs(10); // already stale
        store.put("/k/", e);
        let fetched = store.get("/k/");
        assert_eq!(
            decide(fetched.as_ref(), SystemTime::now()),
            CacheDecision::Revalidate("keep-me".to_string())
        );
    }

    #[test]
    fn purge_removes_only_unrevalidatable_stale_rows() {
        let store = SqliteCacheStore::open_in_memory().unwrap();
        let past = SystemTime::now() - Duration::from_secs(10);
        // stale, no etag → purgeable
        let mut a = entry(0, None);
        a.expires_at = past;
        store.put("/a/", a);
        // stale, with etag → keep (still buys a 304)
        let mut b = entry(0, Some("e"));
        b.expires_at = past;
        store.put("/b/", b);
        // fresh → keep
        store.put("/c/", entry(300, None));

        let removed = store.purge_unrevalidatable(SystemTime::now()).unwrap();
        assert_eq!(removed, 1);
        assert!(store.get("/a/").is_none());
        assert!(store.get("/b/").is_some());
        assert!(store.get("/c/").is_some());
    }
}
