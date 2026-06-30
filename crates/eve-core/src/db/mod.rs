//! The durable user-data store (`app.sqlite`) via `sqlx`.
//!
//! Uses runtime queries (not the compile-time `query!` macros) so the crate
//! builds with no `DATABASE_URL` or offline metadata. WAL mode is enabled for
//! concurrent reads during writes.

pub mod abyss;
pub mod ai_memory;
pub mod characters;
pub mod library;
pub mod names;
pub mod portability;
pub mod recruit;
pub mod settings;
pub mod signatures;
pub mod snapshots;
pub mod srp;
pub mod telemetry;
pub mod timers;

use std::path::Path;
use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::error::Result;

/// Handle to the application database.
#[derive(Clone)]
pub struct Database {
    pub app: SqlitePool,
}

impl Database {
    /// Open (creating if needed) the app database at `path` and run migrations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(path.as_ref())
            .create_if_missing(true)
            .foreign_keys(true);
        let app = SqlitePoolOptions::new().max_connections(5).connect_with(opts).await?;
        let db = Self { app };
        db.migrate().await?;
        Ok(db)
    }

    /// Open an in-memory database (single shared connection) — used in tests.
    pub async fn open_in_memory() -> Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        // A single connection so the in-memory DB persists across queries.
        let app = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
        let db = Self { app };
        db.migrate().await?;
        Ok(db)
    }

    /// Apply the idempotent schema. `raw_sql` lets SQLite parse the multi-
    /// statement script (and its comments) correctly.
    async fn migrate(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql")).execute(&self.app).await?;
        Ok(())
    }
}
