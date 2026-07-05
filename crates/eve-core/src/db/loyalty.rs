//! Persistence for the corp loyalty-points ledger. Board aggregation is pure in
//! [`crate::loyalty`]; this is the store.

use sqlx::Row;

use crate::error::Result;
use crate::loyalty::LoyaltyEntry;

use super::Database;

fn row_to_entry(r: &sqlx::sqlite::SqliteRow) -> LoyaltyEntry {
    LoyaltyEntry {
        id: r.get::<i64, _>("id"),
        member: r.get::<String, _>("member"),
        points: r.get::<i64, _>("points"),
        reason: r.get::<String, _>("reason"),
        category: r.get::<String, _>("category"),
        created_at: r.get::<i64, _>("created_at"),
    }
}

impl Database {
    /// Add a ledger entry (positive = earned, negative = redeemed). Returns id.
    pub async fn add_loyalty_entry(
        &self,
        member: &str,
        points: i64,
        reason: &str,
        category: &str,
        now: i64,
    ) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO loyalty_ledger (member, points, reason, category, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(member)
        .bind(points)
        .bind(reason)
        .bind(category)
        .bind(now)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All ledger entries, newest first.
    pub async fn list_loyalty(&self) -> Result<Vec<LoyaltyEntry>> {
        let rows = sqlx::query(
            "SELECT id, member, points, reason, category, created_at
             FROM loyalty_ledger ORDER BY created_at DESC, id DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_entry).collect())
    }

    /// Delete a ledger entry.
    pub async fn delete_loyalty_entry(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM loyalty_ledger WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_list_and_delete() {
        let db = Database::open_in_memory().await.unwrap();
        let a = db.add_loyalty_entry("Pilot A", 100, "CTA", "pvp", 10).await.unwrap();
        db.add_loyalty_entry("Pilot A", -30, "redeemed Vexor", "redeem", 20).await.unwrap();

        let entries = db.list_loyalty().await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].points, -30); // newest first

        let board = crate::loyalty::summarize(&entries);
        assert_eq!(board[0].member, "Pilot A");
        assert_eq!(board[0].balance, 70);

        db.delete_loyalty_entry(a).await.unwrap();
        assert_eq!(db.list_loyalty().await.unwrap().len(), 1);
    }
}
