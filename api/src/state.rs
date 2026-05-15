use keycast_core::encryption::KeyManager;
use once_cell::sync::OnceCell;
use sqlx_sqlite::SqlitePool;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Database not initialized")]
    DatabaseNotInitialized,
    #[error("Key manager not initialized")]
    KeyManagerNotInitialized,
}

pub struct KeycastState {
    pub db: SqlitePool,
    pub key_manager: Box<dyn KeyManager>,
}

pub static KEYCAST_STATE: OnceCell<Arc<KeycastState>> = OnceCell::new();

pub fn get_db_pool() -> Result<&'static SqlitePool, StateError> {
    KEYCAST_STATE
        .get()
        .map(|state| &state.db)
        .ok_or(StateError::DatabaseNotInitialized)
}

pub fn get_key_manager() -> Result<&'static dyn KeyManager, StateError> {
    KEYCAST_STATE
        .get()
        .map(|state| state.key_manager.as_ref())
        .ok_or(StateError::KeyManagerNotInitialized)
}
