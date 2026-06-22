//! Id→name resolution with a persistent cache.
//!
//! Names (item types, systems, characters, …) are resolved in layers, cheapest
//! first:
//! 1. the persistent `names` cache in `app.sqlite` (instant, survives restarts),
//! 2. the SDE seed / full SDE (for item types),
//! 3. ESI `POST /universe/names/` for whatever's left (up to 1000 ids/call,
//!    public), with the results written back to the cache.
//!
//! Anything still unknown falls back to `Type {id}` at the call site. This makes
//! every item the user actually owns resolve, without shipping a full SDE.

use std::collections::HashMap;

use serde::Deserialize;

use crate::db::Database;
use crate::error::Result;
use crate::esi::EsiClient;
use crate::sde::Sde;

/// Max ids per ESI `/universe/names/` call.
const ESI_NAMES_BATCH: usize = 1000;

/// One resolved name from ESI `/universe/names/`.
#[derive(Debug, Clone, Deserialize)]
struct EsiName {
    id: i64,
    name: String,
    #[serde(default)]
    category: String,
}

/// Upwell structure detail from `/universe/structures/{id}/` (auth).
#[derive(Debug, Clone, Deserialize)]
struct EsiStructure {
    name: String,
}

/// Response of `POST /universe/ids/` — characters + systems + corps buckets.
#[derive(Debug, Clone, Deserialize, Default)]
struct EsiIds {
    #[serde(default)]
    characters: Vec<EsiNameOnly>,
    #[serde(default)]
    systems: Vec<EsiNameOnly>,
    #[serde(default)]
    corporations: Vec<EsiNameOnly>,
}

/// A name→id pair from `/universe/ids/` (it returns `name` + `id`, no category).
#[derive(Debug, Clone, Deserialize)]
struct EsiNameOnly {
    id: i64,
    name: String,
}

/// Player-owned (Upwell) structure ids start here; below this are NPC stations
/// and other ids that `/universe/names/` already resolves.
pub const STRUCTURE_ID_MIN: i64 = 1_000_000_000_000;

/// Resolves ids to names via cache → SDE → ESI, caching ESI results.
#[derive(Clone)]
pub struct NameResolver {
    esi: EsiClient,
    db: Database,
    sde: Sde,
}

impl NameResolver {
    pub fn new(esi: EsiClient, db: Database, sde: Sde) -> Self {
        Self { esi, db, sde }
    }

    /// Borrow the owned SDE handle (for SDE-backed lookups beyond name
    /// resolution, e.g. skill ranks).
    pub fn sde(&self) -> &Sde {
        &self.sde
    }

    /// Resolve `ids` to names. The returned map only contains ids that resolved;
    /// callers apply a `Type {id}` fallback for any that didn't.
    pub async fn resolve(&self, ids: &[i64]) -> Result<HashMap<i64, String>> {
        // Dedup while preserving determinism.
        let mut unique: Vec<i64> = ids.to_vec();
        unique.sort_unstable();
        unique.dedup();

        let mut out: HashMap<i64, String> = HashMap::new();

        // 1. Persistent cache.
        out.extend(self.db.cached_names(&unique).await?);
        let mut missing: Vec<i64> = unique.iter().copied().filter(|id| !out.contains_key(id)).collect();
        if missing.is_empty() {
            return Ok(out);
        }

        // 2. SDE (item types). SDE hits are not re-cached — the SDE is already
        //    fast and always present.
        let mut still_missing = Vec::new();
        for id in missing.drain(..) {
            match self.sde.type_name(id).await? {
                Some(name) => {
                    out.insert(id, name);
                }
                None => still_missing.push(id),
            }
        }
        if still_missing.is_empty() {
            return Ok(out);
        }

        // 3. ESI, in batches. Resilient to bad ids (ESI 404s the whole batch if
        //    any id is unresolvable, e.g. a mailing-list sender). Successes are
        //    cached; ids that never resolve fall back at the call site.
        let mut to_cache: Vec<(i64, String, Option<String>)> = Vec::new();
        for chunk in still_missing.chunks(ESI_NAMES_BATCH) {
            for n in self.resolve_resilient(chunk).await {
                to_cache.push((n.id, n.name.clone(), Some(n.category)));
                out.insert(n.id, n.name);
            }
        }
        if !to_cache.is_empty() {
            self.db.cache_names(&to_cache).await?;
        }

        Ok(out)
    }

    /// Resolve Upwell structure ids (citadels, engineering complexes) to names
    /// via the authenticated `/universe/structures/{id}/` endpoint, with the same
    /// persistent caching as [`resolve`]. These ids are NOT resolvable through the
    /// public `/universe/names/` endpoint, so hangar assets and clones docked in a
    /// citadel would otherwise show a raw id. Requires the character's access
    /// token + `esi-universe.read_structures.v1` (or docking access); ids that
    /// fail are simply omitted and the caller keeps its fallback.
    pub async fn resolve_structures(&self, ids: &[i64], token: &str) -> HashMap<i64, String> {
        let mut unique: Vec<i64> = ids.to_vec();
        unique.sort_unstable();
        unique.dedup();

        let mut out = self.db.cached_names(&unique).await.unwrap_or_default();
        let missing: Vec<i64> = unique
            .iter()
            .copied()
            .filter(|id| !out.contains_key(id))
            .collect();

        let mut to_cache = Vec::new();
        for id in missing {
            let path = format!("/latest/universe/structures/{id}/");
            if let Ok(s) = self.esi.get_auth_json::<EsiStructure>(&path, token).await {
                to_cache.push((id, s.name.clone(), Some("structure".to_string())));
                out.insert(id, s.name);
            }
        }
        if !to_cache.is_empty() {
            let _ = self.db.cache_names(&to_cache).await;
        }
        out
    }

