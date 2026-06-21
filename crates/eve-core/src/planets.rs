//! Planetary Industry: the character's colonies + extractor-cycle countdowns.
//!
//! ESI exposes a colony list (`/characters/{id}/planets/`) and a per-planet
//! layout (`/characters/{id}/planets/{planet_id}/`) whose extractor pins carry
//! an `expiry_time`. Like industry jobs, those expiries are deterministic once
//! fetched, so we surface them as client-side countdowns (the "derive live state
//! locally" principle) — a PI dashboard of ticking extractor timers costs no
//! extra network. The [`summarize_colony`] reduction is pure and unit-tested.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One colony (ESI `GET /characters/{id}/planets/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Colony {
    pub planet_id: i64,
    pub solar_system_id: i64,
    #[serde(default)]
    pub planet_type: String,
    #[serde(default)]
    pub upgrade_level: i64,
    #[serde(default)]
    pub num_pins: i64,
    #[serde(default)]
    pub last_update: String,
}

/// Extractor specifics on a pin (only present for extractor control units).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractorDetails {
    #[serde(default)]
    pub product_type_id: Option<i64>,
    #[serde(default)]
    pub qty_per_cycle: Option<i64>,
    #[serde(default)]
    pub cycle_time: Option<i64>,
}

/// One pin in a colony layout (ESI `GET /characters/{id}/planets/{planet_id}/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pin {
    pub pin_id: i64,
    pub type_id: i64,
    /// When the current extractor program ends (extractors only).
    #[serde(default)]
    pub expiry_time: Option<String>,
    #[serde(default)]
    pub extractor_details: Option<ExtractorDetails>,
    #[serde(default)]
    pub schematic_id: Option<i64>,
}

/// A colony layout: pins, links, routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColonyLayout {
    #[serde(default)]
    pub pins: Vec<Pin>,
    #[serde(default)]
    pub links: Vec<serde_json::Value>,
    #[serde(default)]
    pub routes: Vec<serde_json::Value>,
}

/// A colony rolled up for the UI: counts + the soonest extractor expiry as a
/// client-side countdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColonyStatus {
    pub planet_id: i64,
    pub solar_system_id: i64,
    pub planet_type: String,
    pub upgrade_level: i64,
    pub num_pins: i64,
    pub extractor_count: usize,
    /// Soonest extractor expiry (RFC3339), if any extractor is running.
    pub soonest_expiry: Option<String>,
    /// Seconds until that soonest expiry (0 if passed / none).
    pub seconds_remaining: i64,
    /// Product type ids being extracted (for name resolution).
    pub products: Vec<i64>,
}

/// Reduce a colony + its layout to a [`ColonyStatus`] with a countdown to the
/// next extractor expiry. Pure (time injected) → unit-tested.
pub fn summarize_colony(colony: &Colony, layout: &ColonyLayout, now: OffsetDateTime) -> ColonyStatus {
    let mut soonest: Option<(OffsetDateTime, String)> = None;
    let mut extractor_count = 0;
    let mut products = Vec::new();

    for pin in &layout.pins {
        if let Some(ed) = &pin.extractor_details {
            extractor_count += 1;
            if let Some(p) = ed.product_type_id {
                if !products.contains(&p) {
                    products.push(p);
                }
            }
        }
        if let Some(raw) = &pin.expiry_time {
            if let Ok(t) = OffsetDateTime::parse(raw, &Rfc3339) {
                let is_sooner = match &soonest {
                    Some((s, _)) => t < *s,
                    None => true,
                };
                if is_sooner {
                    soonest = Some((t, raw.clone()));
                }
            }
        }
    }

    let (soonest_expiry, seconds_remaining) = match soonest {
        Some((t, raw)) => (Some(raw), (t - now).whole_seconds().max(0)),
        None => (None, 0),
    };

    ColonyStatus {
        planet_id: colony.planet_id,
        solar_system_id: colony.solar_system_id,
        planet_type: colony.planet_type.clone(),
        upgrade_level: colony.upgrade_level,
        num_pins: colony.num_pins,
        extractor_count,
        soonest_expiry,
        seconds_remaining,
        products,
    }
}

