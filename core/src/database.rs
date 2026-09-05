use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::migrate::Migrator;
use sqlx_sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("failed to create database directory: {0}")]
    Fs(#[from] std::io::Error),
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
}

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn new(db_path: PathBuf, migrations_path: PathBuf) -> Result<Self, DatabaseError> {
        if let Some(parent) = db_path.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(sqlx_sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx_sqlite::SqliteSynchronous::Full)
            .pragma("max_page_count", "65536")
            .pragma("journal_size_limit", "16777216")
            .busy_timeout(Duration::from_secs(10));

        let pool = SqlitePoolOptions::new()
            .acquire_timeout(Duration::from_secs(10))
            .max_connections(5)
            .connect_with(options)
            .await?;

        Migrator::new(migrations_path).await?.run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn integrity_check(&self) -> Result<bool, DatabaseError> {
        let result: String = sqlx::query_scalar::query_scalar("PRAGMA quick_check")
            .fetch_one(&self.pool)
            .await?;
        Ok(result == "ok")
    }
}

pub fn database_path_from_env(default_root: &Path) -> PathBuf {
    std::env::var_os("KEYCAST_DATABASE_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.join("database/keycast-v2.db"))
}

pub fn migrations_path_from_env(default_root: &Path) -> PathBuf {
    std::env::var_os("KEYCAST_MIGRATIONS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_root.join("database/migrations"))
}
