use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, Serialize, Deserialize)]
#[error("clipboard store error")]
pub enum StoreError {
    #[error("not implemented")]
    NotImplemented(String),

    #[error("actix-drop bug")]
    Bug(String),

    #[error("empty clipboard sent")]
    Empty,

    #[serde(skip)]
    #[error("io error")]
    IoError(#[from] std::io::Error),

    #[serde(skip)]
    #[error("bad utf-8")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}

// Do not send IO error to clients
pub fn public_error(err: StoreError) -> Option<StoreError> {
    match err {
        StoreError::IoError(_) => None,
        _ => Some(err),
    }
}
