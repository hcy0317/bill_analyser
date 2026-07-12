// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn update_postgres_virtual_category_handler(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    old_name: &str,
    body: &Value,
) -> Response {
    let new_name = category_name_from_body(body).unwrap_or_else(|| old_name.to_string());
    if new_name != old_name {
        match update_postgres_main_category_name(pool, old_name, &new_name, user_id).await {
            Ok(true) => {}
            Ok(false) => return error_response(StatusCode::CONFLICT, "Category rename conflict"),
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to rename category",
                );
            }
        }
    }

    let updates = Value::Object(virtual_category_update_payload(body, &new_name));
    let category_id =
        match get_postgres_category_by_name(pool, &new_name, "", user_id).await {
            Ok(Some(existing)) => {
                let Some(category_id) = existing.get("id").and_then(value_as_i64) else {
                    return category_db_error_response();
                };
                match update_postgres_category(pool, category_id, &updates, user_id).await {
                    Ok(true) | Ok(false) => category_id,
                    Err(_) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                }
            }
            Ok(None) => {
                let create_payload = Value::Object(virtual_category_create_payload(body, &new_name));
                match create_postgres_category(pool, &create_payload, user_id).await {
                    Ok(Some(category_id)) => category_id,
                    Ok(None) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                    Err(_) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                }
            }
            Err(_) => return category_db_error_response(),
        };

    match get_postgres_category_by_id(pool, category_id, user_id).await {
        Ok(Some(category)) => success_result(
            StatusCode::OK,
            Value::Object(backend_category_to_frontend(&category, "0")),
        ),
        Ok(None) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to load updated category",
        ),
        Err(_) => category_db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_postgres_real_category_handler(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    category_id: i64,
    body: &Value,
) -> Response {
    let mut updates = category_update_payload_from_frontend(body);

    if let Some(new_name) = category_name_from_body(body) {
        let category = match get_postgres_category_by_id(pool, category_id, user_id).await {
            Ok(Some(value)) => value,
            Ok(None) => return not_found("Category not found"),
            Err(_) => return category_db_error_response(),
        };
        if category_text(&category, "sub_category").is_empty() {
            let old_name = category_text(&category, "main_category");
            if new_name != old_name {
                match update_postgres_main_category_name(pool, &old_name, &new_name, user_id).await
                {
                    Ok(true) => {}
                    Ok(false) => {
                        return error_response(StatusCode::CONFLICT, "Category rename conflict");
                    }
                    Err(_) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to rename category",
                        );
                    }
                }
            }
        } else {
            updates.insert("sub_category".to_string(), Value::String(new_name));
        }
    }

    let payload = Value::Object(updates);
    match update_postgres_category(pool, category_id, &payload, user_id).await {
        Ok(true) => match get_postgres_category_by_id(pool, category_id, user_id).await {
            Ok(Some(category)) => success_result(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&category, "0")),
            ),
            Ok(None) => error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load updated category",
            ),
            Err(_) => category_db_error_response(),
        },
        Ok(false) => not_found("Category not found"),
        Err(error) => {
            let text = error.to_string();
            if text.contains("constraint") || text.contains("duplicate") {
                error_response(StatusCode::CONFLICT, "Category update conflict")
            } else {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update category",
                )
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CategoryPayloadMode {
    FrontendDefaults,
    ImportDefaults,
}

#[tracing::instrument(level = "debug", skip_all)]
async fn resolve_postgres_parent_category(
    pool: &bill_analyser_db::PostgresPool,
    parent_id: &str,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<(String, i64)>> {
    if let Some(parent_name) = virtual_category_name(parent_id) {
        let categories = list_postgres_categories(pool, user_id).await?;
        return Ok(resolve_virtual_parent_category(&categories, &parent_name));
    }
    let Ok(parent_id) = parent_id.parse::<i64>() else {
        return Ok(None);
    };
    Ok(get_postgres_category_by_id(pool, parent_id, user_id)
        .await?
        .and_then(|parent| resolved_real_parent_category(&parent)))
}

fn normalized_category_parent_id(parent_id: Option<&Value>) -> String {
    let parent_id = value_string(parent_id, "0");
    let parent_id = parent_id.trim();
    if parent_id.is_empty() || parent_id == "0" {
        "0".to_string()
    } else {
        parent_id.to_string()
    }
}

fn requested_category_type_matches_parent(body: &Value, parent_type: i64) -> bool {
    body.get("type")
        .map(|value| value_as_i64(value) == Some(parent_type))
        .unwrap_or(true)
}

fn resolved_real_parent_category(parent: &CategoryRecord) -> Option<(String, i64)> {
    if parent.get("hidden").is_some_and(value_truthy)
        || !category_text(parent, "sub_category").trim().is_empty()
    {
        return None;
    }
    let name = category_text(parent, "main_category");
    let category_type = parent.get("type").and_then(value_as_i64)?;
    (!name.trim().is_empty()).then_some((name, category_type))
}

fn resolve_virtual_parent_category(
    categories: &[CategoryRecord],
    parent_name: &str,
) -> Option<(String, i64)> {
    let mut matching = categories.iter().filter(|category| {
        category_text(category, "main_category") == parent_name
            && !category.get("hidden").is_some_and(value_truthy)
    });
    let first = matching.next()?;
    let category_type = first.get("type").and_then(value_as_i64)?;
    if matching.any(|category| category.get("type").and_then(value_as_i64) != Some(category_type)) {
        return None;
    }
    Some((parent_name.to_string(), category_type))
}

#[cfg(test)]
mod category_parent_contract_tests {
    use super::*;

    fn category(main: &str, sub: &str, category_type: i64, hidden: bool) -> CategoryRecord {
        Map::from_iter([
            ("main_category".to_string(), Value::String(main.to_string())),
            ("sub_category".to_string(), Value::String(sub.to_string())),
            ("type".to_string(), Value::Number(category_type.into())),
            ("hidden".to_string(), Value::Bool(hidden)),
        ])
    }

    #[test]
    fn parent_id_normalizes_null_empty_and_zero_to_root() {
        assert_eq!(normalized_category_parent_id(None), "0");
        assert_eq!(normalized_category_parent_id(Some(&Value::Null)), "0");
        assert_eq!(normalized_category_parent_id(Some(&Value::String("".into()))), "0");
        assert_eq!(normalized_category_parent_id(Some(&Value::String(" 0 ".into()))), "0");
    }

    #[test]
    fn explicit_child_type_must_match_parent_while_missing_type_inherits() {
        assert!(requested_category_type_matches_parent(&json!({ "name": "早餐" }), 2));
        assert!(requested_category_type_matches_parent(&json!({ "name": "早餐", "type": 2 }), 2));
        assert!(!requested_category_type_matches_parent(&json!({ "name": "早餐", "type": 1 }), 2));
        assert!(!requested_category_type_matches_parent(&json!({ "name": "早餐", "type": "invalid" }), 2));
    }

    #[test]
    fn real_parent_must_be_active_primary_and_inherits_type() {
        assert_eq!(resolved_real_parent_category(&category("餐饮", "", 2, false)), Some(("餐饮".into(), 2)));
        assert_eq!(resolved_real_parent_category(&category("餐饮", "早餐", 2, false)), None);
        assert_eq!(resolved_real_parent_category(&category("餐饮", "", 2, true)), None);
    }

    #[test]
    fn virtual_parent_inherits_consistent_active_tree_type() {
        let categories = vec![
            category("餐饮", "早餐", 2, false),
            category("餐饮", "午餐", 2, false),
        ];
        assert_eq!(resolve_virtual_parent_category(&categories, "餐饮"), Some(("餐饮".into(), 2)));

        let mixed = vec![category("餐饮", "早餐", 2, false), category("餐饮", "午餐", 1, false)];
        assert_eq!(resolve_virtual_parent_category(&mixed, "餐饮"), None);
        assert_eq!(resolve_virtual_parent_category(&[category("餐饮", "早餐", 2, true)], "餐饮"), None);
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn category_parent_id_for_postgres_get(
    pool: &bill_analyser_db::PostgresPool,
    category: &CategoryRecord,
    user_id: i64,
) -> String {
    let sub_category = category_text(category, "sub_category");
    if sub_category.is_empty() {
        return "0".to_string();
    }
    let main_category = category_text(category, "main_category");
    match get_postgres_category_by_name(pool, &main_category, "", user_id).await {
        Ok(Some(parent)) => parent
            .get("id")
            .map(|value| value_string(Some(value), "0"))
            .unwrap_or_else(|| format!("virtual_{main_category}")),
        Ok(None) | Err(_) => format!("virtual_{main_category}"),
    }
}

fn category_name_from_body(payload: &Value) -> Option<String> {
    payload
        .as_object()
        .and_then(|object| object.get("name"))
        .map(|value| string_or_default(Some(value), ""))
}

fn virtual_category_name(category_id: &str) -> Option<String> {
    category_id
        .strip_prefix("virtual_")
        .map(ToString::to_string)
}

fn frontend_category_to_backend(
    payload: &Value,
    main_category: &str,
    sub_category: &str,
    category_type: i64,
    mode: CategoryPayloadMode,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert(
        "type".to_string(),
        Value::Number(Number::from(category_type)),
    );
    result.insert(
        "main_category".to_string(),
        Value::String(main_category.to_string()),
    );
    result.insert(
        "sub_category".to_string(),
        Value::String(sub_category.to_string()),
    );
    result.insert(
        "description".to_string(),
        Value::String(string_or_default(payload.get("comment"), "")),
    );
    result.insert(
        "priority".to_string(),
        Value::Number(Number::from(
            payload
                .get("displayOrder")
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    let hidden = match mode {
        CategoryPayloadMode::FrontendDefaults => payload
            .get("visible")
            .map(|visible| !value_truthy(visible))
            .or_else(|| payload.get("hidden").map(value_truthy))
            .unwrap_or(false),
        CategoryPayloadMode::ImportDefaults => {
            payload.get("hidden").map(value_truthy).unwrap_or(false)
        }
    };
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(payload.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(payload.get("color"), "")),
    );
    result
}

fn virtual_category_update_payload(payload: &Value, main_category: &str) -> Map<String, Value> {
    let category_type = payload.get("type").and_then(value_as_i64).unwrap_or(1);
    frontend_category_to_backend(
        payload,
        main_category,
        "",
        category_type,
        CategoryPayloadMode::FrontendDefaults,
    )
}

fn virtual_category_create_payload(payload: &Value, main_category: &str) -> Map<String, Value> {
    virtual_category_update_payload(payload, main_category)
}

fn category_update_payload_from_frontend(payload: &Value) -> Map<String, Value> {
    let mut result = Map::new();
    if let Some(comment) = payload.get("comment") {
        result.insert(
            "description".to_string(),
            Value::String(string_or_default(Some(comment), "")),
        );
    }
    if let Some(display_order) = payload.get("displayOrder").and_then(value_as_i64) {
        result.insert(
            "priority".to_string(),
            Value::Number(Number::from(display_order)),
        );
    }
    if let Some(category_type) = payload.get("type").and_then(value_as_i64) {
        result.insert(
            "type".to_string(),
            Value::Number(Number::from(category_type)),
        );
    }
    if let Some(visible) = payload.get("visible") {
        result.insert("hidden".to_string(), Value::Bool(!value_truthy(visible)));
    }
    if let Some(icon) = payload.get("icon") {
        result.insert(
            "icon".to_string(),
            Value::String(string_or_default(Some(icon), "")),
        );
    }
    if let Some(color) = payload.get("color") {
        result.insert(
            "color".to_string(),
            Value::String(string_or_default(Some(color), "")),
        );
    }
    result
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_category_payload(
    payload: &Value,
    main_category: &str,
    sub_category: &str,
) -> Map<String, Value> {
    let category_type = payload.get("type").and_then(value_as_i64).unwrap_or(3);
    let mut result = frontend_category_to_backend(
        payload,
        main_category,
        sub_category,
        category_type,
        CategoryPayloadMode::ImportDefaults,
    );
    if let Some(description) = payload.get("description") {
        result.insert(
            "description".to_string(),
            Value::String(string_or_default(Some(description), "")),
        );
    }
    if let Some(priority) = payload.get("priority").and_then(value_as_i64) {
        result.insert(
            "priority".to_string(),
            Value::Number(Number::from(priority)),
        );
    }
    result
}
