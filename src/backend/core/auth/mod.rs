// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{error::RuntimeError, primitives::UserId};

pub const TOKEN_TYPE_DEFAULT: i32 = 0;
pub const TOKEN_TYPE_MCP: i32 = 5;
pub const TOKEN_TYPE_API: i32 = 8;

const API_TOKEN_USER_AGENT: &str = "Bill Analyser API Token";
const MCP_TOKEN_USER_AGENT: &str = "Bill Analyser MCP Token";
const PASSWORD_SPECIAL_CHARS: &str = "!@#$%^&*()_+-=[]{}|;:,.<>?";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthRestError {
    pub status: u16,
    pub error: String,
    pub message: String,
}

impl AuthRestError {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new(status: u16, error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            error: error.into(),
            message: message.into(),
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(401, "Unauthorized", message)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(400, "Invalid request", message)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn invalid_token(status: u16, message: impl Into<String>) -> Self {
        Self::new(status, "Invalid token", message)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_bearer_authorization_header(auth_header: &str) -> Result<String, AuthRestError> {
    if auth_header.is_empty() {
        return Err(AuthRestError::unauthorized("Missing authorization header"));
    }

    let parts: Vec<&str> = auth_header.split_whitespace().collect();
    if parts.len() != 2 || !parts[0].eq_ignore_ascii_case("bearer") {
        return Err(AuthRestError::unauthorized(
            "Invalid authorization header format",
        ));
    }

    Ok(parts[1].to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn extract_bearer_token_or_empty(auth_header: &str) -> String {
    parse_bearer_authorization_header(auth_header).unwrap_or_default()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn json_object_or_empty(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshTokenClaims {
    pub user_id: UserId,
    pub username: String,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_refresh_token_claims(value: &Value) -> Result<RefreshTokenClaims, AuthRestError> {
    if value.get("type").and_then(Value::as_str) != Some("refresh") {
        return Err(AuthRestError::invalid_token(400, "Not a refresh token"));
    }

    let user_id = value
        .get("user_id")
        .and_then(Value::as_u64)
        .and_then(|raw_user_id| UserId::new(raw_user_id).ok());
    let username = value
        .get("username")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|candidate| !candidate.is_empty());

    match (user_id, username) {
        (Some(user_id), Some(username)) => Ok(RefreshTokenClaims { user_id, username }),
        _ => Err(AuthRestError::invalid_token(401, "Invalid refresh token")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Session,
    Api,
    Mcp,
}

impl TokenKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Api => "api",
            Self::Mcp => "mcp",
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn user_agent(self, request_user_agent: &str) -> String {
        match self {
            Self::Api => API_TOKEN_USER_AGENT.to_string(),
            Self::Mcp => MCP_TOKEN_USER_AGENT.to_string(),
            Self::Session => request_user_agent.to_string(),
        }
    }
}

impl FromStr for TokenKind {
    type Err = RuntimeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "session" => Ok(Self::Session),
            "api" => Ok(Self::Api),
            "mcp" => Ok(Self::Mcp),
            _ => Err(RuntimeError::new(
                crate::error::ErrorCode::InvalidInput,
                "invalid token kind",
            )),
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn infer_token_type_from_user_agent(user_agent: &str) -> i32 {
    let normalized = user_agent.to_ascii_lowercase();
    if normalized.contains("mcp token") {
        TOKEN_TYPE_MCP
    } else if normalized.contains("api token") {
        TOKEN_TYPE_API
    } else {
        TOKEN_TYPE_DEFAULT
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_user_agent_device_name(user_agent: &str) -> String {
    if user_agent.is_empty() {
        return "未知设备".to_string();
    }

    let normalized = user_agent.to_ascii_lowercase();
    let os_name = if normalized.contains("windows") {
        if normalized.contains("windows nt 10") {
            "Windows 10"
        } else if normalized.contains("windows nt 11") {
            "Windows 11"
        } else {
            "Windows"
        }
    } else if normalized.contains("iphone") || normalized.contains("ipad") {
        "iOS"
    } else if normalized.contains("mac os") || normalized.contains("macos") {
        "macOS"
    } else if normalized.contains("android") {
        "Android"
    } else if normalized.contains("linux") {
        "Linux"
    } else {
        "其他系统"
    };

    let browser = if normalized.contains("edg/") || normalized.contains("edge/") {
        "Edge"
    } else if normalized.contains("chrome/") && !normalized.contains("edg/") {
        "Chrome"
    } else if normalized.contains("firefox/") {
        "Firefox"
    } else if normalized.contains("safari/") && !normalized.contains("chrome") {
        "Safari"
    } else {
        "其他浏览器"
    };

    format!("{os_name} ({browser})")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: false,
            require_lowercase: false,
            require_digit: false,
            require_special: false,
        }
    }
}

impl PasswordPolicy {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn validate(self, password: &str) -> Result<(), String> {
        if password.chars().count() < self.min_length {
            return Err(format!(
                "Password must be at least {} characters long",
                self.min_length
            ));
        }

        if self.require_uppercase && !password.chars().any(char::is_uppercase) {
            return Err("Password must contain at least one uppercase letter".to_string());
        }

        if self.require_lowercase && !password.chars().any(char::is_lowercase) {
            return Err("Password must contain at least one lowercase letter".to_string());
        }

        if self.require_digit && !password.chars().any(|character| character.is_ascii_digit()) {
            return Err("Password must contain at least one digit".to_string());
        }

        if self.require_special
            && !password
                .chars()
                .any(|character| PASSWORD_SPECIAL_CHARS.contains(character))
        {
            return Err("Password must contain at least one special character".to_string());
        }

        Ok(())
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_recovery_code(recovery_code: &str) -> String {
    recovery_code
        .split_whitespace()
        .collect::<String>()
        .to_ascii_uppercase()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn recovery_code_hash_input(recovery_code: &str) -> Option<String> {
    let normalized = normalize_recovery_code(recovery_code);
    if normalized.is_empty() {
        None
    } else {
        Some(format!("2fa-recovery:{normalized}"))
    }
}
