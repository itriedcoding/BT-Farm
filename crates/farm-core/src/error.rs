use serde::Serialize;

pub type Result<T> = std::result::Result<T, CoreError>;

/// Every failure surfaced to the UI goes through this type so the frontend can
/// distinguish "you have not configured this" from "the provider said no".
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{provider} is not configured: {message}")]
    NotConfigured {
        provider: String,
        message: String,
    },

    #[error("{provider} does not support {capability}")]
    Unsupported {
        provider: String,
        capability: String,
        detail: String,
    },

    #[error("{provider} rejected the request ({status}): {message}")]
    Api {
        provider: String,
        status: u16,
        message: String,
    },

    #[error("{provider} could not be reached: {message}")]
    Transport { provider: String, message: String },

    #[error("{0}")]
    Adb(String),

    #[error("{0}")]
    Automation(String),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("secure storage error: {0}")]
    Secrets(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    pub fn unsupported(provider: impl Into<String>, capability: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::Unsupported {
            provider: provider.into(),
            capability: capability.into(),
            detail: detail.into(),
        }
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::Invalid(msg.into())
    }

    /// Stable machine-readable tag, used by the frontend to decide whether to
    /// offer a "open settings" action.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::NotConfigured { .. } => "not_configured",
            Self::Unsupported { .. } => "unsupported",
            Self::Api { .. } => "api",
            Self::Transport { .. } => "transport",
            Self::Adb(_) => "adb",
            Self::Automation(_) => "automation",
            Self::Db(_) => "database",
            Self::Secrets(_) => "secrets",
            Self::Invalid(_) => "invalid",
            Self::Io(_) => "io",
            Self::Json(_) => "json",
        }
    }

    pub fn is_auth_failure(&self) -> bool {
        matches!(self, Self::Api { status, .. } if *status == 401 || *status == 403)
    }
}

impl From<CoreError> for String {
    fn from(value: CoreError) -> Self {
        value.to_string()
    }
}

/// Serializable error payload crossing the Tauri IPC boundary.
#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub kind: String,
    pub message: String,
}

impl From<CoreError> for ErrorPayload {
    fn from(value: CoreError) -> Self {
        Self {
            kind: value.kind().to_string(),
            message: value.to_string(),
        }
    }
}

impl ErrorPayload {
    pub fn message(err: &CoreError) -> String {
        err.to_string()
    }
}
