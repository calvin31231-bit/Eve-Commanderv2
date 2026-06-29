//! Persistence for SRP claims. Board aggregation is pure in [`crate::srp`].

use sqlx::Row;

use crate::error::Result;
use crate::srp::{SrpClaim, PAID};

use super::Database;

fn row_to_claim(r: &sqlx::sqlite::SqliteRow) -> SrpClaim {
    SrpClaim {
        id: r.get::<i64, _>("id"),
        submitted_at: r.get::<i64, _>("submitted_at"),
        pilot: r.get::<String, _>("pilot"),
        ship: r.get::<String, _>("ship"),
        loss_value: r.get::<f64, _>("loss_value"),
        location: r.get::<String, _>("location"),
        killmail_url: r.get::<String, _>("killmail_url"),
        notes: r.get::<String, _>("notes"),
        status: r.get::<String, _>("status"),
        payout: r.get::<f64, _>("payout"),
        reviewer_note: r.get::<String, _>("reviewer_note"),
        decided_at: r.get::<Option<i64>, _>("decided_at"),
    }
}

impl Database {
    /// Submit a new SRP claim (status `pending`). Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_srp_claim(
        &self,
        submitted_at: i64,
        pilot: &str,
        ship: &str,
        loss_value: f64,
        location: &str,
        killmail_url: &str,
        notes: &str,
    ) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO srp_claims (submitted_at, pilot, ship, loss_value, location, killmail_url, notes, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending')",
        )
        .bind(submitted_at)
        .bind(pilot)
        .bind(ship)
        .bind(loss_value)
        .bind(location)
        .bind(killmail_url)
        .bind(notes)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All claims, most recent first.
    pub async fn list_srp_claims(&self) -> Result<Vec<SrpClaim>> {
        let rows = sqlx::query(
            "SELECT id, submitted_at, pilot, ship, loss_value, location, killmail_url, notes,
                    status, payout, reviewer_note, decided_at
             FROM srp_claims ORDER BY submitted_at DESC, id DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_claim).collect())
    }

    /// Approve or reject a claim, recording the payout, reviewer note, and time.
    pub async fn decide_srp_claim(
        &self,
        id: i64,
        status: &str,
        payout: f64,
        reviewer_note: &str,
        decided_at: i64,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE srp_claims SET status = ?2, payout = ?3, reviewer_note = ?4, decided_at = ?5
             WHERE id = ?1",
        )
        .bind(id)
        .bind(status)
        .bind(payout)
        .bind(reviewer_note)
        .bind(decided_at)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// Mark an approved claim as paid.
    pub async fn mark_srp_paid(&self, id: i64) -> Result<()> {
        sqlx::query("UPDATE srp_claims SET status = ?2 WHERE id = ?1")
            .bind(id)
            .bind(PAID)
            .execute(&self.app)
            .await?;
        Ok(())
    }

    /// Delete a claim.
    pub async fn delete_srp_claim(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM srp_claims WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::srp::{APPROVED, PAID};

    #[tokio::test]
    async fn srp_claim_lifecycle() {
        let db = Database::open_in_memory().await.unwrap();
        let id = db
            .add_srp_claim(1000, "Pilot", "Drake", 300_000_000.0, "J123456", "https://zkb/123", "ratting loss")
            .await
            .unwrap();
        let claims = db.list_srp_claims().await.unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].status, "pending");

        db.decide_srp_claim(id, APPROVED, 250_000_000.0, "doctrine fit", 2000).await.unwrap();
        let c = &db.list_srp_claims().await.unwrap()[0];
        assert_eq!(c.status, APPROVED);
        assert_eq!(c.payout, 250_000_000.0);
        assert_eq!(c.decided_at, Some(2000));

        db.mark_srp_paid(id).await.unwrap();
        assert_eq!(db.list_srp_claims().await.unwrap()[0].status, PAID);

        db.delete_srp_claim(id).await.unwrap();
        assert!(db.list_srp_claims().await.unwrap().is_empty());
    }
}
