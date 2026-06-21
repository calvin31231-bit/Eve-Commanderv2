//! The catalog of pollable ESI endpoints.
//!
//! Each entry pins a route to (a) the [`PollClass`] matching its ESI cache timer
//! and (b) the scope it requires. The scheduler is seeded from this catalog, and
//! **scope-gating happens here**: a character is only given jobs for endpoints
//! whose scope it has granted. That keeps us from repeatedly hitting auth/ 403
//! walls and needlessly burning the shared error budget.
//!
//! Paths use a `{cid}` placeholder filled in per character at fetch time. Routes
//! with no `{cid}` are *shared* (e.g. server status) and are polled once under
//! the synthetic character id `0`.
//!
//! This is an early, representative set (Phase 0 / early Phase 1). Later phases
//! extend the catalog as features land — the shape stays the same.

use crate::model::Character;

use super::scheduler::{PollClass, PollJob};

/// Synthetic character id for shared/public jobs fetched once for everyone.
pub const SHARED_CHARACTER_ID: i64 = 0;

/// A pollable ESI route.
#[derive(Debug, Clone, Copy)]
pub struct Endpoint {
    /// Stable key, also used as the [`PollJob`] endpoint id.
    pub key: &'static str,
    /// Path template; `{cid}` is replaced with the character id.
    pub path: &'static str,
    pub class: PollClass,
    /// Scope required to call it, or `None` for public routes.
    pub scope: Option<&'static str>,
}

impl Endpoint {
    /// Whether this is a public (unauthenticated) route.
    pub fn is_public(&self) -> bool {
        self.scope.is_none()
    }

    /// Whether this route is per-character (its path is parameterized by `{cid}`).
    /// Routes without `{cid}` are shared.
    pub fn is_character_scoped(&self) -> bool {
        self.path.contains("{cid}")
    }

    /// Build the concrete ESI path for `character_id`.
    pub fn path_for(&self, character_id: i64) -> String {
        self.path.replace("{cid}", &character_id.to_string())
    }
}

/// The endpoint catalog. Cadences come from [`PollClass`], chosen to sit at or
/// below each route's published ESI cache timer.
pub const CATALOG: &[Endpoint] = &[
    // --- shared / public ---
    Endpoint {
        key: "server_status",
        path: "/latest/status/",
        class: PollClass::Fast,
        scope: None,
    },
    // --- shared / public market data ---
    Endpoint {
        key: "market_prices",
        path: "/latest/markets/prices/",
        class: PollClass::Daily,
        scope: None,
    },
    // --- per-character public ---
    Endpoint {
        key: "character_public",
        path: "/latest/characters/{cid}/",
        class: PollClass::Slow,
        scope: None,
    },
    // --- per-character authenticated ---
    Endpoint {
        key: "skills",
        path: "/latest/characters/{cid}/skills/",
        class: PollClass::Fast,
        scope: Some("esi-skills.read_skills.v1"),
    },
    Endpoint {
        key: "skillqueue",
        path: "/latest/characters/{cid}/skillqueue/",
        class: PollClass::Fast,
        scope: Some("esi-skills.read_skillqueue.v1"),
    },
    Endpoint {
        key: "wallet_balance",
        path: "/latest/characters/{cid}/wallet/",
        class: PollClass::Fast,
        scope: Some("esi-wallet.read_character_wallet.v1"),
    },
    Endpoint {
        key: "wallet_journal",
        path: "/latest/characters/{cid}/wallet/journal/",
        class: PollClass::Slow,
        scope: Some("esi-wallet.read_character_wallet.v1"),
    },
    Endpoint {
        key: "online",
        path: "/latest/characters/{cid}/online/",
        class: PollClass::Fast,
        scope: Some("esi-location.read_online.v1"),
    },
    Endpoint {
        key: "location",
        path: "/latest/characters/{cid}/location/",
        class: PollClass::OnDemand,
        scope: Some("esi-location.read_location.v1"),
    },
    Endpoint {
        key: "ship",
        path: "/latest/characters/{cid}/ship/",
        class: PollClass::OnDemand,
        scope: Some("esi-location.read_ship_type.v1"),
    },
    Endpoint {
        key: "industry_jobs",
        path: "/latest/characters/{cid}/industry/jobs/",
        class: PollClass::Medium,
        scope: Some("esi-industry.read_character_jobs.v1"),
    },
    Endpoint {
        key: "mining",
        path: "/latest/characters/{cid}/mining/",
        class: PollClass::Slow,
        scope: Some("esi-industry.read_character_mining.v1"),
    },
    Endpoint {
        key: "mail",
        path: "/latest/characters/{cid}/mail/",
        class: PollClass::Fast,
        scope: Some("esi-mail.read_mail.v1"),
    },
    Endpoint {
        key: "market_orders",
        path: "/latest/characters/{cid}/orders/",
        class: PollClass::Medium,
        scope: Some("esi-markets.read_character_orders.v1"),
    },
    Endpoint {
        key: "assets",
        path: "/latest/characters/{cid}/assets/",
        class: PollClass::Slow,
        scope: Some("esi-assets.read_assets.v1"),
    },
    Endpoint {
        key: "clones",
        path: "/latest/characters/{cid}/clones/",
        class: PollClass::Slow,
        scope: Some("esi-clones.read_clones.v1"),
    },
    Endpoint {
        key: "implants",
        path: "/latest/characters/{cid}/implants/",
        class: PollClass::Slow,
        scope: Some("esi-clones.read_implants.v1"),
    },
];

