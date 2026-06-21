//! Character assets: the paginated ESI asset list, aggregated by item type and
//! resolved to human names via the SDE.
//!
//! Assets is the first **paginated** ESI read (often dozens of pages for a
//! wealthy character), exercising [`EsiClient::get_auth_json_paged`]. The raw
//! list is thousands of stacks scattered across stations; the hub wants a
//! glanceable "what do I own" view, so we aggregate by `type_id` and turn ids
//! into names through the static data export.

use std::collections::HashMap;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;
use crate::prices::PriceMap;
use crate::sde::Sde;

/// One asset stack (ESI `GET /characters/{id}/assets/` element).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetItem {
    pub item_id: i64,
    pub type_id: i64,
    pub location_id: i64,
    #[serde(default)]
    pub location_flag: String,
    #[serde(default)]
    pub location_type: String,
    pub quantity: i64,
    #[serde(default)]
    pub is_singleton: bool,
    #[serde(default)]
    pub is_blueprint_copy: Option<bool>,
}

/// Assets of one item type, summed across stacks and locations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetGroup {
    pub type_id: i64,
    pub quantity: i64,
    /// Number of distinct locations the type is found in.
    pub locations: usize,
}

/// An [`AssetGroup`] with its SDE-resolved name (UI-facing, serializable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedAssetGroup {
    pub type_id: i64,
    pub name: String,
    pub quantity: i64,
    pub locations: usize,
}

/// Aggregate raw asset stacks by `type_id`: sum quantities and count distinct
/// locations. Sorted by quantity descending (biggest holdings first).
pub fn aggregate_by_type(items: &[AssetItem]) -> Vec<AssetGroup> {
    let mut acc: HashMap<i64, (i64, HashSet<i64>)> = HashMap::new();
    for item in items {
        let entry = acc.entry(item.type_id).or_insert((0, HashSet::new()));
        entry.0 += item.quantity;
        entry.1.insert(item.location_id);
    }
    let mut groups: Vec<AssetGroup> = acc
        .into_iter()
        .map(|(type_id, (quantity, locs))| AssetGroup {
            type_id,
            quantity,
            locations: locs.len(),
        })
        .collect();
    // Quantity desc, then type_id for a stable order on ties.
    groups.sort_by(|a, b| b.quantity.cmp(&a.quantity).then(a.type_id.cmp(&b.type_id)));
    groups
}

/// An [`AssetGroup`] with its estimated ISK value.
#[derive(Debug, Clone, PartialEq)]
pub struct ValuedGroup {
    pub type_id: i64,
    pub quantity: i64,
    pub locations: usize,
    pub value: f64,
}

/// Asset holdings valued against a [`PriceMap`].
#[derive(Debug, Clone, PartialEq)]
pub struct ValuedHoldings {
    /// Total estimated value across **all** holdings (not just the top ones).
    pub total_value: f64,
    /// The `top_n` groups by value, descending.
    pub groups: Vec<ValuedGroup>,
}

/// Estimated value sitting at one location.
#[derive(Debug, Clone, PartialEq)]
pub struct LocationValue {
    pub location_id: i64,
    pub value: f64,
    pub item_count: usize,
}

/// Aggregate **hangar** assets by location, valued via the price map. Only
/// top-level hangar items are counted (items nested in ships/containers carry a
/// container item-id as their "location", which isn't a place), so this reads as
/// "value sitting in each station". Sorted by value descending.
pub fn aggregate_by_location(items: &[AssetItem], prices: &PriceMap) -> Vec<LocationValue> {
    let mut acc: HashMap<i64, (f64, usize)> = HashMap::new();
    for item in items {
        if item.location_flag != "Hangar" {
            continue;
        }
        let entry = acc.entry(item.location_id).or_insert((0.0, 0));
        entry.0 += prices.value(item.type_id, item.quantity);
        entry.1 += 1;
    }
    let mut out: Vec<LocationValue> = acc
        .into_iter()
        .map(|(location_id, (value, item_count))| LocationValue { location_id, value, item_count })
        .collect();
    out.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.location_id.cmp(&b.location_id))
    });
    out
}

/// Value every group, total them, and return the `top_n` by value (descending).
/// The total reflects all holdings; only the returned rows are truncated.
pub fn value_holdings(groups: &[AssetGroup], prices: &PriceMap, top_n: usize) -> ValuedHoldings {
    let mut valued: Vec<ValuedGroup> = groups
        .iter()
        .map(|g| ValuedGroup {
            type_id: g.type_id,
            quantity: g.quantity,
            locations: g.locations,
            value: prices.value(g.type_id, g.quantity),
        })
        .collect();

    let total_value: f64 = valued.iter().map(|g| g.value).sum();

    // Most valuable first; type_id breaks ties for a stable order.
    valued.sort_by(|a, b| {
        b.value
            .partial_cmp(&a.value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.type_id.cmp(&b.type_id))
    });
    valued.truncate(top_n);

    ValuedHoldings {
        total_value,
        groups: valued,
    }
}

/// Resolve type ids to names via the SDE, falling back to `Type {id}` for ids
/// the (version-pinned) SDE doesn't know.
pub async fn resolve_names(groups: &[AssetGroup], sde: &Sde) -> Result<Vec<NamedAssetGroup>> {
    let mut out = Vec::with_capacity(groups.len());
    for g in groups {
        let name = sde
            .type_name(g.type_id)
            .await?
            .unwrap_or_else(|| format!("Type {}", g.type_id));
        out.push(NamedAssetGroup {
            type_id: g.type_id,
            name,
            quantity: g.quantity,
            locations: g.locations,
        });
    }
    Ok(out)
}

