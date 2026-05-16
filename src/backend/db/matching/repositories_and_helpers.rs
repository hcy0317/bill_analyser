fn get_learning_rule(
    connection: &Connection,
    user_id: i64,
    rule_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    if !table_exists(connection, "import_learning_rules")? {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
            params![rule_id, user_id],
            |row| row_to_map(row, ""),
        )
        .optional()
        .map_err(DbError::from)
}

fn get_learning_rule_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    rule_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    if !table_exists_tx(tx, "import_learning_rules")? {
        return Ok(None);
    }
    tx.query_row(
        "SELECT * FROM import_learning_rules WHERE id = ? AND user_id = ? LIMIT 1",
        params![rule_id, user_id],
        |row| row_to_map(row, ""),
    )
    .optional()
    .map_err(DbError::from)
}

fn validate_learning_revision(
    rule: &Map<String, Value>,
    expected_revision: Option<&str>,
) -> DbResult<()> {
    let Some(expected) = expected_revision
        .map(normalize_learning_rule_revision)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let current = build_learning_rule_revision(rule);
    if expected != current {
        return Err(DbError::InvalidOperation(
            "Learning candidate not available".to_string(),
        ));
    }
    Ok(())
}

fn learning_suppression_revision_map(
    connection: &Connection,
    user_id: i64,
    bill_id: i64,
) -> DbResult<BTreeMap<i64, String>> {
    let mut statement = connection.prepare(
        "SELECT rule_id, created_at FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id = ?",
    )?;
    let rows = statement.query_map(params![user_id, bill_id], |row| {
        Ok((
            row.get::<_, i64>("rule_id")?,
            row.get::<_, String>("created_at")?,
        ))
    })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(DbError::from)
}

fn load_learning_rules(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    let order_by = if column_exists(connection, "import_learning_rules", "confidence")? {
        "ORDER BY COALESCE(confidence, 0) DESC, id DESC"
    } else {
        "ORDER BY id DESC"
    };
    let sql = format!(
        "
        SELECT * FROM import_learning_rules
        WHERE user_id = ? AND COALESCE(enabled, 1) = 1
        {order_by}
        "
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params![user_id], |row| row_to_map(row, ""))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(Value::Object)
        .collect())
}

fn load_categories(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare("SELECT * FROM categories WHERE user_id = ?")?;
    let rows = statement.query_map(params![user_id], |row| row_to_map(row, ""))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(Value::Object)
        .collect())
}

fn load_accounts(connection: &Connection, user_id: i64) -> DbResult<Vec<Value>> {
    if !table_exists(connection, "accounts")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare("SELECT * FROM accounts WHERE user_id = ?")?;
    let rows = statement.query_map(params![user_id], |row| row_to_map(row, ""))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(Value::Object)
        .collect())
}

