//! Persistence for the recruitment pipeline. Summary is pure in
//! [`crate::recruit`].

use sqlx::Row;

use crate::error::Result;
use crate::recruit::{Recruit, ACCEPTED, REJECTED};

use super::Database;

fn row_to_recruit(r: &sqlx::sqlite::SqliteRow) -> Recruit {
    Recruit {
        id: r.get::<i64, _>("id"),
        applied_at: r.get::<i64, _>("applied_at"),
        name: r.get::<String, _>("name"),
        source: r.get::<String, _>("source"),
        notes: r.get::<String, _>("notes"),
        status: r.get::<String, _>("status"),
        recruiter: r.get::<String, _>("recruiter"),
        reviewer_note: r.get::<String, _>("reviewer_note"),
        decided_at: r.get::<Option<i64>, _>("decided_at"),
    }
}

impl Database {
    /// Add an applicant (status `applied`). Returns its id.
    pub async fn add_recruit(
        &self,
        applied_at: i64,
        name: &str,
        source: &str,
        notes: &str,
        recruiter: &str,
    ) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO recruits (applied_at, name, source, notes, recruiter, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'applied')",
        )
        .bind(applied_at)
        .bind(name)
        .bind(source)
        .bind(notes)
        .bind(recruiter)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All applicants, most recent first.
    pub async fn list_recruits(&self) -> Result<Vec<Recruit>> {
        let rows = sqlx::query(
            "SELECT id, applied_at, name, source, notes, status, recruiter, reviewer_note, decided_at
             FROM recruits ORDER BY applied_at DESC, id DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows.iter().map(row_to_recruit).collect())
    }

    /// Move an applicant to a new stage, recording a note. Sets `decided_at` when
    /// the stage is terminal (accepted/rejected).
    pub async fn set_recruit_status(
        &self,
        id: i64,
        status: &str,
        reviewer_note: &str,
        now: i64,
    ) -> Result<()> {
        let decided_at = if status == ACCEPTED || status == REJECTED { Some(now) } else { None };
        sqlx::query(
            "UPDATE recruits SET status = ?2, reviewer_note = ?3, decided_at = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(status)
        .bind(reviewer_note)
        .bind(decided_at)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// Delete an applicant.
    pub async fn delete_recruit(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM recruits WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruit::{INTERVIEW, ACCEPTED};

    #[tokio::test]
    async fn recruit_pipeline_lifecycle() {
        let db = Database::open_in_memory().await.unwrap();
        let id = db.add_recruit(1000, "Newbro", "forum", "keen", "HR Bob").await.unwrap();
        let list = db.list_recruits().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, "applied");

        db.set_recruit_status(id, INTERVIEW, "scheduled", 2000).await.unwrap();
        let r = &db.list_recruits().await.unwrap()[0];
        assert_eq!(r.status, INTERVIEW);
        assert_eq!(r.decided_at, None); // not terminal

        db.set_recruit_status(id, ACCEPTED, "great fit", 3000).await.unwrap();
        let r = &db.list_recruits().await.unwrap()[0];
        assert_eq!(r.status, ACCEPTED);
        assert_eq!(r.decided_at, Some(3000));

        db.delete_recruit(id).await.unwrap();
        assert!(db.list_recruits().await.unwrap().is_empty());
    }
}
