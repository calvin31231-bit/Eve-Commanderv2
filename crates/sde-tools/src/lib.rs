//! Converter: CCP's Static Data Export (YAML) → the prebuilt `sde.sqlite`.
//!
//! `eve-core` ships a **version-pinned, prebuilt `sde.sqlite`** and queries it
//! read-only (see [`eve_core::sde`]). This crate is the build-time tool that
//! produces that file from CCP's published YAML. The output schema is borrowed
//! directly from [`eve_core::sde::SDE_SCHEMA`] so the writer and reader can never
//! drift apart.
//!
//! ## Input formats
//! - **Types** — CCP's `typeIDs.yaml`: a top-level map keyed by integer type id,
//!   each value carrying a localized `name` (we take `en`), `groupID`, `volume`,
//!   and `published`. Unpublished and name-less entries are skipped.
//! - **Systems** — a normalized map keyed by integer system id with `name`,
//!   `regionID`, and `security`. (CCP scatters systems across per-system
//!   `solarsystem.staticdata` files plus a separate name table; emitting this
//!   flattened intermediate is left to the universe-walker — a follow-up.)
//!
//! Both ingests run inside a single transaction so a partial file never leaves a
//! half-written table.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use eve_core::sde::SDE_SCHEMA;
use rusqlite::{params, Connection};
use serde::Deserialize;

/// CCP localizes user-facing strings as `{ en: "...", de: "...", ... }`. We only
/// need English for now.
#[derive(Debug, Deserialize)]
struct Localized {
    en: Option<String>,
}

/// One entry from CCP's `typeIDs.yaml`. Only the fields we persist are declared;
/// the rest of the (large) record is ignored.
#[derive(Debug, Deserialize)]
struct RawType {
    name: Option<Localized>,
    #[serde(rename = "groupID")]
    group_id: Option<i64>,
    volume: Option<f64>,
    /// Defaults to published when absent (older exports omit it for published
    /// items).
    #[serde(default = "default_true")]
    published: bool,
}

/// One entry from the normalized systems map.
#[derive(Debug, Deserialize)]
struct RawSystem {
    name: String,
    #[serde(rename = "regionID")]
    region_id: Option<i64>,
    #[serde(default)]
    security: f64,
}

fn default_true() -> bool {
    true
}

/// Builds an `sde.sqlite` from CCP YAML inputs.
pub struct Converter {
    conn: Connection,
}

impl Converter {
    /// Create (or truncate to) a fresh `sde.sqlite` at `path` with the schema
    /// applied.
    pub fn create(path: impl AsRef<Path>) -> Result<Self> {
        // Start clean so re-running the converter is deterministic.
        let path = path.as_ref();
        if path.exists() {
            std::fs::remove_file(path)
                .with_context(|| format!("removing existing {}", path.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening {}", path.display()))?;
        conn.execute_batch(SDE_SCHEMA)
            .context("applying SDE schema")?;
        Ok(Self { conn })
    }

    /// In-memory converter for tests.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SDE_SCHEMA)?;
        Ok(Self { conn })
    }

    /// Ingest CCP's `typeIDs.yaml`. Returns the number of rows written. Skips
    /// unpublished entries and entries with no English name.
    pub fn ingest_types(&mut self, yaml: &str) -> Result<usize> {
        let raw: BTreeMap<i64, RawType> =
            serde_yaml::from_str(yaml).context("parsing typeIDs YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO types (type_id, name, group_id, volume)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (type_id, t) in raw {
                if !t.published {
                    continue;
                }
                let Some(name) = t.name.and_then(|n| n.en) else {
                    continue;
                };
                stmt.execute(params![type_id, name, t.group_id, t.volume])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest the normalized solar-systems map. Returns the number of rows
    /// written.
    pub fn ingest_systems(&mut self, yaml: &str) -> Result<usize> {
        let raw: BTreeMap<i64, RawSystem> =
            serde_yaml::from_str(yaml).context("parsing systems YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO solar_systems (system_id, name, region_id, security)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (system_id, s) in raw {
                stmt.execute(params![system_id, s.name, s.region_id, s.security])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Borrow the underlying connection (tests/inspection).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPES_YAML: &str = r#"
34:
  groupID: 18
  name:
    en: Tritanium
    de: Tritanium
  volume: 0.01
  published: true
35:
  groupID: 18
  name:
    en: Pyerite
  volume: 0.01
587:
  groupID: 25
  name:
    en: Rifter
  volume: 27289.0
  published: true
9999:
  groupID: 99
  name:
    en: Hidden Test Item
  published: false
"#;

    const SYSTEMS_YAML: &str = r#"
30000142:
  name: Jita
  regionID: 10000002
  security: 0.946
30002187:
  name: Amarr
  regionID: 10000043
  security: 0.93
"#;

    #[test]
    fn ingests_types_and_skips_unpublished() {
        let mut c = Converter::in_memory().unwrap();
        // 4 entries in YAML, one unpublished → 3 written.
        let n = c.ingest_types(TYPES_YAML).unwrap();
        assert_eq!(n, 3);

        let name: String = c
            .connection()
            .query_row("SELECT name FROM types WHERE type_id = 587", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Rifter");

        // Unpublished item must be absent.
        let hidden: i64 = c
            .connection()
            .query_row("SELECT COUNT(*) FROM types WHERE type_id = 9999", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(hidden, 0);

        // group_id and volume persisted.
        let (g, v): (i64, f64) = c
            .connection()
            .query_row(
                "SELECT group_id, volume FROM types WHERE type_id = 34",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(g, 18);
        assert!((v - 0.01).abs() < 1e-9);
    }

    #[test]
    fn ingests_systems() {
        let mut c = Converter::in_memory().unwrap();
        let n = c.ingest_systems(SYSTEMS_YAML).unwrap();
        assert_eq!(n, 2);

        let (name, region, sec): (String, i64, f64) = c
            .connection()
            .query_row(
                "SELECT name, region_id, security FROM solar_systems WHERE system_id = 30000142",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "Jita");
        assert_eq!(region, 10000002);
        assert!((sec - 0.946).abs() < 1e-9);
    }

    #[test]
    fn output_is_queryable_by_eve_core_reader() {
        // The converter's output must satisfy eve-core's read API. We assert the
        // schema/columns line up by querying exactly as the reader does.
        let mut c = Converter::in_memory().unwrap();
        c.ingest_types(TYPES_YAML).unwrap();
        let rows: Vec<String> = {
            let conn = c.connection();
            let mut stmt = conn
                .prepare("SELECT name FROM types WHERE name LIKE 'Tri%' ORDER BY name")
                .unwrap();
            let mapped = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            mapped
        };
        assert_eq!(rows, vec!["Tritanium".to_string()]);
    }

    #[test]
    fn reingest_is_idempotent() {
        let mut c = Converter::in_memory().unwrap();
        c.ingest_types(TYPES_YAML).unwrap();
        c.ingest_types(TYPES_YAML).unwrap();
        let count: i64 = c
            .connection()
            .query_row("SELECT COUNT(*) FROM types", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }
}
