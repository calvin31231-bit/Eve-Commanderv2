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

/// A system positioned for the region map (UI-facing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemNode {
    pub system_id: i64,
    pub name: String,
    pub security: f64,
    pub x: f64,
    pub z: f64,
}

/// A mission agent matched by the finder, with its location resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentMatch {
    pub agent_id: i64,
    pub corporation_id: i64,
    pub division_id: i64,
    pub division_name: String,
    pub level: i64,
    pub agent_type: i64,
    pub is_locator: bool,
    pub station_id: i64,
    pub station_name: String,
    pub system_id: i64,
    pub system_name: String,
    pub region_id: i64,
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
    security  REAL NOT NULL DEFAULT 0,
    -- Map coordinates (EVE's x/z plane) for Dotlan-style region layout.
    x         REAL NOT NULL DEFAULT 0,
    z         REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_systems_name ON solar_systems(name);
CREATE INDEX IF NOT EXISTS idx_systems_region ON solar_systems(region_id);

-- Stargate adjacency (undirected; stored both ways by the converter) for the
-- region map and any local routing.
CREATE TABLE IF NOT EXISTS system_jumps (
    from_system_id INTEGER NOT NULL,
    to_system_id   INTEGER NOT NULL,
    PRIMARY KEY (from_system_id, to_system_id)
);
CREATE INDEX IF NOT EXISTS idx_jumps_from ON system_jumps(from_system_id);

CREATE TABLE IF NOT EXISTS regions (
    region_id INTEGER PRIMARY KEY,
    name      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_regions_name ON regions(name);

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

-- Skills a type requires to use (dogma requiredSkill1..6 + level), driving the
-- "can I fly this fit?" check.
CREATE TABLE IF NOT EXISTS type_required_skills (
    type_id       INTEGER NOT NULL,
    skill_type_id INTEGER NOT NULL,
    level         INTEGER NOT NULL,
    PRIMARY KEY (type_id, skill_type_id)
);

-- A curated set of dogma attributes per type (HP, resonances, capacitor, damage,
-- rate of fire, slots) that feed the fitting-stats engine. Only fitting-relevant
-- attribute ids are stored (see sde-tools FIT_ATTRS) so the table stays small.
CREATE TABLE IF NOT EXISTS type_attributes (
    type_id      INTEGER NOT NULL,
    attribute_id INTEGER NOT NULL,
    value        REAL    NOT NULL,
    PRIMARY KEY (type_id, attribute_id)
);

-- Ship trait bonuses (CCP's own bonus text from typeIDs `traits`): per-skill,
-- role, and misc bonuses shown on a fit. Display-only — these are NOT auto-
-- applied to DPS, which needs the full dogma-effect engine.
CREATE TABLE IF NOT EXISTS type_traits (
    type_id       INTEGER NOT NULL,
    skill_type_id INTEGER,
    bonus         REAL,
    unit_id       INTEGER,
    text          TEXT    NOT NULL,
    kind          TEXT    NOT NULL,
    ordinal       INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_type_traits_type ON type_traits (type_id);

-- NPC stations (Fuzzwork staStations): where agents sit. Carries the system +
-- region + security so the agent finder can filter by location without a join
-- back to the universe walk.
CREATE TABLE IF NOT EXISTS stations (
    station_id INTEGER PRIMARY KEY,
    system_id  INTEGER NOT NULL,
    region_id  INTEGER,
    name       TEXT NOT NULL,
    security   REAL NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_stations_system ON stations(system_id);

-- NPC corporation divisions (CCP npcCorporationDivisions): the authoritative
-- division name (Security / Distribution / Mining / R&D …) keyed by id, so the
-- agent finder never has to guess a label.
CREATE TABLE IF NOT EXISTS divisions (
    division_id INTEGER PRIMARY KEY,
    name        TEXT NOT NULL
);

-- Mission agents (CCP agents.yaml): the finder's core table. location_id is a
-- station id; division_id resolves via `divisions`; corporation_id resolves to
-- a name at runtime via ESI like any other id.
CREATE TABLE IF NOT EXISTS agents (
    agent_id       INTEGER PRIMARY KEY,
    corporation_id INTEGER,
    division_id    INTEGER,
    level          INTEGER NOT NULL DEFAULT 1,
    location_id    INTEGER,
    agent_type     INTEGER,
    is_locator     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_agents_level ON agents(level);
CREATE INDEX IF NOT EXISTS idx_agents_location ON agents(location_id);
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

    /// Packaged volume (m³) of a type, when known. Powers ISK/m³ hauling math.
    pub async fn type_volume(&self, type_id: i64) -> Result<Option<f64>> {
        let row = sqlx::query("SELECT volume FROM types WHERE type_id = ?1")
            .bind(type_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.and_then(|r| r.get::<Option<f64>, _>("volume")))
    }

    /// The market/ship group id of a type — used to classify ships into tactical
    /// roles for gang-composition estimates.
    pub async fn type_group_id(&self, type_id: i64) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT group_id FROM types WHERE type_id = ?1")
            .bind(type_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.and_then(|r| r.get::<Option<i64>, _>("group_id")))
    }

    /// The curated dogma attributes stored for a type, as an `attribute_id →
    /// value` map (empty when the SDE predates the fitting-stats ingestion or the
    /// type has none). Feeds `eve_core::dogma`.
    pub async fn type_attributes(&self, type_id: i64) -> Result<std::collections::HashMap<i64, f64>> {
        let rows = sqlx::query("SELECT attribute_id, value FROM type_attributes WHERE type_id = ?1")
            .bind(type_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get::<i64, _>("attribute_id"), r.get::<f64, _>("value")))
            .collect())
    }

    /// Ship trait bonus lines (CCP's own bonus text) for a type, ordered as
    /// shown in-game. Each is `(kind, bonus, unit_id, text)` where kind is
    /// `skill`/`role`/`misc`. Empty for non-ship types or a pre-trait SDE.
    pub async fn type_traits(&self, type_id: i64) -> Result<Vec<(String, Option<f64>, Option<i64>, String)>> {
        let rows = sqlx::query(
            "SELECT kind, bonus, unit_id, text FROM type_traits WHERE type_id = ?1 ORDER BY ordinal",
        )
        .bind(type_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("kind"),
                    r.get::<Option<f64>, _>("bonus"),
                    r.get::<Option<i64>, _>("unit_id"),
                    r.get::<String, _>("text"),
                )
            })
            .collect())
    }

    /// All types that carry `attribute_id`, as `(type_id, value)`. Used to
    /// enumerate implants (attribute 331 = implant slot).
    pub async fn types_with_attribute(&self, attribute_id: i64) -> Result<Vec<(i64, f64)>> {
        let rows = sqlx::query("SELECT type_id, value FROM type_attributes WHERE attribute_id = ?1")
            .bind(attribute_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|r| (r.get::<i64, _>("type_id"), r.get::<f64, _>("value"))).collect())
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

    /// The region id a system belongs to.
    pub async fn system_region(&self, system_id: i64) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT region_id FROM solar_systems WHERE system_id = ?1")
            .bind(system_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.and_then(|r| r.get::<Option<i64>, _>("region_id")))
    }

    /// Resolve a region name to its id (case-insensitive).
    pub async fn region_id_by_name(&self, name: &str) -> Result<Option<i64>> {
        let row = sqlx::query("SELECT region_id FROM regions WHERE name = ?1 COLLATE NOCASE")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| r.get::<i64, _>("region_id")))
    }

    /// All regions (id + name), alphabetical — for the map's region picker.
    pub async fn list_regions(&self) -> Result<Vec<(i64, String)>> {
        let rows = sqlx::query("SELECT region_id, name FROM regions ORDER BY name")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.get::<i64, _>("region_id"), r.get::<String, _>("name")))
            .collect())
    }

    /// All systems in a region, positioned for the map.
    pub async fn systems_in_region(&self, region_id: i64) -> Result<Vec<SystemNode>> {
        let rows = sqlx::query(
            "SELECT system_id, name, security, x, z FROM solar_systems WHERE region_id = ?1",
        )
        .bind(region_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| SystemNode {
                system_id: r.get::<i64, _>("system_id"),
                name: r.get::<String, _>("name"),
                security: r.get::<f64, _>("security"),
                x: r.get::<f64, _>("x"),
                z: r.get::<f64, _>("z"),
            })
            .collect())
    }

    /// Stargate jumps with both endpoints inside `region_id` (region-internal
    /// edges for the map; from < to to de-duplicate the undirected pair).
    pub async fn jumps_in_region(&self, region_id: i64) -> Result<Vec<(i64, i64)>> {
        let rows = sqlx::query(
            "SELECT j.from_system_id AS a, j.to_system_id AS b
             FROM system_jumps j
             JOIN solar_systems sf ON sf.system_id = j.from_system_id
             JOIN solar_systems st ON st.system_id = j.to_system_id
             WHERE sf.region_id = ?1 AND st.region_id = ?1 AND j.from_system_id < j.to_system_id",
        )
        .bind(region_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.get::<i64, _>("a"), r.get::<i64, _>("b")))
            .collect())
    }

    /// Find mission agents matching optional filters, soonest-useful first
    /// (highest level, then safest security). `level` filters exact level (1–5);
    /// `min_security` keeps agents in systems at or above that security band
    /// (e.g. 0.5 for hisec-only); `region_id` scopes to one region. Joins agents
    /// to their station + system + division. Empty when the SDE lacks agent data.
    pub async fn find_agents(
        &self,
        level: Option<i64>,
        min_security: Option<f64>,
        region_id: Option<i64>,
        limit: i64,
    ) -> Result<Vec<AgentMatch>> {
        let rows = sqlx::query(
            "SELECT a.agent_id, a.corporation_id, a.division_id, a.level, a.agent_type,
                    a.is_locator, s.station_id, s.name AS station_name, s.system_id,
                    s.region_id, s.security, sys.name AS system_name,
                    COALESCE(d.name, '') AS division_name
             FROM agents a
             JOIN stations s ON s.station_id = a.location_id
             LEFT JOIN solar_systems sys ON sys.system_id = s.system_id
             LEFT JOIN divisions d ON d.division_id = a.division_id
             WHERE (?1 IS NULL OR a.level = ?1)
               AND (?2 IS NULL OR s.security >= ?2)
               AND (?3 IS NULL OR s.region_id = ?3)
             ORDER BY a.level DESC, s.security DESC
             LIMIT ?4",
        )
        .bind(level)
        .bind(min_security)
        .bind(region_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| AgentMatch {
                agent_id: r.get::<i64, _>("agent_id"),
                corporation_id: r.get::<Option<i64>, _>("corporation_id").unwrap_or(0),
                division_id: r.get::<Option<i64>, _>("division_id").unwrap_or(0),
                division_name: r.get::<String, _>("division_name"),
                level: r.get::<i64, _>("level"),
                agent_type: r.get::<Option<i64>, _>("agent_type").unwrap_or(0),
                is_locator: r.get::<i64, _>("is_locator") != 0,
                station_id: r.get::<i64, _>("station_id"),
                station_name: r.get::<String, _>("station_name"),
                system_id: r.get::<i64, _>("system_id"),
                system_name: r.get::<Option<String>, _>("system_name").unwrap_or_default(),
                region_id: r.get::<Option<i64>, _>("region_id").unwrap_or(0),
                security: r.get::<f64, _>("security"),
            })
            .collect())
    }

    /// Test/seed helper: insert an NPC station.
    pub async fn insert_station(
        &self,
        station_id: i64,
        system_id: i64,
        region_id: i64,
        name: &str,
        security: f64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO stations (station_id, system_id, region_id, name, security)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(station_id)
        .bind(system_id)
        .bind(region_id)
        .bind(name)
        .bind(security)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Test/seed helper: insert a division name.
    pub async fn insert_division(&self, division_id: i64, name: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO divisions (division_id, name) VALUES (?1, ?2)")
            .bind(division_id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Test/seed helper: insert a mission agent.
    #[allow(clippy::too_many_arguments)]
    pub async fn insert_agent(
        &self,
        agent_id: i64,
        corporation_id: i64,
        division_id: i64,
        level: i64,
        location_id: i64,
        agent_type: i64,
        is_locator: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO agents
             (agent_id, corporation_id, division_id, level, location_id, agent_type, is_locator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(agent_id)
        .bind(corporation_id)
        .bind(division_id)
        .bind(level)
        .bind(location_id)
        .bind(agent_type)
        .bind(is_locator as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
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

    /// The skills (id + level) a type requires to be used.
    pub async fn required_skills(&self, type_id: i64) -> Result<Vec<Material>> {
        let rows = sqlx::query(
            "SELECT skill_type_id, level FROM type_required_skills WHERE type_id = ?1",
        )
        .bind(type_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Material {
                type_id: r.get::<i64, _>("skill_type_id"),
                quantity: r.get::<i64, _>("level"),
            })
            .collect())
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

    /// Test/seed helper: insert a fully-specified system (region + coords).
    pub async fn insert_system_full(
        &self,
        system_id: i64,
        name: &str,
        region_id: i64,
        security: f64,
        x: f64,
        z: f64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO solar_systems (system_id, name, region_id, security, x, z)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(system_id)
        .bind(name)
        .bind(region_id)
        .bind(security)
        .bind(x)
        .bind(z)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Test/seed helper: insert a stargate jump (both directions).
    pub async fn insert_jump(&self, from_system_id: i64, to_system_id: i64) -> Result<()> {
        for (a, b) in [(from_system_id, to_system_id), (to_system_id, from_system_id)] {
            sqlx::query(
                "INSERT OR REPLACE INTO system_jumps (from_system_id, to_system_id) VALUES (?1, ?2)",
            )
            .bind(a)
            .bind(b)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Test/seed helper: insert a region.
    pub async fn insert_region(&self, region_id: i64, name: &str) -> Result<()> {
        sqlx::query("INSERT OR REPLACE INTO regions (region_id, name) VALUES (?1, ?2)")
            .bind(region_id)
            .bind(name)
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

    /// Test/seed helper: insert a required-skill row for a type.
    pub async fn insert_required_skill(
        &self,
        type_id: i64,
        skill_type_id: i64,
        level: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO type_required_skills (type_id, skill_type_id, level)
             VALUES (?1, ?2, ?3)",
        )
        .bind(type_id)
        .bind(skill_type_id)
        .bind(level)
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

    #[tokio::test]
    async fn agent_finder_filters_by_level_security_region() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_region(10000002, "The Forge").await.unwrap();
        sde.insert_system_full(30000142, "Jita", 10000002, 0.95, 0.0, 0.0).await.unwrap();
        sde.insert_system_full(30002053, "Hek", 10000042, 0.5, 0.0, 0.0).await.unwrap();
        sde.insert_division(24, "Security").await.unwrap();
        // L4 security agent in hisec Jita (The Forge).
        sde.insert_station(60003760, 30000142, 10000002, "Jita IV-4", 0.95).await.unwrap();
        sde.insert_agent(3018970, 1000035, 24, 4, 60003760, 2, false).await.unwrap();
        // L1 agent in a lowsec system, different region.
        sde.insert_station(60005686, 30002053, 10000042, "Hek VIII", 0.5).await.unwrap();
        sde.insert_agent(3010001, 1000040, 24, 1, 60005686, 2, false).await.unwrap();

        // Level 4 only → just the Jita agent, with names + division resolved.
        let l4 = sde.find_agents(Some(4), None, None, 50).await.unwrap();
        assert_eq!(l4.len(), 1);
        assert_eq!(l4[0].system_name, "Jita");
        assert_eq!(l4[0].division_name, "Security");
        assert_eq!(l4[0].station_name, "Jita IV-4");

        // Region scope to The Forge → only the Jita agent.
        let forge = sde.find_agents(None, None, Some(10000002), 50).await.unwrap();
        assert_eq!(forge.len(), 1);

        // Hisec-only (>=0.5 keeps both; >=0.9 keeps just Jita).
        assert_eq!(sde.find_agents(None, Some(0.9), None, 50).await.unwrap().len(), 1);
        assert_eq!(sde.find_agents(None, None, None, 50).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn region_map_queries() {
        let sde = Sde::open_in_memory().await.unwrap();
        sde.insert_region(10000002, "The Forge").await.unwrap();
        sde.insert_system_full(30000142, "Jita", 10000002, 0.95, 1.0, 2.0).await.unwrap();
        sde.insert_system_full(30000144, "Perimeter", 10000002, 0.95, 1.5, 2.5).await.unwrap();
        // A system in another region (must not appear in The Forge's map).
        sde.insert_system_full(30002187, "Amarr", 10000043, 1.0, 9.0, 9.0).await.unwrap();
        sde.insert_jump(30000142, 30000144).await.unwrap();
        sde.insert_jump(30000142, 30002187).await.unwrap(); // cross-region

        assert_eq!(sde.region_id_by_name("the forge").await.unwrap(), Some(10000002));
        assert_eq!(sde.system_region(30000142).await.unwrap(), Some(10000002));

        let systems = sde.systems_in_region(10000002).await.unwrap();
        assert_eq!(systems.len(), 2);

        // Only the intra-region edge (Jita↔Perimeter), de-duplicated.
        let edges = sde.jumps_in_region(10000002).await.unwrap();
        assert_eq!(edges, vec![(30000142, 30000144)]);

        let regions = sde.list_regions().await.unwrap();
        assert_eq!(regions.len(), 1);
    }
}
