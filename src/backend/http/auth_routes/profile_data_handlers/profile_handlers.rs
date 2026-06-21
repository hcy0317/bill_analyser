// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：读取当前用户 profile 响应，包含基本资料、偏好、默认账户分类和头像字段。
async fn get_profile_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "get_profile_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match get_postgres_auth_user_profile(runtime.pool(), auth.user_id).await {
            Ok(Some(user)) => success_result(StatusCode::OK, user_profile_payload(&user)),
            Ok(None) => auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            )),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：处理 profile 更新请求，解析用户资料字段后校验 user-scope 引用并写入审计。
async fn update_profile_handler(
    State(state): State<HttpAppState>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "update_profile_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    if body.is_empty() {
        return auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Request body is required",
        ));
    }

        return update_postgres_profile_response(&state, auth.user_id, connect_info, &headers, &body)
            .await;
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：处理头像上传请求，校验文件类型和大小后写入头像 URL/provider。
async fn update_profile_avatar_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "update_profile_avatar_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let avatar = match avatar_data_url_from_multipart(&headers, &body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    update_profile_avatar_value(&state, auth.user_id, avatar).await
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：清除当前用户头像字段，并把 profile 变更结果返回给前端。
async fn remove_profile_avatar_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "remove_profile_avatar_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    update_profile_avatar_value(&state, auth.user_id, String::new()).await
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：根据上传结果更新头像字段，复用 profile 更新事务和响应投影。
async fn update_profile_avatar_value(
    state: &HttpAppState,
    user_id: UserId,
    avatar: String,
) -> Response {

        let runtime = match open_postgres_runtime(state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let updates = [AuthUserProfileUpdate::Avatar(avatar)];
        if !matches!(
            update_postgres_auth_user_profile(runtime.pool(), user_id, &updates, &utc_now_text())
                .await,
            Ok(true)
        ) {
            return auth_rest_error_response(AuthRestError::new(
                500,
                "Update failed",
                "Failed to update avatar",
            ));
        }
        return match get_postgres_auth_user_profile(runtime.pool(), user_id).await {
            Ok(Some(user)) => success_result(StatusCode::OK, user_profile_payload(&user)),
            Ok(None) => {
                auth_rest_error_response(AuthRestError::new(404, "User not found", "User not found"))
            }
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：为已登录用户重发邮箱验证邮件，保持发送开关和限流审计合同。
async fn resend_profile_verification_email_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "resend_profile_verification_email_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user = match get_postgres_auth_user_profile(runtime.pool(), auth.user_id).await {
            Ok(Some(value)) => value,
            Ok(None) => {
                return auth_rest_error_response(AuthRestError::new(
                    404,
                    "User not found",
                    "User not found",
                ));
            }
            Err(_) => return db_error_response(),
        };
        if user.email.trim().is_empty() {
            return auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Email is required to resend verification email",
            ));
        }
        let since =
            (Utc::now() - ChronoDuration::minutes(PROFILE_VERIFICATION_RESEND_WINDOW_MINUTES))
                .format("%Y-%m-%dT%H:%M:%S%.f")
                .to_string();
        let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));
        let created_at = utc_now_text();
        return match create_postgres_auth_log_under_event_limit(
            runtime.pool(),
            auth.user_id,
            "verification_email_resend_requested",
            &since,
            PROFILE_VERIFICATION_RESEND_LIMIT,
            &AuthLogDraft {
                user_id: Some(user.id),
                username: user.username.clone(),
                event_type: "verification_email_resend_requested".to_string(),
                ip_address,
                user_agent: request_user_agent,
                success: true,
                error_message: None,
                metadata: Some(
                    json!({
                        "email_present": true,
                        "email_verified": user.email_verified,
                        "require_email_verification": state.config.auth_require_email_verification,
                        "delivery": "not_configured_mock_success"
                    })
                    .to_string(),
                ),
                created_at,
            },
        )
        .await
        {
            Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
            Ok(false) => auth_rest_error_response(AuthRestError::new(
                429,
                "Too Many Requests",
                "Too many verification email resend requests",
            )),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：读取当前用户 profile cloud settings，用于桌面和移动端同步设置页。
async fn get_profile_cloud_settings_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "get_profile_cloud_settings_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_application_cloud_settings(runtime.pool(), auth.user_id).await {
            Ok(settings) if settings.is_empty() => success_result(StatusCode::OK, Value::Bool(false)),
            Ok(settings) => {
                success_result(StatusCode::OK, application_cloud_settings_payload(settings))
            }
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：更新 profile cloud settings，支持 full update 并按当前用户边界写入。
async fn update_profile_cloud_settings_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "update_profile_cloud_settings_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = request_body_object(&body);
    let empty_settings = Vec::new();
    let settings = match body.get("settings") {
        Some(Value::Array(values)) => values,
        Some(_) => {
            return auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "settings must be an array",
            ));
        }
        None => &empty_settings,
    };
    let mut drafts = Vec::with_capacity(settings.len());
    for setting in settings {
        match validate_application_cloud_setting(setting) {
            Ok(value) => drafts.push(value),
            Err(error) => return auth_rest_error_response(error),
        }
    }
    let full_update = body
        .get("fullUpdate")
        .and_then(Value::as_bool)
        .unwrap_or(false);

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match update_postgres_application_cloud_settings(
            runtime.pool(),
            auth.user_id,
            &drafts,
            full_update,
            &utc_now_text(),
        )
        .await
        {
            Ok(_) => success_result(StatusCode::OK, Value::Bool(true)),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：关闭当前用户 profile cloud settings，同步清理服务端云同步配置。
async fn delete_profile_cloud_settings_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "delete_profile_cloud_settings_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match delete_postgres_application_cloud_settings(runtime.pool(), auth.user_id).await {
            Ok(_) => success_result(StatusCode::OK, Value::Bool(true)),
            Err(_) => db_error_response(),
        };
}

