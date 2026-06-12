// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。
async fn create_draft_from_payload_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    payload: &Value,
) -> RouteResult<BillCreateDraft> {
    let fallback_account_id = get_first_postgres_account_id(pool, user_id.get() as i64)
        .await
        .map_err(|_| Box::new(db_error_response()))?;
    let (mut fields, tag_ids, category_id) = if is_frontend_mutation(payload) {
        let (mut backend_data, metadata) = frontend_transaction_mutation_to_backend(
            payload,
            UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        )
        .map_err(|error| Box::new(bad_request(error.to_string())))?;
        apply_manual_create_defaults(&mut backend_data, payload, fallback_account_id)
            .map_err(|error| Box::new(bad_request(error.to_string())))?;
        (backend_data, metadata.tag_ids, metadata.category_id)
    } else {
        let mut fields = payload_object(payload)?.clone();
        let tag_ids = extract_tag_ids(&fields)?;
        let category_id = value_string(
            fields
                .get("categoryId")
                .or_else(|| fields.get("category_id")),
        )
        .unwrap_or_default();
        strip_route_only_keys(&mut fields);
        apply_manual_create_defaults(&mut fields, payload, fallback_account_id)
            .map_err(|error| Box::new(bad_request(error.to_string())))?;
        (fields, tag_ids, category_id)
    };

    apply_postgres_category_id(pool, user_id, &mut fields, &category_id).await?;
    let category_pair = category_pair_from_fields(&fields);
    apply_create_category_contract(
        &mut fields,
        category_pair
            .as_ref()
            .map(|(main, sub)| (main.as_str(), sub.as_str())),
        None,
    );
    ensure_create_defaults(&mut fields);
    Ok(BillCreateDraft { fields, tag_ids })
}

async fn update_draft_from_payload_postgres(
    pool: &PostgresPool,
    user_id: UserId,
    payload: &Value,
) -> RouteResult<BillUpdateDraft> {
    let (mut fields, tag_ids, category_id) = if is_frontend_mutation(payload) {
        let (backend_data, metadata) = frontend_transaction_mutation_to_backend(
            payload,
            UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        )
        .map_err(|error| Box::new(bad_request(error.to_string())))?;
        (backend_data, Some(metadata.tag_ids), metadata.category_id)
    } else {
        let mut fields = payload_object(payload)?.clone();
        let tag_ids = tag_ids_for_update(&fields)?;
        let category_id = value_string(
            fields
                .get("categoryId")
                .or_else(|| fields.get("category_id")),
        )
        .unwrap_or_default();
        strip_route_only_keys(&mut fields);
        sanitize_backend_update_fields(&mut fields, payload);
        (fields, tag_ids, category_id)
    };
    apply_postgres_category_id(pool, user_id, &mut fields, &category_id).await?;
    Ok(BillUpdateDraft { fields, tag_ids })
}

async fn apply_postgres_category_id(
    pool: &PostgresPool,
    user_id: UserId,
    fields: &mut Map<String, Value>,
    category_id: &str,
) -> RouteResult<()> {
    let Some(category_id) = parse_positive_i64(category_id) else {
        return Ok(());
    };
    let category = resolve_postgres_category_by_id(pool, user_id.get() as i64, category_id)
        .await
        .map_err(|_| Box::new(db_error_response()))?;
    if let Some((main, sub)) = category {
        fields.insert("main_category".to_string(), Value::String(main));
        fields.insert("sub_category".to_string(), Value::String(sub));
    }
    Ok(())
}

fn payload_object(payload: &Value) -> RouteResult<&Map<String, Value>> {
    payload
        .as_object()
        .filter(|object| !object.is_empty())
        .ok_or_else(|| Box::new(bad_request("No data provided")))
}

fn required_json_object_from_body(
    body: &Bytes,
    empty_message: &str,
) -> Result<Value, Box<Response>> {
    if body.is_empty() {
        return Err(Box::new(bad_request(empty_message)));
    }
    let payload = serde_json::from_slice::<Value>(body).map_err(|error| {
        Box::new(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("JSON parse error: {error}"),
        ))
    })?;
    if payload
        .as_object()
        .filter(|object| !object.is_empty())
        .is_none()
    {
        return Err(Box::new(bad_request(empty_message)));
    }
    Ok(payload)
}

