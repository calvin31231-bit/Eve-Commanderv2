//! Wallet journal and the cashflow analytics derived from it.
//!
//! The wallet journal (`GET /characters/{id}/wallet/journal/`, paginated) is the
//! per-character ledger: every ISK in or out with a `ref_type` category. ESI
//! keeps no long history, so this is also the raw material the portfolio
//! analytics later persists. The [`CashflowSummary`] aggregation — income vs
//! expenses and the biggest categories — is pure and unit-tested.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::auth::TokenManager;
use crate::error::{Error, Result};
use crate::esi::endpoints::endpoint;
use crate::esi::EsiClient;

/// One wallet journal entry (ESI `GET /characters/{id}/wallet/journal/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: i64,
    pub date: String,
    pub ref_type: String,
    /// Signed ISK amount: positive is income, negative is an expense.
    #[serde(default)]
    pub amount: f64,
    #[serde(default)]
    pub balance: Option<f64>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub first_party_id: Option<i64>,
    #[serde(default)]
    pub second_party_id: Option<i64>,
    #[serde(default)]
    pub reason: Option<String>,
}

/// One market transaction (ESI `GET /characters/{id}/wallet/transactions/`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub transaction_id: i64,
    pub date: String,
    pub type_id: i64,
    pub quantity: i64,
    pub unit_price: f64,
    pub is_buy: bool,
    #[serde(default)]
    pub is_personal: bool,
    #[serde(default)]
    pub client_id: i64,
    #[serde(default)]
    pub location_id: i64,
}

/// Net total for one `ref_type` category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RefTypeTotal {
    pub ref_type: String,
    pub total: f64,
    pub count: usize,
}

/// Income / expense rollup of a span of journal entries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CashflowSummary {
    pub income: f64,
    /// Total expenses as a negative number (sum of the negative amounts).
    pub expenses: f64,
    pub net: f64,
    pub entry_count: usize,
    /// Categories ordered by magnitude (largest absolute net first).
    pub by_ref_type: Vec<RefTypeTotal>,
}

/// Roll up journal entries into income/expense totals and per-category nets.
pub fn summarize_journal(entries: &[JournalEntry]) -> CashflowSummary {
    let mut income = 0.0;
    let mut expenses = 0.0;
    let mut by_type: HashMap<&str, (f64, usize)> = HashMap::new();

    for e in entries {
        if e.amount >= 0.0 {
            income += e.amount;
        } else {
            expenses += e.amount;
        }
        let slot = by_type.entry(e.ref_type.as_str()).or_insert((0.0, 0));
        slot.0 += e.amount;
        slot.1 += 1;
    }

    let mut by_ref_type: Vec<RefTypeTotal> = by_type
        .into_iter()
        .map(|(ref_type, (total, count))| RefTypeTotal {
            ref_type: ref_type.to_string(),
            total,
            count,
        })
        .collect();
    // Largest absolute mover first; ref_type breaks ties for stable ordering.
    by_ref_type.sort_by(|a, b| {
        b.total
            .abs()
            .partial_cmp(&a.total.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.ref_type.cmp(&b.ref_type))
    });

    CashflowSummary {
        income,
        expenses,
        net: income + expenses,
        entry_count: entries.len(),
        by_ref_type,
    }
}

/// The player's realized earning rate: net ISK across the distinct calendar days
/// the journal covers. This is the honest "status quo" benchmark the income
/// optimizer compares hypothetical activities against — a real number from the
/// wallet, not an estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RealizedIncome {
    /// Net ISK (income − expenses) across the journal window.
    pub net: f64,
    /// Distinct calendar days with wallet activity in the window.
    pub active_days: i64,
    /// net / active_days.
    pub isk_per_day: f64,
}

/// Compute the realized net ISK/day from a wallet journal. Counts distinct
/// `YYYY-MM-DD` dates as "active days" so the rate reflects days actually played
/// rather than a fixed window. Pure.
pub fn realized_rate(entries: &[JournalEntry]) -> RealizedIncome {
    let net: f64 = entries.iter().map(|e| e.amount).sum();
    let days: std::collections::HashSet<&str> = entries
        .iter()
        .map(|e| e.date.get(..10).unwrap_or(e.date.as_str()))
        .collect();
    let active_days = days.len().max(1) as i64;
    RealizedIncome { net, active_days, isk_per_day: net / active_days as f64 }
}

/// Typed, authenticated wallet-journal reads over the cache-first ESI client.
#[derive(Clone)]
pub struct WalletClient {
    esi: EsiClient,
    tokens: TokenManager,
}

impl WalletClient {
    pub fn new(esi: EsiClient, tokens: TokenManager) -> Self {
        Self { esi, tokens }
    }

