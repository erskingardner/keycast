use std::path::{Path, PathBuf};

use keycast_core::database::{database_path_from_env, migrations_path_from_env, Database};
use keycast_core::v2::control::{ControlRequest, ControlResponse};
use keycast_core::v2::envelope::EnvelopeCipher;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    if let Some(op) = std::env::args()
        .nth(1)
        .filter(|op| op == "healthcheck" || op == "status")
    {
        return signer_healthcheck(op == "status").await;
    }

    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("signer crate must have a repository parent");
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if !arguments.is_empty() {
        return keycast_signer::maintenance::command(&arguments, repository_root).await;
    }
    let public_url = url::Url::parse(
        &std::env::var("KEYCAST_PUBLIC_URL").map_err(|_| "KEYCAST_PUBLIC_URL is required")?,
    )?;
    if !(public_url.scheme() == "https"
        || (public_url.scheme() == "http"
            && matches!(
                public_url.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]")
            )))
        || public_url.path().trim_end_matches('/') != "/api"
        || public_url.query().is_some()
        || public_url.fragment().is_some()
        || !public_url.username().is_empty()
        || public_url.password().is_some()
    {
        return Err(
            "KEYCAST_PUBLIC_URL must be an HTTPS /api URL (HTTP allowed only on loopback)".into(),
        );
    }
    for name in ["ALLOWED_PUBKEYS", "KEYCAST_OPERATOR_PUBKEYS"] {
        let raw = std::env::var(name).unwrap_or_default();
        if !raw.is_empty()
            && raw.split(',').any(|value| {
                value.len() != 64 || nostr::prelude::PublicKey::from_hex(value).is_err()
            })
        {
            return Err(format!("{name} must contain comma-separated 64-character hex pubkeys without whitespace or empty entries").into());
        }
    }
    let db_path = database_path_from_env(repository_root);
    if db_path
        .parent()
        .is_some_and(|p| p.join("RESTORE_INCOMPLETE").exists())
    {
        return Err("incomplete restore cannot start".into());
    }
    let _signer_lock = keycast_signer::maintenance::lock(
        &keycast_signer::maintenance::signer_path(&db_path),
        false,
    )?;
    let _maintenance_lock = keycast_signer::maintenance::lock(
        &keycast_signer::maintenance::maintenance_path(&db_path),
        true,
    )?;
    let database = Database::new(
        database_path_from_env(repository_root),
        migrations_path_from_env(repository_root),
    )
    .await?;
    if !database.integrity_check().await? {
        return Err("database integrity check failed".into());
    }
    let cipher = EnvelopeCipher::load()?;
    keycast_signer::maintenance::verify_root(&keycast_signer::store::Store::new(
        database.pool.clone(),
        cipher.clone(),
    ))
    .await?;
    let socket_path = std::env::var_os("KEYCAST_SIGNER_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/keycast/signer.sock"));

    tracing::info!(
        credential_key_id = cipher.key_id(),
        database = %database_path_from_env(repository_root).display(),
        "Keycast multiplexed signer starting"
    );
    keycast_signer::runtime::run(database, cipher, socket_path, public_url).await?;
    tracing::info!("Keycast signer stopped cleanly");
    Ok(())
}

async fn signer_healthcheck(print: bool) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = std::env::var_os("KEYCAST_SIGNER_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/keycast/signer.sock"));
    let operation = async {
        let mut stream = UnixStream::connect(socket_path).await?;
        stream
            .write_all(&serde_json::to_vec(&ControlRequest::Status)?)
            .await?;
        stream.shutdown().await?;
        let mut response = Vec::new();
        (&mut stream)
            .take(64 * 1024)
            .read_to_end(&mut response)
            .await?;
        match serde_json::from_slice::<ControlResponse>(&response)? {
            ControlResponse::Status { status } if print => {
                println!("{}", serde_json::to_string(&status)?);
                Ok(())
            }
            ControlResponse::Status { status } if status.ready => Ok(()),
            _ => Err(std::io::Error::other("signer is not ready")),
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), operation)
        .await
        .map_err(|_| std::io::Error::other("signer healthcheck timed out"))??;
    Ok(())
}
