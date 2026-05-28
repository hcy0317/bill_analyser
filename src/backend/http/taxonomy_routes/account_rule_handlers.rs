// 中文导读：HTTP 运行态层，负责账户识别规则 API 的认证、请求解析和兼容响应。
// 维护重点：handler 只编排 AccountRulesRepository；导入正式切换前不得在这里改变 import 行为。
// 不变式：所有账户规则路由必须 user-scoped，test 端点只读且不增加匹配计数。

use bill_analyser_core::account_rules::AccountRuleMatchContext;
use bill_analyser_db::DbError;

#[tracing::instrument(level = "debug", skip_all)]
async fn list_account_rules_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<AccountRulesQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match list_postgres_account_rules(
            runtime.pool(),
            db_user_id(user_id),
            query.account_id,
            account_rules_enabled_only(&query),
            query.account_role_scope.as_deref(),
            query.transaction_type_scope.as_deref(),
        )
        .await
        {
            Ok(rules) => json_response(StatusCode::OK, format_account_rules_response(rules)),
            Err(DbError::InvalidOperation(message)) => bad_request(message),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    match repository.list_rules(
        db_user_id(user_id),
        query.account_id,
        account_rules_enabled_only(&query),
        query.account_role_scope.as_deref(),
        query.transaction_type_scope.as_deref(),
    ) {
        Ok(rules) => json_response(StatusCode::OK, format_account_rules_response(rules)),
        Err(DbError::InvalidOperation(message)) => bad_request(message),
        Err(_) => account_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_account_rule_handler(
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
    let Some(object) = body.as_object() else {
        return bad_request("No data provided");
    };
    let account_id_missing = !object.contains_key("account_id") && !object.contains_key("accountId");
    let expression_missing = object
        .get("rule_expression")
        .or_else(|| object.get("ruleExpression"))
        .is_none_or(Value::is_null);
    if account_id_missing || expression_missing {
        return bad_request("account_id and rule_expression are required");
    }

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let rule_id = match create_postgres_account_rule(runtime.pool(), &body, user_id).await {
            Ok(Some(value)) => value,
            Ok(None) => return bad_request("Failed to create account rule"),
            Err(DbError::InvalidOperation(message)) => return bad_request(message),
            Err(_) => return account_rule_db_error_response(),
        };
        return match get_postgres_account_rule(runtime.pool(), rule_id, user_id).await {
            Ok(Some(rule)) => account_rule_data_response(StatusCode::CREATED, rule),
            Ok(None) => account_rule_db_error_response(),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);
    let rule_id = match repository.create_rule(&body, user_id) {
        Ok(Some(value)) => value,
        Ok(None) => return bad_request("Failed to create account rule"),
        Err(DbError::InvalidOperation(message)) => return bad_request(message),
        Err(_) => return account_rule_db_error_response(),
    };
    match repository.get_rule(rule_id, user_id) {
        Ok(Some(rule)) => account_rule_data_response(StatusCode::CREATED, rule),
        Ok(None) => account_rule_db_error_response(),
        Err(_) => account_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_account_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
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
    if body
        .as_object()
        .and_then(|object| object.get("rule_expression").or_else(|| object.get("ruleExpression")))
        .is_some_and(Value::is_null)
    {
        return bad_request("rule_expression is required");
    }

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        return match update_postgres_account_rule(runtime.pool(), rule_id, &body, user_id).await {
            Ok(true) => match get_postgres_account_rule(runtime.pool(), rule_id, user_id).await {
                Ok(Some(rule)) => account_rule_data_response(StatusCode::OK, rule),
                Ok(None) => account_rule_db_error_response(),
                Err(_) => account_rule_db_error_response(),
            },
            Ok(false) => not_found("Account rule not found or no change"),
            Err(DbError::InvalidOperation(message)) => bad_request(message),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);
    match repository.update_rule(rule_id, &body, user_id) {
        Ok(true) => match repository.get_rule(rule_id, user_id) {
            Ok(Some(rule)) => account_rule_data_response(StatusCode::OK, rule),
            Ok(None) => account_rule_db_error_response(),
            Err(_) => account_rule_db_error_response(),
        },
        Ok(false) => not_found("Account rule not found or no change"),
        Err(DbError::InvalidOperation(message)) => bad_request(message),
        Err(_) => account_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_account_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match delete_postgres_account_rule(runtime.pool(), rule_id, db_user_id(user_id))
            .await
        {
            Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
            Ok(false) => not_found("Account rule not found"),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    match repository.delete_rule(rule_id, db_user_id(user_id)) {
        Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
        Ok(false) => not_found("Account rule not found"),
        Err(_) => account_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reorder_account_rules_handler(
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
    let Some(rule_ids) = body.get("rule_ids").or_else(|| body.get("ruleIds")) else {
        return bad_request("rule_ids is required");
    };
    let Some(rule_ids) = rule_ids.as_array() else {
        return bad_request("rule_ids must be a list");
    };
    let mut parsed_rule_ids = Vec::with_capacity(rule_ids.len());
    for value in rule_ids {
        let Some(rule_id) = parse_python_int(value) else {
            return bad_request("rule_ids must be a list");
        };
        parsed_rule_ids.push(rule_id);
    }

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match reorder_postgres_account_rules(
            runtime.pool(),
            &parsed_rule_ids,
            db_user_id(user_id),
        )
        .await
        {
            Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
            Ok(false) => bad_request("rule_ids must belong to the current user and be unique"),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    match repository.reorder_rules(&parsed_rule_ids, db_user_id(user_id)) {
        Ok(true) => json_response(StatusCode::OK, json!({"success": true})),
        Ok(false) => bad_request("rule_ids must belong to the current user and be unique"),
        Err(_) => account_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn migrate_account_aliases_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match migrate_postgres_account_aliases_to_rules(
            runtime.pool(),
            db_user_id(user_id),
        )
        .await
        {
            Ok(summary) => json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "migrated": summary.migrated,
                        "skipped": summary.skipped,
                    }
                }),
            ),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    match repository.migrate_aliases_to_rules(db_user_id(user_id)) {
        Ok(summary) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "data": {
                    "migrated": summary.migrated,
                    "skipped": summary.skipped,
                }
            }),
        ),
        Err(_) => account_rule_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn test_account_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "context is required") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let context = account_rule_match_context_from_body(&body);
    let requested_role_scope = string_or_default(
        first_value(
            &body,
            &["account_role_scope", "accountRoleScope", "role", "roleScope"],
        ),
        "any",
    );
    let transaction_type_scope = string_or_default(
        first_value(
            &body,
            &[
                "transaction_type_scope",
                "transactionTypeScope",
                "transactionType",
                "type",
            ],
        ),
        "all",
    );

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let db_user_id = db_user_id(user_id);
        return match test_postgres_account_rule_match(
            runtime.pool(),
            rule_id,
            db_user_id,
            &context,
            &requested_role_scope,
            &transaction_type_scope,
        )
        .await
        {
            Ok(Some(matched)) => json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "matched": true,
                        "accountId": matched.account_id,
                        "ruleId": matched.rule_id,
                        "matchedFields": matched.matched_fields,
                        "priority": matched.priority,
                        "fallbackUsed": matched.fallback_used,
                        "accountRoleScope": matched.account_role_scope,
                        "transactionTypeScope": matched.transaction_type_scope,
                    }
                }),
            ),
            Ok(None) => {
                if get_postgres_account_rule(runtime.pool(), rule_id, db_user_id)
                    .await
                    .ok()
                    .flatten()
                    .is_none()
                {
                    not_found("Account rule not found")
                } else {
                    json_response(
                        StatusCode::OK,
                        json!({"success": true, "data": {"matched": false}}),
                    )
                }
            }
            Err(DbError::InvalidOperation(message)) => bad_request(message),
            Err(_) => account_rule_db_error_response(),
        };
    }
    let mut runtime = match open_runtime(&state, "taxonomy account rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountRulesRepository::new(runtime.connection_mut());
    match repository.test_rule_match(
        rule_id,
        db_user_id(user_id),
        &context,
        &requested_role_scope,
        &transaction_type_scope,
    ) {
        Ok(Some(matched)) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "data": {
                    "matched": true,
                    "accountId": matched.account_id,
                    "ruleId": matched.rule_id,
                    "matchedFields": matched.matched_fields,
                    "priority": matched.priority,
                    "fallbackUsed": matched.fallback_used,
                    "accountRoleScope": matched.account_role_scope,
                    "transactionTypeScope": matched.transaction_type_scope,
                }
            }),
        ),
        Ok(None) => {
            if repository
                .get_rule(rule_id, db_user_id(user_id))
                .ok()
                .flatten()
                .is_none()
            {
                not_found("Account rule not found")
            } else {
                json_response(
                    StatusCode::OK,
                    json!({"success": true, "data": {"matched": false}}),
                )
            }
        }
        Err(DbError::InvalidOperation(message)) => bad_request(message),
        Err(_) => account_rule_db_error_response(),
    }
}

