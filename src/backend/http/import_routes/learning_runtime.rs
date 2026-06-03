// 中文导读：Postgres-only learning center facade。
// 维护重点：只读取当前 Postgres import_learning_* 规则/建议表。
// 不变式：导入预览的 lifecycle/feedback 由 db/import_staging 的 Postgres 表负责，这里只保留当前路由响应壳。

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ImportLearningPromotionResult {
    rules_total: i64,
    created: i64,
    updated: i64,
}

enum LearningSuggestionDecision {
    NotFound,
}

fn init_import_learning_runtime_schema(
    _runtime: &ImportRuntime,
) -> Result<(), ImportV2RouteResponse> {
    Ok(())
}

fn init_global_learning_runtime_schema(
    _runtime: &ImportRuntime,
) -> Result<(), ImportV2RouteResponse> {
    Ok(())
}

fn count_learning_suggestions(
    _connection: &Connection,
    _user_id: UserId,
    _status: Option<&str>,
) -> Result<i64, ImportV2RouteResponse> {
    Ok(0)
}

fn load_learning_suggestions(
    _connection: &Connection,
    _user_id: UserId,
    _status: Option<&str>,
    _limit: usize,
    _offset: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    Ok(Vec::new())
}

fn mine_learning_suggestions(
    _connection: &Connection,
    _user_id: UserId,
) -> Result<Value, ImportV2RouteResponse> {
    Ok(json!({
        "created": 0,
        "updated": 0,
        "skipped": 0,
        "samples": 0,
    }))
}

fn accept_learning_suggestion(
    _connection: &Connection,
    _suggestion_id: i64,
    _user_id: UserId,
) -> Result<LearningSuggestionDecision, ImportV2RouteResponse> {
    Ok(LearningSuggestionDecision::NotFound)
}

fn reject_learning_suggestion(
    _connection: &Connection,
    _suggestion_id: i64,
    _user_id: UserId,
) -> Result<bool, ImportV2RouteResponse> {
    Ok(false)
}

fn promote_import_learning_rules(
    _connection: &Connection,
    _session_id: &str,
    _user_id: UserId,
    _preview_ids: &[i64],
) -> Result<ImportLearningPromotionResult, ImportV2RouteResponse> {
    Ok(ImportLearningPromotionResult::default())
}

fn annotation_samples_from_payload(payload: &Value) -> Vec<ImportAnnotationSampleDraft> {
    let Ok(items) = preview_update_items_from_payload(payload) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let preview_id = preview_id_from_payload(item).ok()?;
            Some(ImportAnnotationSampleDraft {
                preview_id,
                annotated_type: first_value(
                    item,
                    &[
                        "annotated_type",
                        "annotatedType",
                        "preview_type",
                        "previewType",
                        "type",
                    ],
                )
                .and_then(value_to_preview_type_text),
                annotated_category_id: first_value(
                    item,
                    &[
                        "annotated_category_id",
                        "annotatedCategoryId",
                        "category_id",
                        "categoryId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
                annotated_source_account_id: first_value(
                    item,
                    &[
                        "annotated_source_account_id",
                        "annotatedSourceAccountId",
                        "preview_source_account_id",
                        "previewSourceAccountId",
                        "sourceAccountId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
                annotated_destination_account_id: first_value(
                    item,
                    &[
                        "annotated_destination_account_id",
                        "annotatedDestinationAccountId",
                        "preview_destination_account_id",
                        "previewDestinationAccountId",
                        "destinationAccountId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
            })
        })
        .collect()
}

fn count_import_learning_rules(
    _connection: &Connection,
    _user_id: UserId,
    _enabled_only: Option<bool>,
) -> Result<i64, ImportV2RouteResponse> {
    Ok(0)
}

fn load_import_learning_rules(
    _connection: &Connection,
    _user_id: UserId,
    _enabled_only: Option<bool>,
    _limit: usize,
    _offset: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    Ok(Vec::new())
}

fn set_import_learning_rule_enabled(
    _connection: &Connection,
    _rule_id: i64,
    _user_id: UserId,
    _enabled: bool,
) -> Result<bool, ImportV2RouteResponse> {
    Ok(false)
}

fn update_import_learning_rule(
    _connection: &Connection,
    _rule_id: i64,
    _user_id: UserId,
    _object: &Map<String, Value>,
) -> Result<(), ImportV2RouteResponse> {
    Err(learning_error_response(404, "rule_not_found"))
}

fn get_import_learning_rule(
    _connection: &Connection,
    _rule_id: i64,
    _user_id: UserId,
) -> Result<Option<Value>, ImportV2RouteResponse> {
    Ok(None)
}

fn delete_import_learning_rule(
    _connection: &Connection,
    _rule_id: i64,
    _user_id: UserId,
) -> Result<bool, ImportV2RouteResponse> {
    Ok(false)
}

fn learning_data_response(data: Value) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

fn learning_error_response(status_code: u16, error: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

fn learning_rule_camel_to_snake(rule: &Value) -> Value {
    rule.clone()
}
