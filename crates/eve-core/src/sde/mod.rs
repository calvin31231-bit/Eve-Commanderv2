//! Static Data Export (SDE) access.
//!
//! The SDE is CCP's offline static game data (items, systems, blueprints, …).
//! We ship a **prebuilt, version-pinned `sde.sqlite`** (converted from CCP's
//! YAML/CSV at build time in `sde-tools/`) and query it read-only here. This
//! module defines the normalized subset our converter produces and the query
//! API the rest of the app builds on (item/system lookups, name search).

pub mod seed;

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

/// A type id + quantity — a reprocessing yield row or a blueprint input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Material {
    pub type_id: i64,
    pub quantity: i64,
}

/// Skill training metadata: rank and the dogma attribute type ids that drive
/// training speed (e.g. 165 intelligence, 166 memory).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMeta {
    pub rank: i64,
    pub primary_attr: i64,
    pub secondary_attr: i64,
}

/// A blueprint activity's product (manufacturing output / invention result).
#[derive(Debug, Clone, PartialEq)]
pub struct BlueprintProduct {
    pub blueprint_type_id: i64,
    pub product_type_id: i64,
    /// Units produced per run.
    pub quantity: i64,
    /// Base success chance for invention (None for deterministic activities).
    pub probability: Option<f64>,
    /// Base activity time in seconds (ME/TE 0), if known.
    pub time: Option<i64>,
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
    type_id      INTEGER PRIMARY KEY,
    name         TEXT NOT NULL,
    group_id     INTEGER,
    volume       REAL,
    portion_size INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_types_name ON types(name);

CREATE TABLE IF NOT EXISTS solar_systems (
    system_id INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    region_id INTEGER,
    security  REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_systems_name ON solar_systems(name);

-- Reprocessing / refining yields (CCP invTypeMaterials): the materials a single
-- portion of `type_id` reprocesses into. Quantities are pre-efficiency (100%).
CREATE TABLE IF NOT EXISTS type_materials (
    type_id          INTEGER NOT NULL,
    material_type_id INTEGER NOT NULL,
    quantity         INTEGER NOT NULL,
    PRIMARY KEY (type_id, material_type_id)
);

-- Blueprint activity inputs (manufacturing / reaction / invention): the
-- materials consumed per run at ME 0.
CREATE TABLE IF NOT EXISTS blueprint_materials (
    blueprint_type_id INTEGER NOT NULL,
    activity          TEXT NOT NULL,
    material_type_id  INTEGER NOT NULL,
    quantity          INTEGER NOT NULL,
    PRIMARY KEY (blueprint_type_id, activity, material_type_id)
);

-- Blueprint activity outputs: the product(s) of an activity, with per-run
-- quantity and (for invention) base success probability.
CREATE TABLE IF NOT EXISTS blueprint_products (
    blueprint_type_id INTEGER NOT NULL,
    activity          TEXT NOT NULL,
    product_type_id   INTEGER NOT NULL,
    quantity          INTEGER NOT NULL,
    probability       REAL,
    time              INTEGER,
    PRIMARY KEY (blueprint_type_id, activity, product_type_id)
);
CREATE INDEX IF NOT EXISTS idx_bp_products_product
    ON blueprint_products(product_type_id, activity);

-- Skill training metadata: rank (skillTimeConstant, dogma 275) and the two
-- training attribute ids (primary 180, secondary 181 → attribute type ids).
CREATE TABLE IF NOT EXISTS skills (
    type_id        INTEGER PRIMARY KEY,
    rank           INTEGER NOT NULL DEFAULT 1,
    primary_attr   INTEGER NOT NULL DEFAULT 0,
    secondary_attr INTEGER NOT NULL DEFAULT 0
);
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

    /// Open an in-memory SDE pre-populated with the [`seed`] of common types and
    /// systems. Used as the fallback when no full prebuilt `sde.sqlite` is
    /// present, so common items still resolve to real names.
    pub async fn seeded() -> Result<Self> {
        let sde = Self::open_in_memory().await?;
        for (type_id, name) in seed::SEED_TYPES {
            sde.insert_type(*type_id, name, None).await?;
        }
        for (system_id, name, security) in seed::SEED_SYSTEMS {
            sde.insert_system(*system_id, name, *security).await?;
        }
        Ok(sde)
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

    /// Reprocessing batch size for a type (`portionSize`; ore is 100, most items
    /// 1). Defaults to 1 for unknown ids so callers never divide by zero.
    pub async fn portion_size(&self, type_id: i64) -> Result<i64> {
        let row = sqlx::query("SELECT portion_size FROM types WHERE type_id = ?1")
            .bind(type_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<i64, _>("portion_size")).unwrap_or(1).max(1))
    }

    /// The materials one portion of `type_id` reprocesses into (pre-efficiency).
    pub async fn reprocess_materials(&self, type_id: i64) -> Result<Vec<Material>> {
        let rows = sqlx::query(
            "SELECT material_type_id, quantity FROM type_materials WHERE type_id = ?1",
        )
        .bind(type_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Material {
                type_id: r.get::<i64, _>("material_type_id"),
                quantity: r.get::<i64, _>("quantity"),
            })
            .collect())
    }

