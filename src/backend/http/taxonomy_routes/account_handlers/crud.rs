#[tracing::instrument(level = "debug", skip_all)]
async fn list_accounts_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "list_accounts_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_accounts(runtime.pool(), db_user_id(user_id)).await {
            Ok(accounts) => success_result(StatusCode::OK, format_account_list_response(accounts)),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "get_account_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match load_postgres_account_with_sub_accounts(
            runtime.pool(),
            account_id,
            db_user_id(user_id),
        )
        .await
        {
            Ok(Some(account)) => success_result(
                StatusCode::OK,
                Value::Object(backend_account_to_frontend(account)),
            ),
            Ok(None) => not_found("Account not found"),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "create_account_handler", "business operation entered");
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

        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let account_id = match create_postgres_account(runtime.pool(), &payload, user_id).await {
            Ok(value) => value,
            Err(bill_analyser_db::DbError::InvalidOperation(message)) => {
                return bad_request(message)
            }
            Err(_) => return db_error_response(),
        };
        return match load_postgres_account_with_sub_accounts(runtime.pool(), account_id, user_id)
            .await
        {
            Ok(Some(account)) => success_result(
                StatusCode::CREATED,
                Value::Object(backend_account_to_frontend(account)),
            ),
            Ok(None) => db_error_response(),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_account_handler", "business operation entered");
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

        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);

        if let Some(Value::Array(sub_accounts)) = sub_accounts.filter(|value| {
            value
                .as_array()
                .is_some_and(|sub_accounts| !sub_accounts.is_empty())
        }) {
            if let Err(response) =
                update_postgres_sub_accounts(runtime.pool(), account_id, user_id, &sub_accounts)
                    .await
            {
                return *response;
            }
        }

        return match update_postgres_account(
            runtime.pool(),
            account_id,
            &Value::Object(payload),
            user_id,
        )
        .await
        {
            Ok(true) => {
                match load_postgres_account_with_sub_accounts(runtime.pool(), account_id, user_id)
                    .await
                {
                    Ok(Some(account)) => success_result(
                        StatusCode::OK,
                        Value::Object(backend_account_to_frontend(account)),
                    ),
                    Ok(None) => success_result(StatusCode::OK, json!({})),
                    Err(_) => db_error_response(),
                }
            }
            Ok(false) => not_found("Account not found"),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_account_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "delete_account_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);

        let sub_accounts = match get_postgres_sub_accounts(runtime.pool(), account_id, user_id).await
        {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        for sub_account in sub_accounts {
            if let Some(sub_account_id) = sub_account.get("id").and_then(value_as_i64) {
                if delete_postgres_account(runtime.pool(), sub_account_id, user_id)
                    .await
                    .is_err()
                {
                    return db_error_response();
                }
            }
        }

        return match delete_postgres_account(runtime.pool(), account_id, user_id).await {
            Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
            Ok(false) => not_found("Account not found"),
            Err(_) => db_error_response(),
        };
}
