use sqlx::migrate::MigrateDatabase;
use sqlx_sqlite::SqlitePoolOptions;
use sqlx_sqlite::{Sqlite, SqlitePool};
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database not initialized")]
    NotInitialized,
    #[error("FS error: {0}")]
    FsError(#[from] std::io::Error),
    #[error("SQLx error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Migrate error: {0}")]
    MigrateError(#[from] sqlx::migrate::MigrateError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::permission::Permission;
    use serde_json::json;
    use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar};
    use std::fs;
    use uuid::Uuid;

    fn temp_test_root() -> PathBuf {
        std::env::temp_dir().join(format!("keycast-db-test-{}", Uuid::new_v4()))
    }

    fn copy_migration(migrations_dir: &std::path::Path, file_name: &str) {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("core has a repo parent")
            .to_path_buf();
        let source = repo_root.join("database/migrations").join(file_name);
        let destination = migrations_dir.join(file_name);
        fs::copy(source, destination).expect("copy migration fixture");
    }

    async fn seed_existing_install(db_path: &std::path::Path, migrations_dir: &std::path::Path) {
        copy_migration(migrations_dir, "0001_initial.sql");
        let database = Database::new(db_path.to_path_buf(), migrations_dir.to_path_buf())
            .await
            .expect("initial migration runs");

        let pool = &database.pool;
        let team_id: i64 = query_scalar(
            "INSERT INTO teams (name, created_at, updated_at)
             VALUES (?, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind("Existing Team")
        .fetch_one(pool)
        .await
        .expect("seed team");

        let policy_id: i64 = query_scalar(
            "INSERT INTO policies (name, team_id, created_at, updated_at)
             VALUES (?, ?, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind("Existing Policy")
        .bind(team_id)
        .fetch_one(pool)
        .await
        .expect("seed policy");

        let old_permission_id: i64 = query_scalar(
            "INSERT INTO permissions (identifier, config, created_at, updated_at)
             VALUES (?, ?, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind("allowed_kinds")
        .bind(json!({ "sign": [1, 7], "encrypt": null, "decrypt": null }))
        .fetch_one(pool)
        .await
        .expect("seed old permission shape");

        let current_permission_id: i64 = query_scalar(
            "INSERT INTO permissions (identifier, config, created_at, updated_at)
             VALUES (?, ?, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind("allowed_kinds")
        .bind(json!({ "allowed_kinds": [9735] }))
        .fetch_one(pool)
        .await
        .expect("seed current permission shape");

        query(
            "INSERT INTO policy_permissions (policy_id, permission_id, created_at, updated_at)
             VALUES (?, ?, datetime('now'), datetime('now')),
                    (?, ?, datetime('now'), datetime('now'))",
        )
        .bind(policy_id)
        .bind(old_permission_id)
        .bind(policy_id)
        .bind(current_permission_id)
        .execute(pool)
        .await
        .expect("seed policy permissions");

        let key_id: i64 = query_scalar(
            "INSERT INTO stored_keys (team_id, name, public_key, secret_key, created_at, updated_at)
             VALUES (?, ?, ?, ?, datetime('now'), datetime('now'))
             RETURNING id",
        )
        .bind(team_id)
        .bind("existing signing key")
        .bind("existing-public-key")
        .bind(vec![1_u8, 2, 3, 4])
        .fetch_one(pool)
        .await
        .expect("seed stored key");

        query(
            "INSERT INTO authorizations (
                stored_key_id,
                secret,
                bunker_public_key,
                bunker_secret,
                relays,
                policy_id,
                max_uses,
                expires_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))",
        )
        .bind(key_id)
        .bind("existing-secret")
        .bind("existing-bunker-pubkey")
        .bind(vec![5_u8, 6, 7])
        .bind(serde_json::to_string(&vec!["wss://relay.example"]).unwrap())
        .bind(policy_id)
        .bind(10_i64)
        .bind(Option::<String>::None)
        .execute(pool)
        .await
        .expect("seed authorization");

        database.pool.close().await;
    }

    #[tokio::test]
    async fn database_new_migrates_existing_allowed_kinds_rows_and_preserves_data() {
        let root = temp_test_root();
        let migrations_dir = root.join("migrations");
        fs::create_dir_all(&migrations_dir).expect("create migrations dir");
        let db_path = root.join("keycast.db");

        seed_existing_install(&db_path, &migrations_dir).await;
        copy_migration(
            &migrations_dir,
            "0002_normalize_allowed_kinds_permissions.sql",
        );
        copy_migration(&migrations_dir, "0003_add_authorization_name.sql");

        let database = Database::new(db_path.clone(), migrations_dir.clone())
            .await
            .expect("upgrade migrations run");
        let pool = &database.pool;

        let migration_versions: Vec<i64> =
            query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(pool)
                .await
                .expect("read migration versions");
        assert_eq!(migration_versions, vec![1, 2, 3]);

        let migrated_config: serde_json::Value = query_scalar(
            "SELECT config FROM permissions
             WHERE identifier = 'allowed_kinds'
               AND json_type(config, '$.sign') IS NULL
             ORDER BY id
             LIMIT 1",
        )
        .fetch_one(pool)
        .await
        .expect("read migrated config");
        assert_eq!(migrated_config, json!({ "allowed_kinds": [1, 7] }));
        Permission::validate_config("allowed_kinds", &migrated_config)
            .expect("migrated allowed_kinds config remains valid");

        let current_shape_config: serde_json::Value = query_scalar(
            "SELECT config FROM permissions
             WHERE identifier = 'allowed_kinds'
               AND json_extract(config, '$.allowed_kinds[0]') = 9735",
        )
        .fetch_one(pool)
        .await
        .expect("read current-shape config");
        assert_eq!(current_shape_config, json!({ "allowed_kinds": [9735] }));

        let existing_authorization: (String, i64, String, Option<String>) = query_as(
            "SELECT secret, max_uses, bunker_public_key, name FROM authorizations WHERE secret = ?",
        )
        .bind("existing-secret")
        .fetch_one(pool)
        .await
        .expect("existing authorization survives migration");
        assert_eq!(
            existing_authorization,
            (
                "existing-secret".to_string(),
                10,
                "existing-bunker-pubkey".to_string(),
                None
            )
        );

        let foreign_keys_enabled: i64 = query_scalar("PRAGMA foreign_keys")
            .fetch_one(pool)
            .await
            .expect("read foreign key pragma");
        assert_eq!(foreign_keys_enabled, 1);

        database.pool.close().await;

        let database = Database::new(db_path, migrations_dir)
            .await
            .expect("upgrade migration is idempotent");
        let migration_versions: Vec<i64> =
            query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&database.pool)
                .await
                .expect("read migration versions after second open");
        assert_eq!(migration_versions, vec![1, 2, 3]);
        database.pool.close().await;

        let _ = fs::remove_dir_all(root);
    }
}

#[derive(Clone)]
pub struct Database {
    pub pool: SqlitePool,
}

impl Database {
    pub async fn new(db_path: PathBuf, migrations_path: PathBuf) -> Result<Self, DatabaseError> {
        let db_url = format!("{}", db_path.display());

        // Create database if it doesn't exist
        eprintln!("Checking if DB exists...{:?}", db_url);
        if Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            eprintln!("DB exists");
        } else {
            eprintln!("DB does not exist, creating...");
            match Sqlite::create_database(&db_url).await {
                Ok(_) => {
                    eprintln!("DB created");
                }
                Err(e) => {
                    eprintln!("Error creating DB: {:?}", e);
                }
            }
        }

        // Create connection pool with more robust settings
        eprintln!("Creating connection pool...");
        let pool = SqlitePoolOptions::new()
            .acquire_timeout(Duration::from_secs(10)) // Increased timeout
            .max_connections(5)
            .after_connect(|conn, _| {
                Box::pin(async move {
                    let conn = &mut *conn;
                    sqlx::query::query("PRAGMA journal_mode=WAL")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query::query("PRAGMA foreign_keys=ON")
                        .execute(&mut *conn)
                        .await?;
                    sqlx::query::query("PRAGMA busy_timeout=10000")
                        .execute(&mut *conn)
                        .await?;
                    Ok(())
                })
            })
            .connect(&format!("{}?mode=rwc", db_url))
            .await?;

        // Run migrations
        eprintln!("Running migrations...");
        let mut attempts = 0;
        while attempts < 3 {
            match sqlx::migrate::Migrator::new(migrations_path.clone())
                .await?
                .run(&pool)
                .await
            {
                Ok(_) => break,
                Err(_e) if attempts < 2 => {
                    sleep(Duration::from_millis(500)).await;
                    attempts += 1;
                }
                Err(e) => return Err(e.into()),
            }
        }

        Ok(Self { pool })
    }
}
