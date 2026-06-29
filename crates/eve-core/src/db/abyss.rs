//! Persistence for the Abyssal Deadspace run log. Stats aggregation is pure in
//! [`crate::abyss`]; this is the store.

use sqlx::Row;

use crate::abyss::AbyssRun;
use crate::error::Result;

use super::Database;

fn row_to_run(r: &sqlx::sqlite::SqliteRow) -> AbyssRun {
    AbyssRun {
        id: r.get::<i64, _>("id"),
        ran_at: r.get::<i64, _>("ran_at"),
        tier: r.get::<i64, _>("tier"),
        weather: r.get::<String, _>("weather"),
        ship: r.get::<String, _>("ship"),
        fit: r.get::<String, _>("fit"),
        duration_seconds: r.get::<i64, _>("duration_seconds"),
        loot_value: r.get::<f64, _>("loot_value"),
        survived: r.get::<i64, _>("survived") != 0,
        notes: r.get::<String, _>("notes"),
    }
}

impl Database {
    /// Log an abyssal run; returns its new id.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_abyss_run(
        &self,
        ran_at: i64,
        tier: i64,
        weather: &str,
        ship: &str,
        fit: &str,
        duration_seconds: i64,
        loot_value: f64,
        survived: bool,
        notes: &str,
    ) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO abyss_runs
               (ran_at, tier, weather, ship, fit, duration_seconds, loot_value, survived, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(ran_at)
        .bind(tier)
        .bind(weather)
        .bind(ship)
        .bind(fit)
        .bind(duration_seconds)
        .bind(loot_value)
        .bind(if survived { 1 } else { 0 })
        .bind(notes)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All logged abyssal runs, most recent first.
    pub async fn list_abyss_runs(&self) -> Result<Vec<AbyssRun>> {
        let rows = sqlx::query(
            "SELECT id, ran_at, tier, weather, ship, fit, duration_seconds, loot_value, survived, notes
             FROM abyss_runs ORDER BY ran_at DESC, id DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_run).collect())
    }

    /// Delete a logged abyssal run.
    pub async fn delete_abyss_run(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM abyss_runs WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn abyss_log_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        let id = db
            .add_abyss_run(1000, 4, "Dark", "Gila", "T4 Gila", 720, 65_000_000.0, true, "smooth")
            .await
            .unwrap();
        let runs = db.list_abyss_runs().await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].tier, 4);
        assert_eq!(runs[0].ship, "Gila");
        assert!(runs[0].survived);
        assert_eq!(runs[0].loot_value, 65_000_000.0);
        db.delete_abyss_run(id).await.unwrap();
        assert!(db.list_abyss_runs().await.unwrap().is_empty());
    }
}