fn format_account_rules_response(rules: Vec<AccountRuleRecord>) -> Value {
    let total = rules.len();
    json!({
        "success": true,
        "data": Value::Array(rules.into_iter().map(account_rule_api_record).collect()),
        "total": total,
    })
}

fn account_rule_data_response(status: StatusCode, rule: AccountRuleRecord) -> Response {
    json_response(
        status,
        json!({"success": true, "data": account_rule_api_record(rule)}),
    )
}

fn account_rule_api_record(mut rule: AccountRuleRecord) -> Value {
    let field_scope = rule
        .remove("field_scope")
        .unwrap_or_else(|| json!(["counterparty", "payment_method", "description"]));
    json!({
        "id": rule["id"],
        "user_id": rule["user_id"],
        "account_id": rule["account_id"],
        "accountId": rule["account_id"],
        "accountName": string_or_default(rule.get("account_name"), ""),
        "accountType": rule["account_type"],
        "accountHidden": rule.get("account_hidden").is_some_and(value_truthy),
        "name": string_or_default(rule.get("name"), ""),
        "priority": value_as_i64_or(rule.get("priority"), 100),
        "rule_expression": string_or_default(rule.get("rule_expression"), ""),
        "ruleExpression": string_or_default(rule.get("rule_expression"), ""),
        "regex_enabled": rule.get("regex_enabled").is_some_and(value_truthy),
        "regexEnabled": rule.get("regex_enabled").is_some_and(value_truthy),
        "enabled": rule.get("enabled").map(value_truthy).unwrap_or(true),
        "applied_count": value_as_i64_or(rule.get("applied_count"), 0),
        "appliedCount": value_as_i64_or(rule.get("applied_count"), 0),
        "last_applied_at": rule.get("last_applied_at").cloned().unwrap_or(Value::Null),
        "lastAppliedAt": rule.get("last_applied_at").cloned().unwrap_or(Value::Null),
        "match_count": value_as_i64_or(rule.get("match_count"), 0),
        "matchCount": value_as_i64_or(rule.get("match_count"), 0),
        "last_matched_at": rule.get("last_matched_at").cloned().unwrap_or(Value::Null),
        "lastMatchedAt": rule.get("last_matched_at").cloned().unwrap_or(Value::Null),
        "account_role_scope": string_or_default(rule.get("account_role_scope"), "any"),
        "accountRoleScope": string_or_default(rule.get("account_role_scope"), "any"),
        "transaction_type_scope": string_or_default(rule.get("transaction_type_scope"), "all"),
        "transactionTypeScope": string_or_default(rule.get("transaction_type_scope"), "all"),
        "field_scope": field_scope.clone(),
        "fieldScope": field_scope,
        "source": string_or_default(rule.get("source"), "manual"),
        "source_key": rule.get("source_key").cloned().unwrap_or(Value::Null),
        "sourceKey": rule.get("source_key").cloned().unwrap_or(Value::Null),
        "created_at": rule["created_at"],
        "createdAt": rule["created_at"],
        "updated_at": rule["updated_at"],
        "updatedAt": rule["updated_at"],
    })
}

