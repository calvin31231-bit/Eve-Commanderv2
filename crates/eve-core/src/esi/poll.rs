//! Turning a scheduler batch into concrete ESI fetches.
//!
//! The [`Scheduler`](super::scheduler::Scheduler) decides *which* (character,
//! endpoint) jobs are due each tick; this module resolves each into a
//! [`PlannedFetch`] — the exact path to request and whether it needs a bearer
//! token — by consulting the [`endpoints`](super::endpoints) catalog. Keeping
//! this resolution pure makes the background worker's behavior unit-testable;
//! the worker itself only has to execute the plan.

use super::endpoints::endpoint;

/// A fully-resolved fetch the poller will perform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFetch {
    pub character_id: i64,
    /// Endpoint catalog key.
    pub endpoint: &'static str,
    /// Concrete ESI path (placeholders filled in).
    pub path: String,
    /// Whether the request needs an access token (authenticated route).
    pub needs_auth: bool,
}

/// Resolve a scheduler batch (`(character_id, endpoint_key)` pairs) into
/// concrete fetches. Unknown endpoint keys are skipped defensively.
pub fn plan_fetches(batch: &[(i64, &'static str)]) -> Vec<PlannedFetch> {
    batch
        .iter()
        .filter_map(|(character_id, key)| {
            let ep = endpoint(key)?;
            Some(PlannedFetch {
                character_id: *character_id,
                endpoint: ep.key,
                path: ep.path_for(*character_id),
                needs_auth: !ep.is_public(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::esi::endpoints::SHARED_CHARACTER_ID;

    #[test]
    fn resolves_paths_and_auth_flags() {
        let batch = vec![
            (SHARED_CHARACTER_ID, "server_status"),
            (123, "skills"),
            (123, "character_public"),
        ];
        let plans = plan_fetches(&batch);

        let status = plans.iter().find(|p| p.endpoint == "server_status").unwrap();
        assert_eq!(status.path, "/latest/status/");
        assert!(!status.needs_auth);

        let skills = plans.iter().find(|p| p.endpoint == "skills").unwrap();
        assert_eq!(skills.path, "/latest/characters/123/skills/");
        assert!(skills.needs_auth);

        // Public per-character route resolves but needs no token.
        let pub_info = plans.iter().find(|p| p.endpoint == "character_public").unwrap();
        assert_eq!(pub_info.path, "/latest/characters/123/");
        assert!(!pub_info.needs_auth);
    }

    #[test]
    fn unknown_endpoints_are_skipped() {
        let batch = vec![(1, "not_a_real_endpoint"), (1, "skills")];
        let plans = plan_fetches(&batch);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].endpoint, "skills");
    }
}
