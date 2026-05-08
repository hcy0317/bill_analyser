use std::{env, time::Duration};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_PYTHON_UPSTREAM: &str = "http://127.0.0.1:5001";
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;
pub const DEFAULT_AUTH_JWT_ALGORITHM: &str = "HS256";
pub const DEFAULT_AUTH_JWT_EXPIRATION_DAYS: i64 = 7;
pub const DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS: i64 = 30;
pub const MAX_AUTH_JWT_EXPIRATION_DAYS: i64 = 365;
pub const MAX_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS: i64 = 365;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpShellConfig {
    pub python_upstream: String,
    pub timeout: Duration,
    pub body_limit_bytes: usize,
    pub import_route_mode: ImportRouteMode,
    pub sqlite_db_path: Option<String>,
    pub trusted_user_header_secret: Option<String>,
    pub auth_jwt_secret: Option<String>,
    pub auth_jwt_algorithm: String,
    pub auth_jwt_expiration_days: i64,
    pub auth_refresh_token_expiration_days: i64,
    pub public_base_url: Option<String>,
}

impl HttpShellConfig {
    pub fn new(
        python_upstream: impl Into<String>,
        timeout: Duration,
        body_limit_bytes: usize,
    ) -> Result<Self, HttpShellConfigError> {
        Self::new_with_import_route_mode(
            python_upstream,
            timeout,
            body_limit_bytes,
            ImportRouteMode::ProxyOnly,
        )
    }

    pub fn new_with_import_route_mode(
        python_upstream: impl Into<String>,
        timeout: Duration,
        body_limit_bytes: usize,
        import_route_mode: ImportRouteMode,
    ) -> Result<Self, HttpShellConfigError> {
        let python_upstream = normalize_upstream(python_upstream.into())?;
        if body_limit_bytes == 0 {
            return Err(HttpShellConfigError::InvalidBodyLimit);
        }

        Ok(Self {
            python_upstream,
            timeout,
            body_limit_bytes,
            import_route_mode,
            sqlite_db_path: None,
            trusted_user_header_secret: None,
            auth_jwt_secret: None,
            auth_jwt_algorithm: DEFAULT_AUTH_JWT_ALGORITHM.to_string(),
            auth_jwt_expiration_days: DEFAULT_AUTH_JWT_EXPIRATION_DAYS,
            auth_refresh_token_expiration_days: DEFAULT_AUTH_REFRESH_TOKEN_EXPIRATION_DAYS,
            public_base_url: None,
        })
    }

    pub fn with_sqlite_db_path(mut self, sqlite_db_path: impl Into<String>) -> Self {
        self.sqlite_db_path = Some(sqlite_db_path.into());
        self
    }

    pub fn with_trusted_user_header_secret(mut self, secret: impl Into<String>) -> Self {
        self.trusted_user_header_secret = Some(secret.into());
        self
    }

    pub fn with_auth_jwt_secret(mut self, secret: impl Into<String>) -> Self {
        self.auth_jwt_secret = Some(secret.into());
        self
    }

    pub fn with_auth_jwt_algorithm(mut self, algorithm: impl Into<String>) -> Self {
        self.auth_jwt_algorithm = algorithm.into();
        self
    }

    pub fn with_auth_jwt_expiration_days(mut self, days: i64) -> Self {
        self.auth_jwt_expiration_days = days;
        self
    }

    pub fn with_auth_refresh_token_expiration_days(mut self, days: i64) -> Self {
        self.auth_refresh_token_expiration_days = days;
        self
    }

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

    pub fn from_env() -> Result<Self, HttpShellConfigError> {
        Self::from_env_with(|name| env::var(name).ok())
    }

    pub fn from_env_with(
        mut lookup: impl FnMut(&'static str) -> Option<String>,
    ) -> Result<Self, HttpShellConfigError> {
        let upstream = lookup("BILL_ANALYSER_PYTHON_UPSTREAM")
            .unwrap_or_else(|| DEFAULT_PYTHON_UPSTREAM.to_string());
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
        let sqlite_db_path = lookup("BILL_ANALYSER_SQLITE_DB_PATH")
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
        let public_base_url = lookup("BILL_ANALYSER_PUBLIC_BASE_URL")
            .map(normalize_upstream)
            .transpose()?;

        let mut config = Self::new_with_import_route_mode(
            upstream,
            Duration::from_millis(timeout_ms),
            body_limit_bytes,
            import_route_mode,
        )?;
        config.sqlite_db_path = sqlite_db_path;
        config.trusted_user_header_secret = trusted_user_header_secret;
        config.auth_jwt_secret = auth_jwt_secret;
        config.auth_jwt_algorithm = auth_jwt_algorithm;
        config.auth_jwt_expiration_days = auth_jwt_expiration_days;
        config.auth_refresh_token_expiration_days = auth_refresh_token_expiration_days;
        config.public_base_url = public_base_url;
        Ok(config)
    }
}

impl Default for HttpShellConfig {
    fn default() -> Self {
        Self::new(
            DEFAULT_PYTHON_UPSTREAM,
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
    #[error("invalid import route mode")]
    InvalidImportRouteMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportRouteMode {
    ProxyOnly,
    ImportRouteSkeleton,
    ImportDbRuntime,
}

impl ImportRouteMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProxyOnly => "proxy_only",
            Self::ImportRouteSkeleton => "import_route_skeleton",
            Self::ImportDbRuntime => "import_db_runtime",
        }
    }

    pub const fn intercepts_import_routes(self) -> bool {
        matches!(self, Self::ImportRouteSkeleton | Self::ImportDbRuntime)
    }
}

fn normalize_upstream(upstream: String) -> Result<String, HttpShellConfigError> {
    let trimmed = upstream.trim().trim_end_matches('/');
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(HttpShellConfigError::InvalidUpstream);
    }
    Ok(trimmed.to_string())
}

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

fn parse_import_route_mode(value: Option<&str>) -> Result<ImportRouteMode, HttpShellConfigError> {
    let normalized = value.unwrap_or("").trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "proxy" | "proxy_only" | "python_proxy" => Ok(ImportRouteMode::ProxyOnly),
        "skeleton" | "import_skeleton" | "import_route_skeleton" => {
            Ok(ImportRouteMode::ImportRouteSkeleton)
        }
        "runtime" | "import_runtime" | "import_db_runtime" | "import_route_runtime" => {
            Ok(ImportRouteMode::ImportDbRuntime)
        }
        _ => Err(HttpShellConfigError::InvalidImportRouteMode),
    }
}
