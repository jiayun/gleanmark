use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Qdrant error: {0}")]
    Qdrant(#[from] qdrant_client::QdrantError),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("Bookmark not found: {0}")]
    NotFound(String),

    #[error("Gateway error: {0}")]
    Gateway(String),

    /// The cloud gateway denied a save because a plan limit was hit (HTTP 402
    /// with `code: "quota_exceeded"`). `message` is the gateway's
    /// user-presentable text, so `Display` is already friendly for CLI/UI.
    #[error("{message}")]
    QuotaExceeded {
        message: String,
        used: Option<u64>,
        limit: Option<u64>,
    },

    #[error("Auth error: {0}")]
    Auth(String),

    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Gateway(err.to_string())
    }
}

impl From<anyhow::Error> for Error {
    fn from(err: anyhow::Error) -> Self {
        Error::Embedding(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
