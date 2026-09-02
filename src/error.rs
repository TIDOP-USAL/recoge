// error.rs Custom Error and Result

#![allow(dead_code)]

pub type Result<T> = std::result::Result<T, Error>;

// Custom app Error (using thiserror crate)
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Program error: {0}")]
    Program(String),
    #[error("Remote error: {0}")]
    Remote(RemoteError),
    #[error("Client error: {0}")]
    Client(ClientError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RemoteError {
    #[error("Generic error: {0}")]
    Generic(String),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ClientError {
    #[error("Invalid error: {0}")]
    Invalid(String),
    #[error("Conflict error: {0}")]
    Conflict(String),
    #[error("Not found error: {0}")]
    NotFound(String),
    #[error("Unauthorized error: {0}")]
    Unauthorized(String),
}

impl RemoteError {
    #[must_use]
    pub fn err(&self) -> Error {
        Error::Remote(self.clone())
    }
}

impl ClientError {
    #[must_use]
    pub fn err(&self) -> Error {
        Error::Client(self.clone())
    }
}

// reqwest error to Program Error
impl From<reqwest::Error> for Error {
    fn from(reqwest_e: reqwest::Error) -> Error {
        if reqwest_e.is_connect() {
            return RemoteError::Generic(format!("API HTTP connection error: {reqwest_e:?}")).err();
        }
        Error::Program(format!("API HTTP client error: {reqwest_e:?}"))
    }
}
