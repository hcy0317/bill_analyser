use std::{env, time::Duration};

use thiserror::Error;

pub const DEFAULT_PYTHON_UPSTREAM: &str = "http://127.0.0.1:5000";
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_BODY_LIMIT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpShellConfig {
    pub python_upstream: String,
    pub timeout: Duration,
    pub body_limit_bytes: usize,
}

impl HttpShellConfig {
    pub fn new(
        python_upstream: impl Into<String>,
        timeout: Duration,
        body_limit_bytes: usize,
    ) -> Result<Self, HttpShellConfigError> {
        let python_upstream = normalize_upstream(python_upstream.into())?;
        if body_limit_bytes == 0 {
            return Err(HttpShellConfigError::InvalidBodyLimit);
        }

        Ok(Self {
            python_upstream,
            timeout,
            body_limit_bytes,
        })
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

        Self::new(
            upstream,
            Duration::from_millis(timeout_ms),
            body_limit_bytes,
        )
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
