#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：敏感账户操作入口，校验密码后软删除当前账户关联的正式账单并写审计日志。
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
            if sync_all_postgres_account_balances(runtime.pool(), user_id_value)
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
