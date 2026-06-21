//! Character and group persistence.

use sqlx::Row;

use crate::error::Result;
use crate::model::{Character, CharacterGroup};

use super::Database;

/// Serialize scopes to the space-delimited form stored in the DB.
fn scopes_to_str(scopes: &[String]) -> String {
    scopes.join(" ")
}

/// Parse the space-delimited scope string back into a vec.
fn scopes_from_str(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

impl Database {
    /// Insert or update a character (upsert by id). Preserves nothing it isn't
    /// told about — the caller passes the full record.
    pub async fn upsert_character(&self, c: &Character) -> Result<()> {
        let now = now_epoch();
        sqlx::query(
            r#"
            INSERT INTO characters (id, name, corporation_id, alliance_id, scopes, active, added_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(id) DO UPDATE SET
                name           = excluded.name,
                corporation_id = excluded.corporation_id,
                alliance_id    = excluded.alliance_id,
                scopes         = excluded.scopes,
                active         = excluded.active
            "#,
        )
        .bind(c.id)
        .bind(&c.name)
        .bind(c.corporation_id)
        .bind(c.alliance_id)
        .bind(scopes_to_str(&c.scopes))
        .bind(c.active as i64)
        .bind(now)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// Return all characters, ordered by name.
    pub async fn list_characters(&self) -> Result<Vec<Character>> {
        let rows = sqlx::query(
            "SELECT id, name, corporation_id, alliance_id, scopes, active FROM characters ORDER BY name",
        )
        .fetch_all(&self.app)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Character {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                corporation_id: r.get::<Option<i64>, _>("corporation_id"),
                alliance_id: r.get::<Option<i64>, _>("alliance_id"),
                scopes: scopes_from_str(&r.get::<String, _>("scopes")),
                active: r.get::<i64, _>("active") != 0,
            })
            .collect())
    }

    /// Make `character_id` the single active/foreground character.
    pub async fn set_active_character(&self, character_id: i64) -> Result<()> {
        let mut tx = self.app.begin().await?;
        sqlx::query("UPDATE characters SET active = 0").execute(&mut *tx).await?;
        sqlx::query("UPDATE characters SET active = 1 WHERE id = ?1")
            .bind(character_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Remove a character (and its group memberships, via FK cascade).
    pub async fn delete_character(&self, character_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM characters WHERE id = ?1")
            .bind(character_id)
            .execute(&self.app)
            .await?;
        Ok(())
    }

    /// Create a group ("hat") and return it.
    pub async fn create_group(&self, name: &str) -> Result<CharacterGroup> {
        let row = sqlx::query("INSERT INTO character_groups (name) VALUES (?1) RETURNING id")
            .bind(name)
            .fetch_one(&self.app)
            .await?;
        Ok(CharacterGroup {
            id: row.get::<i64, _>("id"),
            name: name.to_string(),
            members: Vec::new(),
        })
    }

    /// Add a character to a group.
    pub async fn add_group_member(&self, group_id: i64, character_id: i64) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO character_group_members (group_id, character_id) VALUES (?1, ?2)",
        )
        .bind(group_id)
        .bind(character_id)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// Remove a character from a group.
    pub async fn remove_group_member(&self, group_id: i64, character_id: i64) -> Result<()> {
        sqlx::query(
            "DELETE FROM character_group_members WHERE group_id = ?1 AND character_id = ?2",
        )
        .bind(group_id)
        .bind(character_id)
        .execute(&self.app)
        .await?;
        Ok(())
    }

    /// Delete a group (members cascade away via the FK).
    pub async fn delete_group(&self, group_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM character_groups WHERE id = ?1")
            .bind(group_id)
            .execute(&self.app)
            .await?;
        Ok(())
    }

    /// List groups with their member character ids.
    pub async fn list_groups(&self) -> Result<Vec<CharacterGroup>> {
        let groups = sqlx::query("SELECT id, name FROM character_groups ORDER BY name")
            .fetch_all(&self.app)
            .await?;
        let mut out = Vec::with_capacity(groups.len());
        for g in groups {
            let id = g.get::<i64, _>("id");
            let members = sqlx::query(
                "SELECT character_id FROM character_group_members WHERE group_id = ?1 ORDER BY character_id",
            )
            .bind(id)
            .fetch_all(&self.app)
            .await?
            .into_iter()
            .map(|r| r.get::<i64, _>("character_id"))
            .collect();
            out.push(CharacterGroup {
                id,
                name: g.get::<String, _>("name"),
                members,
            });
        }
        Ok(out)
    }
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn character_crud_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        assert!(db.list_characters().await.unwrap().is_empty());

        let mut c = Character::new(90000001, "Test Pilot");
        c.scopes = vec!["publicData".into(), "esi-skills.read_skills.v1".into()];
        c.corporation_id = Some(98000001);
        db.upsert_character(&c).await.unwrap();

        let list = db.list_characters().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Test Pilot");
        assert_eq!(list[0].scopes.len(), 2);
        assert_eq!(list[0].corporation_id, Some(98000001));

        // Upsert updates rather than duplicating.
        let mut c2 = c.clone();
        c2.name = "Renamed Pilot".into();
        db.upsert_character(&c2).await.unwrap();
        let list = db.list_characters().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Renamed Pilot");
    }

    #[tokio::test]
    async fn active_character_is_exclusive() {
        let db = Database::open_in_memory().await.unwrap();
        db.upsert_character(&Character::new(1, "A")).await.unwrap();
        db.upsert_character(&Character::new(2, "B")).await.unwrap();
        db.set_active_character(1).await.unwrap();
        db.set_active_character(2).await.unwrap();
        let active: Vec<_> = db
            .list_characters()
            .await
            .unwrap()
            .into_iter()
            .filter(|c| c.active)
            .collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, 2);
    }

    #[tokio::test]
    async fn groups_with_members() {
        let db = Database::open_in_memory().await.unwrap();
        db.upsert_character(&Character::new(1, "A")).await.unwrap();
        db.upsert_character(&Character::new(2, "B")).await.unwrap();
        let g = db.create_group("Industry Stable").await.unwrap();
        db.add_group_member(g.id, 1).await.unwrap();
        db.add_group_member(g.id, 2).await.unwrap();
        db.add_group_member(g.id, 2).await.unwrap(); // idempotent

        let groups = db.list_groups().await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Industry Stable");
        assert_eq!(groups[0].members, vec![1, 2]);

        // FK cascade: deleting a character removes its membership.
        db.delete_character(1).await.unwrap();
        let groups = db.list_groups().await.unwrap();
        assert_eq!(groups[0].members, vec![2]);

        // Remove a member, then delete the group.
        db.remove_group_member(g.id, 2).await.unwrap();
        assert!(db.list_groups().await.unwrap()[0].members.is_empty());
        db.delete_group(g.id).await.unwrap();
        assert!(db.list_groups().await.unwrap().is_empty());
    }
}
