use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

/// Runtime-shell response envelope used by the Rust foundation layer.
///
/// This is not a claim that every existing Flask endpoint already uses this
/// exact payload shape. Later business migration slices must add endpoint-level
/// parity variants or adapters before using Rust responses for those endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    code: String,
    message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<RuntimeError> for ApiError {
    fn from(error: RuntimeError) -> Self {
        Self::new(error.code.as_str(), error.message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn failure(error: RuntimeError) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    pub fn to_json(&self) -> Result<String, RuntimeError> {
        serde_json::to_string(self).map_err(RuntimeError::from)
    }
}