// 中文说明：把已校验 profile update 写入 PostgreSQL 并转换成统一 REST profile 响应。
async fn update_postgres_profile_response(
    state: &HttpAppState,
    user_id: UserId,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: &HeaderMap,
    body: &Map<String, Value>,
) -> Response {
    let runtime = match open_postgres_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let current_user = match get_postgres_auth_user_profile(runtime.pool(), user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User not found",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let updates =
        match validated_postgres_profile_updates(runtime.pool(), user_id, &current_user, body).await {
            Ok(value) => value,
            Err(error) => return auth_rest_error_response(error),
        };
    let email_changed = profile_email_changed(&updates, &current_user.email);
    let updated_at = utc_now_text();
    let update_result = if email_changed.is_some() {
        let request_user_agent = header_value(headers, header::USER_AGENT.as_str());
        let ip_address = client_ip(headers, connect_info.map(|ConnectInfo(addr)| addr));
        update_postgres_auth_user_profile_with_auth_log(
            runtime.pool(),
            user_id,
            &updates,
            &updated_at,
            &AuthLogDraft {
                user_id: Some(user_id),
                username: current_user.username.clone(),
                event_type: "profile_email_changed".to_string(),
                ip_address,
                user_agent: request_user_agent,
                success: true,
                error_message: None,
                metadata: Some(
                    json!({
                        "email_changed": true,
                        "email_verified_reset": true
                    })
                    .to_string(),
                ),
                created_at: updated_at.clone(),
            },
        )
        .await
    } else {
        update_postgres_auth_user_profile(runtime.pool(), user_id, &updates, &updated_at).await
    };
    if !updates.is_empty() && !matches!(update_result, Ok(true)) {
        return auth_rest_error_response(AuthRestError::new(
            500,
            "Update failed",
            "Failed to update user profile",
        ));
    }

    match get_postgres_auth_user_profile(runtime.pool(), user_id).await {
        Ok(Some(user)) => success_result(
            StatusCode::OK,
            json!({ "user": user_profile_payload(&user) }),
        ),
        Ok(None) => auth_rest_error_response(AuthRestError::new(
            404,
            "User not found after update",
            "User not found after update",
        )),
        Err(_) => db_error_response(),
    }
}

// 中文说明：组合校验 profile 更新字段，确保邮箱、账户和分类引用都满足当前用户边界。
async fn validated_postgres_profile_updates(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    current_user: &AuthUserProfileRow,
    body: &Map<String, Value>,
) -> Result<Vec<AuthUserProfileUpdate>, AuthRestError> {
    if body.contains_key("avatar") {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar must be updated via /api/profile/avatar",
        ));
    }
    let updates = profile_updates_from_body(body)?;
    validate_postgres_profile_email_update(pool, user_id, current_user, &updates).await?;
    validate_postgres_profile_reference_ids(pool, user_id, &updates).await?;
    Ok(updates)
}

