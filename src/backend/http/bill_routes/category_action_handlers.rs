// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[derive(Debug, Clone)]
struct CategoryRuleRuntimeRecord {
    id: i64,
    category_id: i64,
    main_category: String,
    sub_category: String,
    category_type: i32,
    category_priority: i32,
    rule_expression: String,
    regex_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CategoryRefreshResult {
    total: usize,
    categorized: usize,
    still_uncategorized: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CategoryRecategorizeResult {
    pub total: usize,
    pub updated: usize,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn quick_add_category_rule_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "quick_add_category_rule_handler", "business operation entered");
    let payload = match required_json_object_from_body(&body, "Request body is required") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(main_category) = value_string(payload.get("main_category")) else {
        return bad_request("main_category and rule_term are required");
    };
    let Some(rule_term) = value_string(payload.get("rule_term")) else {
        return bad_request("main_category and rule_term are required");
    };
    let sub_category = value_string(payload.get("sub_category"));
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state, "bills") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match quick_add_postgres_category_rule(
        runtime.pool(),
        user_id,
        &main_category,
        sub_category.as_deref(),
        &rule_term,
    )
    .await
    {
        Ok(true) => json_response(
            StatusCode::OK,
            json!({"success": true, "message": "Rule added successfully"}),
        ),
        Ok(false) => bad_request("Failed to add rule"),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn refresh_bill_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "refresh_bill_categories_handler", "business operation entered");
    let payload = match optional_json_object_from_body(&body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bill_ids = extract_bill_ids(&payload).filter(|ids| !ids.is_empty());
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match refresh_category_for_bills_postgres(
            runtime.pool(),
            user_id,
            bill_ids.as_deref(),
        )
        .await
        {
            Ok(result) => json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "result": {
                        "success": true,
                        "total": result.total,
                        "categorized": result.categorized,
                        "still_uncategorized": result.still_uncategorized,
                    },
                }),
            ),
            Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };
}

async fn quick_add_postgres_category_rule(
    pool: &PostgresPool,
    user_id: UserId,
    main_category: &str,
    sub_category: Option<&str>,
    rule_term: &str,
) -> bill_analyser_db::DbResult<bool> {
    let db_user_id = user_id.get() as i64;
    let sub_category = sub_category.unwrap_or("");
    let Some(category) =
        get_postgres_category_by_name(pool, main_category, sub_category, db_user_id).await?
    else {
        return Ok(false);
    };
    let category_id = record_i64(&category, "id").unwrap_or_default();
    if category_id <= 0 {
        return Ok(false);
    }
    let rule_expression = format!("OR={{{}}}", escape_rule_expression_term(rule_term));
    let existing_rules = list_postgres_category_rules(pool, db_user_id, Some(category_id), true).await?;
    if existing_rules.iter().any(|rule| record_text(rule, "rule_expression").trim() == rule_expression) {
        return Ok(false);
    }
    let created = create_postgres_category_rule(
        pool,
        &json!({
            "category_id": category_id,
            "name": format!("Quick add: {rule_term}"),
            "priority": 100,
            "rule_expression": rule_expression,
            "regex_enabled": false,
            "enabled": true
        }),
        db_user_id,
    )
    .await?;
    Ok(created.is_some())
}

async fn refresh_category_for_bills_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    bill_ids: Option<&[i64]>,
) -> bill_analyser_db::DbResult<CategoryRefreshResult> {
    let rules = load_category_runtime_rules_postgres(pool, user_id).await?;
    let bills = load_category_refresh_bills_postgres(pool, user_id, bill_ids).await?;
    let total = bills.len();
    let mut categorized = 0_usize;
    let mut still_uncategorized = 0_usize;

    for bill in bills {
        if let Some((main_category, sub_category)) = match_category_for_bill(&bill, &rules) {
            let bill_id = record_i64(&bill, "id").unwrap_or_default();
            let mut fields = Map::new();
            fields.insert("main_category".to_string(), Value::String(main_category));
            fields.insert("sub_category".to_string(), Value::String(sub_category));
            update_postgres_bill(
                pool,
                user_id.get() as i64,
                bill_id,
                &BillUpdateDraft {
                    fields,
                    tag_ids: None,
                },
            )
            .await?;
            categorized += 1;
        } else {
            still_uncategorized += 1;
        }
    }

    Ok(CategoryRefreshResult {
        total,
        categorized,
        still_uncategorized,
    })
}

pub(crate) async fn recategorize_bills_with_category_rules_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    force: bool,
) -> bill_analyser_db::DbResult<CategoryRecategorizeResult> {
    let rules = load_category_runtime_rules_postgres(pool, user_id).await?;
    let bills = load_all_category_refresh_bills_postgres(pool, user_id).await?;
    let total = bills.len();
    let mut updated = 0_usize;

    for bill in bills {
        if !force && !record_text(&bill, "main_category").trim().is_empty() {
            continue;
        }
        if let Some((main_category, sub_category)) = match_category_for_bill(&bill, &rules) {
            let bill_id = record_i64(&bill, "id").unwrap_or_default();
            let mut fields = Map::new();
            fields.insert("main_category".to_string(), Value::String(main_category));
            fields.insert("sub_category".to_string(), Value::String(sub_category));
            update_postgres_bill(
                pool,
                user_id.get() as i64,
                bill_id,
                &BillUpdateDraft {
                    fields,
                    tag_ids: None,
                },
            )
            .await?;
            updated += 1;
        }
    }

    Ok(CategoryRecategorizeResult { total, updated })
}