/// Look up an endpoint by key.
pub fn endpoint(key: &str) -> Option<&'static Endpoint> {
    CATALOG.iter().find(|e| e.key == key)
}

/// The shared/public jobs polled once for everyone (character id
/// [`SHARED_CHARACTER_ID`]).
pub fn shared_jobs() -> Vec<PollJob> {
    CATALOG
        .iter()
        .filter(|e| !e.is_character_scoped())
        .map(|e| PollJob::new(SHARED_CHARACTER_ID, e.key, e.class))
        .collect()
}

/// The poll jobs a character is eligible for: every per-character endpoint that
/// is public, plus the authenticated ones whose scope it has granted.
pub fn jobs_for_character(character: &Character) -> Vec<PollJob> {
    CATALOG
        .iter()
        .filter(|e| e.is_character_scoped())
        .filter(|e| match e.scope {
            None => true,
            Some(scope) => character.has_scope(scope),
        })
        .map(|e| PollJob::new(character.id, e.key, e.class))
        .collect()
}

/// All jobs needed for a set of characters: the shared jobs plus each
/// character's scope-eligible jobs. Ready to feed into the scheduler.
pub fn all_jobs(characters: &[Character]) -> Vec<PollJob> {
    let mut jobs = shared_jobs();
    for c in characters {
        jobs.extend(jobs_for_character(c));
    }
    jobs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn character_with(scopes: &[&str]) -> Character {
        let mut c = Character::new(90000001, "Test Pilot");
        c.scopes = scopes.iter().map(|s| s.to_string()).collect();
        c
    }

    #[test]
    fn path_template_substitutes_character_id() {
        let skills = endpoint("skills").unwrap();
        assert_eq!(skills.path_for(123), "/latest/characters/123/skills/");
    }

    #[test]
    fn shared_jobs_are_polled_under_id_zero() {
        let shared = shared_jobs();
        assert!(shared.iter().any(|j| j.endpoint == "server_status"
            && j.character_id == SHARED_CHARACTER_ID));
        // Per-character routes are never shared.
        assert!(!shared.iter().any(|j| j.endpoint == "skills"));
    }

    #[test]
    fn no_scopes_yields_only_public_per_character_jobs() {
        let jobs = jobs_for_character(&character_with(&[]));
        let keys: Vec<_> = jobs.iter().map(|j| j.endpoint).collect();
        // Public per-character info is allowed; authenticated routes are not.
        assert_eq!(keys, vec!["character_public"]);
    }

    #[test]
    fn scopes_unlock_their_endpoints() {
        let c = character_with(&[
            "esi-skills.read_skills.v1",
            "esi-wallet.read_character_wallet.v1",
        ]);
        let keys: Vec<_> = jobs_for_character(&c).iter().map(|j| j.endpoint).collect();
        assert!(keys.contains(&"skills"));
        assert!(keys.contains(&"wallet_balance"));
        // A scope we did NOT grant stays gated.
        assert!(!keys.contains(&"assets"));
        // Public per-character route is always present.
        assert!(keys.contains(&"character_public"));
    }

    #[test]
    fn all_jobs_combines_shared_and_per_character() {
        let chars = vec![
            character_with(&["esi-skills.read_skills.v1"]),
            {
                let mut c = Character::new(90000002, "Alt");
                c.scopes = vec!["esi-assets.read_assets.v1".to_string()];
                c
            },
        ];
        let jobs = all_jobs(&chars);
        // server_status (shared) present once.
        assert_eq!(
            jobs.iter().filter(|j| j.endpoint == "server_status").count(),
            1
        );
        // Each character contributes its own jobs under its own id.
        assert!(jobs
            .iter()
            .any(|j| j.character_id == 90000001 && j.endpoint == "skills"));
        assert!(jobs
            .iter()
            .any(|j| j.character_id == 90000002 && j.endpoint == "assets"));
    }

    #[test]
    fn catalog_keys_are_unique() {
        let mut keys: Vec<_> = CATALOG.iter().map(|e| e.key).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "duplicate endpoint key in CATALOG");
    }
}
