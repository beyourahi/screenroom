//! Stable error codes shared by every client and binding.
//!
//! Clients render [`CoreError::code`]; the message is a human-readable
//! fallback and is not a stable contract. The codes match the code strings the
//! Flutter candidate used, so UI copy and tests survive the port.

/// Result alias used across the core.
pub type Result<T> = std::result::Result<T, CoreError>;

/// A core failure with a stable machine-readable code.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A domain rule rejected the operation. `code` is stable.
    #[error("{message}")]
    Library {
        /// Stable machine-readable code, for example `duplicate_source`.
        code: &'static str,
        /// Human-readable explanation.
        message: String,
    },
    /// SQLite reported an error.
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// Filesystem access failed.
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    /// Stored JSON could not be decoded.
    #[error("stored data is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    /// Builds a domain error from a stable code and message.
    pub fn library(code: &'static str, message: impl Into<String>) -> Self {
        Self::Library {
            code,
            message: message.into(),
        }
    }

    /// Stable machine-readable code for UI copy and binding consumers.
    pub fn code(&self) -> &str {
        match self {
            Self::Library { code, .. } => code,
            Self::Sqlite(_) => "database_error",
            Self::Io(_) => "io_error",
            Self::Json(_) => "invalid_stored_json",
        }
    }

    /// True when the failure is expected while a device or source is offline.
    pub fn is_availability(&self) -> bool {
        matches!(
            self.code(),
            "source_missing" | "source_disconnected" | "permission_lost" | "io_error"
        )
    }
}
