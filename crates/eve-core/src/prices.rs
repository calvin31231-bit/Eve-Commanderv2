//! Market price reference for valuation.
//!
//! ESI's public `GET /markets/prices/` returns an average and adjusted price for
//! **every** published type in a single, day-cached call — the cheap way to put
//! an ISK figure on assets and mined ore without per-type market lookups. We
//! build a [`PriceMap`] from it (preferring `average_price`, falling back to
//! `adjusted_price`). The map and its valuation helpers are pure and tested.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One entry from `GET /markets/prices/`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypePrice {
    pub type_id: i64,
    #[serde(default)]
    pub average_price: f64,
    #[serde(default)]
    pub adjusted_price: f64,
}

/// A type_id → ISK reference-price lookup.
#[derive(Debug, Clone, Default)]
pub struct PriceMap {
    map: HashMap<i64, f64>,
}

impl PriceMap {
    /// Build from raw price entries, preferring `average_price` and falling back
    /// to `adjusted_price` (some types only carry the latter).
    pub fn from_prices(prices: &[TypePrice]) -> Self {
        let map = prices
            .iter()
            .map(|p| {
                let price = if p.average_price > 0.0 {
                    p.average_price
                } else {
                    p.adjusted_price
                };
                (p.type_id, price)
            })
            .collect();
        Self { map }
    }

    /// Reference price for a type, if known.
    pub fn price(&self, type_id: i64) -> Option<f64> {
        self.map.get(&type_id).copied()
    }

    /// Value of `quantity` units of a type (0 when the type has no known price).
    pub fn value(&self, type_id: i64, quantity: i64) -> f64 {
        self.price(type_id).unwrap_or(0.0) * quantity as f64
    }

    /// Number of priced types.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Fetches the public market price reference.
#[derive(Clone)]
pub struct PricesClient {
    esi: EsiClient,
}

impl PricesClient {
    pub fn new(esi: EsiClient) -> Self {
        Self { esi }
    }

    /// Fetch the full price list (public, day-cached).
    pub async fn prices(&self) -> Result<Vec<TypePrice>> {
        let ep =
            endpoint("market_prices").ok_or_else(|| Error::other("unknown endpoint 'market_prices'"))?;
        self.esi.get_public_json::<Vec<TypePrice>>(ep.path).await
    }

    /// Fetch and build the price map.
    pub async fn price_map(&self) -> Result<PriceMap> {
        Ok(PriceMap::from_prices(&self.prices().await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_type_price() {
        let json = r#"{"type_id": 34, "average_price": 5.5, "adjusted_price": 5.2}"#;
        let p: TypePrice = serde_json::from_str(json).unwrap();
        assert_eq!(p.type_id, 34);
        assert!((p.average_price - 5.5).abs() < 1e-9);
    }

    #[test]
    fn price_map_prefers_average_with_adjusted_fallback() {
        let prices = vec![
            TypePrice { type_id: 34, average_price: 5.5, adjusted_price: 5.0 },
            // No average → use adjusted.
            TypePrice { type_id: 35, average_price: 0.0, adjusted_price: 10.0 },
        ];
        let map = PriceMap::from_prices(&prices);
        assert_eq!(map.price(34), Some(5.5));
        assert_eq!(map.price(35), Some(10.0));
        assert_eq!(map.price(999), None);
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn value_multiplies_and_defaults_to_zero() {
        let map = PriceMap::from_prices(&[TypePrice {
            type_id: 34,
            average_price: 5.0,
            adjusted_price: 0.0,
        }]);
        assert_eq!(map.value(34, 1000), 5000.0);
        // Unknown type values at zero rather than erroring.
        assert_eq!(map.value(999, 1000), 0.0);
    }
}
