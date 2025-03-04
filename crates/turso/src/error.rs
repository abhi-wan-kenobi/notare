#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("failed to generate token: {0}")]
    GenerateTokenError(String),
    #[error("failed to create database: {0}")]
    CreateDatabaseError(String),
    #[error("failed to retrieve database: {0}")]
    RetrieveDatabaseError(String),
    #[error("failed to delete database: {0}")]
    DeleteDatabaseError(String),
}