    /// The blueprint that produces `product_type_id` via `activity`
    /// (e.g. "manufacturing", "reaction"), if any.
    pub async fn blueprint_for_product(
        &self,
        product_type_id: i64,
        activity: &str,
    ) -> Result<Option<BlueprintProduct>> {
        let row = sqlx::query(
            "SELECT blueprint_type_id, product_type_id, quantity, probability, time
             FROM blueprint_products WHERE product_type_id = ?1 AND activity = ?2",
        )
        .bind(product_type_id)
        .bind(activity)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| BlueprintProduct {
            blueprint_type_id: r.get::<i64, _>("blueprint_type_id"),
            product_type_id: r.get::<i64, _>("product_type_id"),
            quantity: r.get::<i64, _>("quantity"),
            probability: r.get::<Option<f64>, _>("probability"),
            time: r.get::<Option<i64>, _>("time"),
        }))
    }

    /// The inputs a blueprint consumes per run of `activity` (at ME 0).
    pub async fn blueprint_materials(
        &self,
        blueprint_type_id: i64,
        activity: &str,
    ) -> Result<Vec<Material>> {
        let rows = sqlx::query(
            "SELECT material_type_id, quantity FROM blueprint_materials
             WHERE blueprint_type_id = ?1 AND activity = ?2",
        )
        .bind(blueprint_type_id)
        .bind(activity)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Material {
                type_id: r.get::<i64, _>("material_type_id"),
                quantity: r.get::<i64, _>("quantity"),
            })
            .collect())
    }

    /// Skill rank + training attributes for a skill type, if the SDE has it.
    pub async fn skill_meta(&self, type_id: i64) -> Result<Option<SkillMeta>> {
        let row = sqlx::query(
            "SELECT rank, primary_attr, secondary_attr FROM skills WHERE type_id = ?1",
        )
        .bind(type_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| SkillMeta {
            rank: r.get::<i64, _>("rank"),
            primary_attr: r.get::<i64, _>("primary_attr"),
            secondary_attr: r.get::<i64, _>("secondary_attr"),
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

    /// Resolve an exact type name to its id (case-insensitive). Used to turn the
    /// names in a pasted fit into type ids.
    pub async fn type_id_by_name(&self, name: &str) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT type_id FROM types WHERE name = ?1 COLLATE NOCASE")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<i64, _>("type_id")))
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

    /// Test helper: set a type's reprocessing portion size.
    pub async fn set_portion_size(&self, type_id: i64, portion_size: i64) -> Result<()> {
        sqlx::query("UPDATE types SET portion_size = ?2 WHERE type_id = ?1")
            .bind(type_id)
            .bind(portion_size)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Test helper: insert a reprocessing yield row.
    pub async fn insert_type_material(
        &self,
        type_id: i64,
        material_type_id: i64,
        quantity: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO type_materials (type_id, material_type_id, quantity)
             VALUES (?1, ?2, ?3)",
        )
        .bind(type_id)
        .bind(material_type_id)
        .bind(quantity)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Test helper: insert a blueprint input row.
    pub async fn insert_blueprint_material(
        &self,
        blueprint_type_id: i64,
        activity: &str,
        material_type_id: i64,
        quantity: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO blueprint_materials
             (blueprint_type_id, activity, material_type_id, quantity) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(blueprint_type_id)
        .bind(activity)
        .bind(material_type_id)
        .bind(quantity)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Test/seed helper: insert skill training metadata.
    pub async fn insert_skill(
        &self,
        type_id: i64,
        rank: i64,
        primary_attr: i64,
        secondary_attr: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO skills (type_id, rank, primary_attr, secondary_attr)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(type_id)
        .bind(rank)
        .bind(primary_attr)
        .bind(secondary_attr)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Test helper: insert a blueprint product row.
    pub async fn insert_blueprint_product(
        &self,
        blueprint_type_id: i64,
        activity: &str,
        product_type_id: i64,
        quantity: i64,
        probability: Option<f64>,
        time: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO blueprint_products
             (blueprint_type_id, activity, product_type_id, quantity, probability, time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(blueprint_type_id)
        .bind(activity)
        .bind(product_type_id)
        .bind(quantity)
        .bind(probability)
        .bind(time)
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

    #[tokio::test]
    async fn seeded_resolves_common_items() {
        let sde = Sde::seeded().await.unwrap();
        // Minerals, ores, ships, and hubs from the curated seed resolve.
        assert_eq!(sde.type_name(34).await.unwrap().as_deref(), Some("Tritanium"));
        assert_eq!(sde.type_name(1230).await.unwrap().as_deref(), Some("Veldspar"));
        assert_eq!(sde.type_name(587).await.unwrap().as_deref(), Some("Rifter"));
        assert_eq!(sde.solar_system(30000142).await.unwrap().unwrap().name, "Jita");
        // Unseeded ids still fall back gracefully.
        assert_eq!(sde.type_name(123456).await.unwrap(), None);
    }
}