fn account_rules_enabled_only(query: &AccountRulesQuery) -> bool {
    !query
        .enabled_only
        .as_deref()
        .unwrap_or("true")
        .eq_ignore_ascii_case("false")
}

fn account_rule_match_context_from_body(body: &Value) -> AccountRuleMatchContext {
    let context = body.get("context").unwrap_or(body);
    let text = string_or_default(context.get("text"), "");
    AccountRuleMatchContext {
        parser_id: string_or_default(first_value(context, &["parser_id", "parserId"]), ""),
        parser_label: string_or_default(first_value(context, &["parser_label", "parserLabel"]), ""),
        parser_tags: value_string_array(first_value(context, &["parser_tags", "parserTags"])),
        counterparty: string_or_default(context.get("counterparty"), &text),
        payment_method: string_or_default(
            first_value(context, &["payment_method", "paymentMethod"]),
            &text,
        ),
        description: string_or_default(context.get("description"), &text),
        expense_counterparty: string_or_default(
            first_value(context, &["expense_counterparty", "expenseCounterparty"]),
            "",
        ),
        expense_payment_method: string_or_default(
            first_value(context, &["expense_payment_method", "expensePaymentMethod"]),
            "",
        ),
        expense_description: string_or_default(
            first_value(context, &["expense_description", "expenseDescription"]),
            "",
        ),
        income_counterparty: string_or_default(
            first_value(context, &["income_counterparty", "incomeCounterparty"]),
            "",
        ),
        income_payment_method: string_or_default(
            first_value(context, &["income_payment_method", "incomePaymentMethod"]),
            "",
        ),
        income_description: string_or_default(
            first_value(context, &["income_description", "incomeDescription"]),
            "",
        ),
        investment_counterparty: string_or_default(
            first_value(context, &["investment_counterparty", "investmentCounterparty"]),
            "",
        ),
        investment_description: string_or_default(
            first_value(context, &["investment_description", "investmentDescription"]),
            "",
        ),
    }
}

fn first_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| value.get(*key))
}

fn value_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|value| string_or_default(Some(value), ""))
        .filter(|value| !value.trim().is_empty())
        .collect()
}