fn user_import_learning_enabled(connection: &Connection, user_id: i64) -> DbResult<bool> {
    if !table_exists(connection, "users")?
        || !column_exists(connection, "users", "import_learning_enabled")?
    {
        return Ok(true);
    }
    connection
        .query_row(
            "SELECT COALESCE(import_learning_enabled, 1) FROM users WHERE id = ? LIMIT 1",
            params![user_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.unwrap_or(1) != 0)
        .map_err(DbError::from)
}

fn user_investment_keyword_config(connection: &Connection, user_id: i64) -> DbResult<Value> {
    if !table_exists(connection, "users")? {
        return Ok(build_user_investment_keyword_settings(None));
    }
    let user = connection
        .query_row(
            "SELECT * FROM users WHERE id = ? LIMIT 1",
            params![user_id],
            |row| row_to_map(row, ""),
        )
        .optional()?;
    Ok(build_user_investment_keyword_settings(user.as_ref()))
}

fn user_investment_keyword_config_tx(tx: &Transaction<'_>, user_id: i64) -> DbResult<Value> {
    if !table_exists_tx(tx, "users")? {
        return Ok(build_user_investment_keyword_settings(None));
    }
    let user = tx
        .query_row(
            "SELECT * FROM users WHERE id = ? LIMIT 1",
            params![user_id],
            |row| row_to_map(row, ""),
        )
        .optional()?;
    Ok(build_user_investment_keyword_settings(user.as_ref()))
}

fn get_category_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<Map<String, Value>>> {
    if !table_exists_tx(tx, "categories")? {
        return Ok(None);
    }
    tx.query_row(
        "SELECT * FROM categories WHERE id = ? AND user_id = ? LIMIT 1",
        params![category_id, user_id],
        |row| row_to_map(row, ""),
    )
    .optional()
    .map_err(DbError::from)
}

fn account_exists_on_tx(tx: &Transaction<'_>, user_id: i64, account_id: i64) -> DbResult<bool> {
    if !table_exists_tx(tx, "accounts")? {
        return Ok(false);
    }
    tx.query_row(
        "SELECT 1 FROM accounts WHERE id = ? AND user_id = ? LIMIT 1",
        params![account_id, user_id],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn record_feedback_on_tx(
    tx: &Transaction<'_>,
    user_id: i64,
    candidate_id: &str,
    action: &str,
    payload: &Value,
    now: &str,
) -> DbResult<i64> {
    tx.execute(
        "
        INSERT INTO bill_pair_feedback(user_id, candidate_id, action, payload_json, created_at)
        VALUES (?, ?, ?, ?, ?)
        ",
        params![user_id, candidate_id, action, payload.to_string(), now],
    )?;
    Ok(tx.last_insert_rowid())
}

fn reconciliation_candidate_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<Map<String, Value>> {
    let mut candidate = row_to_map(row, "")?;
    let import_snapshot =
        parse_json_object(&map_string(&candidate, "import_bill_snapshot_json", "{}"));
    let existing_snapshot =
        parse_json_object(&map_string(&candidate, "existing_bill_snapshot_json", "{}"));
    let source_payload = parse_json_object(&map_string(&candidate, "source_payload_json", "{}"));
    let group_metadata = parse_json_object(&map_string(&candidate, "group_metadata_json", "{}"));
    candidate.insert(
        "import_bill_snapshot".to_string(),
        Value::Object(import_snapshot),
    );
    candidate.insert(
        "existing_bill_snapshot".to_string(),
        Value::Object(existing_snapshot),
    );
    candidate.insert(
        "source_payload".to_string(),
        Value::Object(source_payload.clone()),
    );
    candidate.insert(
        "group_metadata".to_string(),
        Value::Object(group_metadata.clone()),
    );
    candidate.insert(
        "signal_label".to_string(),
        json!(source_payload
            .get("signal_label")
            .map(|value| value_string(Some(value)))
            .filter(|value| !value.is_empty())
            .or_else(|| {
                group_metadata
                    .get("signal_label")
                    .map(|value| value_string(Some(value)))
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or_else(|| map_string(&candidate, "reason", ""))),
    );
    candidate.insert(
        "source_chain".to_string(),
        source_payload
            .get("source_chain")
            .cloned()
            .or_else(|| group_metadata.get("source_chain").cloned())
            .unwrap_or_else(|| json!([])),
    );
    Ok(candidate)
}

fn bill_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Map<String, Value>> {
    row_to_map(row, "")
}

fn row_to_map(row: &rusqlite::Row<'_>, _prefix: &str) -> rusqlite::Result<Map<String, Value>> {
    let mut map = Map::new();
    for index in 0..row.as_ref().column_count() {
        let name = row.as_ref().column_name(index)?.to_string();
        map.insert(name, sql_value_ref_to_json(row.get_ref(index)?));
    }
    Ok(map)
}

fn prefixed_bill_from_row(
    row: &rusqlite::Row<'_>,
    prefix: &str,
) -> rusqlite::Result<Map<String, Value>> {
    let mut map = Map::new();
    for (field, key) in [
        ("id", "id"),
        ("date", "date"),
        ("type", "type"),
        ("amount", "amount"),
        ("counterparty", "counterparty"),
        ("description", "description"),
        ("payment_method", "payment_method"),
        ("main_category", "main_category"),
        ("sub_category", "sub_category"),
        ("source_account_id", "source_account_id"),
        ("destination_account_id", "destination_account_id"),
    ] {
        let column = format!("{prefix}_{field}");
        if let Ok(value) = row.get_ref(column.as_str()) {
            map.insert(key.to_string(), sql_value_ref_to_json(value));
        }
    }
    Ok(map)
}

fn sql_value_ref_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => json!(value),
        ValueRef::Text(value) => json!(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(_) => Value::Null,
    }
}

fn json_value_to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(value) => SqlValue::Integer(i64::from(*value)),
        Value::Number(number) => number
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| number.as_f64().map(SqlValue::Real))
            .unwrap_or(SqlValue::Null),
        Value::String(value) => SqlValue::Text(value.clone()),
        _ => SqlValue::Text(value.to_string()),
    }
}

fn parse_json_object(raw: &str) -> Map<String, Value> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn normalize_pair_type(pair_type: &str) -> MatchingResult<&'static str> {
    match pair_type.trim().to_ascii_lowercase().as_str() {
        TRANSFER_PAIR_TYPE => Ok(TRANSFER_PAIR_TYPE),
        INVESTMENT_PAIR_TYPE => Ok(INVESTMENT_PAIR_TYPE),
        _ => Err(MatchingRuntimeError::BadRequest(
            "Invalid pairType".to_string(),
        )),
    }
}

