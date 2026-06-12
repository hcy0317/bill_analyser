// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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

#[tracing::instrument(level = "debug", skip_all)]
async fn update_account_display_orders_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "update_account_display_orders_handler", "business operation entered");
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


        let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match update_postgres_account_display_orders(
            runtime.pool(),
            &orders,
            db_user_id(user_id),
        )
        .await
        {
            Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
            Ok(false) => db_error_response(),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn sync_account_balances_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "sync_account_balances_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match sync_all_postgres_account_balances(runtime.pool(), user_id).await {
        Ok(result) => success_result(
            StatusCode::OK,
            format_sync_account_balances_response(result),
        ),
        Err(_) => db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn move_account_transactions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "move_account_transactions_handler", "business operation entered");
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

    let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id_value = db_user_id(user_id);

    let password_valid = match verify_sensitive_account_operation_password_postgres(
        runtime.pool(),
        user_id_value,
        password.trim(),
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if !password_valid {
        create_account_audit_log_best_effort_postgres(
            runtime.pool(),
            user_id_value,
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
        )
        .await;
        return error_response(StatusCode::UNAUTHORIZED, "Invalid password");
    }

    let result =
        move_all_postgres_account_transactions(runtime.pool(), account_id, to_account_id, user_id_value)
            .await;
    match result {
        Ok(result) if result.success => {
            if sync_all_postgres_account_balances(runtime.pool(), user_id)
                .await
                .is_err()
            {
                return db_error_response();
            }
            create_account_audit_log_best_effort_postgres(
                runtime.pool(),
                user_id_value,
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
            )
            .await;
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
            create_account_audit_log_best_effort_postgres(
                runtime.pool(),
                user_id_value,
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
            )
            .await;
            error_response(StatusCode::INTERNAL_SERVER_ERROR, result.message)
        }
        Err(_) => db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_account_transactions_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "clear_account_transactions_handler", "business operation entered");
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

    let runtime = match open_postgres_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id_value = db_user_id(user_id);

    let password_valid = match verify_sensitive_account_operation_password_postgres(
        runtime.pool(),
        user_id_value,
        password.trim(),
    )
    .await
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if !password_valid {
        create_account_audit_log_best_effort_postgres(
            runtime.pool(),
            user_id_value,
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
        )
        .await;
        return error_response(StatusCode::UNAUTHORIZED, "Invalid password");
    }

    let result =
        clear_postgres_account_transactions(runtime.pool(), account_id, user_id_value).await;
    match result {
        Ok(result) if result.success => {
            if sync_all_postgres_account_balances(runtime.pool(), user_id)
                .await
                .is_err()
            {
                return db_error_response();
            }
            create_account_audit_log_best_effort_postgres(
                runtime.pool(),
                user_id_value,
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
            )
            .await;
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
            create_account_audit_log_best_effort_postgres(
                runtime.pool(),
                user_id_value,
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
            )
            .await;
            error_response(StatusCode::INTERNAL_SERVER_ERROR, result.message)
        }
        Err(_) => db_error_response(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountTransactionsMoveResult {
    success: bool,
    message: String,
    moved_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountTransactionsClearResult {
    success: bool,
    message: String,
    deleted_count: i64,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn sync_all_postgres_account_balances(
    pool: &PostgresPool,
    user_id: UserId,
) -> bill_analyser_db::DbResult<SyncAllAccountBalancesResult> {
    let user_id_value = db_user_id(user_id);
    let account_rows = sqlx::query(
        "SELECT id, name, balance_cents, metadata FROM accounts WHERE user_id = $1",
    )
    .bind(user_id_value)
    .fetch_all(pool)
    .await?;
    let bill_rows = sqlx::query(
        r#"
        SELECT transaction_type, amount_cents, source_account_id, target_account_id,
               transfer_target_account_id, standard_payload
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
        "#,
    )
    .bind(user_id_value)
    .fetch_all(pool)
    .await?;

    let mut deltas = std::collections::BTreeMap::<i64, i64>::new();
    for row in bill_rows {
        let transaction_type: String = row.try_get("transaction_type")?;
        let amount_cents: i64 = row.try_get("amount_cents")?;
        let source_account_id: Option<i64> = row.try_get("source_account_id")?;
        let target_account_id: Option<i64> = row.try_get("target_account_id")?;
        let transfer_target_account_id: Option<i64> = row.try_get("transfer_target_account_id")?;
        let standard_payload: Value = row.try_get("standard_payload")?;
        for (account_id, delta) in postgres_bill_balance_deltas(
            &transaction_type,
            amount_cents,
            source_account_id,
            target_account_id.or(transfer_target_account_id),
            &standard_payload,
        ) {
            *deltas.entry(account_id).or_default() += delta;
        }
    }

    let mut discrepancies = Vec::new();
    let mut synced_accounts = 0_usize;
    for row in &account_rows {
        let account_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let old_balance_cents: i64 = row.try_get("balance_cents")?;
        let metadata: Value = row.try_get("metadata")?;
        let initial_balance_cents =
            metadata_initial_balance_cents(&metadata, old_balance_cents);
        let new_balance_cents = initial_balance_cents + deltas.get(&account_id).copied().unwrap_or(0);
        if new_balance_cents != old_balance_cents {
            sqlx::query(
                r#"
                UPDATE accounts
                SET balance_cents = $3,
                    updated_at = now(),
                    version = version + 1
                WHERE user_id = $1 AND id = $2
                "#,
            )
            .bind(user_id_value)
            .bind(account_id)
            .bind(new_balance_cents)
            .execute(pool)
            .await?;
            synced_accounts += 1;
            discrepancies.push(AccountBalanceDiscrepancy {
                account_id,
                name,
                old_balance_cents,
                new_balance_cents,
                diff_cents: new_balance_cents - old_balance_cents,
            });
        }
    }

    Ok(SyncAllAccountBalancesResult {
        total_accounts: account_rows.len(),
        synced_accounts,
        discrepancies,
        errors: Vec::new(),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn move_all_postgres_account_transactions(
    pool: &PostgresPool,
    from_account_id: i64,
    to_account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<AccountTransactionsMoveResult> {
    if from_account_id == to_account_id {
        return Ok(account_move_failure(
            "Source and target accounts must be different",
        ));
    }
    let mut transaction = pool.begin().await?;
    if !postgres_account_exists(&mut transaction, from_account_id, user_id).await? {
        return Ok(account_move_failure("Source account not found"));
    }
    if !postgres_account_exists(&mut transaction, to_account_id, user_id).await? {
        return Ok(account_move_failure("Target account not found"));
    }
    let moved_count = sqlx::query(
        r#"
        UPDATE bills
        SET account_id = CASE WHEN account_id = $2 THEN $3 ELSE account_id END,
            source_account_id = CASE WHEN source_account_id = $2 THEN $3 ELSE source_account_id END,
            target_account_id = CASE WHEN target_account_id = $2 THEN $3 ELSE target_account_id END,
            transfer_target_account_id = CASE WHEN transfer_target_account_id = $2 THEN $3 ELSE transfer_target_account_id END,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(from_account_id)
    .bind(to_account_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected() as i64;
    transaction.commit().await?;
    Ok(AccountTransactionsMoveResult {
        success: true,
        message: "Transactions moved successfully".to_string(),
        moved_count,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn clear_postgres_account_transactions(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<AccountTransactionsClearResult> {
    let mut transaction = pool.begin().await?;
    if !postgres_account_exists(&mut transaction, account_id, user_id).await? {
        return Ok(account_clear_failure("Account not found"));
    }
    let affected = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM bills
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        DELETE FROM bill_tags
        WHERE user_id = $1
          AND bill_id IN (
              SELECT id FROM bills
              WHERE user_id = $1
                AND is_deleted = false
                AND (
                    account_id = $2 OR source_account_id = $2 OR target_account_id = $2
                    OR transfer_target_account_id = $2
                )
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE bills
        SET is_deleted = true,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(AccountTransactionsClearResult {
        success: true,
        message: "Transactions deleted successfully".to_string(),
        deleted_count: affected,
    })
}

async fn postgres_account_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<bool> {
    Ok(sqlx::query("SELECT 1 FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&mut **transaction)
        .await?
        .is_some())
}

fn account_move_failure(message: &str) -> AccountTransactionsMoveResult {
    AccountTransactionsMoveResult {
        success: false,
        message: message.to_string(),
        moved_count: 0,
    }
}

fn account_clear_failure(message: &str) -> AccountTransactionsClearResult {
    AccountTransactionsClearResult {
        success: false,
        message: message.to_string(),
        deleted_count: 0,
    }
}

fn postgres_bill_balance_deltas(
    transaction_type: &str,
    amount_cents: i64,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
    standard_payload: &Value,
) -> Vec<(i64, i64)> {
    let mut deltas = Vec::new();
    let amount = amount_cents.abs();
    let destination_amount_cents = standard_payload
        .get("destination_amount_cents")
        .and_then(value_to_cents)
        .unwrap_or(amount)
        .abs();
    match transaction_type {
        "income" => push_account_delta(&mut deltas, source_account_id, amount),
        "transfer" | "investment" => {
            push_account_delta(&mut deltas, source_account_id, -amount);
            push_account_delta(&mut deltas, destination_account_id, destination_amount_cents);
        }
        _ => push_account_delta(&mut deltas, source_account_id, -amount),
    }
    deltas
}

fn push_account_delta(deltas: &mut Vec<(i64, i64)>, account_id: Option<i64>, delta: i64) {
    if let Some(account_id) = account_id.filter(|value| *value > 0) {
        deltas.push((account_id, delta));
    }
}

fn value_to_cents(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn metadata_initial_balance_cents(metadata: &Value, fallback_cents: i64) -> i64 {
    metadata
        .get("initial_balance_cents")
        .and_then(value_to_cents)
        .unwrap_or(fallback_cents)
}
