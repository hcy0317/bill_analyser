// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn system_version_handler() -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "system_version_handler", "business operation entered");
    success_result(
        StatusCode::OK,
        json!({
            "version": BILL_ANALYSER_APP_VERSION,
            "commitHash": "",
            "buildTime": ""
        }),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_user_data_statistics_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "get_user_data_statistics_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match get_postgres_user_data_statistics(runtime.pool(), auth.user_id).await {
            Ok(statistics) => {
                json_response(StatusCode::OK, user_data_statistics_response(&statistics))
            }
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn export_user_data_handler(
    State(state): State<HttpAppState>,
    Path(file_type): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "export_user_data_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let export_type = match normalize_user_data_export_type(&file_type) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let categories = match list_postgres_user_data_categories(runtime.pool(), auth.user_id).await
        {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        let filters = build_user_data_export_filters(&query, &categories);
        let bundle = match load_postgres_user_data_export(runtime.pool(), auth.user_id, &filters)
            .await
        {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        return user_data_export_response(bundle, export_type);
}

fn user_data_export_response(bundle: UserDataExportBundle, export_type: UserDataExportType) -> Response {
    let export_text = match render_user_data_export(&bundle, export_type.delimiter()) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    user_data_export_response_text(export_text, export_type)
}

fn user_data_export_response_text(export_text: String, export_type: UserDataExportType) -> Response {
    let filename = format!(
        "bill_analyser_export_{}.{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        export_type.extension()
    );
    let mut response = Response::new(Body::from(format!("\u{feff}{export_text}")));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(export_type.content_type()),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename={filename}")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_user_transactions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "clear_user_transactions_handler", "business operation entered");
    clear_user_data_handler(
        state,
        headers,
        connect_info,
        body,
        UserDataClearKind::Transactions,
    )
    .await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_all_user_data_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "clear_all_user_data_handler", "business operation entered");
    clear_user_data_handler(state, headers, connect_info, body, UserDataClearKind::All).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_user_data_handler(
    state: HttpAppState,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
    kind: UserDataClearKind,
) -> Response {
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);

        return clear_postgres_user_data_handler(state, auth, connect_info, headers, body, kind).await;
}

async fn clear_postgres_user_data_handler(
    state: HttpAppState,
    auth: AuthenticatedUser,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    body: Map<String, Value>,
    kind: UserDataClearKind,
) -> Response {
    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user = match get_postgres_login_user_by_id(runtime.pool(), auth.user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return json_response(
                StatusCode::NOT_FOUND,
                json!({
                    "success": false,
                    "error": "User not found"
                }),
            );
        }
        Err(_) => return db_error_response(),
    };
    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
    if let Err(response) = ensure_postgres_sensitive_auth_failure_limit(
        runtime.pool(),
        user.profile.id,
        USER_DATA_CLEAR_AUTH_FAILED_EVENT,
    )
    .await
    {
        return *response;
    }
    let auth_mode =
        match resolve_destructive_user_data_auth_postgres(runtime.pool(), &body, &state, &user)
            .await
        {
            Ok(value) => value,
            Err(SensitiveTwoFactorAuthError::Missing) => {
                if let Err(response) = record_postgres_sensitive_auth_failure(
                    runtime.pool(),
                    &user,
                    USER_DATA_CLEAR_AUTH_FAILED_EVENT,
                    &ip_address,
                    &request_user_agent,
                    "Missing current password or step-up token",
                    Some(json!({
                        "operation_type": kind.operation_type(),
                        "reason": "missing_credentials"
                    })),
                )
                .await
                {
                    return *response;
                }
                return sensitive_auth_missing_response();
            }
            Err(SensitiveTwoFactorAuthError::Invalid) => {
                if let Err(response) = record_postgres_sensitive_auth_failure(
                    runtime.pool(),
                    &user,
                    USER_DATA_CLEAR_AUTH_FAILED_EVENT,
                    &ip_address,
                    &request_user_agent,
                    "Invalid current password or step-up token",
                    Some(json!({
                        "operation_type": kind.operation_type(),
                        "reason": "invalid_credentials"
                    })),
                )
                .await
                {
                    return *response;
                }
                return sensitive_auth_invalid_response();
            }
            Err(SensitiveTwoFactorAuthError::Db) => return db_error_response(),
        };
    let now = utc_now_text();
    match kind {
        UserDataClearKind::Transactions => {
            let deleted_count =
                match clear_postgres_user_transactions(runtime.pool(), auth.user_id).await {
                    Ok(value) => value,
                    Err(_) => return db_error_response(),
                };
            create_postgres_user_data_audit_log_best_effort(
                runtime.pool(),
                UserDataAuditLogDraft {
                    operation_type: kind.operation_type(),
                    user_id: auth.user_id,
                    details: json!({
                        "deleted_count": deleted_count,
                        "auth_mode": auth_mode.as_str()
                    }),
                    affected_count: deleted_count,
                    ip_address: &ip_address,
                    user_agent: &request_user_agent,
                    now: &now,
                },
            )
            .await;
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": true,
                    "deletedCount": deleted_count
                }),
            )
        }
        UserDataClearKind::All => {
            let result = match clear_postgres_user_data(runtime.pool(), auth.user_id).await {
                Ok(value) => value,
                Err(_) => return db_error_response(),
            };
            let affected_count = result.counts.values().copied().sum::<i64>();
            let details = user_data_clear_all_audit_details(&result.counts, auth_mode.as_str());
            create_postgres_user_data_audit_log_best_effort(
                runtime.pool(),
                UserDataAuditLogDraft {
                    operation_type: kind.operation_type(),
                    user_id: auth.user_id,
                    details,
                    affected_count,
                    ip_address: &ip_address,
                    user_agent: &request_user_agent,
                    now: &now,
                },
            )
            .await;
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": true,
                    "counts": result.counts
                }),
            )
        }
    }
}
