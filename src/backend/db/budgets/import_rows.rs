// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


fn import_budget_row_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    budget: &BudgetRecord,
    now: &str,
) -> DbResult<ImportBudgetRowAction> {
    let name = record_text(budget, "name");
    if let Some(existing_id) = tx
        .query_row(
            "SELECT id FROM budgets WHERE name = ?1 AND user_id = ?2 LIMIT 1",
            params![name.as_str(), user_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        let values = vec![
            budget_import_sql_value(budget, "category", Value::Null),
            budget_import_sql_value(budget, "sub_category", Value::Null),
            budget_import_sql_value(budget, "period_type", Value::String("monthly".to_string())),
            budget_import_sql_value(budget, "amount", Value::Null),
            budget_import_sql_value(budget, "start_date", Value::Null),
            budget_import_sql_value(budget, "end_date", Value::Null),
            budget_import_sql_value(budget, "alert_threshold", json_i64(80)),
            budget_import_sql_value(budget, "enabled", json_i64(1)),
            SqlValue::Text(now.to_string()),
            SqlValue::Integer(existing_id),
            SqlValue::Integer(user_id),
        ];
        tx.execute(
            "
            UPDATE budgets SET
                category = ?,
                sub_category = ?,
                period_type = ?,
                amount = ?,
                start_date = ?,
                end_date = ?,
                alert_threshold = ?,
                enabled = ?,
                updated_at = ?
            WHERE id = ? AND user_id = ?
            ",
            params_from_iter(values),
        )?;
        Ok(ImportBudgetRowAction::Updated)
    } else {
        let values = vec![
            SqlValue::Text(name),
            budget_import_sql_value(budget, "category", Value::Null),
            budget_import_sql_value(budget, "sub_category", Value::Null),
            budget_import_sql_value(budget, "period_type", Value::String("monthly".to_string())),
            budget_import_sql_value(budget, "amount", Value::Null),
            budget_import_sql_value(budget, "start_date", Value::Null),
            budget_import_sql_value(budget, "end_date", Value::Null),
            budget_import_sql_value(budget, "alert_threshold", json_i64(80)),
            budget_import_sql_value(budget, "enabled", json_i64(1)),
            SqlValue::Text(now.to_string()),
            SqlValue::Text(now.to_string()),
            SqlValue::Integer(user_id),
        ];
        tx.execute(
            "
            INSERT INTO budgets (
                name, category, sub_category, period_type, amount,
                start_date, end_date, alert_threshold, enabled,
                created_at, updated_at, user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ",
            params_from_iter(values),
        )?;
        Ok(ImportBudgetRowAction::Created)
    }
}

fn budget_import_has_required_name_and_amount(budget: &BudgetRecord) -> bool {
    budget
        .get("name")
        .is_some_and(budget_import_value_is_truthy)
        && budget
            .get("amount")
            .is_some_and(budget_import_value_is_truthy)
}

fn budget_import_value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
    }
}

fn budget_import_sql_value(budget: &BudgetRecord, key: &str, default: Value) -> SqlValue {
    json_to_sql_value(
        budget
            .get(key)
            .cloned()
            .filter(|value| !value.is_null())
            .unwrap_or(default),
    )
}
