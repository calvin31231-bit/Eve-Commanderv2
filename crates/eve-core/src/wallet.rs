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
    fn empty_journal_is_zeroed() {
        let s = summarize_journal(&[]);
        assert_eq!(s.income, 0.0);
        assert_eq!(s.expenses, 0.0);
        assert_eq!(s.net, 0.0);
        assert_eq!(s.entry_count, 0);
        assert!(s.by_ref_type.is_empty());
    }
}
