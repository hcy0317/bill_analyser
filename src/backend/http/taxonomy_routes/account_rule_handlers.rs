// 中文导读：HTTP 运行态层，负责账户识别规则 API 的认证、请求解析和当前响应。
// 维护重点：handler 只编排 Postgres 账户识别规则读写；不要回退账户别名迁移或 non-Postgres 规则仓储。
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

        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let warnings = account_rule_query_compat_warnings(&query);
        return match list_postgres_account_rules(
            runtime.pool(),
            db_user_id(user_id),
            query.account_id,
            account_rules_enabled_only(&query),
            None,
            None,
        )
        .await
        {
            Ok(rules) => json_response(StatusCode::OK, format_account_rules_response(rules, warnings)),
            Err(DbError::InvalidOperation(message)) => bad_request(message),
            Err(_) => account_rule_db_error_response(),
        };
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
    let warnings = deprecated_account_rule_scope_warnings(&body);
    let sanitized_body = account_rule_payload_without_deprecated_scope(&body);
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


        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        let rule_id = match create_postgres_account_rule(runtime.pool(), &sanitized_body, user_id).await {
            Ok(Some(value)) => value,
            Ok(None) => return bad_request("Failed to create account rule"),
            Err(DbError::InvalidOperation(message)) => return bad_request(message),
            Err(_) => return account_rule_db_error_response(),
        };
        return match get_postgres_account_rule(runtime.pool(), rule_id, user_id).await {
            Ok(Some(rule)) => account_rule_data_response_with_warnings(
                StatusCode::CREATED,
                rule,
                warnings,
            ),
            Ok(None) => account_rule_db_error_response(),
            Err(_) => account_rule_db_error_response(),
        };
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
    let warnings = deprecated_account_rule_scope_warnings(&body);
    let sanitized_body = account_rule_payload_without_deprecated_scope(&body);
    if body
        .as_object()
        .and_then(|object| object.get("rule_expression").or_else(|| object.get("ruleExpression")))
        .is_some_and(Value::is_null)
    {
        return bad_request("rule_expression is required");
    }


        let runtime = match open_postgres_runtime(&state, "taxonomy account rules") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let user_id = db_user_id(user_id);
        if sanitized_body
            .as_object()
            .is_some_and(Map::is_empty)
            && !warnings.is_empty()
        {
            return match get_postgres_account_rule(runtime.pool(), rule_id, user_id).await {
                Ok(Some(rule)) => {
                    account_rule_data_response_with_warnings(StatusCode::OK, rule, warnings)
                }
                Ok(None) => not_found("Account rule not found"),
                Err(_) => account_rule_db_error_response(),
            };
        }
        return match update_postgres_account_rule(runtime.pool(), rule_id, &sanitized_body, user_id).await {
            Ok(true) => match get_postgres_account_rule(runtime.pool(), rule_id, user_id).await {
                Ok(Some(rule)) => account_rule_data_response_with_warnings(
                    StatusCode::OK,
                    rule,
                    warnings,
                ),
                Ok(None) => account_rule_db_error_response(),
                Err(_) => account_rule_db_error_response(),
            },
            Ok(false) => not_found("Account rule not found or no change"),
            Err(DbError::InvalidOperation(message)) => bad_request(message),
            Err(_) => account_rule_db_error_response(),
        };
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
        let Some(rule_id) = parse_json_int(value) else {
            return bad_request("rule_ids must be a list");
        };
        parsed_rule_ids.push(rule_id);
    }


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
                        "matchRole": matched.account_role_scope,
                        "transactionType": matched.transaction_type_scope,
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

fn format_account_rules_response(rules: Vec<AccountRuleRecord>, warnings: Vec<String>) -> Value {
    let total = rules.len();
    let mut response = json!({
        "success": true,
        "data": Value::Array(rules.into_iter().map(account_rule_api_record).collect()),
        "total": total,
    });
    if !warnings.is_empty() {
        response["warnings"] = json!(warnings);
    }
    response
}

fn account_rule_data_response_with_warnings(
    status: StatusCode,
    rule: AccountRuleRecord,
    warnings: Vec<String>,
) -> Response {
    let mut payload = json!({"success": true, "data": account_rule_api_record(rule)});
    if !warnings.is_empty() {
        payload["warnings"] = json!(warnings);
    }
    json_response(
        status,
        payload,
    )
}

fn account_rule_api_record(mut rule: AccountRuleRecord) -> Value {
    rule.remove("account_role_scope");
    rule.remove("transaction_type_scope");
    rule.remove("field_scope");
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
        "source": string_or_default(rule.get("source"), "manual"),
        "source_key": rule.get("source_key").cloned().unwrap_or(Value::Null),
        "sourceKey": rule.get("source_key").cloned().unwrap_or(Value::Null),
        "created_at": rule["created_at"],
        "createdAt": rule["created_at"],
        "updated_at": rule["updated_at"],
        "updatedAt": rule["updated_at"],
    })
}

fn account_rule_query_compat_warnings(query: &AccountRulesQuery) -> Vec<String> {
    if query.account_role_scope.is_some() || query.transaction_type_scope.is_some() {
        vec![
            "Ignored deprecated account rule scope query filters; account recognition now uses stabilized import context"
                .to_string(),
        ]
    } else {
        Vec::new()
    }
}