    /// Fetch every page of the character's wallet journal.
    pub async fn journal(&self, character_id: i64) -> Result<Vec<JournalEntry>> {
        let ep = endpoint("wallet_journal")
            .ok_or_else(|| Error::other("unknown endpoint 'wallet_journal'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json_paged::<JournalEntry>(&ep.path_for(character_id), &token)
            .await
    }

    /// Fetch the journal and summarize it, keeping only the `top_n` categories.
    pub async fn cashflow(&self, character_id: i64, top_n: usize) -> Result<CashflowSummary> {
        let entries = self.journal(character_id).await?;
        let mut summary = summarize_journal(&entries);
        summary.by_ref_type.truncate(top_n);
        Ok(summary)
    }

    /// The character's most recent market transactions (latest page; ESI uses a
    /// `from_id` cursor for older ones, which we don't page yet).
    pub async fn transactions(&self, character_id: i64) -> Result<Vec<Transaction>> {
        let ep = endpoint("wallet_transactions")
            .ok_or_else(|| Error::other("unknown endpoint 'wallet_transactions'"))?;
        let token = self.tokens.access_token(character_id).await?;
        self.esi
            .get_auth_json::<Vec<Transaction>>(&ep.path_for(character_id), &token)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_journal_entry() {
        let json = r#"{
            "id": 89,
            "date": "2026-06-20T00:00:00Z",
            "ref_type": "market_transaction",
            "amount": -1000000.50,
            "balance": 4000000.0,
            "description": "Market: bought Tritanium"
        }"#;
        let e: JournalEntry = serde_json::from_str(json).unwrap();
        assert_eq!(e.ref_type, "market_transaction");
        assert!((e.amount + 1_000_000.50).abs() < 1e-6);
        assert_eq!(e.balance, Some(4_000_000.0));
    }

    fn entry(ref_type: &str, amount: f64) -> JournalEntry {
        JournalEntry {
            id: 0,
            date: "2026-06-20T00:00:00Z".into(),
            ref_type: ref_type.into(),
            amount,
            balance: None,
            description: String::new(),
            first_party_id: None,
            second_party_id: None,
            reason: None,
        }
    }

    #[test]
    fn summarizes_income_expenses_and_net() {
        let entries = vec![
            entry("bounty_prizes", 5_000_000.0),
            entry("bounty_prizes", 3_000_000.0),
            entry("market_transaction", -2_000_000.0),
            entry("brokers_fee", -100_000.0),
        ];
        let s = summarize_journal(&entries);
        assert_eq!(s.income, 8_000_000.0);
        assert_eq!(s.expenses, -2_100_000.0);
        assert_eq!(s.net, 5_900_000.0);
        assert_eq!(s.entry_count, 4);

        // Largest absolute category first → bounty_prizes (8M).
        assert_eq!(s.by_ref_type[0].ref_type, "bounty_prizes");
        assert_eq!(s.by_ref_type[0].total, 8_000_000.0);
        assert_eq!(s.by_ref_type[0].count, 2);
        // market_transaction (2M) before brokers_fee (0.1M).
        assert_eq!(s.by_ref_type[1].ref_type, "market_transaction");
        assert_eq!(s.by_ref_type[2].ref_type, "brokers_fee");
    }

    #[test]
    fn realized_rate_is_net_over_active_days() {
        let dated = |date: &str, amount: f64| JournalEntry {
            id: 0,
            date: date.into(),
            ref_type: "bounty_prizes".into(),
            amount,
            balance: None,
            description: String::new(),
            first_party_id: None,
            second_party_id: None,
            reason: None,
        };
        let entries = vec![
            dated("2026-06-20T01:00:00Z", 10_000_000.0),
            dated("2026-06-20T05:00:00Z", 5_000_000.0),
            dated("2026-06-21T02:00:00Z", -3_000_000.0),
        ];
        let r = realized_rate(&entries);
        // Two distinct calendar days, net = 12M → 6M/day.
        assert_eq!(r.active_days, 2);
        assert_eq!(r.net, 12_000_000.0);
        assert_eq!(r.isk_per_day, 6_000_000.0);
        // Empty journal never divides by zero.
        assert_eq!(realized_rate(&[]).active_days, 1);
    }

    #[test]
    fn deserializes_transaction() {
        let json = r#"{
            "transaction_id": 1,
            "date": "2026-06-20T10:00:00Z",
            "type_id": 34,
            "quantity": 1000,
            "unit_price": 5.5,
            "is_buy": true,
            "is_personal": true,
            "client_id": 1000,
            "location_id": 60003760
        }"#;
        let t: Transaction = serde_json::from_str(json).unwrap();
        assert_eq!(t.type_id, 34);
        assert!(t.is_buy);
        assert_eq!(t.quantity, 1000);
    }

    #[test]
    fn empty_journal_is_zeroed() {
        let s = summarize_journal(&[]);
        assert_eq!(s.income, 0.0);
        assert_eq!(s.expenses, 0.0);
        assert_eq!(s.net, 0.0);
        assert_eq!(s.entry_count, 0);
        assert!(s.by_ref_type.is_empty());
    }
}
