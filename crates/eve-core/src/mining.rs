//! Mining ledger: the character's personal daily mining records, aggregated by
//! ore type.
//!
//! ESI's `GET /characters/{id}/mining/` (paginated) returns one row per
//! (day, system, ore type) with the units mined. ESI keeps ~30 days, so this is
//! also raw material for longer-term yield analytics later. The aggregation is
//! pure and unit-tested.

use std::collections::HashMap;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One mining-ledger row (ESI `GET /characters/{id}/mining/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiningEntry {
    /// Calendar day, `YYYY-MM-DD` (ESI emits date-only here).
    pub date: String,
    pub quantity: i64,
    pub solar_system_id: i64,
    pub type_id: i64,
}

/// Units of one ore type mined over the ledger window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OreTotal {
    pub type_id: i64,
    pub quantity: i64,
}

/// Rollup of the mining ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MiningSummary {
    pub total_units: i64,
    /// Distinct days that have any mining activity.
    pub day_count: usize,
    /// Ore types mined, most units first.
    pub by_ore: Vec<OreTotal>,
}

/// Aggregate ledger rows by ore type, count active days, and total the units.
/// Sorted by quantity descending. Pure → unit-tested.
pub fn summarize_mining(entries: &[MiningEntry]) -> MiningSummary {
    let mut by_type: HashMap<i64, i64> = HashMap::new();
    let mut days: HashSet<&str> = HashSet::new();
    let mut total_units = 0i64;

    for e in entries {
        *by_type.entry(e.type_id).or_insert(0) += e.quantity;
        days.insert(e.date.as_str());
        total_units += e.quantity;
    }

    let mut by_ore: Vec<OreTotal> = by_type
        .into_iter()
        .map(|(type_id, quantity)| OreTotal { type_id, quantity })
        .collect();
    by_ore.sort_by(|a, b| b.quantity.cmp(&a.quantity).then(a.type_id.cmp(&b.type_id)));

    MiningSummary {
        total_units,
        day_count: days.len(),
        by_ore,
    }
}

/// Typed, authenticated mining-ledger reads over the cache-first ESI client.
#[derive(Clone)]
pub struct MiningClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl MiningClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// Fetch every page of the character's mining ledger.
    pub async fn ledger(&self, character_id: i64) -> Result<Vec<MiningEntry>> {
        let ep = endpoint("mining").ok_or_else(|| Error::other("unknown endpoint 'mining'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json_paged::<MiningEntry>(&ep.path_for(character_id), &token)
            .await
    }

    /// Fetch and aggregate the ledger, keeping only the `top_n` ores.
    pub async fn summary(&self, character_id: i64, top_n: usize) -> Result<MiningSummary> {
        let entries = self.ledger(character_id).await?;
        let mut summary = summarize_mining(&entries);
        summary.by_ore.truncate(top_n);
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_mining_entry() {
        let json = r#"{
            "date": "2026-06-20",
            "quantity": 15000,
            "solar_system_id": 30000142,
            "type_id": 34
        }"#;
        let e: MiningEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.date, "2026-06-20");
        assert_eq!(e.quantity, 15000);
        assert_eq!(e.type_id, 34);
    }

    fn entry(date: &str, type_id: i64, quantity: i64) -> MiningEntry {
        MiningEntry { date: date.into(), quantity, solar_system_id: 30000142, type_id }
    }

    #[test]
    fn summarizes_by_ore_days_and_total() {
        let entries = vec![
            entry("2026-06-20", 34, 10000),
            entry("2026-06-20", 35, 4000),
            entry("2026-06-21", 34, 5000), // same ore, different day
        ];
        let s = summarize_mining(&entries);
        assert_eq!(s.total_units, 19000);
        assert_eq!(s.day_count, 2);
        // Most-mined ore first: 34 (15000) before 35 (4000).
        assert_eq!(s.by_ore[0].type_id, 34);
        assert_eq!(s.by_ore[0].quantity, 15000);
        assert_eq!(s.by_ore[1].type_id, 35);
        assert_eq!(s.by_ore[1].quantity, 4000);
    }

    #[test]
    fn empty_ledger_is_zeroed() {
        let s = summarize_mining(&[]);
        assert_eq!(s.total_units, 0);
        assert_eq!(s.day_count, 0);
        assert!(s.by_ore.is_empty());
    }
}
