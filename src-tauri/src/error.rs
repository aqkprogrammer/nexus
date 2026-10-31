use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

pub type Result<T> = std::result::Result<T, NexusError>;

#[derive(Debug, thiserror::Error)]
pub enum NexusError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    /// A safety rule refused the operation. Never retried automatically.
    #[error("{0}")]
    Safety(String),
    #[error("{0}")]
    Permission(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Busy(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("cancelled")]
    Cancelled,
}

impl NexusError {
    fn kind(&self) -> &'static str {
        match self {
            NexusError::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => "permission",
            NexusError::Io(_) => "io",
            NexusError::Db(_) => "database",
            NexusError::Json(_) => "serialization",
            NexusError::Safety(_) => "safety",
            NexusError::Permission(_) => "permission",
            NexusError::NotFound(_) => "not_found",
            NexusError::Invalid(_) => "invalid",
            NexusError::Busy(_) => "busy",
            NexusError::Unavailable(_) => "unavailable",
            NexusError::Cancelled => "cancelled",
        }
    }
}

impl Serialize for NexusError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("NexusError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}
