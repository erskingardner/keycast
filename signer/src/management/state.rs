use crate::runtime::RuntimeState;
use keycast_core::v2::control::{ControlResponse, LifecycleRequest};
use sqlx_sqlite::SqlitePool;

#[derive(Clone)]
pub struct KeycastState {
    pub db: SqlitePool,
    pub signer: SignerClient,
}
#[derive(Clone)]
pub struct SignerClient {
    pub runtime: RuntimeState,
}
#[derive(Debug, thiserror::Error)]
pub enum SignerClientError {
    #[error("signer unavailable")]
    Unavailable,
    #[error("invalid response")]
    InvalidResponse,
    #[error("operation rejected")]
    Rejected { code: String, message: String },
}
impl SignerClient {
    pub async fn request(
        &self,
        request: &LifecycleRequest,
    ) -> Result<ControlResponse, SignerClientError> {
        match crate::control::dispatch_lifecycle(&self.runtime, request.clone()).await {
            ControlResponse::Error { code, message } => {
                Err(SignerClientError::Rejected { code, message })
            }
            response => Ok(response),
        }
    }
}
