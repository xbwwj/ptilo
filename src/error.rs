use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Error, Eq)]
#[error("{message}")]
pub struct Error {
    pub message: String,
}

impl Error {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

pub type Result<T> = ::core::result::Result<T, Error>;
