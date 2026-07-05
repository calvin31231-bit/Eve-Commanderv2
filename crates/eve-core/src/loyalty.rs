//! Corp loyalty / participation points.
//!
//! A lightweight internal points ledger: directors award points for fleet
//! participation, donations, hauling, etc. (positive entries) and deduct them
//! when a member redeems against a corp store (negative entries). The board is
//! just a per-member running balance. The aggregation is pure and unit-tested;
//! persistence lives in `db::loyalty`.

use serde::{Deserialize, Serialize};

/// One ledger entry (earned when `points > 0`, redeemed when `< 0`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoyaltyEntry {
    pub id: i64,
    pub member: String,
    pub points: i64,
    /// A free-text reason ("CTA fleet", "ratting tax", "redeemed: Vexor").
    pub reason: String,
    /// A category for filtering ("pvp", "donation", "hauling", "redeem", …).
    pub category: String,
    pub created_at: i64,
}

/// A member's rolled-up standing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberBalance {
    pub member: String,
    pub balance: i64,
    pub earned: i64,
    pub redeemed: i64,
    pub entry_count: usize,
}

/// Roll a ledger up into per-member balances, highest balance first. Member
/// names are matched case-insensitively but reported in their first-seen form.
/// Pure.
pub fn summarize(entries: &[LoyaltyEntry]) -> Vec<MemberBalance> {
    use std::collections::HashMap;
    // key = lowercased name → (display name, earned, redeemed, count)
    let mut acc: HashMap<String, (String, i64, i64, usize)> = HashMap::new();
    for e in entries {
        let key = e.member.trim().to_lowercase();
        if key.is_empty() {
            continue;
        }
        let slot = acc.entry(key).or_insert((e.member.trim().to_string(), 0, 0, 0));
        if e.points >= 0 {
            slot.1 += e.points;
        } else {
            slot.2 += -e.points;
        }
        slot.3 += 1;
    }
    let mut out: Vec<MemberBalance> = acc
        .into_values()
        .map(|(member, earned, redeemed, entry_count)| MemberBalance {
            member,
            balance: earned - redeemed,
            earned,
            redeemed,
            entry_count,
        })
        .collect();
    out.sort_by(|a, b| b.balance.cmp(&a.balance).then_with(|| a.member.cmp(&b.member)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(id: i64, member: &str, points: i64) -> LoyaltyEntry {
        LoyaltyEntry {
            id,
            member: member.into(),
            points,
            reason: String::new(),
            category: String::new(),
            created_at: 0,
        }
    }

    #[test]
    fn balances_net_earn_and_redeem_case_insensitively() {
        let entries = vec![
            e(1, "Pilot A", 100),
            e(2, "pilot a", 50),   // same member, different case → merged
            e(3, "Pilot A", -30),  // redemption
            e(4, "Pilot B", 200),
            e(5, "  ", 999),        // blank member → ignored
        ];
        let board = summarize(&entries);
        assert_eq!(board.len(), 2);
        // Pilot B (200) outranks Pilot A (120) → sorted by balance.
        assert_eq!(board[0].member, "Pilot B");
        let a = board.iter().find(|m| m.member == "Pilot A").unwrap();
        assert_eq!(a.earned, 150);
        assert_eq!(a.redeemed, 30);
        assert_eq!(a.balance, 120);
        assert_eq!(a.entry_count, 3);
    }

    #[test]
    fn empty_ledger_is_empty() {
        assert!(summarize(&[]).is_empty());
    }
}
