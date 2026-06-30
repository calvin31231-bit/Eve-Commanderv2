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

    /// Delete a memory note (the "forget" control) and its vector, if any.
    pub async fn delete_memory(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM ai_memory WHERE id = ?1").bind(id).execute(&self.app).await?;
        sqlx::query("DELETE FROM ai_memory_vectors WHERE note_id = ?1")
            .bind(id)
            .execute(&self.app)
            .await?;
        Ok(())
    }

    /// Store (or replace) the embedding vector for a note. `vec` is held as
    /// little-endian f32 bytes. Best-effort: callers ignore failures so a missing
    /// embeddings model never blocks a memory write.
    pub async fn set_memory_embedding(&self, note_id: i64, vec: &[f32]) -> Result<()> {
        let mut bytes = Vec::with_capacity(vec.len() * 4);
        for f in vec {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        sqlx::query(
            "INSERT OR REPLACE INTO ai_memory_vectors (note_id, dim, vec) VALUES (?1, ?2, ?3)",
        )
        .bind(note_id)
        .bind(vec.len() as i64)
        .bind(bytes)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// All stored note embeddings as `(note_id, vector)` for vector recall.
    pub async fn memory_embeddings(&self) -> Result<Vec<(i64, Vec<f32>)>> {
        let rows = sqlx::query("SELECT note_id, vec FROM ai_memory_vectors")
            .fetch_all(&self.app)
            .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let id = r.get::<i64, _>("note_id");
                let bytes = r.get::<Vec<u8>, _>("vec");
                let vec: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                (id, vec)
            })
            .collect())
    }

    /// Ids of notes that have no embedding yet (for backfill/reindex).
    pub async fn memory_ids_without_embedding(&self) -> Result<Vec<i64>> {
        let rows = sqlx::query(
            "SELECT id FROM ai_memory
             WHERE id NOT IN (SELECT note_id FROM ai_memory_vectors)",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(|r| r.get::<i64, _>("id")).collect())
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

    #[tokio::test]
    async fn embeddings_roundtrip_and_backfill() {
        let db = Database::open_in_memory().await.unwrap();
        let a = db.add_memory("goal", "Carrier", "Train toward a Nyx", 0.8, 100).await.unwrap();
        let b = db.add_memory("preference", "Lowsec", "Avoids lowsec", 0.65, 90).await.unwrap();

        // Both notes start without a vector.
        let mut missing = db.memory_ids_without_embedding().await.unwrap();
        missing.sort();
        assert_eq!(missing, vec![a, b]);

        // Store a vector for one; it round-trips exactly and leaves only b missing.
        db.set_memory_embedding(a, &[0.5, -0.25, 1.0]).await.unwrap();
        let embs = db.memory_embeddings().await.unwrap();
        assert_eq!(embs.len(), 1);
        assert_eq!(embs[0], (a, vec![0.5, -0.25, 1.0]));
        assert_eq!(db.memory_ids_without_embedding().await.unwrap(), vec![b]);

        // Deleting the note removes its vector too.
        db.delete_memory(a).await.unwrap();
        assert!(db.memory_embeddings().await.unwrap().is_empty());
    }
}
