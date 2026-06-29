//! Reusable libraries: named skill plans and fits stored independent of any
//! character, so they can be loaded onto a freshly-created alt. A skill plan's
//! `body` is the importable text form (see `skillplan_import`); a fit's `eft` is
//! raw EFT.

use sqlx::Row;

use crate::error::Result;

use super::Database;

/// A saved skill plan in the library.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedPlan {
    pub id: i64,
    pub name: String,
    pub body: String,
    pub updated_at: i64,
}

/// A saved fit in the library.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedFit {
    pub id: i64,
    pub name: String,
    pub ship: String,
    pub eft: String,
    pub updated_at: i64,
}

/// A saved implant loadout: a named clone's implant set, stored as the implant
/// type ids (comma-separated on disk).
#[derive(Debug, Clone, PartialEq)]
pub struct SavedLoadout {
    pub id: i64,
    pub name: String,
    pub implant_ids: Vec<i64>,
    pub updated_at: i64,
}

impl Database {
    // ---- skill plans -------------------------------------------------------

    /// Save a skill plan; returns its new id.
    pub async fn save_skill_plan(&self, name: &str, body: &str, now: i64) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO skill_plans (name, body, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        )
        .bind(name)
        .bind(body)
        .bind(now)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All saved skill plans, most recently updated first.
    pub async fn list_skill_plans(&self) -> Result<Vec<SavedPlan>> {
        let rows = sqlx::query(
            "SELECT id, name, body, updated_at FROM skill_plans ORDER BY updated_at DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SavedPlan {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                body: r.get::<String, _>("body"),
                updated_at: r.get::<i64, _>("updated_at"),
            })
            .collect())
    }

    /// Delete a saved skill plan.
    pub async fn delete_skill_plan(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM skill_plans WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }

    // ---- fits --------------------------------------------------------------

    /// Save a fit; returns its new id.
    pub async fn save_fit(&self, name: &str, ship: &str, eft: &str, now: i64) -> Result<i64> {
        let id = sqlx::query(
            "INSERT INTO saved_fits (name, ship, eft, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        )
        .bind(name)
        .bind(ship)
        .bind(eft)
        .bind(now)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All saved fits, most recently updated first.
    pub async fn list_fits(&self) -> Result<Vec<SavedFit>> {
        let rows = sqlx::query(
            "SELECT id, name, ship, eft, updated_at FROM saved_fits ORDER BY updated_at DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SavedFit {
                id: r.get::<i64, _>("id"),
                name: r.get::<String, _>("name"),
                ship: r.get::<String, _>("ship"),
                eft: r.get::<String, _>("eft"),
                updated_at: r.get::<i64, _>("updated_at"),
            })
            .collect())
    }

    /// Delete a saved fit.
    pub async fn delete_fit(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM saved_fits WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }

    // ---- implant loadouts --------------------------------------------------

    /// Save an implant loadout (the equipped implant type ids); returns its id.
    pub async fn save_loadout(&self, name: &str, implant_ids: &[i64], now: i64) -> Result<i64> {
        let csv = implant_ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",");
        let id = sqlx::query(
            "INSERT INTO implant_loadouts (name, implant_ids, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)",
        )
        .bind(name)
        .bind(csv)
        .bind(now)
        .execute(&self.app)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// All saved implant loadouts, most recently updated first.
    pub async fn list_loadouts(&self) -> Result<Vec<SavedLoadout>> {
        let rows = sqlx::query(
            "SELECT id, name, implant_ids, updated_at FROM implant_loadouts ORDER BY updated_at DESC",
        )
        .fetch_all(&self.app)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let csv = r.get::<String, _>("implant_ids");
                let implant_ids = csv
                    .split(',')
                    .filter_map(|s| s.trim().parse::<i64>().ok())
                    .collect();
                SavedLoadout {
                    id: r.get::<i64, _>("id"),
                    name: r.get::<String, _>("name"),
                    implant_ids,
                    updated_at: r.get::<i64, _>("updated_at"),
                }
            })
            .collect())
    }

    /// Delete a saved implant loadout.
    pub async fn delete_loadout(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM implant_loadouts WHERE id = ?1").bind(id).execute(&self.app).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn skill_plan_library_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        let id = db.save_skill_plan("Tackle Frig", "Evasive Maneuvering 4\nPropulsion Jamming 4", 100).await.unwrap();
        let plans = db.list_skill_plans().await.unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].name, "Tackle Frig");
        assert!(plans[0].body.contains("Propulsion Jamming"));
        db.delete_skill_plan(id).await.unwrap();
        assert!(db.list_skill_plans().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn fit_library_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        let id = db.save_fit("Solo Rifter", "Rifter", "[Rifter, Solo]\n200mm AutoCannon II", 100).await.unwrap();
        let fits = db.list_fits().await.unwrap();
        assert_eq!(fits.len(), 1);
        assert_eq!(fits[0].ship, "Rifter");
        assert!(fits[0].eft.starts_with("[Rifter"));
        db.delete_fit(id).await.unwrap();
        assert!(db.list_fits().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn implant_loadout_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        let id = db.save_loadout("PvP clone", &[10228, 10229, 13256], 100).await.unwrap();
        let loadouts = db.list_loadouts().await.unwrap();
        assert_eq!(loadouts.len(), 1);
        assert_eq!(loadouts[0].name, "PvP clone");
        assert_eq!(loadouts[0].implant_ids, vec![10228, 10229, 13256]);
        db.delete_loadout(id).await.unwrap();
        assert!(db.list_loadouts().await.unwrap().is_empty());
    }
}
