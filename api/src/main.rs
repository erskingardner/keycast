mod api;
mod state;

use crate::state::{get_db_pool, KeycastState, KEYCAST_STATE};
use axum::{
    http::{HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use keycast_core::database::Database;
use keycast_core::encryption::file_key_manager::FileKeyManager;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n\n================================================");
    println!("🔑 Keycast API Starting...");

    // Initialize tracing with debug level
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup shutdown signal handler
    tokio::spawn(async {
        match signal::ctrl_c().await {
            Ok(()) => {
                println!("\n\n================================================");
                println!("🫡 Shutdown signal received, cleaning up...");
                println!("✔︎ API shutdown complete");
                if let Ok(pool) = get_db_pool() {
                    pool.close().await;
                }
                println!("✔︎ Database pool closed");
                println!("🤙 Pura Vida!");
                println!("================================================");
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                std::process::exit(1);
            }
        }
    });

    // Set up database
    let root_dir = env!("CARGO_MANIFEST_DIR");
    let database_url = PathBuf::from(root_dir)
        .parent()
        .unwrap()
        .join("database/keycast.db");
    let database_migrations = PathBuf::from(root_dir)
        .parent()
        .unwrap()
        .join("database/migrations");
    let database = Database::new(database_url.clone(), database_migrations.clone()).await?;
    println!("✔︎ Database initialized");

    // Setup basic file based key manager for encryption
    let key_manager = FileKeyManager::new().expect("Failed to create key manager");
    println!("✔︎ Encryption key loaded");

    // Create a shared state with the database and key manager
    let state = Arc::new(KeycastState {
        db: database.pool,
        key_manager: Box::new(key_manager),
    });

    // Set the shared state in the once cell
    KEYCAST_STATE
        .set(state)
        .map_err(|_| "Failed to set KeycastState")?;

    // Start up the API
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/health", get(health_check))
        .nest("/api", api::http::routes(get_db_pool().unwrap().clone()))
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("✔︎ API listening on {}", listener.local_addr().unwrap());
    println!("🤙 Keycast API ready! LFG!");
    println!("================================================");

    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}
