// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Serialize};

use crate::error::RuntimeError;

/// Rust response envelope for handlers that return the shared `data` shape.
///
/// Some current routes expose API `result` or raw payload
/// variants, so route modules choose the envelope family that matches the
/// frontend contract instead of forcing every endpoint through this helper.
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
