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

async fn quick_add_category_keyword_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let payload = match required_json_object_from_body(&body, "Request body is required") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(main_category) = value_string(payload.get("main_category")) else {
        return bad_request("main_category and keyword are required");
    };
    let Some(keyword) = value_string(payload.get("keyword")) else {
        return bad_request("main_category and keyword are required");
    };
    let sub_category = value_string(payload.get("sub_category"));
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match quick_add_category_keyword(
        runtime.connection_mut(),
        user_id,
        &main_category,
        sub_category.as_deref(),
        &keyword,
    ) {
        Ok(true) => json_response(
            StatusCode::OK,
            json!({"success": true, "message": "Keyword added successfully"}),
        ),
        Ok(false) => bad_request("Failed to add keyword"),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn refresh_bill_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let payload = optional_json_object_from_body(&body);
    let bill_ids = extract_bill_ids(&payload).filter(|ids| !ids.is_empty());
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match refresh_category_for_bills(runtime.connection_mut(), user_id, bill_ids.as_deref()) {
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
    }
}

fn quick_add_category_keyword(
    connection: &mut Connection,
    user_id: UserId,
    main_category: &str,
    sub_category: Option<&str>,
    keyword: &str,
) -> bill_analyser_db::DbResult<bool> {
    if !table_exists(connection, "categories")? {
        return Ok(false);
    }
    let sub_category = sub_category.unwrap_or("");
    let row = connection
        .query_row(
            concat!(
                "SELECT id, COALESCE(keywords, '') FROM categories ",
                "WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3 LIMIT 1"
            ),
            params![user_id.get() as i64, main_category, sub_category],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((category_id, current_keywords)) = row else {
        return Ok(false);
    };

    let mut keyword_list = current_keywords
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if keyword_list.iter().any(|value| value == keyword) {
        return Ok(false);
    }
    keyword_list.push(keyword.to_string());
    let new_keywords = keyword_list.join(",");
    let updated = connection.execute(
        "UPDATE categories SET keywords = ?1 WHERE id = ?2 AND user_id = ?3",
        params![new_keywords, category_id, user_id.get() as i64],
    )?;
    Ok(updated > 0)
}

fn refresh_category_for_bills(
    connection: &mut Connection,
    user_id: UserId,
    bill_ids: Option<&[i64]>,
) -> bill_analyser_db::DbResult<CategoryRefreshResult> {
    let rules = load_category_runtime_rules(connection, user_id)?;
    let bills = load_category_refresh_bills(connection, user_id, bill_ids)?;
    let total = bills.len();
    let mut categorized = 0_usize;
    let mut still_uncategorized = 0_usize;

    for bill in bills {
        if let Some((main_category, sub_category)) = match_category_for_bill(&bill, &rules) {
            let bill_id = record_i64(&bill, "id").unwrap_or_default();
            let mut fields = Map::new();
            fields.insert("main_category".to_string(), Value::String(main_category));
            fields.insert("sub_category".to_string(), Value::String(sub_category));
            update_bill(
                connection,
                user_id,
                bill_id,
                &BillUpdateDraft {
                    fields,
                    tag_ids: None,
                },
            )?;
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

pub(crate) fn recategorize_bills_with_category_rules(
    connection: &mut Connection,
    user_id: UserId,
    force: bool,
) -> bill_analyser_db::DbResult<CategoryRecategorizeResult> {
    let rules = load_category_runtime_rules(connection, user_id)?;
    let bills = load_all_category_refresh_bills(connection, user_id)?;
    let total = bills.len();
    let mut updated = 0_usize;

    for (bill_id, bill) in bills {
        if !force && !record_text(&bill, "main_category").trim().is_empty() {
            continue;
        }
        if let Some((main_category, sub_category)) = match_category_for_bill(&bill, &rules) {
            let mut fields = Map::new();
            fields.insert("main_category".to_string(), Value::String(main_category));
            fields.insert("sub_category".to_string(), Value::String(sub_category));
            let draft = BillUpdateDraft {
                fields,
                tag_ids: None,
            };
            update_bill(connection, user_id, bill_id, &draft)?;
            updated += 1;
        }
    }

    Ok(CategoryRecategorizeResult { total, updated })
}

fn load_category_runtime_rules(
    connection: &Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<CategoryRuleRuntimeRecord>> {
    if !table_exists(connection, "categories")? || !table_exists(connection, "category_rules")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(concat!(
        "SELECT cr.id, cr.category_id, c.main_category, c.sub_category, ",
        "COALESCE(c.type, 1), COALESCE(c.priority, cr.priority, 100), ",
        "COALESCE(cr.priority, 100), cr.rule_expression, COALESCE(cr.regex_enabled, 0) ",
        "FROM category_rules cr ",
        "JOIN categories c ON cr.category_id = c.id ",
        "WHERE cr.user_id = ?1 AND cr.enabled = 1 ",
        "ORDER BY COALESCE(c.priority, cr.priority, 100) ASC, c.id ASC, cr.id ASC"
    ))?;
    let rows = statement.query_map(params![user_id.get() as i64], |row| {
        Ok(CategoryRuleRuntimeRecord {
            id: row.get(0)?,
            category_id: row.get(1)?,
            main_category: row.get::<_, String>(2)?.trim().to_string(),
            sub_category: row.get::<_, String>(3)?.trim().to_string(),
            category_type: normalize_category_rule_type(row.get::<_, Option<i32>>(4)?).unwrap_or(3),
            category_priority: row.get::<_, Option<i32>>(5)?.unwrap_or(100),
            rule_expression: row.get::<_, String>(7)?,
            regex_enabled: row.get::<_, Option<i64>>(8)?.unwrap_or(0) != 0,
        })
    })?;
    let mut rules = Vec::new();
    for row in rows {
        let rule = row?;
        if rule.category_id > 0
            && !rule.main_category.is_empty()
            && !rule.sub_category.is_empty()
            && !rule.rule_expression.is_empty()
        {
            rules.push(rule);
        }
    }
    rules.sort_by_key(|rule| (rule.category_priority, rule.category_id, rule.id));
    Ok(rules)
}

fn load_category_refresh_bills(
    connection: &Connection,
    user_id: UserId,
    bill_ids: Option<&[i64]>,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    if let Some(bill_ids) = bill_ids {
        let mut bills = Vec::new();
        for bill_id in bill_ids.iter().copied().filter(|value| *value > 0) {
            if let Some(bill) = get_bill_by_id(connection, user_id, bill_id)? {
                bills.push(bill);
            }
        }
        return Ok(bills);
    }

    let mut statement = connection.prepare(concat!(
        "SELECT id, user_id, date, type, amount, counterparty, description, ",
        "payment_method, main_category, sub_category, batch_id, hash, created_at, updated_at, ",
        "source_account_id, destination_account_id, destination_amount, created_from_template, ",
        "created_from_recurring, import_history_id ",
        "FROM bills WHERE user_id = ?1 ",
        "AND (main_category IS NULL OR main_category = '') ORDER BY date DESC"
    ))?;
    let rows = statement.query_map(params![user_id.get() as i64], row_to_bill_record)?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(bills)
}

fn load_all_category_refresh_bills(
    connection: &Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<(i64, BillRecord)>> {
    let mut statement = connection.prepare(concat!(
        "SELECT id, user_id, date, type, amount, counterparty, description, ",
        "payment_method, main_category, sub_category, batch_id, hash, created_at, updated_at, ",
        "source_account_id, destination_account_id, destination_amount, created_from_template, ",
        "created_from_recurring, import_history_id ",
        "FROM bills WHERE user_id = ?1 ORDER BY date DESC"
    ))?;
    let rows = statement.query_map(params![user_id.get() as i64], |row| {
        Ok((row.get::<_, i64>(0)?, row_to_bill_record(row)?))
    })?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(bills)
}

fn row_to_bill_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json!(row.get::<_, i64>(0)?));
    record.insert("user_id".to_string(), json!(row.get::<_, i64>(1)?));
    record.insert("date".to_string(), json!(row.get::<_, String>(2)?));
    record.insert("type".to_string(), json!(row.get::<_, String>(3)?));
    record.insert("amount".to_string(), json!(row.get::<_, f64>(4)?));
    record.insert("counterparty".to_string(), json!(row.get::<_, String>(5)?));
    record.insert("description".to_string(), json!(row.get::<_, String>(6)?));
    record.insert(
        "payment_method".to_string(),
        json!(row.get::<_, Option<String>>(7)?.unwrap_or_default()),
    );
    record.insert(
        "main_category".to_string(),
        optional_string_value(row.get::<_, Option<String>>(8)?),
    );
    record.insert(
        "sub_category".to_string(),
        optional_string_value(row.get::<_, Option<String>>(9)?),
    );
    record.insert(
        "batch_id".to_string(),
        optional_string_value(row.get::<_, Option<String>>(10)?),
    );
    record.insert(
        "hash".to_string(),
        optional_string_value(row.get::<_, Option<String>>(11)?),
    );
    record.insert("created_at".to_string(), json!(row.get::<_, String>(12)?));
    record.insert("updated_at".to_string(), json!(row.get::<_, String>(13)?));
    record.insert(
        "source_account_id".to_string(),
        json!(row.get::<_, Option<i64>>(14)?.unwrap_or_default()),
    );
    record.insert(
        "destination_account_id".to_string(),
        json!(row.get::<_, Option<i64>>(15)?.unwrap_or_default()),
    );
    record.insert(
        "destination_amount".to_string(),
        json!(row.get::<_, Option<f64>>(16)?.unwrap_or_default()),
    );
    record.insert(
        "created_from_template".to_string(),
        optional_i64_value(row.get::<_, Option<i64>>(17)?),
    );
    record.insert(
        "created_from_recurring".to_string(),
        optional_i64_value(row.get::<_, Option<i64>>(18)?),
    );
    record.insert(
        "import_history_id".to_string(),
        optional_i64_value(row.get::<_, Option<i64>>(19)?),
    );
    Ok(record)
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map(Value::String).unwrap_or(Value::Null)
}

fn optional_i64_value(value: Option<i64>) -> Value {
    value.map(|value| json!(value)).unwrap_or(Value::Null)
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
    let amount = record_f64(bill, "amount").unwrap_or_default();
    let suppress_investment = bill_analyser_core::is_ordinary_bank_interest_income(bill, None);

    match bill_type.as_str() {
        "支出" | "expense" | "3" => expense_category_types(suppress_investment),
        "收入" | "income" | "2" => income_category_types(suppress_investment),
        "转账" | "transfer" | "4" => vec![4],
        "投资" | "investment" | "5" => vec![5],
        _ if amount < 0.0 => expense_category_types(suppress_investment),
        _ if amount > 0.0 => income_category_types(suppress_investment),
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

fn normalize_category_rule_type(value: Option<i32>) -> Option<i32> {
    match value {
        Some(1) => Some(3),
        Some(value @ 2..=5) => Some(value),
        _ => None,
    }
}
