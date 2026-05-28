// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
async fn ensure_postgres_sensitive_auth_failure_limit(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    event_type: &str,
) -> RouteResult<()> {
    let failure_count = count_postgres_auth_events_since(
        pool,
        user_id,
        event_type,
        &sensitive_auth_failure_window_start_text(),
    )
    .await
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

async fn record_postgres_sensitive_auth_failure(
    pool: &bill_analyser_db::PostgresPool,
    user: &AuthLoginUserRow,
    event_type: &str,
    ip_address: &str,
    user_agent: &str,
    error_message: &str,
    metadata: Option<Value>,
) -> RouteResult<()> {
    create_postgres_auth_log(
        pool,
        &AuthLogDraft {
            user_id: Some(user.profile.id),
            username: user.profile.username.clone(),
            event_type: event_type.to_string(),
            ip_address: ip_address.to_string(),
            user_agent: user_agent.to_string(),
            success: false,
            error_message: Some(error_message.to_string()),
            metadata: metadata.map(|value| value.to_string()),
            created_at: utc_now_text(),
        },
    )
    .await
    .map(|_| ())
    .map_err(|_| Box::new(db_error_response()))
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
