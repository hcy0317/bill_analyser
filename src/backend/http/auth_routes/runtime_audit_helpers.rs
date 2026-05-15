fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            "Rust auth token runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        )))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            error.to_string(),
        )))
    })?;
    let runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: false,
        busy_timeout: state.config.timeout,
    })
    .map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            503,
            "Service Unavailable",
            format!("Rust auth token runtime cannot open configured SQLite database: {error}"),
        )))
    })?;
    init_auth_security_schema(runtime.connection()).map_err(|error| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            500,
            "Internal Server Error",
            format!("Rust auth token runtime schema initialization failed: {error}"),
        )))
    })?;
    Ok(runtime)
}

fn client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    if let Some(addr) = peer_addr {
        return addr.ip().to_string();
    }
    forwarded_header_ip(headers).unwrap_or_else(|| FALLBACK_CLIENT_IP.to_string())
}

fn login_client_ip(headers: &HeaderMap, peer_addr: Option<SocketAddr>) -> String {
    forwarded_header_ip(headers)
        .or_else(|| peer_addr.map(|addr| addr.ip().to_string()))
        .unwrap_or_else(|| FALLBACK_CLIENT_IP.to_string())
}

fn forwarded_header_ip(headers: &HeaderMap) -> Option<String> {
    let forwarded_for = header_value(headers, "x-forwarded-for");
    if !forwarded_for.is_empty() {
        return Some(
            forwarded_for
                .split(',')
                .next()
                .unwrap_or_default()
                .trim()
                .to_string(),
        );
    }
    let real_ip = header_value(headers, "x-real-ip");
    if !real_ip.is_empty() {
        return Some(real_ip);
    }
    None
}

fn request_origin(state: &HttpAppState) -> RouteResult<String> {
    if let Some(public_base_url) = state.config.public_base_url.as_deref() {
        return Ok(public_base_url.to_string());
    }
    Err(Box::new(auth_rest_error_response(AuthRestError::new(
        503,
        "Service Unavailable",
        "Rust auth token runtime requires BILL_ANALYSER_PUBLIC_BASE_URL",
    ))))
}

fn header_value(headers: &HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

struct AuthEvent<'a> {
    user_id: Option<bill_analyser_core::UserId>,
    username: &'a str,
    event_type: &'a str,
    ip_address: &'a str,
    user_agent: &'a str,
    success: bool,
    error_message: Option<String>,
    metadata: Option<String>,
}

struct TwoFactorRecoveryLoginDraft<'a> {
    recovery_code: &'a str,
    session_draft: &'a CreateTokenSessionDraft,
    user_id: UserId,
    username: &'a str,
    ip_address: &'a str,
    user_agent: &'a str,
    now: &'a str,
}

fn log_auth_event(
    connection: &rusqlite::Connection,
    event: AuthEvent<'_>,
) -> bill_analyser_db::DbResult<i64> {
    create_auth_log(
        connection,
        &AuthLogDraft {
            user_id: event.user_id,
            username: event.username.to_string(),
            event_type: event.event_type.to_string(),
            ip_address: event.ip_address.to_string(),
            user_agent: event.user_agent.to_string(),
            success: event.success,
            error_message: event.error_message,
            metadata: event.metadata,
            created_at: utc_now_text(),
        },
    )
}

fn ensure_sensitive_auth_failure_limit(
    connection: &rusqlite::Connection,
    user_id: UserId,
    event_type: &str,
) -> RouteResult<()> {
    let failure_count = count_auth_events_since(
        connection,
        user_id,
        event_type,
        &sensitive_auth_failure_window_start_text(),
    )
    .map_err(|_| Box::new(db_error_response()))?;
    if failure_count >= SENSITIVE_AUTH_FAILURE_LIMIT {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            429,
            "Too Many Requests",
            "Too many failed sensitive-operation authentication attempts, please try again later",
        ))));
    }
    Ok(())
}

