// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn authenticated_user(headers: &HeaderMap, state: &HttpAppState) -> RouteResult<AuthenticatedUser> {
    resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| Box::new(auth_error_response(error)))
}

#[tracing::instrument(level = "debug", skip_all)]
fn verify_sensitive_operation_password_with_policy(
    connection: &rusqlite::Connection,
    user: &AuthLoginUserRow,
    password: &str,
    operation_password_policy: OperationPasswordPolicy,
) -> bill_analyser_db::DbResult<bool> {
    if password.is_empty() {
        return Ok(false);
    }
    if bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        return Ok(true);
    }
    verify_operation_password(connection, password, operation_password_policy)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationPasswordPolicy {
    AllowUnset,
    RequireConfigured,
}

#[tracing::instrument(level = "debug", skip_all)]
fn verify_operation_password(
    connection: &rusqlite::Connection,
    password: &str,
    operation_password_policy: OperationPasswordPolicy,
) -> bill_analyser_db::DbResult<bool> {
    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    init_app_settings_schema(connection)?;
    let stored_password = get_app_setting(connection, "operation_password")?;
    Ok(
        match stored_password.as_deref().filter(|value| !value.is_empty()) {
            Some(value) => password == value,
            None => operation_password_policy == OperationPasswordPolicy::AllowUnset,
        },
    )
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
fn resolve_sensitive_two_factor_auth(
    connection: &rusqlite::Connection,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_with_policy(
        connection,
        body,
        state,
        user,
        OperationPasswordPolicy::AllowUnset,
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_destructive_user_data_auth(
    connection: &rusqlite::Connection,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_with_policy(
        connection,
        body,
        state,
        user,
        OperationPasswordPolicy::RequireConfigured,
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_sensitive_two_factor_auth_postgres(
    pool: &bill_analyser_db::PostgresPool,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_postgres_with_policy(
        pool,
        body,
        state,
        user,
        OperationPasswordPolicy::AllowUnset,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_destructive_user_data_auth_postgres(
    pool: &bill_analyser_db::PostgresPool,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
) -> Result<SensitiveTwoFactorAuthMode, SensitiveTwoFactorAuthError> {
    resolve_sensitive_two_factor_auth_postgres_with_policy(
        pool,
        body,
        state,
        user,
        OperationPasswordPolicy::RequireConfigured,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_sensitive_two_factor_auth_postgres_with_policy(
    pool: &bill_analyser_db::PostgresPool,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
    operation_password_policy: OperationPasswordPolicy,
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
    match verify_postgres_sensitive_operation_password_with_policy(
        pool,
        user,
        password,
        operation_password_policy,
    )
    .await
    {
        Ok(true) => Ok(SensitiveTwoFactorAuthMode::Password),
        Ok(false) => Err(SensitiveTwoFactorAuthError::Invalid),
        Err(_) => Err(SensitiveTwoFactorAuthError::Db),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_sensitive_two_factor_auth_with_policy(
    connection: &rusqlite::Connection,
    body: &Map<String, Value>,
    state: &HttpAppState,
    user: &AuthLoginUserRow,
    operation_password_policy: OperationPasswordPolicy,
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
    match verify_sensitive_operation_password_with_policy(
        connection,
        user,
        password,
        operation_password_policy,
    ) {
        Ok(true) => Ok(SensitiveTwoFactorAuthMode::Password),
        Ok(false) => Err(SensitiveTwoFactorAuthError::Invalid),
        Err(_) => Err(SensitiveTwoFactorAuthError::Db),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn verify_postgres_sensitive_operation_password_with_policy(
    pool: &bill_analyser_db::PostgresPool,
    user: &AuthLoginUserRow,
    password: &str,
    operation_password_policy: OperationPasswordPolicy,
) -> bill_analyser_db::DbResult<bool> {
    if password.is_empty() {
        return Ok(false);
    }
    if bcrypt::verify(password, &user.password_hash).unwrap_or(false) {
        return Ok(true);
    }
    verify_postgres_operation_password(
        pool,
        user.profile.id,
        password,
        operation_password_policy,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn verify_postgres_operation_password(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    password: &str,
    operation_password_policy: OperationPasswordPolicy,
) -> bill_analyser_db::DbResult<bool> {
    if let Some(env_password) = std::env::var("BILL_ANALYSER_OPERATION_PASSWORD")
        .ok()
        .filter(|value| !value.is_empty())
    {
        return Ok(password == env_password);
    }

    let stored_password = get_postgres_operation_password(pool, user_id).await?;
    Ok(match stored_password
        .as_ref()
        .filter(|value| !value.is_empty())
    {
        Some(value) => password == value,
        None => operation_password_policy == OperationPasswordPolicy::AllowUnset,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserDataExportType {
    Csv,
    Tsv,
}
