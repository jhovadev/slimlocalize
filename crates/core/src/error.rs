use std::path::PathBuf;

/// Typed domain error shared by every adapter.
///
/// Adapters translate this error into their own protocol shape (CLI exit
/// codes, Tauri error payloads, HTTP problem responses) without leaking
/// storage internals or secrets.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("invalid {field}: {message}")]
    Validation { field: String, message: String },

    #[error("{entity} not found: {reference}")]
    NotFound { entity: String, reference: String },

    #[error("{entity} conflict on {field}: {message}")]
    Conflict {
        entity: String,
        field: String,
        message: String,
    },

    #[error("unsupported operation: {feature}")]
    Unsupported { feature: String },

    #[error("path escapes the allowed root: {path}")]
    PathTraversal { path: PathBuf },

    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid JSON in {context}: {source}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("storage error: {message}")]
    Storage { message: String },

    #[error("{context} exceeds the limit of {limit}")]
    LimitExceeded { context: String, limit: u64 },

    #[error("internal error: {message}")]
    Internal { message: String },
}

impl DomainError {
    #[must_use]
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn not_found(entity: impl Into<String>, reference: impl Into<String>) -> Self {
        Self::NotFound {
            entity: entity.into(),
            reference: reference.into(),
        }
    }

    #[must_use]
    pub fn conflict(
        entity: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::Conflict {
            entity: entity.into(),
            field: field.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn storage(message: impl Into<String>) -> Self {
        Self::Storage {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Stable machine-readable error code for API consumers.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation { .. } => "validation_error",
            Self::NotFound { .. } => "not_found",
            Self::Conflict { .. } => "conflict",
            Self::Unsupported { .. } => "unsupported",
            Self::PathTraversal { .. } => "path_traversal",
            Self::Io { .. } => "io_error",
            Self::Json { .. } => "invalid_json",
            Self::Storage { .. } => "storage_error",
            Self::LimitExceeded { .. } => "limit_exceeded",
            Self::Internal { .. } => "internal_error",
        }
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(source: serde_json::Error) -> Self {
        Self::Json {
            context: "payload".to_owned(),
            source,
        }
    }
}

pub type Result<T> = std::result::Result<T, DomainError>;
