// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn authenticated_user(headers: &HeaderMap, state: &HttpAppState) -> RouteResult<AuthenticatedUser> {
    resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| Box::new(auth_error_response(error)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SensitiveTwoFactorAuthMode {
    Password,
    StepUp,
}

impl SensitiveTwoFactorAuthMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::StepUp => "step_up",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SensitiveTwoFactorAuthError {
    Missing,
    Invalid,
    Db,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_sensitive_two_factor_auth_postgres(
    pool: &bill_analyser_db::PostgresPool,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_postgres_inner(pool, body, state, user).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_destructive_user_data_auth_postgres(
    pool: &bill_analyser_db::PostgresPool,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_postgres_inner(pool, body, state, user).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_sensitive_two_factor_auth_postgres_inner(
    pool: &bill_analyser_db::PostgresPool,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    let step_up_token = body
        .get("stepUpToken")
        .or_else(|| body.get("step_up_token"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if !step_up_token.is_empty() {
        let payload = validate_action_jwt(step_up_token, state, "step_up", "Invalid step-up token")
            .map_err(|_| SensitiveTwoFactorAuthError::Invalid)?;
        let token_user_id = payload
            .get("user_id")
            .and_then(Value::as_u64)
            .ok_or(SensitiveTwoFactorAuthError::Invalid)?;
        if token_user_id == user.profile.id.get() {
            return Ok(SensitiveTwoFactorAuthMode::StepUp);
        }
        return Err(SensitiveTwoFactorAuthError::Invalid);
    }

    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if password.is_empty() {
        return Err(SensitiveTwoFactorAuthError::Missing);
    }
    match verify_sensitive_operation_password(pool, user, password).await {
        Ok(true) => Ok(SensitiveTwoFactorAuthMode::Password),
        Ok(false) => Err(SensitiveTwoFactorAuthError::Invalid),
        Err(_) => Err(SensitiveTwoFactorAuthError::Db),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserDataExportType {
    Csv,
    Tsv,
}
