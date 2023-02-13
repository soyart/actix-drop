use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize)]
#[error("clipboard store error")]
pub enum StoreError {
    NotImplemented(String),
    Bug(String),
    #[serde(skip)]
    IoError(#[from] std::io::Error),
    #[serde(skip)]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

// Do not send IO error to clients
pub fn public_error(err: StoreError) -> Option<StoreError> {
    match err {
        StoreError::IoError(_) => None,
        _ => Some(err),
    }
}