fn decision_from_action(action: &str) -> MatchingResult<ImportPreviewDecision> {
    match action {
        "accept" => Ok(ImportPreviewDecision::Accept),
        "reject" => Ok(ImportPreviewDecision::Reject),
        "clear" => Ok(ImportPreviewDecision::Clear),
        _ => Err(MatchingRuntimeError::BadRequest(
            "Invalid action".to_string(),
        )),
    }
}

fn map_write_error(error: DbError) -> MatchingRuntimeError {
    match error {
        DbError::InvalidOperation(message)
            if message.contains("not found") || message.contains("Bill not found") =>
        {
            MatchingRuntimeError::NotFound(message)
        }
        DbError::InvalidOperation(message) => MatchingRuntimeError::Conflict(message),
        other => MatchingRuntimeError::Db(other.to_string()),
    }
}

fn map_reconciliation_error(error: DbError) -> MatchingRuntimeError {
    match error {
        DbError::InvalidOperation(message) if message.contains("not found") => {
            MatchingRuntimeError::NotFound(message)
        }
        DbError::InvalidOperation(message) if message.contains("Invalid") => {
            MatchingRuntimeError::BadRequest(message)
        }
        DbError::InvalidOperation(message) => MatchingRuntimeError::Conflict(message),
        other => MatchingRuntimeError::Db(other.to_string()),
    }
}

fn table_exists(connection: &Connection, table: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
            params![table],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}

fn table_exists_tx(tx: &Transaction<'_>, table: &str) -> DbResult<bool> {
    tx.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
        params![table],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn column_exists(connection: &Connection, table: &str, column: &str) -> DbResult<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>("name"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == column))
}

fn column_exists_tx(tx: &Transaction<'_>, table: &str, column: &str) -> DbResult<bool> {
    let mut statement = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>("name"))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .any(|name| name == column))
}

fn map_string(map: &Map<String, Value>, key: &str, fallback: &str) -> String {
    map.get(key)
        .map(|value| value_string(Some(value)))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn map_i64(map: &Map<String, Value>, key: &str) -> i64 {
    map_optional_i64(map, key).unwrap_or(0)
}

fn map_optional_i64(map: &Map<String, Value>, key: &str) -> Option<i64> {
    value_i64(map.get(key)).filter(|value| *value > 0)
}

fn map_f64(map: &Map<String, Value>, key: &str) -> f64 {
    value_f64(map.get(key))
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn value_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Some(Value::String(text)) => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(true)) => Some(1),
        Some(Value::Bool(false)) => Some(0),
        _ => None,
    }
}

fn value_f64(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn utc_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn merge_description_values(values: impl IntoIterator<Item = String>) -> String {
    let mut merged = Vec::new();
    for value in values {
        for part in value.split('|') {
            let part = part.trim();
            if !part.is_empty() && !merged.iter().any(|item: &String| item == part) {
                merged.push(part.to_string());
            }
        }
    }
    merged.join("|")
}
