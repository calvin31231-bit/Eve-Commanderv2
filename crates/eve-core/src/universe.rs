//! Universe topology reads (systems + stargate adjacency).
//!
//! Powers the System Safety surface: given the character's current system, find
//! its neighbours (one jump away) so we can show recent kill volume in and
//! around it. Topology is static, so these public reads sit behind the ESI cache
//! and cost almost nothing after the first fetch. The reductions are pure where
//! they can be; the fetches are exercised live.

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::esi::EsiClient;

/// A solar system (ESI `GET /universe/systems/{id}/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemInfo {
    pub system_id: i64,
    pub name: String,
    #[serde(default)]
    pub security_status: f64,
    #[serde(default)]
    pub stargates: Vec<i64>,
}

/// A stargate (ESI `GET /universe/stargates/{id}/`) — we only need where it goes.
#[derive(Debug, Clone, Deserialize)]
struct Stargate {
    destination: StargateDest,
}

#[derive(Debug, Clone, Deserialize)]
struct StargateDest {
    system_id: i64,
}

/// Per-system kill activity (ESI `GET /universe/system_kills/`), the canonical
/// hourly kill heatmap — one public call covers all of New Eden.
#[derive(Debug, Clone, Deserialize)]
pub struct SystemKills {
    pub system_id: i64,
    #[serde(default)]
    pub ship_kills: i64,
    #[serde(default)]
    pub npc_kills: i64,
    #[serde(default)]
    pub pod_kills: i64,
}

/// Sovereignty ownership of a system (ESI `GET /sovereignty/map/`).
#[derive(Debug, Clone, Deserialize)]
pub struct SovEntry {
    pub system_id: i64,
    #[serde(default)]
    pub alliance_id: Option<i64>,
    #[serde(default)]
    pub faction_id: Option<i64>,
}

/// Reads universe topology over the cache-first ESI client.
#[derive(Clone)]
pub struct UniverseClient {
    esi: EsiClient,
}

impl UniverseClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    /// Fetch a system's info (name, security, stargate ids).
    pub async fn system_info(&self, system_id: i64) -> Result<SystemInfo> {
        let path = format!("/latest/universe/systems/{system_id}/");
        self.esi.get_public_json::<SystemInfo>(&path).await
    }

    /// System kill counts across New Eden (ESI hourly cache) — the map heatmap.
    pub async fn system_kills(&self) -> Result<Vec<SystemKills>> {
        self.esi
            .get_public_json::<Vec<SystemKills>>("/latest/universe/system_kills/")
            .await
    }

    /// Sovereignty ownership for every claimable system (one public call).
    pub async fn sovereignty(&self) -> Result<Vec<SovEntry>> {
        self.esi
            .get_public_json::<Vec<SovEntry>>("/latest/sovereignty/map/")
            .await
    }

    /// The systems one jump from `system_id` (via its stargates). Best-effort:
    /// a stargate that fails to resolve is skipped rather than failing the set.
    pub async fn neighbors(&self, system_id: i64) -> Result<Vec<i64>> {
        let info = self.system_info(system_id).await?;
        let mut out = Vec::with_capacity(info.stargates.len());
        for sg in info.stargates {
            let path = format!("/latest/universe/stargates/{sg}/");
            if let Ok(gate) = self.esi.get_public_json::<Stargate>(&path).await {
                out.push(gate.destination.system_id);
            }
        }
        Ok(out)
    }
}
