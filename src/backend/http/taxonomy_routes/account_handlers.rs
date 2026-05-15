async fn encryption_status_handler() -> Response {
    let encrypt_raw = std::env::var("BILL_DB_ENCRYPT").ok();
    let key_raw = std::env::var("BILL_DB_KEY").ok();
    let status = normalize_sqlcipher_status(encrypt_raw.as_deref(), key_raw.as_deref(), false);
    json_response(StatusCode::OK, encryption_status_response(&status))
}

async fn list_accounts_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());

    match repository.list_accounts(db_user_id(user_id)) {
        Ok(accounts) => success_result(StatusCode::OK, format_account_list_response(accounts)),
        Err(_) => db_error_response(),
    }
}

async fn get_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());

    match load_account_with_sub_accounts(&mut repository, account_id, db_user_id(user_id)) {
        Ok(Some(account)) => success_result(
            StatusCode::OK,
            Value::Object(backend_account_to_frontend(account)),
        ),
        Ok(None) => not_found("Account not found"),
        Err(_) => db_error_response(),
    }
}

async fn create_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = match frontend_account_to_backend(&body) {
        Ok(value) => Value::Object(value),
        Err(message) => return bad_request(message),
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());

    let account_id = match repository.create_account(&payload, db_user_id(user_id)) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match load_account_with_sub_accounts(&mut repository, account_id, db_user_id(user_id)) {
        Ok(Some(account)) => success_result(
            StatusCode::CREATED,
            Value::Object(backend_account_to_frontend(account)),
        ),
        Ok(None) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn update_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let sub_accounts = body.get("subAccounts").cloned();
    let mut payload = match frontend_account_to_backend(&body) {
        Ok(value) => value,
        Err(message) => return bad_request(message),
    };
    payload.remove("subAccounts");
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    if let Some(Value::Array(sub_accounts)) = sub_accounts.filter(|value| {
        value
            .as_array()
            .is_some_and(|sub_accounts| !sub_accounts.is_empty())
    }) {
        if let Err(response) =
            update_sub_accounts(&mut repository, account_id, user_id, &sub_accounts)
        {
            return *response;
        }
    }

    match repository.update_account(account_id, &Value::Object(payload), user_id) {
        Ok(true) => match load_account_with_sub_accounts(&mut repository, account_id, user_id) {
            Ok(Some(account)) => success_result(
                StatusCode::OK,
                Value::Object(backend_account_to_frontend(account)),
            ),
            Ok(None) => success_result(StatusCode::OK, json!({})),
            Err(_) => db_error_response(),
        },
        Ok(false) => not_found("Account not found"),
        Err(_) => db_error_response(),
    }
}

async fn delete_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.get_sub_accounts(account_id, user_id) {
        Ok(sub_accounts) => {
            for sub_account in sub_accounts {
                if let Some(sub_account_id) = sub_account.get("id").and_then(value_as_i64) {
                    if repository.delete_account(sub_account_id, user_id).is_err() {
                        return db_error_response();
                    }
                }
            }
        }
        Err(_) => return db_error_response(),
    }

    match repository.delete_account(account_id, user_id) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Account not found"),
        Err(_) => db_error_response(),
    }
}

async fn update_account_display_orders_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(new_display_orders) = body.get("newDisplayOrders") else {
        return bad_request("Missing newDisplayOrders parameter");
    };
    let Some(items) = new_display_orders.as_array() else {
        return bad_request("newDisplayOrders must be a list");
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(account_id) = item.get("id").and_then(value_as_i64) else {
            return bad_request("Each item must have id and displayOrder");
        };
        let Some(display_order) = item.get("displayOrder").and_then(value_as_i64) else {
            return bad_request("Each item must have id and displayOrder");
        };
        orders.push(AccountDisplayOrder {
            account_id,
            display_order,
        });
    }

    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());
    match repository.update_display_orders(&orders, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn sync_account_balances_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match sync_all_account_balances(runtime.connection_mut(), user_id) {
        Ok(result) => success_result(
            StatusCode::OK,
            format_sync_account_balances_response(result),
        ),
        Err(_) => db_error_response(),
    }
}