// 中文说明：校验 profile 邮箱变更，重置验证状态并阻止邮箱被其他用户占用。
async fn validate_postgres_profile_email_update(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    current_user: &AuthUserProfileRow,
    updates: &[AuthUserProfileUpdate],
) -> Result<(), AuthRestError> {
    let Some(new_email) = updates.iter().find_map(|update| match update {
        AuthUserProfileUpdate::Email(value) => Some(value),
        _ => None,
    }) else {
        return Ok(());
    };
    if !valid_email_address(new_email) {
        return Err(AuthRestError::new(
            400,
            "Bad Request",
            "Invalid email address",
        ));
    }
    if new_email == &current_user.email {
        return Ok(());
    }
    match postgres_auth_email_exists_for_other_user(pool, user_id, new_email).await {
        Ok(false) => Ok(()),
        Ok(true) => Err(AuthRestError::new(
            409,
            "Email exists",
            "Email already exists",
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

// 中文说明：校验 profile 中默认账户、现金账户和现金转账分类 ID，非法引用会被拒绝。
async fn validate_postgres_profile_reference_ids(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
) -> Result<(), AuthRestError> {
    for update in updates {
        match update {
            AuthUserProfileUpdate::DefaultAccountId(Some(account_id)) => {
                validate_postgres_profile_account_id(pool, user_id, *account_id, "defaultAccountId")
                    .await?;
            }
            AuthUserProfileUpdate::CashAccountId(Some(account_id)) => {
                validate_postgres_profile_account_id(pool, user_id, *account_id, "cashAccountId")
                    .await?;
            }
            AuthUserProfileUpdate::CashTransferCategoryId(Some(category_id)) => {
                validate_postgres_profile_category_id(
                    pool,
                    user_id,
                    *category_id,
                    "cashTransferCategoryId",
                )
                .await?;
            }
            _ => {}
        }
    }
    Ok(())
}

// 中文说明：校验单个 profile 账户 ID 是否属于当前用户，空值按未指定处理。
async fn validate_postgres_profile_account_id(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    account_id: i64,
    field_name: &str,
) -> Result<(), AuthRestError> {
    match postgres_auth_account_belongs_to_user(pool, user_id, account_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("{field_name} is invalid"),
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

// 中文说明：校验单个 profile 分类 ID 是否属于当前用户，空值按未指定处理。
async fn validate_postgres_profile_category_id(
    pool: &bill_analyser_db::PostgresPool,
    user_id: UserId,
    category_id: i64,
    field_name: &str,
) -> Result<(), AuthRestError> {
    match postgres_auth_category_belongs_to_user(pool, user_id, category_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(AuthRestError::new(
            400,
            "Bad Request",
            format!("{field_name} is invalid"),
        )),
        Err(_) => Err(AuthRestError::new(
            500,
            "Internal Server Error",
            "Rust auth token runtime DB error",
        )),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：列出当前用户绑定的外部登录身份，供安全设置页展示和解绑。
async fn list_profile_external_auths_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "list_profile_external_auths_handler", "business operation entered");
    let auth = match authenticated_user(&headers, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let rows = match list_postgres_user_external_auths(runtime.pool(), auth.user_id).await {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        let mut result: Vec<ExternalAuthInfo> =
            rows.into_iter().map(ExternalAuthInfo::linked).collect();

        let oauth2_provider = state.config.auth_oauth2_provider.trim();
        if state.config.auth_enable_oauth2
            && !oauth2_provider.is_empty()
            && !result
                .iter()
                .any(|item| item.external_auth_type == oauth2_provider)
        {
            result.push(ExternalAuthInfo {
                external_auth_category: "oauth2".to_string(),
                external_auth_type: oauth2_provider.to_string(),
                linked: false,
                external_username: String::new(),
                created_at: 0,
            });
        }

        result.sort_by(|left, right| {
            right
                .linked
                .cmp(&left.linked)
                .then_with(|| left.external_auth_type.cmp(&right.external_auth_type))
                .then_with(|| right.created_at.cmp(&left.created_at))
        });

        return success_result(StatusCode::OK, external_auths_payload(result));
}
