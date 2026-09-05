//! Trusted host operations. Secrets are accepted from private files or stdin, never argv.
use crate::store::Store;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use keycast_core::{
    database::{database_path_from_env, migrations_path_from_env, Database},
    v2::envelope::{EnvelopeCipher, EnvelopeContext},
};
use sqlx::{query::query, query_as::query_as, query_scalar::query_scalar};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
/// Separate single-signer and maintenance locks allow a consistent online backup.
pub fn lock(path: &Path, shared: bool) -> Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    if shared {
        file.try_lock_shared()?;
    } else {
        file.try_lock()?;
    }
    Ok(file)
}
pub fn maintenance_path(db: &Path) -> PathBuf {
    db.with_extension("maintenance.lock")
}
pub fn signer_path(db: &Path) -> PathBuf {
    db.with_extension("signer.lock")
}
fn private_write(path: &Path, data: &[u8]) -> Result<()> {
    private_write_with(path, |file| {
        file.write_all(data)?;
        Ok(())
    })
}
fn private_write_with(path: &Path, write: impl FnOnce(&mut File) -> Result<()>) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let temporary = parent.join(format!(
        ".keycast-write-{}",
        nostr::prelude::Keys::generate().public_key()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)?;
        write(&mut file)?;
        file.sync_all()?;
        // Atomic publication with no overwrite, including if the destination is a symlink.
        std::fs::hard_link(&temporary, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&temporary);
    result
}

pub async fn verify_root(store: &Store) -> Result<()> {
    let expected: Option<String> =
        query_scalar("SELECT root_key_id FROM instance_settings WHERE singleton=1")
            .fetch_one(&store.pool)
            .await?;
    let mismatches:i64=query_scalar("SELECT (SELECT count(*) FROM stored_keys WHERE key_encryption_key_id<>?)+(SELECT count(*) FROM grants WHERE key_encryption_key_id<>?)")
        .bind(store.cipher.key_id()).bind(store.cipher.key_id()).fetch_one(&store.pool).await?;
    if expected
        .as_deref()
        .is_some_and(|id| id != store.cipher.key_id())
        || (expected.is_none() && mismatches != 0)
    {
        return Err("root credential does not match this database".into());
    }
    query("UPDATE instance_settings SET root_key_id=? WHERE singleton=1 AND root_key_id IS NULL")
        .bind(store.cipher.key_id())
        .execute(&store.pool)
        .await?;
    Ok(())
}