/// Typed, authenticated asset reads over the cache-first ESI client.
#[derive(Clone)]
pub struct AssetsClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl AssetsClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// Fetch every page of the character's assets.
    pub async fn items(&self, character_id: i64) -> Result<Vec<AssetItem>> {
        let ep = endpoint("assets").ok_or_else(|| Error::other("unknown endpoint 'assets'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json_paged::<AssetItem>(&ep.path_for(character_id), &token)
            .await
    }

    /// Fetch and aggregate **all** holdings by type (untruncated), so callers
    /// can rank by value once prices are known.
    pub async fn all_holdings(&self, character_id: i64) -> Result<Vec<AssetGroup>> {
        let items = self.items(character_id).await?;
        Ok(aggregate_by_type(&items))
    }

    /// Fetch assets and aggregate hangar value by location.
    pub async fn by_location(&self, character_id: i64, prices: &PriceMap) -> Result<Vec<LocationValue>> {
        let items = self.items(character_id).await?;
        Ok(aggregate_by_location(&items, prices))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_asset_item() {
        let json = r#"{
            "is_singleton": true,
            "item_id": 1000000016835,
            "location_flag": "Hangar",
            "location_id": 60002959,
            "location_type": "station",
            "quantity": 3,
            "type_id": 587
        }"#;
        let item: AssetItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.type_id, 587);
        assert_eq!(item.quantity, 3);
        assert_eq!(item.location_flag, "Hangar");
        assert!(item.is_singleton);
    }

    fn item(type_id: i64, location_id: i64, quantity: i64) -> AssetItem {
        AssetItem {
            item_id: type_id * 1000 + location_id,
            type_id,
            location_id,
            location_flag: "Hangar".into(),
            location_type: "station".into(),
            quantity,
            is_singleton: false,
            is_blueprint_copy: None,
        }
    }

    #[test]
    fn aggregates_sums_quantity_and_counts_locations() {
        let items = vec![
            item(34, 60000001, 1000), // Tritanium in station A
            item(34, 60000002, 500),  // Tritanium in station B
            item(587, 60000001, 1),   // one Rifter
        ];
        let groups = aggregate_by_type(&items);
        // Sorted by quantity desc → Tritanium first.
        assert_eq!(groups[0].type_id, 34);
        assert_eq!(groups[0].quantity, 1500);
        assert_eq!(groups[0].locations, 2);
        assert_eq!(groups[1].type_id, 587);
        assert_eq!(groups[1].quantity, 1);
        assert_eq!(groups[1].locations, 1);
    }

    #[test]
    fn values_rank_by_value_and_total_covers_all() {
        use crate::prices::{PriceMap, TypePrice};
        let prices = PriceMap::from_prices(&[
            TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 },   // Tritanium
            TypePrice { type_id: 587, average_price: 1_000_000.0, adjusted_price: 0.0 }, // Rifter
        ]);
        let groups = vec![
            AssetGroup { type_id: 34, quantity: 1000, locations: 1 },  // 5,000
            AssetGroup { type_id: 587, quantity: 2, locations: 1 },    // 2,000,000
            AssetGroup { type_id: 999, quantity: 50, locations: 1 },   // unpriced → 0
        ];
        let valued = value_holdings(&groups, &prices, 2);
        // Total spans all three groups (incl. the zero-valued one).
        assert_eq!(valued.total_value, 2_005_000.0);
        // Top 2 by value: Rifter then Tritanium.
        assert_eq!(valued.groups.len(), 2);
        assert_eq!(valued.groups[0].type_id, 587);
        assert_eq!(valued.groups[0].value, 2_000_000.0);
        assert_eq!(valued.groups[1].type_id, 34);
    }

    #[test]
    fn aggregates_hangar_value_by_location() {
        use crate::prices::{PriceMap, TypePrice};
        let prices = PriceMap::from_prices(&[TypePrice { type_id: 34, average_price: 5.0, adjusted_price: 0.0 }]);
        let mut a = item(34, 60000001, 1000); // Hangar, station A → 5000
        a.location_flag = "Hangar".into();
        let mut b = item(34, 60000002, 200); // Hangar, station B → 1000
        b.location_flag = "Hangar".into();
        let mut nested = item(34, 99999, 5000); // in a container → excluded
        nested.location_flag = "Cargo".into();
        let locs = aggregate_by_location(&[a, b, nested], &prices);
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0].location_id, 60000001);
        assert_eq!(locs[0].value, 5000.0);
        assert_eq!(locs[1].location_id, 60000002);
    }

    #[tokio::test]
    async fn resolves_names_with_fallback() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_type(34, "Tritanium", Some(18)).await.unwrap();

        let groups = vec![
            AssetGroup { type_id: 34, quantity: 1500, locations: 2 },
            AssetGroup { type_id: 99999, quantity: 1, locations: 1 }, // unknown to SDE
        ];
        let named = resolve_names(&groups, &sde).await.unwrap();
        assert_eq!(named[0].name, "Tritanium");
        assert_eq!(named[1].name, "Type 99999"); // graceful fallback
    }
}
