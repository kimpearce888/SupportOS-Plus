use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("migration {version} failed: {message}")]
    Migration { version: u32, message: String },

    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("secret not found: {key}")]
    SecretNotFound { key: String },

    #[error("webhook signature verification failed")]
    WebhookSignature,

    #[error("webhook event duplicated: {event_id}")]
    WebhookDuplicate { event_id: String },

    #[error("oauth state reused or unknown")]
    OauthStateInvalid,

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    Validation(String),

    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

pub type Result<T> = std::result::Result<T, Error>;
