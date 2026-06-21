//! Application settings (key/value) persistence.

use sqlx::Row;

use crate::error::Result;

use super::Database;

impl Database {
    /// Read a setting value, or `None` if unset.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT value FROM settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(&self.app)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    /// Read a setting value or fall back to `default`.
    pub async fn get_setting_or(&self, key: &str, default: &str) -> Result<String> {
        Ok(self.get_setting(key).await?.unwrap_or_else(|| default.to_string()))
    }

    /// Upsert a setting value.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.app)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn settings_roundtrip_and_default() {
        let db = Database::open_in_memory().await.unwrap();
        assert_eq!(db.get_setting("intensity").await.unwrap(), None);
        assert_eq!(db.get_setting_or("intensity", "Balanced").await.unwrap(), "Balanced");

        db.set_setting("intensity", "Aggressive").await.unwrap();
        assert_eq!(db.get_setting("intensity").await.unwrap().as_deref(), Some("Aggressive"));

        // Upsert overwrites.
        db.set_setting("intensity", "Light").await.unwrap();
        assert_eq!(db.get_setting_or("intensity", "Balanced").await.unwrap(), "Light");
    }
}
