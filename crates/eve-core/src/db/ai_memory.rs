//! Persistence for durable AI memory notes. The salience/eviction *policy* is
//! pure in [`crate::ai_memory`]; this module is the store.

use sqlx::Row;

use crate::ai_memory::{evict_candidates, MemoryRef};
use crate::error::Result;

use super::Database;

/// A stored memory note.
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryNote {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub salience: f64,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

fn row_to_note(r: &sqlx::sqlite::SqliteRow) -> MemoryNote {
    MemoryNote {
        id: r.get::<i64, _>("id"),
        kind: r.get::<String, _>("kind"),
        title: r.get::<String, _>("title"),
        body: r.get::<String, _>("body"),
        salience: r.get::<f64, _>("salience"),
        pinned: r.get::<i64, _>("pinned") != 0,
        created_at: r.get::<i64, _>("created_at"),
        updated_at: r.get::<i64, _>("updated_at"),
    }
}

impl Database {
    /// Insert a memory note. Returns its new id.
    pub async fn add_memory(
        &self,
        kind: &str,
        title: &str,
        body: &str,
        salience: f64,
        now: i64,
    ) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO ai_memory (kind, title, body, salience, pinned, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
        )
        .bind(kind)
        .bind(title)
        .bind(body)
        .bind(salience)
        .bind(now)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All memory notes, most salient first then most recent.
    pub async fn list_memory(&self) -> Result<Vec<MemoryNote>> {
        let rows = sqlx::query(
            "SELECT id, kind, title, body, salience, pinned, created_at, updated_at
             FROM ai_memory ORDER BY salience DESC, updated_at DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_note).collect())
    }

    /// Delete a memory note (the "forget" control).
    pub async fn delete_memory(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM ai_memory WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }

    /// Pin/unpin a note so it never (or may again) evict.
    pub async fn set_memory_pinned(&self, id: i64, pinned: bool) -> Result<()> {
        sqlx::query("UPDATE ai_memory SET pinned = ?2 WHERE id = ?1")
            .bind(id)
            .bind(if pinned { 1 } else { 0 })
            .execute(&self.app)
            .await?;
        Ok(())
    }

    /// Enforce the storage cap: evict the lowest importance×recency notes until
    /// at most `cap` remain, sparing pinned notes. Returns how many were removed.
    pub async fn enforce_memory_cap(&self, cap: usize, now: i64) -> Result<usize> {
        let rows = sqlx::query("SELECT id, salience, pinned, updated_at FROM ai_memory")
            .fetch_all(&self.app)
            .await?;
        let refs: Vec<MemoryRef> = rows
            .iter()
            .map(|r| MemoryRef {
                id: r.get::<i64, _>("id"),
                salience: r.get::<f64, _>("salience"),
                updated_at: r.get::<i64, _>("updated_at"),
                pinned: r.get::<i64, _>("pinned") != 0,
            })
            .collect();
        let victims = evict_candidates(&refs, cap, now);
        for id in &victims {
            self.delete_memory(*id).await?;
        }
        Ok(victims.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn add_list_pin_forget_and_cap() {
        let db = Database::open_in_memory().await.unwrap();
        let a = db.add_memory("goal", "Carrier", "Train toward a Nyx", 0.8, 100).await.unwrap();
        let b = db.add_memory("chitchat", "Hi", "hello", 0.1, 50).await.unwrap();
        let _c = db.add_memory("preference", "Lowsec", "Avoids lowsec", 0.65, 90).await.unwrap();

        let all = db.list_memory().await.unwrap();
        assert_eq!(all.len(), 3);
        // Most salient first.
        assert_eq!(all[0].id, a);

        // Pin the goal so the cap can't evict it.
        db.set_memory_pinned(a, true).await.unwrap();

        // Cap to 1 → evicts the two unpinned, keeps the pinned goal.
        let removed = db.enforce_memory_cap(1, 200).await.unwrap();
        assert_eq!(removed, 2);
        let after = db.list_memory().await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, a);

        // Forget it.
        db.delete_memory(a).await.unwrap();
        assert!(db.list_memory().await.unwrap().is_empty());
        let _ = b;
    }
}
