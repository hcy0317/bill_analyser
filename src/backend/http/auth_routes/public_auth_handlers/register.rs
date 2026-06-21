#[tracing::instrument(level = "debug", skip_all)]
async fn register_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "register_handler", "business operation entered");
    let body = request_body_object(&body);
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let email = body
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let password = body
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if username.is_empty() || email.is_empty() || password.is_empty() {
        return auth_rest_error_response(AuthRestError::invalid_request(
            "Username, email and password are required",
        ));
    }
    if !state.config.auth_enable_user_registration {
        return auth_rest_error_response(AuthRestError::new(
            403,
            "Registration disabled",
            "User registration is currently disabled",
        ));
    }
    if let Err(message) = state.config.auth_password_policy.validate(password) {
        return auth_rest_error_response(AuthRestError::new(400, "Invalid password", message));
    }
    let default_package = match register_default_package_from_body(&body) {
        Ok(value) => value,
        Err(error) => return auth_rest_error_response(error),
    };

    let request_user_agent = header_value(&headers, header::USER_AGENT.as_str());
    let ip_address = login_client_ip(&headers, connect_info.map(|ConnectInfo(addr)| addr));

    let password_hash = match bcrypt::hash(password, bcrypt::DEFAULT_COST) {
        Ok(value) => value,
        Err(_) => {
            return auth_rest_error_response(AuthRestError::new(
                500,
                "Internal Server Error",
                "Rust auth register runtime password hashing error",
            ))
        }
    };
    let language = body
        .get("language")
        .and_then(Value::as_str)
        .unwrap_or("zh_Hans")
        .to_string();
    let default_currency = body
        .get("defaultCurrency")
        .and_then(Value::as_str)
        .unwrap_or("CNY")
        .to_string();
    let first_day_of_week = body
        .get("firstDayOfWeek")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let nickname = body
        .get("nickname")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let nickname = if nickname.is_empty() {
        username.clone()
    } else {
        nickname
    };
    let created_at = utc_now_text();

        return register_postgres_response(
            &state,
            &body,
            username,
            email,
            password_hash,
            nickname,
            language,
            default_currency,
            first_day_of_week,
            request_user_agent,
            ip_address,
            created_at,
            default_package,
        )
        .await;
}


#[allow(clippy::too_many_arguments)]
async fn register_postgres_response(
    state: &HttpAppState,
    body: &Map<String, Value>,
    username: String,
    email: String,
    password_hash: String,
    nickname: String,
    language: String,
    default_currency: String,
    first_day_of_week: i64,
    request_user_agent: String,
    ip_address: String,
    created_at: String,
    default_package: RegisterDefaultSeedPackage,
) -> Response {
    let runtime = match open_postgres_runtime(state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match postgres_auth_username_exists(runtime.pool(), &username).await {
        Ok(true) => {
            let _ = create_postgres_auth_log(
                runtime.pool(),
                &AuthLogDraft {
                    user_id: None,
                    username: username.clone(),
                    event_type: "register_failed".to_string(),
                    ip_address,
                    user_agent: request_user_agent,
                    success: false,
                    error_message: Some("Username already exists".to_string()),
                    metadata: None,
                    created_at,
                },
            )
            .await;
            return auth_rest_error_response(AuthRestError::new(
                409,
                "Username exists",
                "Username already exists",
            ));
        }
        Ok(false) => {}
        Err(_) => return db_error_response(),
    }
    match postgres_auth_email_exists(runtime.pool(), &email).await {
        Ok(true) => {
            let _ = create_postgres_auth_log(
                runtime.pool(),
                &AuthLogDraft {
                    user_id: None,
                    username: username.clone(),
                    event_type: "register_failed".to_string(),
                    ip_address,
                    user_agent: request_user_agent,
                    success: false,
                    error_message: Some("Email already exists".to_string()),
                    metadata: None,
                    created_at,
                },
            )
            .await;
            return auth_rest_error_response(AuthRestError::new(
                409,
                "Email exists",
                "Email already exists",
            ));
        }
        Ok(false) => {}
        Err(_) => return db_error_response(),
    }

    let register_result = match create_postgres_registered_user_with_defaults(
        runtime.pool(),
        &RegisterUserDraft {
            username: username.clone(),
            email: email.clone(),
            password_hash,
            nickname,
            language,
            default_currency,
            first_day_of_week,
            email_verified: !state.config.auth_require_email_verification,
            created_at: created_at.clone(),
        },
        &register_preset_categories_from_body(body),
        default_package,
        &AuthLogDraft {
            user_id: None,
            username: username.clone(),
            event_type: "register_success".to_string(),
            ip_address,
            user_agent: request_user_agent,
            success: true,
            error_message: None,
            metadata: None,
            created_at,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    let mut result = json!({
        "user_id": register_result.user_id,
        "username": username,
        "email": email,
        "needVerifyEmail": state.config.auth_require_email_verification,
        "presetCategoriesSaved": register_result.preset_categories_saved,
        "presetAccountsSaved": register_result.preset_accounts_saved,
        "message": "Registration successful",
    });
    if let Some(default_seed) = register_default_seed_response(&register_result.default_seed) {
        if let Some(object) = result.as_object_mut() {
            object.insert("defaultSeed".to_string(), default_seed);
        }
    }
    success_result(
        StatusCode::OK,
        result,
    )
}

fn register_default_seed_response(summary: &RegisterDefaultSeedSummary) -> Option<Value> {
    let package = summary.package.as_ref()?;
    Some(json!({
        "package": package,
        "categoriesCreated": summary.categories_created,
        "categoriesSkipped": summary.categories_skipped,
        "categoryRulesCreated": summary.rules_created,
        "categoryRulesSkipped": summary.rules_skipped,
        "accountsCreated": summary.accounts_created,
        "accountsSkipped": summary.accounts_skipped,
        "accountRulesCreated": summary.account_rules_created,
        "accountRulesSkipped": summary.account_rules_skipped,
        "rulesMissingTargets": summary.rules_missing_targets,
    }))
}