pub async fn command(args: &[String], root: &Path) -> Result<()> {
    let op = args.first().map(String::as_str).unwrap_or("help");
    if op == "help" {
        println!("Trusted host commands:\n  status                                         (read daemon indicators over socket)\n  audit-export NEW_FILE                          (redacted JSONL, private file)\n  generate-key NEW_PRIVATE_FILE\n  import TEAM_ID ACTOR_HEX NAME < private-key.txt  (stop signer first)\n  backup BACKUP_KEY_FILE NEW_BACKUP_FILE          (online SQLite snapshot)\n  restore BACKUP_KEY_FILE BACKUP_FILE NEW_DIRECTORY\n  rotate-root NEW_ROOT_KEY_FILE                  (stop signer first)\n  review-restore                                (stop signer; review administrators and policies first)\nDatabase/root paths use KEYCAST_DATABASE_PATH and KEYCAST_ROOT_KEY_FILE. No existing file is overwritten.");
        return Ok(());
    }
    if op == "generate-key" && args.len() == 2 {
        let mut bytes = Zeroizing::new([0u8; 32]);
        getrandom::fill(&mut *bytes)?;
        let encoded = Zeroizing::new(BASE64.encode(*bytes));
        private_write(Path::new(&args[1]), encoded.as_bytes())?;
        return Ok(());
    }
    if op == "restore" && args.len() == 4 {
        return restore(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
            root,
        )
        .await;
    }
    let db_path = database_path_from_env(root);
    if !db_path.is_file() {
        return Err("existing database required".into());
    }
    let _maintenance = lock(
        &maintenance_path(&db_path),
        op == "backup" || op == "audit-export",
    )?;
    let db = Database::new(db_path.clone(), migrations_path_from_env(root)).await?;
    let cipher = EnvelopeCipher::load()?;
    let store = Store::new(db.pool.clone(), cipher);
    verify_root(&store).await?;
    match op {
        "import" if args.len() == 4 => {
            let team: i64 = args[1].parse()?;
            let actor = nostr::prelude::PublicKey::from_hex(&args[2])?.to_hex();
            let admin:bool=query_scalar("SELECT EXISTS(SELECT 1 FROM team_members WHERE team_id=? AND user_public_key=? AND role='admin')").bind(team).bind(&actor).fetch_one(&db.pool).await?;
            if !admin {
                return Err("actor must be a current team administrator".into());
            }
            let mut secret = Zeroizing::new(String::new());
            std::io::stdin().take(4096).read_to_string(&mut secret)?;
            let key = store
                .seal_stored_key(
                    team,
                    &actor,
                    args[3].clone(),
                    Zeroizing::new(secret.trim().to_owned()),
                )
                .await?;
            println!("Imported public key {}", key.public_key);
        }
        "audit-export" if args.len() == 2 => {
            // Intentionally omit details/payloads/ciphertext, even if database contents are malformed.
            let rows:Vec<String>=query_scalar("SELECT json_object('id',id,'occurred_at',occurred_at,'team_id',team_id,'stored_key_id',stored_key_id,'grant_id',grant_id,'session_id',session_id,'actor_public_key',actor_public_key,'action',action,'outcome',outcome,'reason_code',reason_code,'request_event_id',request_event_id) FROM audit_events ORDER BY id").fetch_all(&db.pool).await?;
            private_write_with(Path::new(&args[1]), |file| {
                for row in rows {
                    file.write_all(row.as_bytes())?;
                    file.write_all(b"\n")?;
                }
                Ok(())
            })?;
        }
        "backup" if args.len() == 3 => {
            let backup_cipher = EnvelopeCipher::from_file(Path::new(&args[1]))?;
            let root_path = keycast_core::v2::envelope::credential_path()
                .ok_or("root credential path required")?;
            let private_root = Zeroizing::new(std::fs::read_to_string(root_path)?);
            let temporary = db_path.with_extension(format!(
                "snapshot-{}",
                nostr::prelude::Keys::generate().public_key()
            ));
            // VACUUM INTO creates a transactionally consistent copy, including WAL state.
            private_write(&temporary, &[])?;
            let snapshot = async {
                query("VACUUM INTO ?")
                    .bind(temporary.to_str().ok_or("invalid snapshot path")?)
                    .execute(&db.pool)
                    .await?;
                let length = std::fs::metadata(&temporary)?.len();
                private_write_with(Path::new(&args[2]), |output| {
                    crate::backup::seal(
                        &backup_cipher,
                        private_root.trim(),
                        length,
                        &mut File::open(&temporary)?,
                        output,
                    )
                })
            }
            .await;
            let _ = std::fs::remove_file(temporary);
            snapshot?;
            query("UPDATE instance_settings SET last_backup_at=unixepoch() WHERE singleton=1")
                .execute(&db.pool)
                .await?;
        }
        "rotate-root" if args.len() == 2 => {
            rotate(&store, EnvelopeCipher::from_file(Path::new(&args[1]))?).await?;
            println!("Rotation committed. Configure KEYCAST_ROOT_KEY_FILE to the new file before restarting. Keep both credentials until a new backup has been verified.");
        }
        "review-restore" if args.len() == 1 => {
            query("UPDATE instance_settings SET recovery_pending=0,authority_revision=authority_revision+1 WHERE singleton=1").execute(&db.pool).await?;
            query("INSERT INTO audit_events(action,outcome) VALUES('recovery.review','succeeded')")
                .execute(&db.pool)
                .await?;
            println!("Recovery review recorded. Previous grants remain revoked; create new grants and invitations.");
        }
        _ => return Err("invalid command arguments; run help".into()),
    }
    db.pool.close().await;
    Ok(())
}
async fn rotate(store: &Store, next: EnvelopeCipher) -> Result<()> {
    if next.key_id() == store.cipher.key_id() {
        return Err("new root credential must be different".into());
    }
    let mut tx = store.pool.begin().await?;
    // All active and revoked records are rewrapped in one SQLite transaction.
    for (table, purpose) in [
        ("stored_keys", "stored-key"),
        ("grants", "remote-signer-key"),
    ] {
        let rows: Vec<(i64, i64, String, Vec<u8>)> = query_as(if table == "stored_keys" {
            "SELECT id,team_id,public_key,secret_envelope FROM stored_keys"
        } else {
            "SELECT id,team_id,remote_signer_public_key,remote_signer_secret_envelope FROM grants"
        })
        .fetch_all(&mut *tx)
        .await?;
        for (id, team, public, envelope) in rows {
            let context = EnvelopeContext {
                team_id: team,
                record_id: id,
                public_key: &public,
                purpose,
            };
            let plain = store.cipher.open(&envelope, &context)?;
            let sealed = next.seal(&plain, &context)?;
            query(if table=="stored_keys" {"UPDATE stored_keys SET secret_envelope=?,key_encryption_key_id=? WHERE id=?"} else {"UPDATE grants SET remote_signer_secret_envelope=?,key_encryption_key_id=? WHERE id=?"})
                .bind(sealed).bind(next.key_id()).bind(id).execute(&mut *tx).await?;
        }
    }
    query("UPDATE instance_settings SET root_key_id=?,authority_revision=authority_revision+1 WHERE singleton=1")
        .bind(next.key_id())
        .execute(&mut *tx)
        .await?;
    query("INSERT INTO audit_events(action,outcome) VALUES('root.rotate','succeeded')")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
async fn restore(key: &Path, archive: &Path, directory: &Path, root: &Path) -> Result<()> {
    let cipher = EnvelopeCipher::from_file(key)?;
    let mut input = File::open(archive)?;
    let manifest = crate::backup::manifest(&cipher, &mut input)?;
    // Fresh directory only; a failed restore is never mistaken for a usable instance.
    std::fs::create_dir(directory)?;
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))?;
    private_write(
        &directory.join("RESTORE_INCOMPLETE"),
        b"Do not start this database",
    )?;
    private_write(&directory.join("root.key"), manifest.root.as_bytes())?;
    let path = directory.join("keycast-v2.db");
    private_write_with(&path, |output| {
        crate::backup::open(&cipher, &manifest, &mut input, output)
    })?;
    let db = Database::new(path, migrations_path_from_env(root)).await?;
    if !db.integrity_check().await? {
        return Err("restored database integrity check failed".into());
    }
    verify_root(&Store::new(
        db.pool.clone(),
        EnvelopeCipher::from_file(&directory.join("root.key"))?,
    ))
    .await?;
    let mut tx = db.pool.begin().await?;
    query("UPDATE sessions SET ended_at=unixepoch(),end_reason='revoked' WHERE ended_at IS NULL")
        .execute(&mut *tx)
        .await?;
    query("UPDATE invitations SET revoked_at=unixepoch() WHERE consumed_at IS NULL AND revoked_at IS NULL").execute(&mut *tx).await?;
    query("UPDATE grants SET revoked_at=unixepoch() WHERE revoked_at IS NULL")
        .execute(&mut *tx)
        .await?;
    query("DELETE FROM processed_requests")
        .execute(&mut *tx)
        .await?;
    query("DELETE FROM management_nonces")
        .execute(&mut *tx)
        .await?;
    query("UPDATE instance_settings SET recovery_pending=1,instance_id=lower(hex(randomblob(32))),authority_revision=authority_revision+1 WHERE singleton=1").execute(&mut *tx).await?;
    query("INSERT INTO audit_events(action,outcome) VALUES('recovery.restore','succeeded')")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    db.pool.close().await;
    std::fs::remove_file(directory.join("RESTORE_INCOMPLETE"))?;
    println!("Restored with signing disabled and old grants revoked. Review team administrators, policies, relays and host allowlists, then run review-restore against the new database.");
    Ok(())
}
