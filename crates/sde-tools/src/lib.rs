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
use std::collections::HashMap;
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
    /// Reprocessing batch size (ore = 100, most items 1). Defaults to 1.
    #[serde(rename = "portionSize", default = "default_portion")]
    portion_size: i64,
    /// Defaults to published when absent (older exports omit it for published
    /// items).
    #[serde(default = "default_true")]
    published: bool,
}

/// One `typeMaterials.yaml` entry: the materials a type reprocesses into.
#[derive(Debug, Deserialize)]
struct RawTypeMaterials {
    #[serde(default)]
    materials: Vec<RawMaterial>,
}

#[derive(Debug, Deserialize)]
struct RawMaterial {
    #[serde(rename = "materialTypeID")]
    material_type_id: i64,
    quantity: i64,
}

/// One `blueprints.yaml` entry: a blueprint's per-activity materials/products.
#[derive(Debug, Deserialize)]
struct RawBlueprint {
    #[serde(default)]
    activities: RawActivities,
}

#[derive(Debug, Default, Deserialize)]
struct RawActivities {
    manufacturing: Option<RawActivity>,
    reaction: Option<RawActivity>,
    invention: Option<RawActivity>,
}

#[derive(Debug, Deserialize)]
struct RawActivity {
    #[serde(default)]
    materials: Vec<RawActivityMaterial>,
    #[serde(default)]
    products: Vec<RawActivityProduct>,
    time: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawActivityMaterial {
    #[serde(rename = "typeID")]
    type_id: i64,
    quantity: i64,
}

#[derive(Debug, Deserialize)]
struct RawActivityProduct {
    #[serde(rename = "typeID")]
    type_id: i64,
    quantity: i64,
    probability: Option<f64>,
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

/// One normalized required-skills entry for a type: `{type_id: {skills:
/// [{skillTypeID, level}]}}`.
#[derive(Debug, Deserialize)]
struct RawRequiredSkills {
    #[serde(default)]
    skills: Vec<RawRequiredSkill>,
}

#[derive(Debug, Deserialize)]
struct RawRequiredSkill {
    #[serde(rename = "skillTypeID")]
    skill_type_id: i64,
    level: i64,
}

/// One entry from CCP's `typeDogma.yaml`: a type's dogma attribute values. We
/// only read the attributes that drive skills + skill requirements.
#[derive(Debug, Deserialize)]
struct RawTypeDogma {
    #[serde(rename = "dogmaAttributes", default)]
    dogma_attributes: Vec<RawDogmaAttr>,
}

#[derive(Debug, Deserialize)]
struct RawDogmaAttr {
    #[serde(rename = "attributeID")]
    attribute_id: i64,
    value: f64,
}

/// One normalized skill entry: rank + the two training attribute type ids.
#[derive(Debug, Deserialize)]
struct RawSkill {
    #[serde(default = "default_portion")]
    rank: i64,
    #[serde(rename = "primaryAttribute", default)]
    primary_attribute: i64,
    #[serde(rename = "secondaryAttribute", default)]
    secondary_attribute: i64,
}

fn default_true() -> bool {
    true
}

fn default_portion() -> i64 {
    1
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
                "INSERT OR REPLACE INTO types (type_id, name, group_id, volume, portion_size)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (type_id, t) in raw {
                if !t.published {
                    continue;
                }
                let Some(name) = t.name.and_then(|n| n.en) else {
                    continue;
                };
                stmt.execute(params![type_id, name, t.group_id, t.volume, t.portion_size])?;
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

    /// Ingest CCP's `typeMaterials.yaml` (reprocessing yields). Returns the
    /// number of yield rows written.
    pub fn ingest_type_materials(&mut self, yaml: &str) -> Result<usize> {
        let raw: BTreeMap<i64, RawTypeMaterials> =
            serde_yaml::from_str(yaml).context("parsing typeMaterials YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO type_materials (type_id, material_type_id, quantity)
                 VALUES (?1, ?2, ?3)",
            )?;
            for (type_id, tm) in raw {
                for m in tm.materials {
                    stmt.execute(params![type_id, m.material_type_id, m.quantity])?;
                    written += 1;
                }
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest CCP's `blueprints.yaml` (manufacturing / reaction / invention
    /// materials and products). Returns the number of (material + product) rows
    /// written across all activities.
    pub fn ingest_blueprints(&mut self, yaml: &str) -> Result<usize> {
        let raw: BTreeMap<i64, RawBlueprint> =
            serde_yaml::from_str(yaml).context("parsing blueprints YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut mat_stmt = tx.prepare(
                "INSERT OR REPLACE INTO blueprint_materials
                 (blueprint_type_id, activity, material_type_id, quantity) VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut prod_stmt = tx.prepare(
                "INSERT OR REPLACE INTO blueprint_products
                 (blueprint_type_id, activity, product_type_id, quantity, probability, time)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for (bp_id, bp) in raw {
                for (activity, act) in [
                    ("manufacturing", bp.activities.manufacturing),
                    ("reaction", bp.activities.reaction),
                    ("invention", bp.activities.invention),
                ] {
                    let Some(act) = act else { continue };
                    for m in &act.materials {
                        mat_stmt.execute(params![bp_id, activity, m.type_id, m.quantity])?;
                        written += 1;
                    }
                    for p in &act.products {
                        prod_stmt.execute(params![
                            bp_id,
                            activity,
                            p.type_id,
                            p.quantity,
                            p.probability,
                            act.time
                        ])?;
                        written += 1;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest a normalized skills map (`{type_id: {rank, primaryAttribute,
    /// secondaryAttribute}}`, derived from dogma attributes 275/180/181).
    /// Returns the number of rows written.
    pub fn ingest_skills(&mut self, yaml: &str) -> Result<usize> {
        let raw: BTreeMap<i64, RawSkill> =
            serde_yaml::from_str(yaml).context("parsing skills YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO skills (type_id, rank, primary_attr, secondary_attr)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (type_id, s) in raw {
                stmt.execute(params![
                    type_id,
                    s.rank,
                    s.primary_attribute,
                    s.secondary_attribute
                ])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest a normalized required-skills map (derived from dogma
    /// requiredSkill1..6 + their level attributes). Returns rows written.
    pub fn ingest_required_skills(&mut self, yaml: &str) -> Result<usize> {
        let raw: BTreeMap<i64, RawRequiredSkills> =
            serde_yaml::from_str(yaml).context("parsing required-skills YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO type_required_skills (type_id, skill_type_id, level)
                 VALUES (?1, ?2, ?3)",
            )?;
            for (type_id, rs) in raw {
                for s in rs.skills {
                    stmt.execute(params![type_id, s.skill_type_id, s.level])?;
                    written += 1;
                }
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest CCP's `typeDogma.yaml` directly, deriving **both** the `skills`
    /// table (rank 275 + training attributes 180/181) and `type_required_skills`
    /// (requiredSkill1..6 = attrs 182/183/184/1285/1289/1290 with their level
    /// attrs 277/278/279/1286/1287/1288). This is the single real-CCP-file path
    /// that lights up skill plans + can-I-fly without a hand-normalized step.
    /// Returns the number of rows written across both tables.
    pub fn ingest_type_dogma(&mut self, yaml: &str) -> Result<usize> {
        // (requiredSkill attribute id, its level attribute id).
        const REQ_PAIRS: [(i64, i64); 6] =
            [(182, 277), (183, 278), (184, 279), (1285, 1286), (1289, 1287), (1290, 1288)];

        let raw: BTreeMap<i64, RawTypeDogma> =
            serde_yaml::from_str(yaml).context("parsing typeDogma YAML")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut skill_stmt = tx.prepare(
                "INSERT OR REPLACE INTO skills (type_id, rank, primary_attr, secondary_attr)
                 VALUES (?1, ?2, ?3, ?4)",
            )?;
            let mut req_stmt = tx.prepare(
                "INSERT OR REPLACE INTO type_required_skills (type_id, skill_type_id, level)
                 VALUES (?1, ?2, ?3)",
            )?;
            for (type_id, td) in raw {
                let attrs: HashMap<i64, f64> = td
                    .dogma_attributes
                    .iter()
                    .map(|a| (a.attribute_id, a.value))
                    .collect();

                // Skill training metadata: present iff the type has a rank (275).
                if let Some(&rank) = attrs.get(&275) {
                    let primary = attrs.get(&180).copied().unwrap_or(0.0) as i64;
                    let secondary = attrs.get(&181).copied().unwrap_or(0.0) as i64;
                    skill_stmt.execute(params![type_id, rank as i64, primary, secondary])?;
                    written += 1;
                }

                // Required skills to use this type.
                for (skill_attr, level_attr) in REQ_PAIRS {
                    if let Some(&skill) = attrs.get(&skill_attr) {
                        let level = attrs.get(&level_attr).copied().unwrap_or(1.0) as i64;
                        req_stmt.execute(params![type_id, skill as i64, level.max(1)])?;
                        written += 1;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest Fuzzwork's `mapSolarSystems.csv` (systems with region, security,
    /// and x/z map coordinates). Column order is read from the header, so it
    /// tolerates layout changes. Returns rows written.
    pub fn ingest_systems_csv(&mut self, csv: &str) -> Result<usize> {
        let mut lines = csv.lines();
        let header = lines.next().context("empty systems CSV")?;
        let col = csv_columns(header);
        let idx = |name: &str| col.get(name).copied();
        let (c_id, c_name, c_region, c_sec, c_x, c_z) = (
            idx("solarSystemID").context("missing solarSystemID")?,
            idx("solarSystemName").context("missing solarSystemName")?,
            idx("regionID").context("missing regionID")?,
            idx("security").context("missing security")?,
            idx("x").context("missing x")?,
            idx("z").context("missing z")?,
        );

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO solar_systems (system_id, name, region_id, security, x, z)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            )?;
            for line in lines {
                if line.trim().is_empty() {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                let max = c_id.max(c_name).max(c_region).max(c_sec).max(c_x).max(c_z);
                if f.len() <= max {
                    continue;
                }
                let (Ok(id), Ok(region), Ok(sec), Ok(x), Ok(z)) = (
                    f[c_id].parse::<i64>(),
                    f[c_region].parse::<i64>(),
                    f[c_sec].parse::<f64>(),
                    f[c_x].parse::<f64>(),
                    f[c_z].parse::<f64>(),
                ) else {
                    continue;
                };
                stmt.execute(params![id, f[c_name], region, sec, x, z])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest Fuzzwork's `mapSolarSystemJumps.csv` (stargate adjacency). Stores
    /// each pair both ways. Returns rows written.
    pub fn ingest_jumps_csv(&mut self, csv: &str) -> Result<usize> {
        let mut lines = csv.lines();
        let header = lines.next().context("empty jumps CSV")?;
        let col = csv_columns(header);
        let c_from = *col.get("fromSolarSystemID").context("missing fromSolarSystemID")?;
        let c_to = *col.get("toSolarSystemID").context("missing toSolarSystemID")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt = tx.prepare(
                "INSERT OR REPLACE INTO system_jumps (from_system_id, to_system_id) VALUES (?1, ?2)",
            )?;
            for line in lines {
                if line.trim().is_empty() {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                if f.len() <= c_from.max(c_to) {
                    continue;
                }
                let (Ok(from), Ok(to)) = (f[c_from].parse::<i64>(), f[c_to].parse::<i64>()) else {
                    continue;
                };
                stmt.execute(params![from, to])?;
                stmt.execute(params![to, from])?;
                written += 1;
            }
        }
        tx.commit()?;
        Ok(written)
    }

    /// Ingest Fuzzwork's `mapRegions.csv` (region id + name). Returns rows.
    pub fn ingest_regions_csv(&mut self, csv: &str) -> Result<usize> {
        let mut lines = csv.lines();
        let header = lines.next().context("empty regions CSV")?;
        let col = csv_columns(header);
        let c_id = *col.get("regionID").context("missing regionID")?;
        let c_name = *col.get("regionName").context("missing regionName")?;

        let tx = self.conn.transaction()?;
        let mut written = 0usize;
        {
            let mut stmt =
                tx.prepare("INSERT OR REPLACE INTO regions (region_id, name) VALUES (?1, ?2)")?;
            for line in lines {
                if line.trim().is_empty() {
                    continue;
                }
                let f: Vec<&str> = line.split(',').collect();
                if f.len() <= c_id.max(c_name) {
                    continue;
                }
                let Ok(id) = f[c_id].parse::<i64>() else { continue };
                stmt.execute(params![id, f[c_name]])?;
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

/// Map a CSV header row to `column name → index`.
fn csv_columns(header: &str) -> BTreeMap<String, usize> {
    header
        .split(',')
        .enumerate()
        .map(|(i, name)| (name.trim().to_string(), i))
        .collect()
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

    const TYPE_MATERIALS_YAML: &str = r#"
1230:
  materials:
    - materialTypeID: 34
      quantity: 415
1228:
  materials:
    - materialTypeID: 34
      quantity: 346
    - materialTypeID: 35
      quantity: 173
"#;

    const BLUEPRINTS_YAML: &str = r#"
681:
  activities:
    manufacturing:
      materials:
        - typeID: 34
          quantity: 1000
        - typeID: 35
          quantity: 200
      products:
        - typeID: 587
          quantity: 1
      time: 6000
    invention:
      products:
        - typeID: 587
          quantity: 1
          probability: 0.34
      time: 60000
"#;

    #[test]
    fn ingests_type_materials() {
        let mut c = Converter::in_memory().unwrap();
        let n = c.ingest_type_materials(TYPE_MATERIALS_YAML).unwrap();
        assert_eq!(n, 3); // 1 + 2 yield rows

        let qty: i64 = c
            .connection()
            .query_row(
                "SELECT quantity FROM type_materials WHERE type_id = 1230 AND material_type_id = 34",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(qty, 415);
    }

    #[test]
    fn ingests_blueprints_with_activities() {
        let mut c = Converter::in_memory().unwrap();
        // 2 manufacturing materials + 1 product + 1 invention product = 4 rows.
        let n = c.ingest_blueprints(BLUEPRINTS_YAML).unwrap();
        assert_eq!(n, 4);

        let (bp, qty): (i64, i64) = c
            .connection()
            .query_row(
                "SELECT blueprint_type_id, quantity FROM blueprint_products
                 WHERE product_type_id = 587 AND activity = 'manufacturing'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(bp, 681);
        assert_eq!(qty, 1);

        let prob: f64 = c
            .connection()
            .query_row(
                "SELECT probability FROM blueprint_products
                 WHERE blueprint_type_id = 681 AND activity = 'invention'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!((prob - 0.34).abs() < 1e-9);

        let mat: i64 = c
            .connection()
            .query_row(
                "SELECT quantity FROM blueprint_materials
                 WHERE blueprint_type_id = 681 AND activity = 'manufacturing' AND material_type_id = 34",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(mat, 1000);
    }

    #[test]
    fn ingests_skills() {
        let mut c = Converter::in_memory().unwrap();
        let n = c
            .ingest_skills(
                r#"
3300:
  rank: 1
  primaryAttribute: 167
  secondaryAttribute: 168
3327:
  rank: 3
  primaryAttribute: 165
  secondaryAttribute: 166
"#,
            )
            .unwrap();
        assert_eq!(n, 2);
        let (rank, p, s): (i64, i64, i64) = c
            .connection()
            .query_row(
                "SELECT rank, primary_attr, secondary_attr FROM skills WHERE type_id = 3327",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(rank, 3);
        assert_eq!(p, 165);
        assert_eq!(s, 166);
    }

    #[test]
    fn ingests_required_skills() {
        let mut c = Converter::in_memory().unwrap();
        let n = c
            .ingest_required_skills(
                r#"
587:
  skills:
    - skillTypeID: 3331
      level: 1
    - skillTypeID: 3330
      level: 3
"#,
            )
            .unwrap();
        assert_eq!(n, 2);
        let lvl: i64 = c
            .connection()
            .query_row(
                "SELECT level FROM type_required_skills WHERE type_id = 587 AND skill_type_id = 3330",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lvl, 3);
    }

    #[test]
    fn type_dogma_derives_skills_and_requirements() {
        let mut c = Converter::in_memory().unwrap();
        // Type 3300 is a skill (rank 1, primary perception/167, secondary
        // willpower/168). Type 587 (Rifter) requires skill 3327 at level 1.
        let n = c
            .ingest_type_dogma(
                r#"
3300:
  dogmaAttributes:
  - attributeID: 275
    value: 1.0
  - attributeID: 180
    value: 167.0
  - attributeID: 181
    value: 168.0
587:
  dogmaAttributes:
  - attributeID: 182
    value: 3327.0
  - attributeID: 277
    value: 1.0
  - attributeID: 183
    value: 3301.0
  - attributeID: 278
    value: 3.0
"#,
            )
            .unwrap();
        // 1 skill row + 2 required-skill rows.
        assert_eq!(n, 3);

        let (rank, p, s): (i64, i64, i64) = c
            .connection()
            .query_row(
                "SELECT rank, primary_attr, secondary_attr FROM skills WHERE type_id = 3300",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((rank, p, s), (1, 167, 168));

        let lvl: i64 = c
            .connection()
            .query_row(
                "SELECT level FROM type_required_skills WHERE type_id = 587 AND skill_type_id = 3301",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lvl, 3);
    }

    #[test]
    fn ingests_map_csvs() {
        let mut c = Converter::in_memory().unwrap();

        let systems = "regionID,solarSystemID,solarSystemName,x,y,z,security\n\
                       10000002,30000142,Jita,1.0,0,2.0,0.945913\n\
                       10000002,30000144,Perimeter,1.5,0,2.5,0.946";
        assert_eq!(c.ingest_systems_csv(systems).unwrap(), 2);

        let jumps = "fromRegionID,fromSolarSystemID,toSolarSystemID,toRegionID\n\
                     10000002,30000142,30000144,10000002";
        assert_eq!(c.ingest_jumps_csv(jumps).unwrap(), 1);

        let regions = "regionID,regionName\n10000002,The Forge";
        assert_eq!(c.ingest_regions_csv(regions).unwrap(), 1);

        // Coordinates + security landed, and the jump is stored both ways.
        let (name, sec, x): (String, f64, f64) = c
            .connection()
            .query_row(
                "SELECT name, security, x FROM solar_systems WHERE system_id = 30000142",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "Jita");
        assert!((sec - 0.945913).abs() < 1e-6);
        assert!((x - 1.0).abs() < 1e-9);

        let jump_count: i64 = c
            .connection()
            .query_row("SELECT COUNT(*) FROM system_jumps", [], |r| r.get(0))
            .unwrap();
        assert_eq!(jump_count, 2); // both directions
    }

    #[test]
    fn ingests_portion_size() {
        let mut c = Converter::in_memory().unwrap();
        c.ingest_types(
            r#"
1230:
  groupID: 462
  name:
    en: Veldspar
  portionSize: 100
  published: true
"#,
        )
        .unwrap();
        let ps: i64 = c
            .connection()
            .query_row("SELECT portion_size FROM types WHERE type_id = 1230", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ps, 100);
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