fn record_sensitive_auth_failure(
    connection: &rusqlite::Connection,
    user: &AuthLoginUserRow,
    event_type: &str,
    ip_address: &str,
    user_agent: &str,
    error_message: &str,
    metadata: Option<Value>,
) -> RouteResult<()> {
    log_auth_event(
        connection,
        AuthEvent {
            user_id: Some(user.profile.id),
            username: &user.profile.username,
            event_type,
            ip_address,
            user_agent,
            success: false,
            error_message: Some(error_message.to_string()),
            metadata: metadata.map(|value| value.to_string()),
        },
    )
    .map(|_| ())
    .map_err(|_| Box::new(db_error_response()))
}

fn persist_login_success(
    connection: &rusqlite::Connection,
    session_draft: &CreateTokenSessionDraft,
    user_id: UserId,
    username: &str,
    now: &str,
    ip_address: &str,
    user_agent: &str,
) -> bill_analyser_db::DbResult<i64> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let session_id = create_token_session(connection, session_draft)?;
        if !update_user_last_login(connection, user_id, now, ip_address)? {
            return Err(DbError::InvalidOperation(
                "login user row was not updated".to_string(),
            ));
        }
        log_auth_event(
            connection,
            AuthEvent {
                user_id: Some(user_id),
                username,
                event_type: "login_success",
                ip_address,
                user_agent,
                success: true,
                error_message: None,
                metadata: Some(json!({ "session_id": session_id }).to_string()),
            },
        )?;
        Ok(session_id)
    })();

    match result {
        Ok(session_id) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(session_id)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn persist_two_factor_login_success(
    connection: &rusqlite::Connection,
    session_draft: &CreateTokenSessionDraft,
    user_id: UserId,
    username: &str,
    ip_address: &str,
    user_agent: &str,
) -> bill_analyser_db::DbResult<i64> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        let session_id = create_token_session(connection, session_draft)?;
        log_auth_event(
            connection,
            AuthEvent {
                user_id: Some(user_id),
                username,
                event_type: "login_2fa_success",
                ip_address,
                user_agent,
                success: true,
                error_message: None,
                metadata: Some(json!({ "session_id": session_id }).to_string()),
            },
        )?;
        Ok(session_id)
    })();

    match result {
        Ok(session_id) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(session_id)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn persist_two_factor_recovery_login_success(
    connection: &rusqlite::Connection,
    draft: TwoFactorRecoveryLoginDraft<'_>,
) -> bill_analyser_db::DbResult<Option<i64>> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = (|| {
        if !consume_two_factor_recovery_code(
            connection,
            draft.user_id,
            draft.recovery_code,
            draft.now,
        )? {
            return Ok(None);
        }
        let session_id = create_token_session(connection, draft.session_draft)?;
        log_auth_event(
            connection,
            AuthEvent {
                user_id: Some(draft.user_id),
                username: draft.username,
                event_type: "login_2fa_recovery_success",
                ip_address: draft.ip_address,
                user_agent: draft.user_agent,
                success: true,
                error_message: None,
                metadata: Some(json!({ "session_id": session_id }).to_string()),
            },
        )?;
        create_two_factor_recovery_audit_log_best_effort(
            connection,
            draft.user_id,
            draft.ip_address,
            draft.user_agent,
            draft.now,
        );
        Ok(Some(session_id))
    })();

    match result {
        Ok(Some(session_id)) => {
            if let Err(error) = connection.execute_batch("COMMIT") {
                let _ = connection.execute_batch("ROLLBACK");
                return Err(error.into());
            }
            Ok(Some(session_id))
        }
        Ok(None) => {
            let _ = connection.execute_batch("ROLLBACK");
            Ok(None)
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn create_two_factor_recovery_audit_log_best_effort(
    connection: &rusqlite::Connection,
    user_id: UserId,
    ip_address: &str,
    user_agent: &str,
    now: &str,
) {
    create_user_audit_log_best_effort(
        connection,
        UserAuditLogDraft {
            operation_type: "2fa_recovery_code_used",
            user_id,
            details: json!({ "verification": "recovery_code" }),
            affected_count: 1_i64,
            ip_address,
            user_agent,
            now,
        },
    );
}

struct UserAuditLogDraft<'a> {
    operation_type: &'a str,
    user_id: UserId,
    details: Value,
    affected_count: i64,
    ip_address: &'a str,
    user_agent: &'a str,
    now: &'a str,
}

struct UserDataAuditLogDraft<'a> {
    operation_type: &'a str,
    user_id: UserId,
    details: Value,
    affected_count: i64,
    ip_address: &'a str,
    user_agent: &'a str,
    now: &'a str,
}

fn create_user_audit_log_best_effort(
    connection: &rusqlite::Connection,
    draft: UserAuditLogDraft<'_>,
) {
    let Ok(target_id) = i64::try_from(draft.user_id.get()) else {
        return;
    };
    let details = draft.details.to_string();
    let _ = connection.execute(
        r#"
        INSERT INTO audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        (
            draft.operation_type,
            "user",
            target_id,
            details,
            draft.affected_count,
            draft.ip_address,
            draft.user_agent,
            Option::<String>::None,
            "success",
            Option::<String>::None,
            draft.now,
        ),
    );
}

fn create_user_data_audit_log_best_effort(
    connection: &rusqlite::Connection,
    draft: UserDataAuditLogDraft<'_>,
) {
    let Ok(target_id) = i64::try_from(draft.user_id.get()) else {
        return;
    };
    let _ = connection.execute(
        r#"
        INSERT INTO audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        (
            draft.operation_type,
            "user_data",
            target_id,
            draft.details.to_string(),
            draft.affected_count,
            draft.ip_address,
            draft.user_agent,
            Option::<String>::None,
            "success",
            Option::<String>::None,
            draft.now,
        ),
    );
}

fn session_payload(session: TokenSessionRow, current_session_id: Option<i64>) -> Value {
    let is_current = current_session_id == Some(session.id);
    let last_activity_at = session.last_activity_at;
    let created_at = session.created_at;
    let last_seen_source = if last_activity_at.is_empty() {
        created_at.as_str()
    } else {
        last_activity_at.as_str()
    };

    json!({
        "tokenId": session.id.to_string(),
        "tokenType": infer_token_type_from_user_agent(&session.user_agent),
        "userAgent": session.user_agent,
        "deviceName": parse_user_agent_device_name(&session.user_agent),
        "ipAddress": session.ip_address,
        "createdAt": created_at,
        "expiresAt": session.expires_at,
        "lastActivityAt": last_activity_at,
        "lastSeen": datetime_to_unix_millis(last_seen_source),
        "isCurrent": is_current,
        "isCurrentToken": is_current
    })
}

fn user_profile_payload(user: &AuthUserProfileRow) -> Value {
    let nickname = if user.nickname.is_empty() {
        user.username.clone()
    } else {
        user.nickname.clone()
    };
    let mut keyword_source = Map::new();
    if let Some(value) = user.investment_platform_keywords.as_deref() {
        keyword_source.insert(
            "investment_platform_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = user.investment_product_keywords.as_deref() {
        keyword_source.insert(
            "investment_product_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    if let Some(value) = user.investment_exclude_keywords.as_deref() {
        keyword_source.insert(
            "investment_exclude_keywords".to_string(),
            Value::String(value.to_string()),
        );
    }
    let investment_settings = build_user_investment_keyword_settings(Some(&keyword_source));
    json!({
        "username": user.username,
        "email": user.email,
        "nickname": nickname,
        "avatar": user.avatar,
        "avatarProvider": "internal",
        "defaultAccountId": optional_id_string(user.default_account_id),
        "transactionEditScope": user.transaction_edit_scope,
        "language": user.language,
        "defaultCurrency": user.default_currency,
        "firstDayOfWeek": user.first_day_of_week,
        "fiscalYearStart": user.fiscal_year_start,
        "calendarDisplayType": user.calendar_display_type,
        "dateDisplayType": user.date_display_type,
        "longDateFormat": user.long_date_format,
        "shortDateFormat": user.short_date_format,
        "longTimeFormat": user.long_time_format,
        "shortTimeFormat": user.short_time_format,
        "fiscalYearFormat": user.fiscal_year_format,
        "currencyDisplayType": user.currency_display_type,
        "numeralSystem": user.numeral_system,
        "decimalSeparator": user.decimal_separator,
        "digitGroupingSymbol": user.digit_grouping_symbol,
        "digitGrouping": user.digit_grouping,
        "coordinateDisplayType": user.coordinate_display_type,
        "expenseAmountColor": user.expense_amount_color,
        "incomeAmountColor": user.income_amount_color,
        "cashAccountId": optional_id_string(user.cash_account_id),
        "cashTransferCategoryId": optional_id_string(user.cash_transfer_category_id),
        "importLearningEnabled": user.import_learning_enabled,
        "investmentPlatformKeywords": investment_settings["platform_keywords"].clone(),
        "investmentProductKeywords": investment_settings["product_keywords"].clone(),
        "investmentExcludeKeywords": investment_settings["exclude_keywords"].clone(),
        "emailVerified": user.email_verified,
    })
}

fn application_cloud_settings_payload(settings: Vec<ApplicationCloudSettingRow>) -> Value {
    Value::Array(
        settings
            .into_iter()
            .map(|setting| {
                json!({
                    "settingKey": setting.setting_key,
                    "settingValue": setting.setting_value,
                })
            })
            .collect(),
    )
}

struct ExternalAuthInfo {
    external_auth_category: String,
    external_auth_type: String,
    linked: bool,
    external_username: String,
    created_at: i64,
}

impl ExternalAuthInfo {
    fn linked(row: ExternalAuthRow) -> Self {
        Self {
            external_auth_category: row.external_auth_category,
            external_auth_type: row.external_auth_type,
            linked: true,
            external_username: row.external_username,
            created_at: datetime_to_unix_millis(&row.created_at),
        }
    }
}

fn external_auths_payload(auths: Vec<ExternalAuthInfo>) -> Value {
    Value::Array(
        auths
            .into_iter()
            .map(|auth| {
                json!({
                    "externalAuthCategory": auth.external_auth_category,
                    "externalAuthType": auth.external_auth_type,
                    "linked": auth.linked,
                    "externalUsername": auth.external_username,
                    "createdAt": auth.created_at,
                })
            })
            .collect(),
    )
}

fn optional_id_string(value: Option<i64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn datetime_to_unix_millis(value: &str) -> i64 {
    let value = value.trim();
    if value.is_empty() {
        return 0;
    }
    let parsed = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"));
    parsed
        .ok()
        .and_then(|datetime| Local.from_local_datetime(&datetime).single())
        .map(|datetime| datetime.timestamp_millis())
        .unwrap_or(0)
}

fn now_text() -> String {
    Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn utc_now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn login_lockout_until_text(minutes: i64) -> String {
    (Utc::now().naive_utc() + ChronoDuration::minutes(minutes))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn token_failure_window_start_text() -> String {
    (Utc::now().naive_utc() - ChronoDuration::minutes(TOKEN_PASSWORD_FAILURE_WINDOW_MINUTES))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn sensitive_auth_failure_window_start_text() -> String {
    (Utc::now().naive_utc() - ChronoDuration::minutes(SENSITIVE_AUTH_FAILURE_WINDOW_MINUTES))
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn sha256_hex(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn db_error_response() -> Response {
    auth_rest_error_response(AuthRestError::new(
        500,
        "Internal Server Error",
        "Rust auth token runtime DB error",
    ))
}

fn auth_error_response(error: RustRouteAuthError) -> Response {
    let error_label = match error.status {
        401 => "Unauthorized",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        _ => "Authentication Error",
    };
    auth_rest_error_response(AuthRestError::new(error.status, error_label, error.message))
}

fn auth_rest_error_response(error: AuthRestError) -> Response {
    json_response(
        status_or_internal(error.status),
        json!({
            "success": false,
            "error": error.error,
            "message": error.message
        }),
    )
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn apply_auth_cors_headers(origin: Option<&HeaderValue>, headers: &mut HeaderMap) {
    let Some(origin) = origin else {
        return;
    };
    let Ok(origin_text) = origin.to_str() else {
        return;
    };
    if !AUTH_ALLOWED_CORS_ORIGINS.contains(&origin_text) {
        return;
    }
    headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin.clone());
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
        HeaderValue::from_static("true"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Content-Type, Authorization"),
    );
    headers.append(header::VARY, HeaderValue::from_static("Origin"));
}

fn apply_auth_preflight_headers(requested_headers: Option<&HeaderValue>, headers: &mut HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        requested_headers
            .cloned()
            .unwrap_or_else(|| HeaderValue::from_static("authorization, content-type")),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("600"),
    );
    headers.append(
        header::VARY,
        HeaderValue::from_static("Access-Control-Request-Headers"),
    );
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::HeaderValue;
    use bill_analyser_core::UserId;

    use super::*;
    use crate::config::HttpShellConfig;

    fn test_state(config: HttpShellConfig) -> HttpAppState {
        HttpAppState::new(config).expect("http app state")
    }

    #[test]
    fn helper_edges_cover_origin_ip_and_expires_parsing() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.10"));
        assert_eq!(client_ip(&headers, None), "203.0.113.10");
        assert_eq!(
            client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "198.51.100.20"
        );
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("192.0.2.99, 198.51.100.2"),
        );
        assert_eq!(
            client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "198.51.100.20"
        );
        assert_eq!(
            login_client_ip(&headers, Some("198.51.100.20:4300".parse().expect("addr"))),
            "192.0.2.99"
        );
        assert_eq!(client_ip(&HeaderMap::new(), None), FALLBACK_CLIENT_IP);

        assert!(request_origin(&test_state(HttpShellConfig::default())).is_err());
        assert_eq!(
            request_origin(&test_state(
                HttpShellConfig::default().with_public_base_url("https://public.test/")
            ))
            .ok()
            .as_deref(),
            Some("https://public.test")
        );

        let mut body = Map::new();
        assert_eq!(parse_expires_in_seconds(&body).expect("missing expires"), 0);
        body.insert("expiresInSeconds".to_string(), Value::Null);
        assert_eq!(parse_expires_in_seconds(&body).expect("null expires"), 0);
        body.insert("expiresInSeconds".to_string(), Value::Bool(true));
        assert_eq!(parse_expires_in_seconds(&body).expect("bool expires"), 1);
        body.insert("expiresInSeconds".to_string(), json!(12.8));
        assert_eq!(parse_expires_in_seconds(&body).expect("float expires"), 12);
        body.insert(
            "expiresInSeconds".to_string(),
            Value::String("30".to_string()),
        );
        assert_eq!(parse_expires_in_seconds(&body).expect("string expires"), 30);

        let warning = logout_session_not_found_warning_payload("0123456789abcdefdeadbeefcafebabe");
        assert_eq!(warning["event"], "logout_session_not_found");
        assert_eq!(warning["token_hash_prefix"], "0123456789abcdef");
        assert!(!warning.to_string().contains("deadbeef"));
    }

    #[test]
    fn issue_token_and_auth_error_edges_are_pinned() {
        let user_id = UserId::new(7).expect("user id");
        assert!(issue_access_token(
            user_id,
            "alice",
            &test_state(HttpShellConfig::default()),
            TokenKind::Api,
            60,
        )
        .is_err());
        assert!(issue_access_token(
            user_id,
            "alice",
            &test_state(
                HttpShellConfig::default()
                    .with_auth_jwt_secret("secret")
                    .with_auth_jwt_algorithm("none")
            ),
            TokenKind::Api,
            60,
        )
        .is_err());

        let issued = issue_access_token(
            user_id,
            "alice",
            &test_state(
                HttpShellConfig::new("http://127.0.0.1:5001", Duration::from_millis(100), 1024)
                    .expect("config")
                    .with_auth_jwt_secret("secret")
                    .with_auth_jwt_algorithm("HS512"),
            ),
            TokenKind::Session,
            0,
        )
        .expect("long-lived token");
        assert!(!issued.access_token.is_empty());
        assert!(issued.expires_at.contains('T'));

        assert_eq!(
            auth_error_response(RustRouteAuthError {
                status: 500,
                message: "internal".to_string(),
            })
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            auth_error_response(RustRouteAuthError {
                status: 418,
                message: "teapot".to_string(),
            })
            .status(),
            StatusCode::IM_A_TEAPOT
        );
    }

    #[test]
    fn profile_helper_edges_cover_updates_multipart_and_cloud_validation() {
        let body = json!({
            "nickname": "Alice",
            "email": "alice@example.test",
            "avatar": true,
            "language": "en",
            "defaultCurrency": "USD",
            "firstDayOfWeek": 2,
            "defaultAccountId": "",
            "transactionEditScope": "3",
            "fiscalYearStart": 4,
            "calendarDisplayType": 5,
            "dateDisplayType": "6",
            "longDateFormat": 7,
            "shortDateFormat": 8,
            "longTimeFormat": 9,
            "shortTimeFormat": 10,
            "fiscalYearFormat": 11,
            "currencyDisplayType": 12,
            "numeralSystem": 13,
            "decimalSeparator": 14,
            "digitGroupingSymbol": 15,
            "digitGrouping": 16,
            "coordinateDisplayType": 17,
            "expenseAmountColor": 18,
            "incomeAmountColor": 19,
            "cashAccountId": null,
            "cashTransferCategoryId": "200",
            "importLearningEnabled": "0",
            "investmentPlatformKeywords": ["蚂蚁财富", "雪球"],
            "investmentProductKeywords": "基金",
            "investmentExcludeKeywords": ["还款"]
        });
        let updates = profile_updates_from_body(body.as_object().expect("object"))
            .expect("profile updates parse");
        assert_eq!(updates.len(), 29);
        assert!(updates.contains(&AuthUserProfileUpdate::Nickname("Alice".to_string())));
        assert!(!updates
            .iter()
            .any(|update| matches!(update, AuthUserProfileUpdate::Avatar(_))));
        assert!(updates.contains(&AuthUserProfileUpdate::DefaultAccountId(None)));
        assert!(updates.contains(&AuthUserProfileUpdate::TransactionEditScope(3)));
        assert!(updates.contains(&AuthUserProfileUpdate::FiscalYearStart(4)));
        assert!(updates.contains(&AuthUserProfileUpdate::CashTransferCategoryId(Some(200))));
        assert!(updates.contains(&AuthUserProfileUpdate::ImportLearningEnabled(false)));
        assert_eq!(
            profile_string(&json!("en"), "language").expect("profile string"),
            "en"
        );
        assert!(profile_string(&Value::Null, "language").is_err());
        assert!(valid_email_address("alice@example.test"));
        assert!(!valid_email_address("not-an-email"));
        assert!(!valid_email_address("a@b@c.com"));
        assert!(!valid_email_address("alice @example.test"));
        assert!(!valid_email_address("alice@example .test"));
        assert!(!valid_email_address("alice@-example.test"));
        assert!(!valid_email_address("alice@example-.test"));
        assert_eq!(
            profile_email_changed(
                &[AuthUserProfileUpdate::Email("new@example.test".to_string())],
                "old@example.test",
            )
            .as_deref(),
            Some("new@example.test")
        );
        assert!(profile_email_changed(
            &[AuthUserProfileUpdate::Email(
                "same@example.test".to_string()
            )],
            "same@example.test",
        )
        .is_none());

        assert!(profile_i64(&Value::Bool(true), "firstDayOfWeek").is_err());
        assert_eq!(
            profile_i64(&json!("42"), "firstDayOfWeek").expect("numeric string"),
            42
        );
        assert!(profile_i64(
            &Value::Number(serde_json::Number::from(u64::MAX)),
            "firstDayOfWeek"
        )
        .is_err());
        assert!(profile_i64(&json!({"unexpected": true}), "firstDayOfWeek").is_err());
        assert!(profile_bool(&Value::Bool(true), "importLearningEnabled").expect("bool"));
        assert!(profile_bool(&json!(1), "importLearningEnabled").expect("one"));
        assert!(profile_bool(&json!("yes"), "importLearningEnabled").is_err());
        assert!(!profile_bool(&json!("false"), "importLearningEnabled").expect("false string"));
        assert!(!profile_bool(&json!("0"), "importLearningEnabled").expect("zero string"));
        assert!(profile_bool(&Value::Null, "importLearningEnabled").is_err());
        assert_eq!(
            optional_profile_id(&Value::Null, "defaultAccountId").expect("null clears"),
            None
        );
        assert_eq!(
            optional_profile_id(&json!(""), "defaultAccountId").expect("empty clears"),
            None
        );
        assert!(optional_profile_id(&json!("-1"), "defaultAccountId").is_err());
        assert!(optional_profile_id(&json!("0"), "defaultAccountId").is_err());
        assert!(optional_profile_id(&json!("abc"), "defaultAccountId").is_err());
        assert_eq!(
            optional_profile_id(&json!("7"), "defaultAccountId").expect("positive id"),
            Some(7)
        );

        let mut headers = HeaderMap::new();
        assert!(avatar_data_url_from_multipart(&headers, b"").is_err());
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=\"quoted\""),
        );
        assert_eq!(
            multipart_boundary(
                headers
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
            )
            .as_deref(),
            Some("quoted")
        );

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=plain"),
        );
        let no_avatar =
            b"--plain\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nvalue\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, no_avatar).is_err());
        let empty_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\n\r\n\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, empty_avatar).is_err());
        let text_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nhello\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, text_avatar).is_err());
        let png_payload = b"\x89PNG\r\n\x1A\navatar";
        let png_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n\x89PNG\r\n\x1A\navatar\r\n--plain--\r\n";
        assert_eq!(
            avatar_data_url_from_multipart(&headers, png_avatar).expect("png avatar"),
            format!(
                "data:image/png;base64,{}",
                general_purpose::STANDARD.encode(png_payload)
            )
        );
        let mismatched_avatar = b"--plain\r\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.png\"\r\nContent-Type: image/jpeg\r\n\r\n\x89PNG\r\n\x1A\navatar\r\n--plain--\r\n";
        assert!(avatar_data_url_from_multipart(&headers, mismatched_avatar).is_err());
        assert!(avatar_data_url_from_payload(
            &vec![0_u8; MAX_AVATAR_BYTES + 1],
            Some("image/png".to_string())
        )
        .is_err());

        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data; boundary=lf"),
        );
        let lf_avatar =
            b"--lf\nContent-Disposition: form-data; name=\"avatar\"; filename=\"a.webp\"\n\nRIFFxxxxWEBPdata\n--lf--\n";
        assert_eq!(
            avatar_data_url_from_multipart(&headers, lf_avatar).expect("lf avatar"),
            "data:image/webp;base64,UklGRnh4eHhXRUJQZGF0YQ=="
        );
        assert_eq!(find_bytes(b"abc", b""), None);
        assert_eq!(find_bytes(b"abc", b"abcd"), None);

        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "autoSaveTransactionDraft", "settingValue": "draft"})
            )
            .expect("string setting")
            .setting_value,
            "draft"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "itemsCountInTransactionListPage", "settingValue": "25"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "itemsCountInTransactionListPage", "settingValue": "NaN?"})
            )
            .expect_err("invalid number")
            .message,
            "Invalid number value for itemsCountInTransactionListPage"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "showAmountInHomePage", "settingValue": "true"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "showAmountInHomePage", "settingValue": "1"})
            )
            .expect_err("invalid boolean")
            .message,
            "Invalid boolean value for showAmountInHomePage"
        );
        assert!(validate_application_cloud_setting(
            &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":true}"})
        )
        .is_ok());
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{"})
            )
            .expect_err("invalid json")
            .message,
            "Invalid JSON value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "[]"})
            )
            .expect_err("invalid map")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"\":false}"})
            )
            .expect_err("empty map key")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "overviewAccountFilterInHomePage", "settingValue": "{\"1\":\"yes\"}"})
            )
            .expect_err("non boolean map value")
            .message,
            "Invalid map value for overviewAccountFilterInHomePage"
        );
        assert_eq!(
            validate_application_cloud_setting(&json!({"settingKey": "", "settingValue": "x"}))
                .expect_err("empty key")
                .message,
            "settingKey is required"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "autoSaveTransactionDraft", "settingValue": true})
            )
            .expect_err("non string value")
            .message,
            "Invalid setting value for autoSaveTransactionDraft"
        );
        assert_eq!(
            validate_application_cloud_setting(
                &json!({"settingKey": "unsupported", "settingValue": "x"})
            )
            .expect_err("unsupported key")
            .message,
            "Unsupported setting key: unsupported"
        );
        assert_eq!(application_cloud_setting_type("unknown"), None);
    }
}
