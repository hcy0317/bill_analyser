/// 从批量创建请求中抽取交易条目数组，保证后续逐条转换的输入形状稳定。
pub fn batch_create_transaction_items(
    payload: &Value,
) -> Result<Vec<&Map<String, Value>>, RuntimeError> {
    let transactions = match payload {
        Value::Object(object) => object
            .get("transactions")
            .filter(|value| json_value_is_truthy(value))
            .or_else(|| object.get("bills")),
        Value::Array(_) => Some(payload),
        _ => None,
    };
    let Some(Value::Array(items)) = transactions else {
        return Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "transactions is required",
        ));
    };
    if items.is_empty() {
        return Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "transactions is required",
        ));
    }

    let mut objects = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                format!("transactions[{index}] must be an object"),
            ));
        };
        objects.push(object);
    }
    Ok(objects)
}

/// 校验单条创建交易在路由层必须存在的字段。
pub fn validate_bill_create_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    validate_fields(
        fields,
        BILL_CREATE_COLUMNS,
        "unsupported bill create fields",
    )
}

/// 校验单条更新交易在路由层必须存在的字段。
pub fn validate_bill_update_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    validate_fields(
        fields,
        BILL_UPDATE_COLUMNS,
        "unsupported bill update fields",
    )
}

/// 校验批量更新条目必须携带 id 和局部更新字段。
pub fn validate_batch_route_update_fields<'a>(
    fields: impl IntoIterator<Item = &'a str>,
) -> Result<(), RuntimeError> {
    validate_fields(
        fields,
        ROUTE_ALLOWED_BATCH_UPDATE_FIELDS,
        "unsupported update fields",
    )
}

/// 生成批量更新响应，保持旧接口的 updatedCount 字段合同。
pub fn batch_update_response(
    success_count: usize,
    failed_count: usize,
    failed_ids: Vec<i64>,
) -> BatchUpdateResponse {
    BatchUpdateResponse {
        updated_count: success_count,
        failed_count,
        failed_ids,
    }
}

/// 生成批量创建成功 payload，保留逐条结果和 id 列表两个兼容字段。
pub fn batch_create_success_response(items: Vec<Value>, ids: Vec<String>) -> BatchCreateResult {
    BatchCreateResult {
        failed_index: None,
        created_count: items.len(),
        items,
        ids,
    }
}

/// 将批量创建成功 payload 包装成路由响应合同。
pub fn batch_create_success_route_response(
    items: Vec<Value>,
    ids: Vec<String>,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 201,
        body: success_result_body(
            serde_json::to_value(batch_create_success_response(items, ids))
                .expect("batch create result should serialize"),
        ),
    }
}

/// 生成批量创建部分失败的业务响应，保留成功项和逐条错误明细。
pub fn batch_create_failure_response(
    failed_index: usize,
    created_items: Vec<Value>,
    created_ids: Vec<String>,
) -> BatchCreateResult {
    BatchCreateResult {
        failed_index: Some(failed_index),
        created_count: created_items.len(),
        items: created_items,
        ids: created_ids,
    }
}

/// 生成批量创建在 payload 预处理阶段失败时的路由响应。
pub fn batch_create_prepare_error_route_response(
    error: impl Into<String>,
    failed_index: usize,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 400,
        body: error_result_body(error, batch_create_prepare_error_result(failed_index)),
    }
}

/// 生成批量创建在持久化阶段失败时的路由响应。
pub fn batch_create_persist_error_route_response(
    error: impl Into<String>,
    failed_index: usize,
    created_items: Vec<Value>,
    created_ids: Vec<String>,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 500,
        body: error_result_body(
            error,
            serde_json::to_value(batch_create_failure_response(
                failed_index,
                created_items,
                created_ids,
            ))
            .expect("batch create failure result should serialize"),
        ),
    }
}

/// 生成删除单笔正式账单后的兼容 payload。
pub fn delete_bill_success_payload() -> Value {
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Bool(true));
    payload.insert(
        "message".to_string(),
        Value::String("Bill deleted successfully".to_string()),
    );
    Value::Object(payload)
}

/// 生成批量删除后的删除数量 payload。
pub fn batch_delete_success_payload(deleted_count: usize) -> Value {
    let mut result = Map::new();
    result.insert(
        "deleted_count".to_string(),
        Value::Number(Number::from(deleted_count)),
    );
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Object(result));
    Value::Object(payload)
}

/// 汇总批量更新前后受影响账户，供余额重算去重使用。
pub fn batch_update_balance_sync_account_ids<'a>(
    _updated_fields: impl IntoIterator<Item = &'a str>,
) -> Vec<i64> {
    Vec::new()
}
