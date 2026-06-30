//! Persistence for the opt-in telemetry buffer. The event *shape* (and its
//! privacy guarantees) is pure in [`crate::telemetry`]; this is just the store.

use sqlx::Row;

use crate::error::Result;

use super::Database;

/// A buffered telemetry event row.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryRow {
    pub id: i64,
    pub name: String,
    pub counts_json: String,
    pub created_at: i64,
}

impl Database {
    /// Append an event to the local buffer. Callers gate this on consent.
    pub async fn record_telemetry(&self, name: &str, counts_json: &str, now: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO telemetry_events (name, counts_json, created_at) VALUES (?1, ?2, ?3)",
        )
        .bind(name)
        .bind(counts_json)
        .bind(now)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// All buffered events, oldest first.
    pub async fn list_telemetry(&self) -> Result<Vec<TelemetryRow>> {
        let rows = sqlx::query(
            "SELECT id, name, counts_json, created_at FROM telemetry_events ORDER BY created_at ASC, id ASC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows
            .iter()
            .map(|r| TelemetryRow {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                counts_json: r.get::<String, _>("counts_json"),
                created_at: r.get::<i64, _>("created_at"),
            })
            .collect())
    }

    /// Empty the buffer (the "clear" control, and what a flush calls on success).
    pub async fn clear_telemetry(&self) -> Result<()> {
        sqlx::query("DELETE FROM telemetry_events").execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn record_list_and_clear() {
        let db = Database::open_in_memory().await.unwrap();
        db.record_telemetry("feature_open", "{\"hub\":3}", 100).await.unwrap();
        db.record_telemetry("ai_chat", "{}", 110).await.unwrap();

        let rows = db.list_telemetry().await.unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "feature_open");
        assert_eq!(rows[0].counts_json, "{\"hub\":3}");

        db.clear_telemetry().await.unwrap();
        assert!(db.list_telemetry().await.unwrap().is_empty());
    }
}