fn deprecated_account_rule_scope_warnings(body: &Value) -> Vec<String> {
    let Some(object) = body.as_object() else {
        return Vec::new();
    };
    if [
        "account_role_scope",
        "accountRoleScope",
        "transaction_type_scope",
        "transactionTypeScope",
        "field_scope",
        "fieldScope",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
    {
        vec![
            "Ignored deprecated account rule scope fields; account recognition now uses stabilized import context"
                .to_string(),
        ]
    } else {
        Vec::new()
    }
}

fn account_rule_payload_without_deprecated_scope(body: &Value) -> Value {
    let Some(object) = body.as_object() else {
        return body.clone();
    };
    let mut sanitized = object.clone();
    for key in [
        "account_role_scope",
        "accountRoleScope",
        "transaction_type_scope",
        "transactionTypeScope",
        "field_scope",
        "fieldScope",
    ] {
        sanitized.remove(key);
    }
    Value::Object(sanitized)
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

#[cfg(test)]
mod account_rule_handler_tests {
    use super::*;

    #[test]
    fn account_rule_api_projection_omits_deprecated_scope_fields() {
        let mut rule = Map::new();
        rule.insert("id".to_string(), json!(1));
        rule.insert("user_id".to_string(), json!(9));
        rule.insert("account_id".to_string(), json!(42));
        rule.insert("account_name".to_string(), json!("招商银行"));
        rule.insert("account_type".to_string(), json!("bank"));
        rule.insert("account_hidden".to_string(), json!(false));
        rule.insert("name".to_string(), json!("工资卡"));
        rule.insert("priority".to_string(), json!(10));
        rule.insert("rule_expression".to_string(), json!("OR={工资卡}"));
        rule.insert("regex_enabled".to_string(), json!(false));
        rule.insert("enabled".to_string(), json!(true));
        rule.insert("applied_count".to_string(), json!(0));
        rule.insert("match_count".to_string(), json!(0));
        rule.insert("account_role_scope".to_string(), json!("destination"));
        rule.insert("transaction_type_scope".to_string(), json!("income"));
        rule.insert("field_scope".to_string(), json!(["parser"]));
        rule.insert("source".to_string(), json!("manual"));
        rule.insert("created_at".to_string(), json!("2026-06-06T00:00:00Z"));
        rule.insert("updated_at".to_string(), json!("2026-06-06T00:00:00Z"));

        let projected = account_rule_api_record(rule);

        assert!(projected.get("accountRoleScope").is_none());
        assert!(projected.get("transactionTypeScope").is_none());
        assert!(projected.get("fieldScope").is_none());
        assert_eq!(projected["accountId"], json!(42));
    }

    #[test]
    fn deprecated_account_rule_scope_payload_is_stripped_with_warning() {
        let payload = json!({
            "accountId": 42,
            "ruleExpression": "OR={工资卡}",
            "accountRoleScope": "destination",
            "transaction_type_scope": "income",
            "fieldScope": ["parser"]
        });

        let sanitized = account_rule_payload_without_deprecated_scope(&payload);

        assert_eq!(deprecated_account_rule_scope_warnings(&payload).len(), 1);
        assert_eq!(sanitized["accountId"], json!(42));
        assert_eq!(sanitized["ruleExpression"], json!("OR={工资卡}"));
        assert!(sanitized.get("accountRoleScope").is_none());
        assert!(sanitized.get("transaction_type_scope").is_none());
        assert!(sanitized.get("fieldScope").is_none());
    }

    #[test]
    fn account_rule_response_helpers_surface_scope_warnings() {
        let query = AccountRulesQuery {
            account_id: None,
            enabled_only: None,
            account_role_scope: Some("source".to_string()),
            transaction_type_scope: None,
        };
        let warnings = account_rule_query_compat_warnings(&query);
        let response = format_account_rules_response(Vec::new(), warnings);

        assert_eq!(response["total"], json!(0));
        assert!(response["warnings"][0]
            .as_str()
            .unwrap()
            .contains("deprecated account rule scope query"));

        let clean_payload = json!({"accountId": 42, "ruleExpression": "OR={工资卡}"});
        assert!(deprecated_account_rule_scope_warnings(&clean_payload).is_empty());
    }

    #[test]
    fn account_rule_response_helpers_cover_clean_and_scalar_inputs() {
        let mut rule = Map::new();
        rule.insert("id".to_string(), json!(1));
        rule.insert("user_id".to_string(), json!(9));
        rule.insert("account_id".to_string(), json!(42));
        rule.insert("account_name".to_string(), json!("招商银行"));
        rule.insert("account_type".to_string(), json!("bank"));
        rule.insert("account_hidden".to_string(), json!(false));
        rule.insert("name".to_string(), json!("工资卡"));
        rule.insert("priority".to_string(), json!(10));
        rule.insert("rule_expression".to_string(), json!("OR={工资卡}"));
        rule.insert("regex_enabled".to_string(), json!(false));
        rule.insert("enabled".to_string(), json!(true));
        rule.insert("applied_count".to_string(), json!(0));
        rule.insert("match_count".to_string(), json!(0));
        rule.insert("source".to_string(), json!("manual"));
        rule.insert("created_at".to_string(), json!("2026-06-06T00:00:00Z"));
        rule.insert("updated_at".to_string(), json!("2026-06-06T00:00:00Z"));

        let response = account_rule_data_response_with_warnings(
            StatusCode::CREATED,
            rule,
            vec!["compat warning".to_string()],
        );
        assert_eq!(response.status(), StatusCode::CREATED);

        let clean_query = AccountRulesQuery {
            account_id: None,
            enabled_only: None,
            account_role_scope: None,
            transaction_type_scope: None,
        };
        assert!(account_rule_query_compat_warnings(&clean_query).is_empty());
        assert!(deprecated_account_rule_scope_warnings(&Value::Null).is_empty());
        assert_eq!(
            account_rule_payload_without_deprecated_scope(&json!("raw")),
            json!("raw")
        );
    }
}
