//! Historical snapshots (net worth, wallet, SP) — the durable time-series that
//! powers portfolio analytics. ESI has no history, so we persist our own.

use sqlx::Row;

use crate::error::Result;

use super::Database;

/// One persisted snapshot point.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub taken_at: i64,
    pub value: f64,
}

impl Database {
    /// Append a snapshot point for a character + metric `kind`
    /// (e.g. "networth", "wallet", "sp").
    pub async fn record_snapshot(
        &self,
        character_id: i64,
        kind: &str,
        value: f64,
        taken_at: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO snapshots (character_id, kind, taken_at, value) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(character_id)
        .bind(kind)
        .bind(taken_at)
        .bind(value)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// The most recent snapshot for a character + kind, if any.
    pub async fn latest_snapshot(&self, character_id: i64, kind: &str) -> Result<Option<Snapshot>> {
        let row = sqlx::query(
            "SELECT taken_at, value FROM snapshots
             WHERE character_id = ?1 AND kind = ?2 ORDER BY taken_at DESC LIMIT 1",
        )
        .bind(character_id)
        .bind(kind)
        .fetch_optional(&self.app)
        .await?;
        Ok(row.map(|r| Snapshot {
            taken_at: r.get::<i64, _>("taken_at"),
            value: r.get::<f64, _>("value"),
        }))
    }

    /// All snapshot points for a character + kind at or after `since`, oldest
    /// first (for a left→right chart).
    pub async fn snapshots(
        &self,
        character_id: i64,
        kind: &str,
        since: i64,
    ) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query(
            "SELECT taken_at, value FROM snapshots
             WHERE character_id = ?1 AND kind = ?2 AND taken_at >= ?3 ORDER BY taken_at ASC",
        )
        .bind(character_id)
        .bind(kind)
        .bind(since)
        .fetch_all(&self.app)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Snapshot {
                taken_at: r.get::<i64, _>("taken_at"),
                value: r.get::<f64, _>("value"),
            })
            .collect())
    }

    /// Snapshot points summed across **all** characters per timestamp at or after
    /// `since` — the account-wide series. Oldest first.
    pub async fn snapshots_total(&self, kind: &str, since: i64) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query(
            "SELECT taken_at, SUM(value) AS total FROM snapshots
             WHERE kind = ?1 AND taken_at >= ?2 GROUP BY taken_at ORDER BY taken_at ASC",
        )
        .bind(kind)
        .bind(since)
        .fetch_all(&self.app)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Snapshot {
                taken_at: r.get::<i64, _>("taken_at"),
                value: r.get::<f64, _>("total"),
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn records_and_queries_series() {
        let db = Database::open_in_memory().await.unwrap();
        db.record_snapshot(1, "networth", 100.0, 1000).await.unwrap();
        db.record_snapshot(1, "networth", 150.0, 2000).await.unwrap();
        db.record_snapshot(1, "wallet", 50.0, 2000).await.unwrap();
        db.record_snapshot(2, "networth", 300.0, 2000).await.unwrap();

        let series = db.snapshots(1, "networth", 0).await.unwrap();
        assert_eq!(series.len(), 2);
        assert_eq!(series[0].value, 100.0); // oldest first
        assert_eq!(series[1].value, 150.0);

        // `since` filters.
        assert_eq!(db.snapshots(1, "networth", 1500).await.unwrap().len(), 1);

        let latest = db.latest_snapshot(1, "networth").await.unwrap().unwrap();
        assert_eq!(latest.value, 150.0);

        // Account-wide total at t=2000: char1 150 + char2 300 = 450.
        let total = db.snapshots_total("networth", 0).await.unwrap();
        assert_eq!(total.last().unwrap().value, 450.0);
    }
}
