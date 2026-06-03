// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::{env, time::Duration};

use bill_analyser_core::auth::PasswordPolicy;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config_database::{normalize_postgres_url, redact_postgres_url};
use crate::config_weaviate::WeaviateRuntimeConfig;

pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;
pub const DEFAULT_UPLOADS_DIR: &str = "data/uploads";
pub const DEFAULT_DATA_DIR: &str = "data";
pub const DEFAULT_BACKUP_DIR: &str = "backup";
pub const DEFAULT_AUTH_JWT_ALGORITHM: &str = "HS256";
pub const DEFAULT_AUTH_JWT_EXPIRATION_DAYS: i64 = 7;
pub const DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS: i64 = 30;
pub const DEFAULT_AUTH_MAX_LOGIN_ATTEMPTS: i64 = 5;
pub const DEFAULT_AUTH_LOCKOUT_DURATION_MINUTES: i64 = 15;
pub const DEFAULT_AUTH_ENABLE_USER_REGISTRATION: bool = true;
pub const DEFAULT_AUTH_REQUIRE_EMAIL_VERIFICATION: bool = false;
pub const DEFAULT_AUTH_ENABLE_USER_FORGET_PASSWORD: bool = false;
pub const DEFAULT_AUTH_ENABLE_OAUTH2: bool = false;
pub const DEFAULT_AUTH_PASSWORD_MIN_LENGTH: usize = 8;
pub const DEFAULT_LOCAL_POSTGRES_URL: &str =
    "postgres://bill_analyser:bill_analyser_dev@127.0.0.1:5432/bill_analyser";
pub const MAX_AUTH_JWT_EXPIRATION_DAYS: i64 = 365;
pub const MAX_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS: i64 = 365;
pub const MAX_AUTH_MAX_LOGIN_ATTEMPTS: i64 = 100;
pub const MAX_AUTH_LOCKOUT_DURATION_MINUTES: i64 = 24 * 60;
pub const MAX_AUTH_PASSWORD_MIN_LENGTH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpShellConfig {
    pub timeout: Duration,
    pub body_limit_bytes: usize,
    pub uploads_dir: String,
    pub data_dir: String,
    pub backup_dir: String,
    pub backup_encryption_key: Option<String>,
    pub import_route_mode: ImportRouteMode,
    pub postgres_url: Option<String>,
    pub database_backend: DatabaseBackend,
    pub trusted_user_header_secret: Option<String>,
    pub auth_jwt_secret: Option<String>,
    pub auth_jwt_algorithm: String,
    pub auth_jwt_expiration_days: i64,
    pub auth_refresh_token_expiration_days: i64,
    pub auth_max_login_attempts: i64,
    pub auth_lockout_duration_minutes: i64,
    pub auth_enable_user_registration: bool,
    pub auth_require_email_verification: bool,
    pub auth_enable_user_forget_password: bool,
    pub auth_enable_oauth2: bool,
    pub auth_oauth2_provider: String,
    pub auth_password_policy: PasswordPolicy,
    pub public_base_url: Option<String>,
    pub weaviate: WeaviateRuntimeConfig,
}

