//! Persistent id→name cache (ESI `/universe/names/` results).

use std::collections::HashMap;

use sqlx::Row;

use crate::error::Result;

use super::Database;

impl Database {
    /// Look up cached names for the given ids. Missing ids are simply absent
    /// from the returned map.
    pub async fn cached_names(&self, ids: &[i64]) -> Result<HashMap<i64, String>> {
        let mut out = HashMap::new();
        if ids.is_empty() {
            return Ok(out);
        }
        // SQLite has no array binding; build a placeholder list. Ids are i64 from
        // ESI, so there's no injection surface, but we still bind them.
        let placeholders = std::iter::repeat("?").take(ids.len()).collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id, name FROM names WHERE id IN ({placeholders})");
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(&self.app).await?;
        for row in rows {
            out.insert(row.get::<i64, _>("id"), row.get::<String, _>("name"));
        }
        Ok(out)
    }

    /// Upsert resolved names into the cache.
    pub async fn cache_names(&self, entries: &[(i64, String, Option<String>)]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut tx = self.app.begin().await?;
        for (id, name, category) in entries {
            sqlx::query(
                "INSERT INTO names (id, name, category) VALUES (?1, ?2, ?3)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, category = excluded.category",
            )
            .bind(id)
            .bind(name)
            .bind(category)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn names_cache_roundtrip() {
        let db = Database::open_in_memory().await.unwrap();
        // Empty lookups are fine.
        assert!(db.cached_names(&[]).await.unwrap().is_empty());
        assert!(db.cached_names(&[34]).await.unwrap().is_empty());

        db.cache_names(&[
            (34, "Tritanium".into(), Some("inventory_type".into())),
            (587, "Rifter".into(), Some("inventory_type".into())),
        ])
        .await
        .unwrap();

        let got = db.cached_names(&[34, 587, 999]).await.unwrap();
        assert_eq!(got.get(&34).map(String::as_str), Some("Tritanium"));
        assert_eq!(got.get(&587).map(String::as_str), Some("Rifter"));
        assert!(!got.contains_key(&999)); // uncached id absent

        // Upsert updates the name.
        db.cache_names(&[(34, "Tritanium (updated)".into(), None)]).await.unwrap();
        assert_eq!(
            db.cached_names(&[34]).await.unwrap().get(&34).map(String::as_str),
            Some("Tritanium (updated)")
        );
    }
}
