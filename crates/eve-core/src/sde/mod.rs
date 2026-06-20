//! Static Data Export (SDE) access.
//!
//! The SDE is CCP's offline static game data (items, systems, blueprints, …).
//! We ship a **prebuilt, version-pinned `sde.sqlite`** (converted from CCP's
//! YAML/CSV at build time in `sde-tools/`) and query it read-only here. This
//! module defines the normalized subset our converter produces and the query
//! API the rest of the app builds on (item/system lookups, name search).

use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Row, SqlitePool};

use crate::error::Result;

/// A type id paired with its resolved name (UI-facing, serializable). The shared
/// shape for any "id → name" resolution (implants, ship types, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedType {
    pub type_id: i64,
    pub name: String,
}

/// A minimal SDE item type.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemType {
    pub type_id: i64,
    pub name: String,
    pub group_id: Option<i64>,
}

/// A minimal solar system.
#[derive(Debug, Clone, PartialEq)]
pub struct SolarSystem {
    pub system_id: i64,
    pub name: String,
    pub security: f64,
}

/// Read-only handle to the static data.
#[derive(Clone)]
pub struct Sde {
    pool: SqlitePool,
}

/// The normalized schema our `sde-tools` converter emits. Kept here so tests and
/// the converter agree on shape.
pub const SDE_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS types (
    type_id  INTEGER PRIMARY KEY,
    name     TEXT NOT NULL,
    group_id INTEGER,
    volume   REAL
);
CREATE INDEX IF NOT EXISTS idx_types_name ON types(name);

CREATE TABLE IF NOT EXISTS solar_systems (
    system_id INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    region_id INTEGER,
    security  REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_systems_name ON solar_systems(name);
"#;

impl Sde {
    /// Open a prebuilt `sde.sqlite` read-only.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .read_only(true);
        let pool = SqlitePool::connect_with(opts).await?;
        Ok(Self { pool })
    }

    /// Open an in-memory SDE with the schema applied — used in tests and as a
    /// stand-in until the prebuilt DB is present.
    pub async fn open_in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?;
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await?;
        sqlx::raw_sql(SDE_SCHEMA).execute(&pool).await?;
        Ok(Self { pool })
    }

    /// Resolve a type id to its name.
    pub async fn type_name(&self, type_id: i64) -> Result<Option<String>> {
        let row = sqlx::query("SELECT name FROM types WHERE type_id = ?1")
            .bind(type_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("name")))
    }

    /// Resolve a solar system id to a [`SolarSystem`].
    pub async fn solar_system(&self, system_id: i64) -> Result<Option<SolarSystem>> {
        let row = sqlx::query("SELECT system_id, name, security FROM solar_systems WHERE system_id = ?1")
            .bind(system_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| SolarSystem {
            system_id: r.get::<i64, _>("system_id"),
            name: r.get::<String, _>("name"),
            security: r.get::<f64, _>("security"),
        }))
    }

    /// Resolve a list of type ids to [`NamedType`]s, falling back to `Type {id}`
    /// for ids the version-pinned SDE doesn't know.
    pub async fn name_types(&self, ids: &[i64]) -> Result<Vec<NamedType>> {
        let mut out = Vec::with_capacity(ids.len());
        for &type_id in ids {
            let name = self
                .type_name(type_id)
                .await?
                .unwrap_or_else(|| format!("Type {type_id}"));
            out.push(NamedType { type_id, name });
        }
        Ok(out)
    }

    /// Prefix-search item types by name (for the universal search bar).
    pub async fn search_types(&self, prefix: &str, limit: i64) -> Result<Vec<ItemType>> {
        let pattern = format!("{prefix}%");
        let rows = sqlx::query(
            "SELECT type_id, name, group_id FROM types WHERE name LIKE ?1 ORDER BY name LIMIT ?2",
        )
        .bind(pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| ItemType {
                type_id: r.get::<i64, _>("type_id"),
                name: r.get::<String, _>("name"),
                group_id: r.get::<Option<i64>, _>("group_id"),
            })
            .collect())
    }

    /// Test/seed helper: insert a type. (The real DB is shipped prebuilt.)
    pub async fn insert_type(&self, type_id: i64, name: &str, group_id: Option<i64>) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO types (type_id, name, group_id) VALUES (?1, ?2, ?3)")
            .bind(type_id)
            .bind(name)
            .bind(group_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Test/seed helper: insert a solar system.
    pub async fn insert_system(&self, system_id: i64, name: &str, security: f64) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO solar_systems (system_id, name, security) VALUES (?1, ?2, ?3)")
            .bind(system_id)
            .bind(name)
            .bind(security)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lookup_and_search() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_type(34, "Tritanium", Some(18)).await.unwrap();
        sde.insert_type(35, "Pyerite", Some(18)).await.unwrap();
        sde.insert_type(587, "Rifter", Some(25)).await.unwrap();
        sde.insert_system(30000142, "Jita", 0.946).await.unwrap();

        assert_eq!(sde.type_name(34).await.unwrap().as_deref(), Some("Tritanium"));
        assert_eq!(sde.type_name(99999).await.unwrap(), None);

        let jita = sde.solar_system(30000142).await.unwrap().unwrap();
        assert_eq!(jita.name, "Jita");
        assert!((jita.security - 0.946).abs() < 1e-9);

        let results = sde.search_types("Tri", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Tritanium");
    }

    #[tokio::test]
    async fn name_types_resolves_with_fallback() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_type(587, "Rifter", Some(25)).await.unwrap();
        let named = sde.name_types(&[587, 99999]).await.unwrap();
        assert_eq!(named[0].name, "Rifter");
        assert_eq!(named[1].name, "Type 99999"); // unknown id falls back
    }
}
