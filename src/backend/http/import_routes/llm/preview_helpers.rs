fn preview_patches_from_payload(
    connection: &Connection,
    user_id: UserId,
    payload: &Value,
    limit: usize,
) -> Result<Vec<ImportPreviewPatch>, ImportV2RouteResponse> {
    limited_preview_update_items_from_payload(payload, limit)?
        .into_iter()
        .map(|object| {
            let preview_id = preview_id_from_payload(object)?;
            build_preview_patch_from_payload_with_category_lookup(
                connection,
                user_id,
                preview_id,
                object,
            )
        })
        .collect()
}

fn selected_preview_rows_for_llm(
    connection: &Connection,
    payload: &Value,
    session_id: &str,
    user_id: UserId,
    limit: usize,
) -> Result<Vec<ImportPreviewRow>, ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let preview_ids =
        limited_id_list_field_from_object(object, &["preview_ids", "previewIds"], limit)?;
    let mut seen_update_ids = BTreeSet::new();
    let update_ids = limited_preview_update_items_from_payload(payload, limit)?
        .into_iter()
        .filter_map(|item| preview_id_from_payload(item).ok())
        .filter(|id| *id > 0 && seen_update_ids.insert(*id))
        .collect::<Vec<_>>();
    let mut rows = if let Some(preview_ids) = preview_ids {
        if preview_ids.is_empty() {
            return Err(llm_contract_error_response(
                "No preview rows selected",
                "PREVIEW_SELECTION_EMPTY",
                400,
            ));
        }
        load_preview_rows_by_ids(connection, session_id, user_id, &preview_ids)?
    } else if !update_ids.is_empty() {
        load_preview_rows_by_ids(connection, session_id, user_id, &update_ids)?
    } else {
        get_preview_by_session(connection, session_id, user_id, true).map_err(db_error_response)?
    };
    rows.truncate(limit);
    Ok(rows)
}

fn validate_llm_preview_selection_limits(
    payload: &Value,
    limit: usize,
) -> Result<(), ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let _ = limited_id_list_field_from_object(object, &["preview_ids", "previewIds"], limit)?;
    let _ = limited_preview_update_items_from_payload(payload, limit)?;
    Ok(())
}

fn load_preview_rows_by_ids(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    preview_ids: &[i64],
) -> Result<Vec<ImportPreviewRow>, ImportV2RouteResponse> {
    let mut rows = Vec::new();
    for preview_id in preview_ids.iter().copied().filter(|value| *value > 0) {
        if let Some(row) =
            get_preview_bill_by_id(connection, preview_id, user_id).map_err(db_error_response)?
        {
            if row.session_id == session_id {
                rows.push(row);
            }
        }
    }
    Ok(rows)
}

fn preview_row_prompt_value(row: &ImportPreviewRow) -> Value {
    json!({
        "id": row.id,
        "date": row.preview_date,
        "amount": row.preview_amount,
        "type": row.preview_type,
        "counterparty": row.preview_counterparty,
        "description": row.preview_description,
        "payment_method": row.preview_payment_method,
        "main_category": row.preview_main_category,
        "sub_category": row.preview_sub_category,
    })
}

fn load_llm_memory_prompt_context(
    connection: &Connection,
    user_id: UserId,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let events = get_llm_memory_events(connection, user_id, None, Some("feedback"), 20, 0)
        .map_err(db_error_response)?;
    Ok(events
        .into_iter()
        .map(|event| {
            let description_hint = event
                .metadata
                .as_ref()
                .and_then(|metadata| {
                    metadata
                        .get("description")
                        .or_else(|| metadata.get("counterparty"))
                })
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            json!({
                "decision": event.decision.unwrap_or_default(),
                "suggested_main_category": event.suggested_main_category.unwrap_or_default(),
                "suggested_sub_category": event.suggested_sub_category.unwrap_or_default(),
                "description_hint": description_hint,
            })
        })
        .collect())
}

fn load_existing_category_paths(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<String>, ImportV2RouteResponse> {
    Ok(load_existing_category_values(connection, user_id)?
        .into_iter()
        .filter_map(|value| {
            value
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect())
}

fn load_existing_category_values(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT id, main_category, sub_category FROM categories WHERE user_id = ?1 ORDER BY id ASC",
        )
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], |row| {
            let id = row.get::<_, i64>(0)?;
            let main_category = row.get::<_, Option<String>>(1)?.unwrap_or_default();
            let sub_category = row.get::<_, Option<String>>(2)?.unwrap_or_default();
            Ok(json!({
                "id": id,
                "main_category": main_category,
                "sub_category": sub_category,
                "path": category_path(&main_category, &sub_category),
            }))
        })
        .map_err(db_error_response)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)
}

fn load_existing_account_names(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<String>, ImportV2RouteResponse> {
    Ok(load_account_id_map(connection, user_id)?
        .into_keys()
        .collect::<Vec<_>>())
}

fn load_account_id_map(
    connection: &Connection,
    user_id: i64,
) -> Result<BTreeMap<String, i64>, ImportV2RouteResponse> {
    if !table_exists(connection, "accounts")? {
        return Ok(BTreeMap::new());
    }
    let mut statement = connection
        .prepare("SELECT id, name FROM accounts WHERE user_id = ?1 ORDER BY id ASC")
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                row.get::<_, i64>(0)?,
            ))
        })
        .map_err(db_error_response)?;
    let mut accounts = BTreeMap::new();
    for row in rows {
        let (name, id) = row.map_err(db_error_response)?;
        if !name.trim().is_empty() {
            accounts.insert(name, id);
        }
    }
    Ok(accounts)
}

fn fill_llm_suggestion_account_ids(value: Value, account_ids: &BTreeMap<String, i64>) -> Value {
    let Some(mut object) = value.as_object().cloned() else {
        return value;
    };
    for key in [
        "resolved_source_account_id",
        "resolvedSourceAccountId",
        "sourceAccountId",
        "source_account_id",
        "resolved_destination_account_id",
        "resolvedDestinationAccountId",
        "destinationAccountId",
        "destination_account_id",
    ] {
        object.remove(key);
    }
    for (name_keys, id_key) in [
        (
            &[
                "suggested_source_account",
                "suggestedSourceAccount",
                "sourceAccount",
                "source_account",
            ][..],
            "resolved_source_account_id",
        ),
        (
            &[
                "suggested_destination_account",
                "suggestedDestinationAccount",
                "destinationAccount",
                "destination_account",
            ][..],
            "resolved_destination_account_id",
        ),
    ] {
        if let Some(account_name) = first_value(&object, name_keys).and_then(value_to_text) {
            if let Some(account_id) = account_ids.get(account_name.trim()) {
                object.insert(id_key.to_string(), json!(account_id));
            }
        }
    }
    Value::Object(object)
}

fn llm_preview_recommendation_item(
    result: ImportPreviewLlmDecisionResult,
    preview_id: i64,
) -> Option<Value> {
    let preview = result.preview?;
    let llm_payload = preview
        .preview_matching_feedback
        .get("llm")
        .cloned()
        .unwrap_or_else(|| json!({}));
    Some(json!({
        "preview_id": preview_id,
        "preview": preview_row_to_value(preview),
        "matching": {"llm": llm_payload},
        "event_id": result.event_id,
        "applied_fields": result.applied_fields,
    }))
}