async fn move_account_transactions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(to_account_value) = body.get("toAccountId") else {
        return bad_request("toAccountId is required");
    };
    let Some(to_account_id) = value_as_i64(to_account_value) else {
        return bad_request("Account IDs must be valid integers");
    };
    if account_id == to_account_id {
        return bad_request("Source and target accounts must be different");
    }
    let password = string_or_default(body.get("password"), "");
    if password.trim().is_empty() {
        return bad_request("password is required");
    }

    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id_value = db_user_id(user_id);

    let password_valid = match verify_sensitive_account_operation_password(
        runtime.connection(),
        user_id_value,
        password.trim(),
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if !password_valid {
        create_account_audit_log_best_effort(
            runtime.connection(),
            AccountAuditLogDraft {
                operation_type: "move_transactions",
                target_id: account_id,
                details: json!({
                    "from_account_id": account_id,
                    "to_account_id": to_account_id,
                }),
                affected_count: 0,
                status: "failed",
                error_message: Some("Invalid password".to_string()),
                ip_address: audit_ip_address(&headers),
                user_agent: audit_user_agent(&headers),
            },
        );
        return error_response(StatusCode::UNAUTHORIZED, "Invalid password");
    }

    let result = {
        let mut repository = AccountsRepository::new(runtime.connection_mut());
        repository.move_all_transactions(account_id, to_account_id, user_id_value)
    };
    match result {
        Ok(result) if result.success => {
            if sync_all_account_balances(runtime.connection_mut(), user_id).is_err() {
                return db_error_response();
            }
            create_account_audit_log_best_effort(
                runtime.connection(),
                AccountAuditLogDraft {
                    operation_type: "move_transactions",
                    target_id: account_id,
                    details: json!({
                        "from_account_id": account_id,
                        "to_account_id": to_account_id,
                        "moved_count": result.moved_count,
                    }),
                    affected_count: result.moved_count,
                    status: "success",
                    error_message: None,
                    ip_address: audit_ip_address(&headers),
                    user_agent: audit_user_agent(&headers),
                },
            );
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": true,
                    "moved_count": result.moved_count,
                }),
            )
        }
        Ok(result) => {
            create_account_audit_log_best_effort(
                runtime.connection(),
                AccountAuditLogDraft {
                    operation_type: "move_transactions",
                    target_id: account_id,
                    details: json!({
                        "from_account_id": account_id,
                        "to_account_id": to_account_id,
                    }),
                    affected_count: 0,
                    status: "failed",
                    error_message: Some(result.message.clone()),
                    ip_address: audit_ip_address(&headers),
                    user_agent: audit_user_agent(&headers),
                },
            );
            error_response(StatusCode::INTERNAL_SERVER_ERROR, result.message)
        }
        Err(_) => db_error_response(),
    }
}

async fn clear_account_transactions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let password = string_or_default(body.get("password"), "");
    if password.trim().is_empty() {
        return bad_request("password is required");
    }

    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id_value = db_user_id(user_id);

    let password_valid = match verify_sensitive_account_operation_password(
        runtime.connection(),
        user_id_value,
        password.trim(),
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if !password_valid {
        create_account_audit_log_best_effort(
            runtime.connection(),
            AccountAuditLogDraft {
                operation_type: "delete_transactions",
                target_id: account_id,
                details: json!({ "account_id": account_id }),
                affected_count: 0,
                status: "failed",
                error_message: Some("Invalid password".to_string()),
                ip_address: audit_ip_address(&headers),
                user_agent: audit_user_agent(&headers),
            },
        );
        return error_response(StatusCode::UNAUTHORIZED, "Invalid password");
    }

    let result = {
        let mut repository = AccountsRepository::new(runtime.connection_mut());
        repository.delete_all_transactions_by_account(account_id, user_id_value)
    };
    match result {
        Ok(result) if result.success => {
            if sync_all_account_balances(runtime.connection_mut(), user_id).is_err() {
                return db_error_response();
            }
            create_account_audit_log_best_effort(
                runtime.connection(),
                AccountAuditLogDraft {
                    operation_type: "delete_transactions",
                    target_id: account_id,
                    details: json!({
                        "account_id": account_id,
                        "deleted_count": result.deleted_count,
                    }),
                    affected_count: result.deleted_count,
                    status: "success",
                    error_message: None,
                    ip_address: audit_ip_address(&headers),
                    user_agent: audit_user_agent(&headers),
                },
            );
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": true,
                    "deleted_count": result.deleted_count,
                }),
            )
        }
        Ok(result) => {
            create_account_audit_log_best_effort(
                runtime.connection(),
                AccountAuditLogDraft {
                    operation_type: "delete_transactions",
                    target_id: account_id,
                    details: json!({ "account_id": account_id }),
                    affected_count: 0,
                    status: "failed",
                    error_message: Some(result.message.clone()),
                    ip_address: audit_ip_address(&headers),
                    user_agent: audit_user_agent(&headers),
                },
            );
            error_response(StatusCode::INTERNAL_SERVER_ERROR, result.message)
        }
        Err(_) => db_error_response(),
    }
}