fn optional_json_object_from_body(body: &Bytes) -> Value {
    if body.is_empty() {
        return Value::Object(Map::new());
    }
    serde_json::from_slice::<Value>(body).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn is_frontend_mutation(payload: &Value) -> bool {
    payload.as_object().is_some_and(|object| {
        object.contains_key("sourceAmountCents")
            || object.contains_key("destinationAmountCents")
            || object.contains_key("sourceAccountId")
            || object.contains_key("destinationAccountId")
            || object.contains_key("categoryId")
            || object.contains_key("tagIds")
            || object.get("type").is_some_and(Value::is_number)
    })
}

fn sanitize_backend_update_fields(fields: &mut Map<String, Value>, original: &Value) {
    if !fields.contains_key("payment_method") {
        if let Some(value) = fields.remove("channel").filter(is_non_empty_value) {
            fields.insert("payment_method".to_string(), value);
        }
    }
    if !fields.contains_key("main_category") {
        if let Some(value) = fields.remove("category").filter(is_non_empty_value) {
            fields.insert("main_category".to_string(), value);
        }
    }
    if !fields.contains_key("description") {
        if let Some(value) = original
            .get("remark")
            .or_else(|| original.get("comment"))
            .and_then(|value| value_string(Some(value)))
        {
            fields.insert("description".to_string(), Value::String(value));
        }
    }
    let _ = fields.remove("remark");
    let _ = fields.remove("comment");
}

fn strip_route_only_keys(fields: &mut Map<String, Value>) {
    for key in [
        "id",
        "tagIds",
        "tag_ids",
        "tags",
        "categoryId",
        "category_id",
        "clientSessionId",
        "client_session_id",
    ] {
        let _ = fields.remove(key);
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_create_defaults(fields: &mut Map<String, Value>) {
    fields
        .entry("payment_method".to_string())
        .or_insert_with(|| Value::String("manual".to_string()));
    fields
        .entry("destination_account_id".to_string())
        .or_insert_with(|| Value::Number(Number::from(0)));
    fields
        .entry("destination_amount_cents".to_string())
        .or_insert_with(|| Value::Number(Number::from(0)));
}

fn category_pair_from_fields(fields: &Map<String, Value>) -> Option<(String, String)> {
    let main = value_string(fields.get("main_category"))?;
    if main.trim().is_empty() {
        return None;
    }
    let sub = value_string(fields.get("sub_category")).unwrap_or_default();
    Some((main, sub))
}

fn extract_tag_ids(fields: &Map<String, Value>) -> RouteResult<Vec<i64>> {
    match fields.get("tagIds").or_else(|| fields.get("tag_ids")) {
        Some(value) => tag_ids_from_value(value),
        None => Ok(Vec::new()),
    }
}

fn tag_ids_for_update(fields: &Map<String, Value>) -> RouteResult<Option<Vec<i64>>> {
    match fields.get("tagIds").or_else(|| fields.get("tag_ids")) {
        Some(value) => Ok(Some(tag_ids_from_value(value)?)),
        None => Ok(None),
    }
}

fn tag_ids_from_value(value: &Value) -> RouteResult<Vec<i64>> {
    match value {
        Value::Array(items) => Ok(items.iter().filter_map(value_to_positive_i64).collect()),
        Value::String(text) => Ok(parse_csv_i64(Some(text))),
        Value::Null => Ok(Vec::new()),
        other => value_to_positive_i64(other)
            .map(|value| vec![value])
            .ok_or_else(|| Box::new(bad_request("Invalid tag IDs"))),
    }
}

fn extract_bill_ids(payload: &Value) -> Option<Vec<i64>> {
    let object = payload.as_object()?;
    object
        .get("bill_ids")
        .or_else(|| object.get("billIds"))
        .or_else(|| object.get("ids"))
        .and_then(|value| match value {
            Value::Array(items) => Some(items.iter().filter_map(value_to_positive_i64).collect()),
            Value::String(text) => Some(parse_csv_i64(Some(text))),
            other => value_to_positive_i64(other).map(|value| vec![value]),
        })
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_csv_i64(value: Option<&String>) -> Vec<i64> {
    value
        .map(|text| {
            text.split(',')
                .filter_map(|part| parse_positive_i64(part.trim()))
                .collect()
        })
        .unwrap_or_default()
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_positive_i64(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|value| *value > 0)
}

fn value_to_positive_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64().filter(|value| *value > 0),
        Value::String(text) => parse_positive_i64(text),
        _ => None,
    }
}
