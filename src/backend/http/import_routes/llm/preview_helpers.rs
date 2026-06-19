// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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

#[tracing::instrument(level = "debug", skip_all)]
fn validate_llm_preview_selection_limits(
    payload: &Value,
    limit: usize,
) -> Result<(), ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let _ = limited_id_list_field_from_object(object, &["preview_ids", "previewIds"], limit)?;
    let _ = limited_preview_update_items_from_payload(payload, limit)?;
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
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
        "amount_cents": row.preview_amount_cents,
        "type": row.preview_type,
        "counterparty": row.preview_counterparty,
        "description": row.preview_description,
        "payment_method": row.preview_payment_method,
        "main_category": row.preview_main_category,
        "sub_category": row.preview_sub_category,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_llm_memory_prompt_context(
    connection: &Connection,
    user_id: UserId,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let events = get_llm_memory_events(connection, user_id, None, 20)
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

#[tracing::instrument(level = "debug", skip_all)]
async fn load_existing_category_paths(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<String>, ImportV2RouteResponse> {
    Ok(load_existing_category_values(connection, user_id).await?
        .into_iter()
        .filter_map(|value| {
            let id = value.get("id").and_then(Value::as_i64)?;
            let category_type = value.get("type").and_then(Value::as_str).unwrap_or_default();
            let path = value
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if path.is_empty() {
                return None;
            }
            Some(format!("ID={id} | {category_type} | {path}"))
        })
        .collect())
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_existing_category_values(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let rows = sqlx::query(
        r#"
        SELECT id, category_type, path, name
        FROM categories
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await
    .map_err(db_error_response)?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id").map_err(db_error_response)?;
            let category_type: Option<String> =
                row.try_get("category_type").map_err(db_error_response)?;
            let path: Option<String> = row.try_get("path").map_err(db_error_response)?;
            let name: String = row.try_get("name").map_err(db_error_response)?;
            let path = category_prompt_path(path.as_deref(), &name);
            Ok(json!({
                "id": id,
                "type": category_type_label(category_type.as_deref()).unwrap_or_default(),
                "path": path,
            }))
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_existing_account_names(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<String>, ImportV2RouteResponse> {
    Ok(load_account_id_map(connection, user_id).await?
        .into_keys()
        .collect::<Vec<_>>())
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_account_id_map(
    connection: &Connection,
    user_id: i64,
) -> Result<BTreeMap<String, i64>, ImportV2RouteResponse> {
    let rows = sqlx::query(
        r#"
        SELECT id, name
        FROM accounts
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(connection)
    .await
    .map_err(db_error_response)?;
    let mut accounts = BTreeMap::new();
    for row in rows {
        let id: i64 = row.try_get("id").map_err(db_error_response)?;
        let name: String = row.try_get("name").map_err(db_error_response)?;
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        accounts.insert(trimmed.to_string(), id);
    }
    Ok(accounts)
}

fn category_prompt_path(path: Option<&str>, name: &str) -> String {
    let parts = path
        .unwrap_or_default()
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return name.trim().to_string();
    }
    parts.join("/")
}

fn category_type_label(value: Option<&str>) -> Option<&'static str> {
    match value.unwrap_or_default().trim().to_ascii_lowercase().as_str() {
        "2" | "income" | "收入" => Some("收入"),
        "3" | "expense" | "支出" => Some("支出"),
        "4" | "transfer" | "转账" => Some("转账"),
        "5" | "investment" | "投资" => Some("投资"),
        _ => None,
    }
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
