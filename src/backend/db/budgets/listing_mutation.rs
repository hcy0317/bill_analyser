// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


fn enrich_budget_listing(
    connection: &Connection,
    user_id: i64,
    budget_type: Option<i32>,
    budgets: &mut [BudgetRecord],
) -> DbResult<Vec<BudgetRecord>> {
    let categories = load_category_context_values(connection, user_id)?;
    let category_context = build_budget_category_context(&categories);
    let mut enriched = Vec::new();
    for budget in budgets {
        let resolved_type = resolve_budget_category_type(
            &category_context,
            &record_text(budget, "category"),
            Some(&normalize_sub_category(record_value(
                budget,
                "sub_category",
            ))),
            budget_type,
        )
        .map(|value| value.code());
        if budget_type.is_some() && resolved_type != budget_type {
            continue;
        }
        if let Some(resolved_type) = resolved_type {
            let category_info = resolve_budget_category_info(
                &category_context,
                &record_text(budget, "category"),
                Some(&normalize_sub_category(record_value(
                    budget,
                    "sub_category",
                ))),
                Some(resolved_type),
            );
            budget.insert("type".to_string(), json_i64(i64::from(resolved_type)));
            budget.insert(
                "category_id".to_string(),
                category_info
                    .get("id")
                    .and_then(value_to_i64)
                    .map(|id| Value::String(id.to_string()))
                    .unwrap_or_else(|| Value::String(String::new())),
            );
            budget.insert("category_info".to_string(), category_info);
        } else {
            budget.insert("category_info".to_string(), Value::Null);
            budget.insert("category_id".to_string(), Value::String(String::new()));
        }
        enriched.push(budget.clone());
    }
    Ok(enriched)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_category_context_values(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "
        SELECT id, type, main_category, sub_category, icon, color
        FROM categories
        WHERE user_id = ?
        ",
    )?;
    let rows = statement.query_map([user_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>(0)?,
            "type": row.get::<_, Option<i32>>(1)?,
            "main_category": row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            "sub_category": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
            "icon": row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            "color": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
        }))
    })?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row?);
    }
    Ok(categories)
}

#[tracing::instrument(level = "debug", skip_all)]
fn insert_budget_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    payload: &mut BudgetRecord,
) -> DbResult<i64> {
    payload.insert("user_id".to_string(), json_i64(user_id));
    let mut columns = BUDGET_WRITE_COLUMNS
        .iter()
        .copied()
        .filter(|column| payload.contains_key(*column))
        .collect::<Vec<_>>();
    columns.push("user_id");
    let placeholders = std::iter::repeat_n("?", columns.len())
        .collect::<Vec<_>>()
        .join(", ");
    let values = columns
        .iter()
        .map(|column| json_to_sql_value(payload.get(*column).cloned().unwrap_or(Value::Null)))
        .collect::<Vec<_>>();
    tx.execute(
        &format!(
            "INSERT INTO budgets ({}) VALUES ({})",
            columns.join(", "),
            placeholders
        ),
        params_from_iter(values),
    )?;
    Ok(tx.last_insert_rowid())
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_create_payload(fields: &BudgetRecord, now: &str) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    payload
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    payload.insert(
        "sub_category".to_string(),
        Value::String(normalize_sub_category(payload.get("sub_category"))),
    );
    payload
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Bool(true));
    payload
        .entry("alert_threshold".to_string())
        .or_insert_with(|| json_i64(80));
    payload
        .entry("created_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now.to_string()));
    for field in ["category", "period_type", "amount", "start_date"] {
        if missing_required_field(&payload, field) {
            return Err(DbError::InvalidOperation(format!(
                "missing required budget field: {field}"
            )));
        }
    }
    Ok(payload)
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_update_payload(
    existing: &BudgetRecord,
    fields: &BudgetRecord,
) -> DbResult<BudgetRecord> {
    let mut payload = fields.clone();
    if payload.contains_key("sub_category") {
        payload.insert(
            "sub_category".to_string(),
            Value::String(normalize_sub_category(payload.get("sub_category"))),
        );
    } else if should_copy_existing_sub_category(&payload) {
        payload.insert(
            "sub_category".to_string(),
            Value::String(normalize_sub_category(existing.get("sub_category"))),
        );
    }
    payload
        .entry("updated_at".to_string())
        .or_insert_with(|| Value::String(now_text()));
    for key in payload.keys() {
        if !BUDGET_UPDATE_COLUMNS.contains(&key.as_str()) {
            return Err(DbError::InvalidOperation(format!(
                "invalid budget update field: {key}"
            )));
        }
    }
    Ok(payload)
}

fn should_copy_existing_sub_category(payload: &BudgetRecord) -> bool {
    [
        "category",
        "period_type",
        "amount",
        "start_date",
        "end_date",
    ]
    .iter()
    .any(|key| payload.contains_key(*key))
}

#[tracing::instrument(level = "debug", skip_all)]
fn update_payload_to_sql(payload: &mut BudgetRecord) -> DbResult<(Vec<String>, Vec<SqlValue>)> {
    let mut keys = payload
        .keys()
        .filter(|key| BUDGET_UPDATE_COLUMNS.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    if keys.is_empty() {
        return Err(DbError::InvalidOperation("empty budget update".to_string()));
    }
    let assignments = keys
        .iter()
        .map(|key| format!("{key} = ?"))
        .collect::<Vec<_>>();
    let values = keys
        .iter()
        .map(|key| json_to_sql_value(payload.get(key).cloned().unwrap_or(Value::Null)))
        .collect::<Vec<_>>();
    Ok((assignments, values))
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_budget_by_id_on_connection(
    connection: &Connection,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let sql = format!(
        "SELECT {} FROM budgets WHERE id = ?1 AND user_id = ?2",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    connection
        .query_row(&sql, params![budget_id, user_id], budget_record_from_row)
        .optional()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_budget_by_id_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let sql = format!(
        "SELECT {} FROM budgets WHERE id = ?1 AND user_id = ?2",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    tx.query_row(&sql, params![budget_id, user_id], budget_record_from_row)
        .optional()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_budgets_for_sync_group_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    category: &str,
    period_type: &str,
    start_date: &str,
) -> DbResult<Vec<BudgetRecord>> {
    let sql = format!(
        "
        SELECT {} FROM budgets
        WHERE category = ?1
          AND period_type = ?2
          AND start_date = ?3
          AND user_id = ?4
        ",
        BUDGET_SELECT_COLUMNS.join(", ")
    );
    let mut statement = tx.prepare(&sql)?;
    let rows = statement.query_map(
        params![category, period_type, start_date, user_id],
        budget_record_from_row,
    )?;
    let mut budgets = Vec::new();
    for row in rows {
        budgets.push(row?);
    }
    Ok(budgets)
}
