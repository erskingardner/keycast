use std::path::PathBuf;
use std::time::Duration;

use keycast_core::v2::control::{ControlRequest, ControlResponse};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

const MAX_CONTROL_RESPONSE_BYTES: u64 = keycast_core::v2::management::MAX_CONTROL_BYTES;

#[derive(Clone)]
pub struct KeycastState {
    pub signer: SignerClient,
}

#[derive(Clone)]
pub struct SignerClient {
    socket_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum SignerClientError {
    #[error("signer is unavailable")]
    Unavailable,
    #[error("signer returned an invalid response")]
    InvalidResponse,
    #[error("signer rejected the operation ({code}): {message}")]
    Rejected { code: String, message: String },
}

impl SignerClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self { socket_path }
    }

    pub async fn request(
        &self,
        request: &ControlRequest,
    ) -> Result<ControlResponse, SignerClientError> {
        let operation = async {
            let mut stream = UnixStream::connect(&self.socket_path)
                .await
                .map_err(|_| SignerClientError::Unavailable)?;
            let bytes =
                serde_json::to_vec(request).map_err(|_| SignerClientError::InvalidResponse)?;
            stream
                .write_all(&bytes)
                .await
                .map_err(|_| SignerClientError::Unavailable)?;
            stream
                .shutdown()
                .await
                .map_err(|_| SignerClientError::Unavailable)?;
            let mut response = Vec::new();
            (&mut stream)
                .take(MAX_CONTROL_RESPONSE_BYTES + 1)
                .read_to_end(&mut response)
                .await
                .map_err(|_| SignerClientError::Unavailable)?;
            if response.len() as u64 > MAX_CONTROL_RESPONSE_BYTES {
                return Err(SignerClientError::InvalidResponse);
            }
            let response: ControlResponse = serde_json::from_slice(&response)
                .map_err(|_| SignerClientError::InvalidResponse)?;
            match response {
                ControlResponse::Error { code, message } => {
                    Err(SignerClientError::Rejected { code, message })
                }
                response => Ok(response),
            }
        };

        tokio::time::timeout(Duration::from_secs(10), operation)
            .await
            .map_err(|_| SignerClientError::Unavailable)?
    }
}
