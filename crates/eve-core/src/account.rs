//! Account-wide aggregation across all of a user's characters.
//!
//! The "one app for your whole account" view: each character's net worth
//! (wallet + asset value), SP, and balances rolled into combined totals plus a
//! per-character breakdown. Fetching each character's figures is done by the
//! shell (network); the [`aggregate`] roll-up here is pure and unit-tested.

use serde::{Deserialize, Serialize};

/// One character's headline figures.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterWorth {
    pub character_id: i64,
    pub name: String,
    pub wallet_balance: f64,
    pub asset_value: f64,
    pub total_sp: i64,
    /// wallet + assets.
    pub net_worth: f64,
}

impl CharacterWorth {
    pub fn new(
        character_id: i64,
        name: impl Into<String>,
        wallet_balance: f64,
        asset_value: f64,
        total_sp: i64,
    ) -> Self {
        Self {
            character_id,
            name: name.into(),
            wallet_balance,
            asset_value,
            total_sp,
            net_worth: wallet_balance + asset_value,
        }
    }
}

/// Combined account figures plus the per-character breakdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountOverview {
    /// Per-character, richest first.
    pub characters: Vec<CharacterWorth>,
    pub total_net_worth: f64,
    pub total_wallet: f64,
    pub total_asset_value: f64,
    pub total_sp: i64,
}

/// Roll up per-character figures into account totals (characters sorted by net
/// worth, descending).
pub fn aggregate(mut characters: Vec<CharacterWorth>) -> AccountOverview {
    characters.sort_by(|a, b| {
        b.net_worth
            .partial_cmp(&a.net_worth)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.character_id.cmp(&b.character_id))
    });

    AccountOverview {
        total_net_worth: characters.iter().map(|c| c.net_worth).sum(),
        total_wallet: characters.iter().map(|c| c.wallet_balance).sum(),
        total_asset_value: characters.iter().map(|c| c.asset_value).sum(),
        total_sp: characters.iter().map(|c| c.total_sp).sum(),
        characters,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_worth_is_wallet_plus_assets() {
        let c = CharacterWorth::new(1, "Pilot", 1_000_000.0, 4_000_000.0, 5_000_000);
        assert_eq!(c.net_worth, 5_000_000.0);
    }

    #[test]
    fn aggregate_totals_and_orders_by_net_worth() {
        let overview = aggregate(vec![
            CharacterWorth::new(1, "Alt", 500_000.0, 500_000.0, 2_000_000), // 1.0M
            CharacterWorth::new(2, "Main", 10_000_000.0, 40_000_000.0, 80_000_000), // 50M
        ]);
        // Totals.
        assert_eq!(overview.total_wallet, 10_500_000.0);
        assert_eq!(overview.total_asset_value, 40_500_000.0);
        assert_eq!(overview.total_net_worth, 51_000_000.0);
        assert_eq!(overview.total_sp, 82_000_000);
        // Richest first.
        assert_eq!(overview.characters[0].name, "Main");
        assert_eq!(overview.characters[1].name, "Alt");
    }

    #[test]
    fn empty_account_is_zeroed() {
        let overview = aggregate(vec![]);
        assert_eq!(overview.total_net_worth, 0.0);
        assert!(overview.characters.is_empty());
    }
}
