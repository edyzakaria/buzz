use thiserror::Error;

/// Errors returned by ISM client operations.
#[derive(Debug, Error)]
pub enum IsmError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization or deserialization failed
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// ISM API returned an error response
    #[error("ISM API error: {0}")]
    ApiError(String),

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Issue not found
    #[error("Issue not found: {id}")]
    NotFound { id: String },

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal error
    #[error("{0}")]
    Internal(String),
}

/// Result type for ISM operations
pub type Result<T> = std::result::Result<T, IsmError>;
