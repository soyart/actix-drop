use thiserror::Error;

#[derive(Error, Debug)]
#[error("clipboard store error")]
pub enum StoreError {
    NotImplemented(String),
    Bug(String),
    IoError(#[from] std::io::Error),
    InvalidUtf8(#[from] std::string::FromUtf8Error),
}