async fn load_category_runtime_rules_postgres(
    pool: &PostgresPool,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<CategoryRuleRuntimeRecord>> {
    let records = list_postgres_category_rules(pool, user_id.get() as i64, None, true).await?;
    let mut rules = records
        .iter()
        .filter_map(category_rule_runtime_record_from_postgres)
        .collect::<Vec<_>>();
    rules.sort_by_key(|rule| (rule.category_priority, rule.category_id, rule.id));
    Ok(rules)
}

async fn load_category_refresh_bills_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    bill_ids: Option<&[i64]>,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    if let Some(bill_ids) = bill_ids {
        let mut bills = Vec::new();
        for bill_id in bill_ids.iter().copied().filter(|value| *value > 0) {
            if let Some(bill) =
                get_postgres_bill_by_id(pool, user_id.get() as i64, bill_id).await?
            {
                bills.push(bill);
            }
        }
        return Ok(bills);
    }

    let bills = load_all_category_refresh_bills_postgres(pool, user_id).await?;
    Ok(bills
        .into_iter()
        .filter(|bill| record_text(bill, "main_category").trim().is_empty())
        .collect())
}

async fn load_all_category_refresh_bills_postgres(
    pool: &PostgresPool,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    let mut bills = Vec::new();
    let mut scanned = 0_usize;
    let mut page = 1;
    while scanned < RECONCILIATION_QUERY_LIMIT {
        let page_size = (RECONCILIATION_QUERY_LIMIT - scanned).min(RECONCILIATION_QUERY_PAGE_SIZE);
        let bill_page =
            query_postgres_bills(pool, user_id.get() as i64, page, page_size, &BillFilters::default())
                .await?;
        let fetched = bill_page.bills.len();
        let total = usize::try_from(bill_page.total.max(0)).unwrap_or(usize::MAX);
        scanned += fetched;
        bills.extend(bill_page.bills);
        if fetched == 0 || fetched < page_size || scanned >= total {
            break;
        }
        page += 1;
    }
    Ok(bills)
}

fn category_rule_runtime_record_from_postgres(
    record: &CategoryRuleRecord,
) -> Option<CategoryRuleRuntimeRecord> {
    let rule = CategoryRuleRuntimeRecord {
        id: record_i64(record, "id")?,
        category_id: record_i64(record, "category_id")?,
        main_category: record_text(record, "main_category").trim().to_string(),
        sub_category: record_text(record, "sub_category").trim().to_string(),
        category_type: normalize_category_rule_type(
            record_i64(record, "category_type").and_then(|value| i32::try_from(value).ok()),
        )
        .unwrap_or(3),
        category_priority: record_i64(record, "priority")
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(100),
        rule_expression: record_text(record, "rule_expression"),
        regex_enabled: record_i64(record, "regex_enabled").unwrap_or_default() != 0,
    };
    if rule.category_id > 0
        && !rule.main_category.is_empty()
        && !rule.sub_category.is_empty()
        && !rule.rule_expression.is_empty()
    {
        Some(rule)
    } else {
        None
    }
}
fn match_category_for_bill(
    bill: &BillRecord,
    rules: &[CategoryRuleRuntimeRecord],
) -> Option<(String, String)> {
    let type_filter = category_rule_type_filter(bill);
    let combined_text = [
        record_text(bill, "counterparty"),
        record_text(bill, "description"),
        record_text(bill, "original_category"),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ");

    rules
        .iter()
        .filter(|rule| type_filter.contains(&rule.category_type))
        .find(|rule| {
            if rule.category_type == 4 && !bill_is_transfer_refresh_candidate(bill) {
                return false;
            }
            match_rule_expression(&combined_text, &rule.rule_expression, rule.regex_enabled)
        })
        .map(|rule| (rule.main_category.clone(), rule.sub_category.clone()))
}

fn category_rule_type_filter(bill: &BillRecord) -> Vec<i32> {
    let bill_type = record_text(bill, "type").to_lowercase();
    let amount_cents = record_i64(bill, "amount_cents").unwrap_or_default();
    let suppress_investment = bill_analyser_core::is_ordinary_bank_interest_income(bill, None);

    match bill_type.as_str() {
        "支出" | "expense" | "3" => expense_category_types(suppress_investment),
        "收入" | "income" | "2" => income_category_types(suppress_investment),
        "转账" | "transfer" | "4" => vec![4],
        "投资" | "investment" | "5" => vec![5],
        _ if amount_cents < 0 => expense_category_types(suppress_investment),
        _ if amount_cents > 0 => income_category_types(suppress_investment),
        _ => vec![2, 3, 4, 5],
    }
}

fn expense_category_types(suppress_investment: bool) -> Vec<i32> {
    if suppress_investment {
        vec![3]
    } else {
        vec![3, 5]
    }
}

fn income_category_types(suppress_investment: bool) -> Vec<i32> {
    if suppress_investment {
        vec![2]
    } else {
        vec![2, 5]
    }
}

fn bill_is_transfer_refresh_candidate(bill: &BillRecord) -> bool {
    let bill_type = record_text(bill, "type").to_lowercase();
    matches!(bill_type.as_str(), "转账" | "transfer" | "4")
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_category_rule_type(value: Option<i32>) -> Option<i32> {
    match value {
        Some(1) => Some(3),
        Some(value @ 2..=5) => Some(value),
        _ => None,
    }
}
