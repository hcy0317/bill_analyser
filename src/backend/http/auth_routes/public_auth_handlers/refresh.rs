#[tracing::instrument(level = "debug", skip_all)]
async fn refresh_token_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "refresh_token_handler", "business operation entered");
    let body = request_body_object(&body);
    let refresh_token = body
        .get("refreshToken")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if refresh_token.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Refresh token is required",
        ));
    }

    let claims = match validate_refresh_jwt(refresh_token, &state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        return refresh_postgres_response(
            &state,
            claims.user_id,
            refresh_token,
            &headers,
            connect_info.map(|ConnectInfo(addr)| addr),
        )
        .await;
}


async fn refresh_postgres_response(
    state: &HttpAppState,
    user_id: UserId,
    refresh_token: &str,
    headers: &HeaderMap,
    peer_addr: Option<SocketAddr>,
) -> Response {
    let runtime = match open_postgres_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let refresh_token_hash = sha256_hex(refresh_token);
    let refresh_session =
        match get_postgres_active_refresh_session(runtime.pool(), &refresh_token_hash).await {
            Ok(Some(value)) => value,
            Ok(None) => return invalid_refresh_token_response(),
            Err(_) => return db_error_response(),
        };
    if refresh_session.user_id != user_id || !refresh_session.user_is_active {
        return invalid_refresh_token_response();
    }
    if refresh_session_is_expired(&refresh_session.refresh_expires_at) {
        return refresh_token_expired_response();
    }
    let user = match get_postgres_auth_user_profile(runtime.pool(), user_id).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return auth_rest_error_response(AuthRestError::new(
                404,
                "User not found",
                "User does not exist",
            ));
        }
        Err(_) => return db_error_response(),
    };
    let tokens = match issue_session_tokens(user.id, &user.username, state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let request_user_agent = header_value(headers, header::USER_AGENT.as_str());
    let ip_address = client_ip(headers, peer_addr);
    let session_id = match rotate_postgres_refresh_token_session(
        runtime.pool(),
        refresh_session.id,
        &refresh_token_hash,
        &CreateTokenSessionDraft {
            user_id: user.id,
            token_hash: sha256_hex(&tokens.access_token),
            refresh_token_hash: Some(sha256_hex(&tokens.refresh_token)),
            expires_at: tokens.expires_at.clone(),
            refresh_expires_at: Some(tokens.refresh_expires_at.clone()),
            user_agent: request_user_agent.clone(),
            ip_address: ip_address.clone(),
            created_at: now_text(),
        },
    )
    .await
    {
        Ok(Some(value)) => value,
        Ok(None) => return invalid_refresh_token_response(),
        Err(_) => return db_error_response(),
    };
    if session_id <= 0 {
        return db_error_response();
    }
    let _ = create_postgres_auth_log(
        runtime.pool(),
        &AuthLogDraft {
            user_id: Some(user.id),
            username: user.username.clone(),
            event_type: "token_refresh".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: Some(json!({ "session_id": session_id }).to_string()),
            created_at: utc_now_text(),
        },
    )
    .await;
    let cloud_settings = match list_postgres_application_cloud_settings(runtime.pool(), user.id).await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    success_result(
        StatusCode::OK,
        json!({
            "token": tokens.access_token,
            "refreshToken": tokens.refresh_token,
            "newToken": tokens.access_token,
            "user": user_profile_payload(&user),
            "applicationCloudSettings": application_cloud_settings_payload(cloud_settings),
        }),
    )
}