    /// Resolve a chunk via ESI, splitting in half on failure so a single
    /// unresolvable id only loses itself rather than the whole batch.
    async fn resolve_resilient(&self, ids: &[i64]) -> Vec<EsiName> {
        let mut out = Vec::new();
        let mut stack: Vec<(usize, usize)> = vec![(0, ids.len())];
        while let Some((lo, hi)) = stack.pop() {
            if lo >= hi {
                continue;
            }
            match self.resolve_via_esi(&ids[lo..hi]).await {
                Ok(names) => out.extend(names),
                Err(_) if hi - lo <= 1 => {} // single bad id — drop it
                Err(_) => {
                    let mid = lo + (hi - lo) / 2;
                    stack.push((lo, mid));
                    stack.push((mid, hi));
                }
            }
        }
        out
    }

    /// Prefix-search item types by name (via the SDE). Limited to the seed until
    /// a full `sde.sqlite` is shipped.
    pub async fn search_types(&self, prefix: &str, limit: i64) -> Result<Vec<crate::sde::ItemType>> {
        self.sde.search_types(prefix, limit).await
    }

    /// Resolve a single id to a name (with `Type {id}` fallback).
    pub async fn name_or_id(&self, id: i64) -> String {
        self.resolve(&[id])
            .await
            .ok()
            .and_then(|m| m.get(&id).cloned())
            .unwrap_or_else(|| format!("Type {id}"))
    }

    async fn resolve_via_esi(&self, ids: &[i64]) -> Result<Vec<EsiName>> {
        self.esi
            .post_public_json::<[i64], Vec<EsiName>>("/latest/universe/names/", ids)
            .await
    }

    /// Resolve character *names* to ids via ESI `POST /universe/ids/` (the
    /// inverse of [`resolve`]). Used by the Local threat scanner to turn pasted
    /// pilot names into ids. Matching is case-insensitive; the returned map is
    /// keyed by the lowercased name. Names that don't match a character are
    /// simply absent.
    pub async fn character_ids(&self, names: &[String]) -> Result<HashMap<String, i64>> {
        let mut out = HashMap::new();
        for chunk in names.chunks(ESI_NAMES_BATCH) {
            let res: EsiIds = self
                .esi
                .post_public_json::<[String], EsiIds>("/latest/universe/ids/", chunk)
                .await?;
            for c in res.characters {
                out.insert(c.name.to_lowercase(), c.id);
            }
        }
        Ok(out)
    }

    /// Resolve a solar-system *name* to its id via ESI `POST /universe/ids/`.
    /// (The prebuilt SDE may not ship the universe tables, so we go to ESI.)
    pub async fn system_id(&self, name: &str) -> Result<Option<i64>> {
        let body = [name.to_string()];
        let res: EsiIds = self
            .esi
            .post_public_json::<[String], EsiIds>("/latest/universe/ids/", &body)
            .await?;
        let want = name.to_lowercase();
        Ok(res
            .systems
            .into_iter()
            .find(|s| s.name.to_lowercase() == want)
            .map(|s| s.id))
    }

    /// Resolve a corporation *name* to its id via ESI `POST /universe/ids/`
    /// (for the LP-store optimizer's corp picker).
    pub async fn corporation_id(&self, name: &str) -> Result<Option<i64>> {
        let body = [name.to_string()];
        let res: EsiIds = self
            .esi
            .post_public_json::<[String], EsiIds>("/latest/universe/ids/", &body)
            .await?;
        let want = name.to_lowercase();
        Ok(res
            .corporations
            .into_iter()
            .find(|c| c.name.to_lowercase() == want)
            .map(|c| c.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn resolver_with_seed() -> NameResolver {
        let cache: std::sync::Arc<dyn crate::esi::CacheStore> =
            std::sync::Arc::new(crate::esi::MemoryCacheStore::default());
        let esi = EsiClient::new("test/1.0", cache).unwrap();
        let db = Database::open_in_memory().await.unwrap();
        let sde = Sde::seeded().await.unwrap();
        NameResolver::new(esi, db, sde)
    }

    #[tokio::test]
    async fn resolves_from_sde_without_network() {
        let r = resolver_with_seed().await;
        // Tritanium (34) and Rifter (587) are in the SDE seed → resolved with no
        // ESI call needed.
        let names = r.resolve(&[34, 587]).await.unwrap();
        assert_eq!(names.get(&34).map(String::as_str), Some("Tritanium"));
        assert_eq!(names.get(&587).map(String::as_str), Some("Rifter"));
    }

    #[tokio::test]
    async fn serves_from_persistent_cache_first() {
        let r = resolver_with_seed().await;
        // Pre-seed the DB cache with an id the SDE doesn't know.
        r.db
            .cache_names(&[(99001, "Cached Widget".into(), Some("inventory_type".into()))])
            .await
            .unwrap();
        let names = r.resolve(&[99001]).await.unwrap();
        assert_eq!(names.get(&99001).map(String::as_str), Some("Cached Widget"));
    }

    #[tokio::test]
    async fn unknown_id_is_absent_from_map() {
        let r = resolver_with_seed().await;
        // 99999 isn't seeded or cached; with no network it simply doesn't
        // resolve (the ESI attempt fails offline and is swallowed).
        let names = r.resolve(&[99999]).await.unwrap();
        assert!(!names.contains_key(&99999));
    }
}
