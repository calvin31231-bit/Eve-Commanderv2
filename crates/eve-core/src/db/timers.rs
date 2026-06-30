//! Persistence for the structure reinforcement timerboard. Countdown is computed
//! at read time by the command layer (injecting `now`).

use sqlx::Row;

use crate::error::Result;

use super::Database;

/// A stored reinforcement timer.
#[derive(Debug, Clone, PartialEq)]
pub struct Timer {
    pub id: i64,
    pub title: String,
    pub system: String,
    pub structure: String,
    pub timer_type: String,
    pub side: String,
    pub exits_at: i64,
    pub notes: String,
}

fn row_to_timer(r: &sqlx::sqlite::SqliteRow) -> Timer {
    Timer {
        id: r.get::<i64, _>("id"),
        title: r.get::<String, _>("title"),
        system: r.get::<String, _>("system"),
        structure: r.get::<String, _>("structure"),
        timer_type: r.get::<String, _>("timer_type"),
        side: r.get::<String, _>("side"),
        exits_at: r.get::<i64, _>("exits_at"),
        notes: r.get::<String, _>("notes"),
    }
}

impl Database {
    /// Add a timer. Returns its id.
    #[allow(clippy::too_many_arguments)]
    pub async fn add_timer(
        &self,
        created_at: i64,
        title: &str,
        system: &str,
        structure: &str,
        timer_type: &str,
        side: &str,
        exits_at: i64,
        notes: &str,
    ) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO timers (created_at, title, system, structure, timer_type, side, exits_at, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(created_at)
        .bind(title)
        .bind(system)
        .bind(structure)
        .bind(timer_type)
        .bind(side)
        .bind(exits_at)
        .bind(notes)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All timers, soonest exit first.
    pub async fn list_timers(&self) -> Result<Vec<Timer>> {
        let rows = sqlx::query(
            "SELECT id, title, system, structure, timer_type, side, exits_at, notes
             FROM timers ORDER BY exits_at ASC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_timer).collect())
    }

    /// Delete a timer.
    pub async fn delete_timer(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM timers WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timer_crud_sorted_by_exit() {
        let db = Database::open_in_memory().await.unwrap();
        db.add_timer(0, "Astrahus", "J100001", "Astrahus", "armor", "hostile", 5000, "").await.unwrap();
        let early = db.add_timer(0, "Fort", "Jita", "Fortizar", "hull", "friendly", 1000, "ours").await.unwrap();
        let list = db.list_timers().await.unwrap();
        assert_eq!(list.len(), 2);
        // Soonest first.
        assert_eq!(list[0].id, early);
        assert_eq!(list[0].exits_at, 1000);
        db.delete_timer(early).await.unwrap();
        assert_eq!(db.list_timers().await.unwrap().len(), 1);
    }
}