/// Typed, authenticated planetary-industry reads over the cache-first client.
#[derive(Clone)]
pub struct PlanetsClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl PlanetsClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// The character's colonies.
    pub async fn colonies(&self, character_id: i64) -> Result<Vec<Colony>> {
        let ep = endpoint("planets").ok_or_else(|| Error::other("unknown endpoint 'planets'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<Vec<Colony>>(&ep.path_for(character_id), &token)
            .await
    }

    /// A single colony's layout (pins/links/routes).
    pub async fn layout(&self, character_id: i64, planet_id: i64) -> Result<ColonyLayout> {
        let token = self.tokens.access_token(character_id).await?;
        let path = format!("/latest/characters/{character_id}/planets/{planet_id}/");
        self.esi.get_auth_json::<ColonyLayout>(&path, &token).await
    }

    /// Fetch every colony and summarize it with extractor countdowns, soonest
    /// expiry first. A colony whose layout fails to load still appears (with its
    /// list-level counts) rather than dropping the whole view.
    pub async fn summary(&self, character_id: i64) -> Result<Vec<ColonyStatus>> {
        let colonies = self.colonies(character_id).await?;
        let now = OffsetDateTime::now_utc();
        let mut out = Vec::with_capacity(colonies.len());
        for c in &colonies {
            let layout = self
                .layout(character_id, c.planet_id)
                .await
                .unwrap_or(ColonyLayout { pins: Vec::new(), links: Vec::new(), routes: Vec::new() });
            out.push(summarize_colony(c, &layout, now));
        }
        // Colonies with a running extractor first (by soonest expiry); idle last.
        out.sort_by_key(|c| {
            if c.soonest_expiry.is_some() {
                (0, c.seconds_remaining)
            } else {
                (1, 0)
            }
        });
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Rfc3339).unwrap()
    }

    fn extractor(pin_id: i64, product: i64, expiry: &str) -> Pin {
        Pin {
            pin_id,
            type_id: 2848,
            expiry_time: Some(expiry.to_string()),
            extractor_details: Some(ExtractorDetails {
                product_type_id: Some(product),
                qty_per_cycle: Some(1000),
                cycle_time: Some(3600),
            }),
            schematic_id: None,
        }
    }

    #[test]
    fn summarize_picks_soonest_expiry_and_counts_extractors() {
        let colony = Colony {
            planet_id: 40000001,
            solar_system_id: 30000142,
            planet_type: "barren".into(),
            upgrade_level: 3,
            num_pins: 6,
            last_update: String::new(),
        };
        let layout = ColonyLayout {
            pins: vec![
                extractor(1, 2268, "2026-06-21T12:00:00Z"),
                extractor(2, 2305, "2026-06-21T10:00:00Z"), // sooner
                Pin { pin_id: 3, type_id: 2473, expiry_time: None, extractor_details: None, schematic_id: Some(127) },
            ],
            links: vec![],
            routes: vec![],
        };
        let now = ts("2026-06-21T09:00:00Z");
        let s = summarize_colony(&colony, &layout, now);
        assert_eq!(s.extractor_count, 2);
        assert_eq!(s.soonest_expiry.as_deref(), Some("2026-06-21T10:00:00Z"));
        assert_eq!(s.seconds_remaining, 3600); // one hour out
        assert_eq!(s.products, vec![2268, 2305]);
    }

    #[test]
    fn passed_expiry_clamps_to_zero_and_idle_has_none() {
        let colony = Colony {
            planet_id: 1,
            solar_system_id: 30000142,
            planet_type: "lava".into(),
            upgrade_level: 1,
            num_pins: 1,
            last_update: String::new(),
        };
        // Idle colony: no extractor pins.
        let idle = ColonyLayout { pins: vec![], links: vec![], routes: vec![] };
        let s = summarize_colony(&colony, &idle, ts("2026-06-21T09:00:00Z"));
        assert!(s.soonest_expiry.is_none());
        assert_eq!(s.seconds_remaining, 0);

        // Expired extractor clamps to zero rather than going negative.
        let expired = ColonyLayout {
            pins: vec![extractor(1, 2268, "2026-06-21T08:00:00Z")],
            links: vec![],
            routes: vec![],
        };
        let s = summarize_colony(&colony, &expired, ts("2026-06-21T09:00:00Z"));
        assert_eq!(s.seconds_remaining, 0);
    }
}
