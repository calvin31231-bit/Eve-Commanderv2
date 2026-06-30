//! Persistence for the cosmic-signature / wormhole chain log.

use sqlx::Row;

use crate::error::Result;
use crate::signatures::parse_signatures;

use super::Database;

/// A stored signature with its (optional) wormhole connection annotations.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredSig {
    pub id: i64,
    pub added_at: i64,
    pub system: String,
    pub sig_id: String,
    pub category: String,
    pub name: String,
    pub wh_type: String,
    pub destination: String,
    pub mass_state: String,
    pub eol: bool,
    pub notes: String,
}

fn row_to_sig(r: &sqlx::sqlite::SqliteRow) -> StoredSig {
    StoredSig {
        id: r.get::<i64, _>("id"),
        added_at: r.get::<i64, _>("added_at"),
        system: r.get::<String, _>("system"),
        sig_id: r.get::<String, _>("sig_id"),
        category: r.get::<String, _>("category"),
        name: r.get::<String, _>("name"),
        wh_type: r.get::<String, _>("wh_type"),
        destination: r.get::<String, _>("destination"),
        mass_state: r.get::<String, _>("mass_state"),
        eol: r.get::<i64, _>("eol") != 0,
        notes: r.get::<String, _>("notes"),
    }
}

impl Database {
    /// Merge a pasted probe-scanner dump into `system`'s signature list: new ids
    /// are inserted; existing ids have their category/name refreshed (annotations
    /// preserved). Returns the number of new signatures added.
    pub async fn upsert_signatures(&self, system: &str, paste: &str, now: i64) -> Result<usize> {
        let scanned = parse_signatures(paste);
        let mut added = 0;
        for s in scanned {
            let existing: Option<i64> = sqlx::query(
                "SELECT id FROM signatures WHERE system = ?1 AND sig_id = ?2",
            )
            .bind(system)
            .bind(&s.sig_id)
            .fetch_optional(&self.app)
            .await?
            .map(|r| r.get::<i64, _>("id"));

            match existing {
                Some(id) => {
                    // Refresh category/name only when newly resolved.
                    sqlx::query(
                        "UPDATE signatures
                         SET category = CASE WHEN ?2 <> '' THEN ?2 ELSE category END,
                             name     = CASE WHEN ?3 <> '' THEN ?3 ELSE name END
                         WHERE id = ?1",
                    )
                    .bind(id)
                    .bind(&s.category)
                    .bind(&s.name)
                    .execute(&self.app)
                    .await?;
                }
                None => {
                    sqlx::query(
                        "INSERT INTO signatures (added_at, system, sig_id, category, name)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                    )
                    .bind(now)
                    .bind(system)
                    .bind(&s.sig_id)
                    .bind(&s.category)
                    .bind(&s.name)
                    .execute(&self.app)
                    .await?;
                    added += 1;
                }
            }
        }
        Ok(added)
    }

    /// All signatures, newest first.
    pub async fn list_signatures(&self) -> Result<Vec<StoredSig>> {
        let rows = sqlx::query(
            "SELECT id, added_at, system, sig_id, category, name, wh_type, destination, mass_state, eol, notes
             FROM signatures ORDER BY system, added_at DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_sig).collect())
    }

    /// Annotate a signature's wormhole connection details.
    pub async fn annotate_signature(
        &self,
        id: i64,
        wh_type: &str,
        destination: &str,
        mass_state: &str,
        eol: bool,
        notes: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE signatures
             SET wh_type = ?2, destination = ?3, mass_state = ?4, eol = ?5, notes = ?6
             WHERE id = ?1",
        )
        .bind(id)
        .bind(wh_type)
        .bind(destination)
        .bind(mass_state)
        .bind(if eol { 1 } else { 0 })
        .bind(notes)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// Delete a signature.
    pub async fn delete_signature(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM signatures WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }

    /// Clear every signature for a system (e.g. when the chain is stale).
    pub async fn clear_signatures(&self, system: &str) -> Result<()> {
        sqlx::query("DELETE FROM signatures WHERE system = ?1").bind(system).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upsert_annotate_and_clear() {
        let db = Database::open_in_memory().await.unwrap();
        let paste = "ABC-123\tCosmic Signature\tWormhole\tUnstable Wormhole\t100%\t1 AU\n\
                     DEF-456\tCosmic Signature\tRelic Site\tRuins\t50%\t2 AU";
        let added = db.upsert_signatures("J100001", paste, 1000).await.unwrap();
        assert_eq!(added, 2);

        // Re-scan adds nothing new but refreshes.
        let again = db.upsert_signatures("J100001", paste, 1001).await.unwrap();
        assert_eq!(again, 0);

        let sigs = db.list_signatures().await.unwrap();
        let wh = sigs.iter().find(|s| s.sig_id == "ABC-123").unwrap();
        db.annotate_signature(wh.id, "K162", "Jita", "critical", true, "rolling").await.unwrap();
        let after = db.list_signatures().await.unwrap();
        let wh2 = after.iter().find(|s| s.sig_id == "ABC-123").unwrap();
        assert_eq!(wh2.destination, "Jita");
        assert!(wh2.eol);
        assert_eq!(wh2.mass_state, "critical");

        db.clear_signatures("J100001").await.unwrap();
        assert!(db.list_signatures().await.unwrap().is_empty());
    }
}
