fn authenticated_user(headers: &HeaderMap, state: &HttpAppState) -> RouteResult<AuthenticatedUser> {
    resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| Box::new(auth_error_response(error)))
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserDataExportType {
    Csv,
    Tsv,
}
