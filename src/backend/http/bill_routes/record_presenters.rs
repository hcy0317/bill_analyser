fn page_to_frontend(
    connection: &Connection,
    user_id: UserId,
    page: usize,
    page_size: usize,
    bill_page: bill_analyser_db::BillPage,
) -> bill_analyser_db::DbResult<Value> {
    let mut items = Vec::with_capacity(bill_page.bills.len());
    for bill in bill_page.bills {
        items.push(record_to_frontend_value(connection, user_id, bill)?);
    }
    let total = usize::try_from(bill_page.total.max(0)).unwrap_or(usize::MAX);
    Ok(json!({
        "success": true,
        "result": {
            "items": items,
            "totalCount": bill_page.total,
            "page": page,
            "pageSize": page_size,
            "total": bill_page.total,
            "page_size": page_size,
            "total_pages": total.div_ceil(page_size),
        }
    }))
}

fn get_frontend_bill(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> bill_analyser_db::DbResult<Option<Value>> {
    get_bill_by_id(connection, user_id, bill_id)?
        .map(|record| record_to_frontend_value(connection, user_id, record))
        .transpose()
}

fn record_to_frontend_value(
    connection: &Connection,
    user_id: UserId,
    record: BillRecord,
) -> bill_analyser_db::DbResult<Value> {
    let bill_id = record_i64(&record, "id").unwrap_or(0);
    let tags = get_bill_tags(connection, user_id, bill_id)?;
    let frontend_tags = tags
        .iter()
        .filter_map(frontend_tag_from_value)
        .collect::<Vec<_>>();
    let tag_ids = frontend_tags.iter().map(|tag| tag.id.clone()).collect();
    let bill = BackendTransactionView {
        id: bill_id.to_string(),
        time_sequence_id: None,
        transaction_type: frontend_transaction_type_from_backend(&record_text(&record, "type"))
            .map_err(runtime_error)?,
        category_id: category_id_for_record(connection, user_id, &record)?,
        main_category: record_text(&record, "main_category"),
        sub_category: record_text(&record, "sub_category"),
        date: record_text(&record, "date"),
        amount: money_from_record(&record, "amount")?,
        destination_amount: Some(money_from_record(&record, "destination_amount")?),
        source_account_id: positive_record_i64(&record, "source_account_id"),
        destination_account_id: positive_record_i64(&record, "destination_account_id"),
        utc_offset: UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        hide_amount: false,
        tag_ids,
        tags: frontend_tags,
        category: None,
        source_account: None,
        destination_account: None,
        description: record_text(&record, "description"),
    };
    serde_json::to_value(frontend_transaction_from_backend(&bill))
        .map_err(|error| bill_analyser_db::DbError::InvalidOperation(error.to_string()))
}

fn category_filters_for_ids(
    connection: &Connection,
    user_id: UserId,
    category_ids: &[i64],
) -> rusqlite::Result<Vec<BillCategoryFilter>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let category_ids = category_ids
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", category_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut params = category_ids
        .iter()
        .copied()
        .map(SqlValue::Integer)
        .collect::<Vec<_>>();
    params.push(SqlValue::Integer(user_id.get() as i64));
    let mut statement = connection.prepare(&format!(
        "SELECT main_category, sub_category FROM categories WHERE id IN ({placeholders}) AND user_id = ?"
    ))?;
    let rows = statement.query_map(params_from_iter(params), |row| {
        Ok(BillCategoryFilter {
            main: row.get::<_, String>(0)?,
            sub: row
                .get::<_, Option<String>>(1)?
                .filter(|value| !value.is_empty()),
        })
    })?;
    let mut filters = Vec::new();
    for row in rows {
        filters.push(row?);
    }
    Ok(filters)
}

fn category_id_for_record(
    connection: &Connection,
    user_id: UserId,
    record: &BillRecord,
) -> bill_analyser_db::DbResult<Option<String>> {
    if !table_exists(connection, "categories").map_err(bill_analyser_db::DbError::from)? {
        return Ok(None);
    }
    let main = record_text(record, "main_category");
    if main.trim().is_empty() {
        return Ok(None);
    }
    let sub = record_text(record, "sub_category");
    let id = connection
        .query_row(
            "SELECT id FROM categories WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3 LIMIT 1",
            params![user_id.get() as i64, main, sub],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(bill_analyser_db::DbError::from)?;
    Ok(id.map(|value| value.to_string()))
}

fn apply_category_id(
    connection: &Connection,
    user_id: UserId,
    fields: &mut Map<String, Value>,
    category_id: &str,
) -> RouteResult<()> {
    let Some(category_id) = parse_positive_i64(category_id) else {
        return Ok(());
    };
    let category = resolve_category_by_id(connection, user_id, category_id)
        .map_err(|_| Box::new(db_error_response()))?;
    if let Some((main, sub)) = category {
        fields.insert("main_category".to_string(), Value::String(main));
        fields.insert("sub_category".to_string(), Value::String(sub));
    }
    Ok(())
}

fn resolve_category_by_id(
    connection: &Connection,
    user_id: UserId,
    category_id: i64,
) -> rusqlite::Result<Option<(String, String)>> {
    if !table_exists(connection, "categories")? {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT main_category, sub_category FROM categories WHERE id = ?1 AND user_id = ?2",
            params![category_id, user_id.get() as i64],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
}

fn table_exists(connection: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
}