impl HttpShellConfig {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new(
        _unused_upstream: impl Into<String>,
        timeout: Duration,
        body_limit_bytes: usize,
    ) -> Result<Self, HttpShellConfigError> {
        Self::new_with_import_route_mode(
            "",
            timeout,
            body_limit_bytes,
            ImportRouteMode::ImportDbRuntime,
        )
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new_with_import_route_mode(
        _unused_upstream: impl Into<String>,
        timeout: Duration,
        body_limit_bytes: usize,
        import_route_mode: ImportRouteMode,
    ) -> Result<Self, HttpShellConfigError> {
        if body_limit_bytes == 0 {
            return Err(HttpShellConfigError::InvalidBodyLimit);
        }

        Ok(Self {
            timeout,
            body_limit_bytes,
            uploads_dir: DEFAULT_UPLOADS_DIR.to_string(),
            data_dir: DEFAULT_DATA_DIR.to_string(),
            backup_dir: DEFAULT_BACKUP_DIR.to_string(),
            backup_encryption_key: None,
            import_route_mode,
            postgres_url: normalize_postgres_url(Some(DEFAULT_LOCAL_POSTGRES_URL.to_string()))?,
            database_backend: DatabaseBackend::Postgres,
            trusted_user_header_secret: None,
            auth_jwt_secret: None,
            auth_jwt_algorithm: DEFAULT_AUTH_JWT_ALGORITHM.to_string(),
            auth_jwt_expiration_days: DEFAULT_AUTH_JWT_EXPIRATION_DAYS,
            auth_refresh_token_expiration_days: DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS,
            auth_max_login_attempts: DEFAULT_AUTH_MAX_LOGIN_ATTEMPTS,
            auth_lockout_duration_minutes: DEFAULT_AUTH_LOCKOUT_DURATION_MINUTES,
            auth_enable_user_registration: DEFAULT_AUTH_ENABLE_USER_REGISTRATION,
            auth_require_email_verification: DEFAULT_AUTH_REQUIRE_EMAIL_VERIFICATION,
            auth_enable_user_forget_password: DEFAULT_AUTH_ENABLE_USER_FORGET_PASSWORD,
            auth_enable_oauth2: DEFAULT_AUTH_ENABLE_OAUTH2,
            auth_oauth2_provider: String::new(),
            auth_password_policy: PasswordPolicy {
                min_length: DEFAULT_AUTH_PASSWORD_MIN_LENGTH,
                ..PasswordPolicy::default()
            },
            public_base_url: None,
            weaviate: WeaviateRuntimeConfig::disabled(),
        })
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_postgres_url(
        mut self,
        postgres_url: impl Into<String>,
    ) -> Result<Self, HttpShellConfigError> {
        self.postgres_url = normalize_postgres_url(Some(postgres_url.into()))?;
        Ok(self)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_database_backend(mut self, database_backend: DatabaseBackend) -> Self {
        self.database_backend = database_backend;
        self
    }

    pub fn postgres_configured(&self) -> bool {
        self.postgres_url.is_some()
    }

    pub fn redacted_postgres_url(&self) -> Option<String> {
        self.postgres_url.as_deref().map(redact_postgres_url)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_uploads_dir(mut self, uploads_dir: impl Into<String>) -> Self {
        let uploads_dir = uploads_dir.into().trim().to_string();
        self.uploads_dir = if uploads_dir.is_empty() {
            DEFAULT_UPLOADS_DIR.to_string()
        } else {
            uploads_dir
        };
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_data_dir(mut self, data_dir: impl Into<String>) -> Self {
        let data_dir = data_dir.into().trim().to_string();
        self.data_dir = if data_dir.is_empty() {
            DEFAULT_DATA_DIR.to_string()
        } else {
            data_dir
        };
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_backup_dir(mut self, backup_dir: impl Into<String>) -> Self {
        let backup_dir = backup_dir.into().trim().to_string();
        self.backup_dir = if backup_dir.is_empty() {
            DEFAULT_BACKUP_DIR.to_string()
        } else {
            backup_dir
        };
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_backup_encryption_key(mut self, backup_encryption_key: impl Into<String>) -> Self {
        let backup_encryption_key = backup_encryption_key.into().trim().to_string();
        self.backup_encryption_key =
            (!backup_encryption_key.is_empty()).then_some(backup_encryption_key);
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_trusted_user_header_secret(mut self, secret: impl Into<String>) -> Self {
        self.trusted_user_header_secret = Some(secret.into());
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_jwt_secret(mut self, secret: impl Into<String>) -> Self {
        self.auth_jwt_secret = Some(secret.into());
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_jwt_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.auth_jwt_algorithm = algorithm.into();
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_jwt_expiration_days(mut self, days: i64) -> Self {
        self.auth_jwt_expiration_days = days;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_refresh_token_expiration_days(mut self, days: i64) -> Self {
        self.auth_refresh_token_expiration_days = days;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_max_login_attempts(mut self, attempts: i64) -> Self {
        self.auth_max_login_attempts = attempts;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_lockout_duration_minutes(mut self, minutes: i64) -> Self {
        self.auth_lockout_duration_minutes = minutes;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_enable_user_registration(mut self, enabled: bool) -> Self {
        self.auth_enable_user_registration = enabled;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_require_email_verification(mut self, required: bool) -> Self {
        self.auth_require_email_verification = required;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_enable_user_forget_password(mut self, enabled: bool) -> Self {
        self.auth_enable_user_forget_password = enabled;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_enable_oauth2(mut self, enabled: bool) -> Self {
        self.auth_enable_oauth2 = enabled;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_oauth2_provider(mut self, provider: impl Into<String>) -> Self {
        self.auth_oauth2_provider = provider.into().trim().to_string();
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_auth_password_policy(mut self, policy: PasswordPolicy) -> Self {
        self.auth_password_policy = policy;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_public_base_url(mut self, public_base_url: impl Into<String>) -> Self {
        let public_base_url = public_base_url.into();
        let public_base_url = public_base_url.trim().trim_end_matches('/').to_string();
        self.public_base_url = if public_base_url.is_empty() {
            None
        } else {
            Some(public_base_url)
        };
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn with_weaviate_config(mut self, weaviate: WeaviateRuntimeConfig) -> Self {
        self.weaviate = weaviate;
        self
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn from_env() -> Result<Self, HttpShellConfigError> {
        Self::from_env_with(|name| env::var(name).ok())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn from_env_with(
        mut lookup: impl FnMut(&'static str) -> Option<String>,
    ) -> Result<Self, HttpShellConfigError> {
        let timeout_ms = parse_env_u64_value(
            "BILL_ANALYSER_HTTP_TIMEOUT_MS",
            lookup("BILL_ANALYSER_HTTP_TIMEOUT_MS"),
            DEFAULT_TIMEOUT_MS,
        )?;
        let body_limit_bytes = parse_env_usize_value(
            "BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES",
            lookup("BILL_ANALYSER_HTTP_BODY_LIMIT_BYTES"),
            DEFAULT_BODY_LIMIT_BYTES,
        )?;
        let import_route_mode =
            parse_import_route_mode(lookup("BILL_ANALYSER_HTTP_IMPORT_ROUTE_MODE").as_deref())?;
        let database_backend =
            parse_database_backend(lookup("BILL_ANALYSER_DATABASE_BACKEND").as_deref())?;
        let postgres_url = normalize_postgres_url(
            lookup("BILL_ANALYSER_POSTGRES_URL")
                .filter(|value| !value.trim().is_empty())
                .or_else(|| Some(DEFAULT_LOCAL_POSTGRES_URL.to_string())),
        )?;
        let uploads_dir = lookup("BILL_ANALYSER_UPLOADS_DIR")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_UPLOADS_DIR.to_string());
        let data_dir = lookup("BILL_ANALYSER_DATA_DIR")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_DATA_DIR.to_string());
        let backup_dir = lookup("BILL_ANALYSER_BACKUP_DIR")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_BACKUP_DIR.to_string());
        let backup_encryption_key = lookup("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let trusted_user_header_secret = lookup("BILL_ANALYSER_TRUSTED_USER_HEADER_SECRET")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let auth_jwt_secret = lookup("BILL_ANALYSER_AUTH_JWT_SECRET")
            .or_else(|| lookup("JWT_SECRET_KEY"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let auth_jwt_algorithm = lookup("BILL_ANALYSER_AUTH_JWT_ALGORITHM")
            .or_else(|| lookup("JWT_ALGORITHM"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| DEFAULT_AUTH_JWT_ALGORITHM.to_string());
        let auth_jwt_expiration_days = parse_env_i64_range_value(
            "BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS",
            lookup("BILL_ANALYSER_AUTH_JWT_EXPIRATION_DAYS")
                .or_else(|| lookup("JWT_EXPIRATION_DAYS")),
            DEFAULT_AUTH_JWT_EXPIRATION_DAYS,
            1,
            MAX_AUTH_JWT_EXPIRATION_DAYS,
        )?;
        let auth_refresh_token_expiration_days = parse_env_i64_range_value(
            "BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS",
            lookup("BILL_ANALYSER_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS")
                .or_else(|| lookup("REFRESH_TOKEN_EXPIRATION_DAYS")),
            DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS,
            1,
            MAX_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS,
        )?;
        let auth_max_login_attempts = parse_env_i64_range_value(
            "BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS",
            lookup("BILL_ANALYSER_AUTH_MAX_LOGIN_ATTEMPTS")
                .or_else(|| lookup("MAX_LOGIN_ATTEMPTS")),
            DEFAULT_AUTH_MAX_LOGIN_ATTEMPTS,
            1,
            MAX_AUTH_MAX_LOGIN_ATTEMPTS,
        )?;
        let auth_lockout_duration_minutes = parse_env_i64_range_value(
            "BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES",
            lookup("BILL_ANALYSER_AUTH_LOCKOUT_DURATION_MINUTES")
                .or_else(|| lookup("LOCKOUT_DURATION_MINUTES")),
            DEFAULT_AUTH_LOCKOUT_DURATION_MINUTES,
            1,
            MAX_AUTH_LOCKOUT_DURATION_MINUTES,
        )?;
        let auth_enable_user_registration = parse_env_bool_value(
            "BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION",
            lookup("BILL_ANALYSER_AUTH_ENABLE_USER_REGISTRATION")
                .or_else(|| lookup("ENABLE_USER_REGISTRATION")),
            DEFAULT_AUTH_ENABLE_USER_REGISTRATION,
        )?;
        let auth_require_email_verification = parse_env_bool_value(
            "BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION",
            lookup("BILL_ANALYSER_AUTH_REQUIRE_EMAIL_VERIFICATION")
                .or_else(|| lookup("REQUIRE_EMAIL_VERIFICATION")),
            DEFAULT_AUTH_REQUIRE_EMAIL_VERIFICATION,
        )?;
        let auth_enable_user_forget_password = parse_env_bool_value(
            "BILL_ANALYSER_AUTH_ENABLE_USER_FORGET_PASSWORD",
            lookup("BILL_ANALYSER_AUTH_ENABLE_USER_FORGET_PASSWORD")
                .or_else(|| lookup("ENABLE_USER_FORGET_PASSWORD")),
            DEFAULT_AUTH_ENABLE_USER_FORGET_PASSWORD,
        )?;
        let auth_enable_oauth2 = parse_env_bool_value(
            "BILL_ANALYSER_AUTH_ENABLE_OAUTH2",
            lookup("BILL_ANALYSER_AUTH_ENABLE_OAUTH2").or_else(|| lookup("ENABLE_OAUTH2")),
            DEFAULT_AUTH_ENABLE_OAUTH2,
        )?;
        let auth_oauth2_provider = lookup("BILL_ANALYSER_AUTH_OAUTH2_PROVIDER")
            .or_else(|| lookup("OAUTH2_PROVIDER"))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let auth_password_policy = PasswordPolicy {
            min_length: parse_env_usize_range_value(
                "BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH",
                lookup("BILL_ANALYSER_AUTH_PASSWORD_MIN_LENGTH")
                    .or_else(|| lookup("PASSWORD_MIN_LENGTH")),
                DEFAULT_AUTH_PASSWORD_MIN_LENGTH,
                1,
                MAX_AUTH_PASSWORD_MIN_LENGTH,
            )?,
            require_uppercase: parse_env_bool_value(
                "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_UPPERCASE",
                lookup("BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_UPPERCASE")
                    .or_else(|| lookup("PASSWORD_REQUIRE_UPPERCASE")),
                false,
            )?,
            require_lowercase: parse_env_bool_value(
                "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_LOWERCASE",
                lookup("BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_LOWERCASE")
                    .or_else(|| lookup("PASSWORD_REQUIRE_LOWERCASE")),
                false,
            )?,
            require_digit: parse_env_bool_value(
                "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_DIGIT",
                lookup("BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_DIGIT")
                    .or_else(|| lookup("PASSWORD_REQUIRE_DIGIT")),
                false,
            )?,
            require_special: parse_env_bool_value(
                "BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_SPECIAL",
                lookup("BILL_ANALYSER_AUTH_PASSWORD_REQUIRE_SPECIAL")
                    .or_else(|| lookup("PASSWORD_REQUIRE_SPECIAL")),
                false,
            )?,
        };
        let public_base_url = lookup("BILL_ANALYSER_PUBLIC_BASE_URL")
            .map(normalize_upstream)
            .transpose()?;
        let weaviate = WeaviateRuntimeConfig::from_env_with(&mut lookup)?;

        let mut config = Self::new_with_import_route_mode(
            "",
            Duration::from_millis(timeout_ms),
            body_limit_bytes,
            import_route_mode,
        )?;
        config.postgres_url = postgres_url;
        config.database_backend = database_backend;
        config.uploads_dir = uploads_dir;
        config.data_dir = data_dir;
        config.backup_dir = backup_dir;
        config.backup_encryption_key = backup_encryption_key;
        config.trusted_user_header_secret = trusted_user_header_secret;
        config.auth_jwt_secret = auth_jwt_secret;
        config.auth_jwt_algorithm = auth_jwt_algorithm;
        config.auth_jwt_expiration_days = auth_jwt_expiration_days;
        config.auth_refresh_token_expiration_days = auth_refresh_token_expiration_days;
        config.auth_max_login_attempts = auth_max_login_attempts;
        config.auth_lockout_duration_minutes = auth_lockout_duration_minutes;
        config.auth_enable_user_registration = auth_enable_user_registration;
        config.auth_require_email_verification = auth_require_email_verification;
        config.auth_enable_user_forget_password = auth_enable_user_forget_password;
        config.auth_enable_oauth2 = auth_enable_oauth2;
        config.auth_oauth2_provider = auth_oauth2_provider;
        config.auth_password_policy = auth_password_policy;
        config.public_base_url = public_base_url;
        config.weaviate = weaviate;
        Ok(config)
    }
}

impl Default for HttpShellConfig {
    fn default() -> Self {
        Self::new(
            "",
            Duration::from_millis(DEFAULT_TIMEOUT_MS),
            DEFAULT_BODY_LIMIT_BYTES,
        )
        .expect("default HTTP shell config is valid")
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HttpShellConfigError {
    #[error("invalid upstream URL")]
    InvalidUpstream,
    #[error("invalid body limit")]
    InvalidBodyLimit,
    #[error("invalid integer for {0}")]
    InvalidInteger(&'static str),
    #[error("invalid boolean for {0}")]
    InvalidBoolean(&'static str),
    #[error("invalid import route mode")]
    InvalidImportRouteMode,
    #[error("invalid database backend")]
    InvalidDatabaseBackend,
    #[error("invalid PostgreSQL URL")]
    InvalidPostgresUrl,
    #[error("invalid Weaviate endpoint")]
    InvalidWeaviateEndpoint,
    #[error("invalid Weaviate collection prefix")]
    InvalidWeaviateCollectionPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRouteMode {
    ImportDbRuntime,
}

impl ImportRouteMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ImportDbRuntime => "import_db_runtime",
        }
    }

    pub const fn intercepts_import_routes(self) -> bool {
        matches!(self, Self::ImportDbRuntime)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseBackend {
    Postgres,
}

impl DatabaseBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_upstream(upstream: String) -> Result<String, HttpShellConfigError> {
    let trimmed = upstream.trim().trim_end_matches('/');
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(HttpShellConfigError::InvalidUpstream);
    }
    Ok(trimmed.to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_u64_value(
    name: &'static str,
    value: Option<String>,
    default_value: u64,
) -> Result<u64, HttpShellConfigError> {
    match value {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| HttpShellConfigError::InvalidInteger(name)),
        None => Ok(default_value),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_usize_value(
    name: &'static str,
    value: Option<String>,
    default_value: usize,
) -> Result<usize, HttpShellConfigError> {
    match value {
        Some(value) => value
            .parse::<usize>()
            .map_err(|_| HttpShellConfigError::InvalidInteger(name)),
        None => Ok(default_value),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_usize_range_value(
    name: &'static str,
    value: Option<String>,
    default_value: usize,
    min_value: usize,
    max_value: usize,
) -> Result<usize, HttpShellConfigError> {
    let parsed = parse_env_usize_value(name, value, default_value)?;
    if parsed < min_value || parsed > max_value {
        return Err(HttpShellConfigError::InvalidInteger(name));
    }
    Ok(parsed)
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_i64_range_value(
    name: &'static str,
    value: Option<String>,
    default_value: i64,
    min_value: i64,
    max_value: i64,
) -> Result<i64, HttpShellConfigError> {
    match value {
        Some(value) => {
            let parsed = value
                .parse::<i64>()
                .map_err(|_| HttpShellConfigError::InvalidInteger(name))?;
            if parsed < min_value || parsed > max_value {
                return Err(HttpShellConfigError::InvalidInteger(name));
            }
            Ok(parsed)
        }
        None => Ok(default_value),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_env_bool_value(
    name: &'static str,
    value: Option<String>,
    default_value: bool,
) -> Result<bool, HttpShellConfigError> {
    let Some(value) = value else {
        return Ok(default_value);
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(HttpShellConfigError::InvalidBoolean(name)),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_import_route_mode(value: Option<&str>) -> Result<ImportRouteMode, HttpShellConfigError> {
    let normalized = value.unwrap_or("").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "runtime" | "import_runtime" | "import_db_runtime" | "import_route_runtime" => {
            Ok(ImportRouteMode::ImportDbRuntime)
        }
        _ => Err(HttpShellConfigError::InvalidImportRouteMode),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_database_backend(value: Option<&str>) -> Result<DatabaseBackend, HttpShellConfigError> {
    let normalized = value.unwrap_or("").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" => Ok(DatabaseBackend::Postgres),
        "postgres" | "postgresql" => Ok(DatabaseBackend::Postgres),
        _ => Err(HttpShellConfigError::InvalidDatabaseBackend),
    }
}
