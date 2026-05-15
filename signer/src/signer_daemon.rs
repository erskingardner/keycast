use dotenvy::dotenv;
use keycast_core::encryption::file_key_manager::FileKeyManager;
use keycast_core::encryption::KeyManager;
use keycast_core::traits::AuthorizationValidations;
use keycast_core::types::authorization::Authorization;
use nostr_connect::prelude::*;
use sqlx_sqlite::SqlitePool;
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

struct SignerDaemon<T: AuthorizationValidations> {
    authorization: T,
    pool: SqlitePool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenv().ok();

    // Get configuration from environment
    let auth_id: u32 = env::var("AUTH_ID")?.parse()?;

    // Connect to the main application database
    let database_url = env::var("DATABASE_URL").map_err(|_| "DATABASE_URL not set")?;
    let pool = SqlitePool::connect(&database_url).await?;

    tracing::info!(target: "keycast_signer::signer_daemon", "Starting signing daemon for authorization {:?}", auth_id);

    let authorization = Authorization::find(&pool, auth_id).await?;

    let signer_daemon = SignerDaemon {
        authorization,
        pool,
    };

    // Decrypt the bunker secret
    let key_manager = FileKeyManager::new()?;
    let decrypted_secret_bytes = key_manager
        .decrypt(&signer_daemon.authorization.bunker_secret)
        .await?;
    let signer_secret_key = SecretKey::from_slice(&decrypted_secret_bytes)?;

    // Decrypt the stored key for this authentication
    let stored_key = signer_daemon
        .authorization
        .stored_key(&signer_daemon.pool)
        .await?;

    let decrypted_stored_key_bytes = key_manager.decrypt(&stored_key.secret_key).await?;
    let user_secret_key = SecretKey::from_slice(&decrypted_stored_key_bytes)?;

    let keys = NostrConnectKeys {
        signer: Keys::new(signer_secret_key),
        user: Keys::new(user_secret_key),
    };

    let signer = NostrConnectRemoteSigner::new(
        keys,
        signer_daemon.authorization.relays.clone(),
        Some(signer_daemon.authorization.secret.clone()),
        None,
    )?;

    tracing::info!(target: "keycast_signer::signer_daemon", "Signing daemon for authorization {:?} started", auth_id);
    tracing::info!(target: "keycast_signer::signer_daemon", "Bunker signing URI: {:?}", signer.bunker_uri());

    // Start the signer with custom actions
    signer.serve(SignerActions::new(signer_daemon)).await?;

    Ok(())
}

struct SignerActions<T: AuthorizationValidations> {
    signer_daemon: SignerDaemon<T>,
}

impl<T: AuthorizationValidations> SignerActions<T> {
    fn new(signer_daemon: SignerDaemon<T>) -> Self {
        Self { signer_daemon }
    }
}

impl<T: AuthorizationValidations> NostrConnectSignerActions for SignerActions<T> {
    fn approve(&self, pubkey: &PublicKey, request: &NostrConnectRequest) -> bool {
        tracing::debug!(target: "keycast_signer::signer_daemon", "Evaluating request: {:?}", request);

        // Validate the request against the authorization's policy and permissions
        match self.signer_daemon.authorization.validate_policy(
            &self.signer_daemon.pool,
            pubkey,
            request,
        ) {
            Ok(true) => true,
            Ok(false) => {
                tracing::error!(target: "keycast_signer::signer_daemon", "Authorization does not have the required permissions");
                false
            }
            Err(e) => {
                tracing::error!(target: "keycast_signer::signer_daemon", "Error validating permissions: {:?}", e);
                false
            }
        }
    }
}
