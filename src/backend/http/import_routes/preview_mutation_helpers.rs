// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn bool_field_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    first_value(object, keys).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    })
}

fn id_list_field_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<Vec<i64>> {
    first_value(object, keys).and_then(|value| {
        let ids = value
            .as_array()?
            .iter()
            .filter_map(value_to_i64)
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();
        Some(ids)
    })
}

fn limited_id_list_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
    limit: usize,
) -> Result<Option<Vec<i64>>, ImportV2RouteResponse> {
    let Some(value) = first_value(object, keys) else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(llm_contract_error_response(
            "ID list fields must be arrays",
            "INVALID_REQUEST",
            400,
        ));
    };
    let mut seen_ids = BTreeSet::new();
    let mut ids = Vec::new();
    for value in values {
        let Some(id) = value_to_i64(value) else {
            return Err(llm_contract_error_response(
                "ID list fields must contain integer IDs",
                "INVALID_REQUEST",
                400,
            ));
        };
        if id <= 0 || !seen_ids.insert(id) {
            continue;
        }
        ids.push(id);
    }
    if ids.len() > limit {
        return Err(preview_selection_too_large_response());
    }
    Ok(Some(ids))
}

fn preview_selection_too_large_response() -> ImportV2RouteResponse {
    llm_contract_error_response(
        "Selected preview rows exceed the maximum batch size",
        "PREVIEW_SELECTION_TOO_LARGE",
        422,
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn generate_import_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = IMPORT_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("rust-import-{nanos}-{counter}")
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn preview_update_items_from_payload(
    payload: &Value,
) -> Result<Vec<&Map<String, Value>>, ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let Some(updates) = first_value(object, &["preview_updates", "previewUpdates"]) else {
        return Ok(Vec::new());
    };
    let updates = updates
        .as_array()
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))?;
    let mut items = Vec::with_capacity(updates.len());
    for item in updates {
        items.push(payload_object(item)?);
    }
    Ok(items)
}

fn limited_preview_update_items_from_payload(
    payload: &Value,
    limit: usize,
) -> Result<Vec<&Map<String, Value>>, ImportV2RouteResponse> {
    let items = preview_update_items_from_payload(payload)?;
    if items.len() > limit {
        return Err(preview_selection_too_large_response());
    }
    Ok(items)
}

fn preview_id_from_payload(object: &Map<String, Value>) -> Result<i64, ImportV2RouteResponse> {
    first_value(object, &["id", "preview_id", "previewId"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| import_v2_error_response(400, "Missing bill id"))
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_preview_patch_from_payload(
    preview_id: i64,
    object: &Map<String, Value>,
) -> ImportPreviewPatch {
    let mut changes = Vec::new();
    push_text_change(
        &mut changes,
        object,
        &["date", "preview_date", "previewDate"],
        ImportPreviewPatchField::Date,
    );
    push_preview_type_change(
        &mut changes,
        object,
        &["type", "preview_type", "previewType"],
    );
    push_minor_units_change(
        &mut changes,
        object,
        &[
            "amountCents",
            "amount_cents",
            "previewAmountCents",
            "preview_amount_cents",
            "sourceAmountCents",
        ],
        ImportPreviewPatchField::Amount,
    );
    push_minor_units_change(
        &mut changes,
        object,
        &[
            "destinationAmountCents",
            "destination_amount_cents",
            "previewDestinationAmountCents",
            "preview_destination_amount_cents",
        ],
        ImportPreviewPatchField::DestinationAmount,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "mainCategory",
            "preview_main_category",
            "previewMainCategory",
        ],
        ImportPreviewPatchField::MainCategory,
    );
    push_text_change(
        &mut changes,
        object,
        &["subCategory", "preview_sub_category", "previewSubCategory"],
        ImportPreviewPatchField::SubCategory,
    );
    push_nullable_i64_change(
        &mut changes,
        object,
        &[
            "sourceAccountId",
            "preview_source_account_id",
            "previewSourceAccountId",
        ],
        ImportPreviewPatchField::SourceAccountId,
    );
    push_nullable_i64_change(
        &mut changes,
        object,
        &[
            "destinationAccountId",
            "preview_destination_account_id",
            "previewDestinationAccountId",
        ],
        ImportPreviewPatchField::DestinationAccountId,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "counterparty",
            "preview_counterparty",
            "previewCounterparty",
        ],
        ImportPreviewPatchField::Counterparty,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "paymentMethod",
            "preview_payment_method",
            "previewPaymentMethod",
        ],
        ImportPreviewPatchField::PaymentMethod,
    );
    push_text_change(
        &mut changes,
        object,
        &["description", "preview_description", "previewDescription"],
        ImportPreviewPatchField::Description,
    );
    push_nullable_i64_change(
        &mut changes,
        object,
        &["recurringId", "preview_recurring_id", "previewRecurringId"],
        ImportPreviewPatchField::RecurringId,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "recurringName",
            "preview_recurring_name",
            "previewRecurringName",
        ],
        ImportPreviewPatchField::RecurringName,
    );
    push_i64_change(
        &mut changes,
        object,
        &[
            "recurringCandidateCount",
            "preview_recurring_candidate_count",
            "previewRecurringCandidateCount",
        ],
        ImportPreviewPatchField::RecurringCandidateCount,
    );
    push_real_change(
        &mut changes,
        object,
        &[
            "recurringMatchScore",
            "preview_recurring_match_score",
            "previewRecurringMatchScore",
        ],
        ImportPreviewPatchField::RecurringMatchScore,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "recurringMatchReasons",
            "preview_recurring_match_reasons",
            "previewRecurringMatchReasons",
        ],
        ImportPreviewPatchField::RecurringMatchReasons,
    );
    push_text_change(
        &mut changes,
        object,
        &[
            "recurringMatchedDate",
            "preview_recurring_matched_date",
            "previewRecurringMatchedDate",
        ],
        ImportPreviewPatchField::RecurringMatchedDate,
    );
    if let Some(value) = first_value(object, &["isSelected", "selected", "preview_selected"]) {
        changes.push((
            ImportPreviewPatchField::Selected,
            ImportPreviewPatchValue::Bool(coerce_preview_selected_value(Some(value), true)),
        ));
    }
    if let Some(value) = first_value(
        object,
        &[
            "matchingFeedback",
            "preview_matching_feedback",
            "previewMatchingFeedback",
        ],
    ) {
        changes.push((
            ImportPreviewPatchField::MatchingFeedback,
            ImportPreviewPatchValue::Json(value.clone()),
        ));
    }

    let mut patch = ImportPreviewPatch::new(preview_id).with_changes(changes);
    let clear_transfer = first_value(
        object,
        &["clear_transfer_decision", "clearTransferDecision"],
    )
    .is_some_and(|value| coerce_preview_selected_value(Some(value), false))
        || clear_actionable_suggestion_family(object, "transfer");
    let clear_learning = first_value(
        object,
        &["clear_learning_decision", "clearLearningDecision"],
    )
    .is_some_and(|value| coerce_preview_selected_value(Some(value), false))
        || clear_actionable_suggestion_family(object, "learning");
    let clear_llm = first_value(object, &["clear_llm_decision", "clearLlmDecision"])
        .is_some_and(|value| coerce_preview_selected_value(Some(value), false))
        || clear_actionable_suggestion_family(object, "llm");
    if clear_transfer {
        patch = patch.with_transfer_decision_cleared();
    }
    if clear_learning {
        patch = patch.with_learning_decision_cleared();
    }
    if clear_llm {
        patch = patch.with_llm_decision_cleared();
    }
    patch
}

#[tracing::instrument(level = "debug", skip_all)]
fn clear_actionable_suggestion_family(object: &Map<String, Value>, family: &str) -> bool {
    let Some(value) = first_value(
        object,
        &[
            "clear_actionable_suggestions",
            "clearActionableSuggestions",
        ],
    ) else {
        return false;
    };
    if value
        .as_bool()
        .is_some_and(|enabled| enabled)
    {
        return true;
    }
    if let Some(items) = value.as_array() {
        return items.iter().any(|item| {
            item.as_str()
                .is_some_and(|text| text.trim().eq_ignore_ascii_case(family))
        });
    }
    value.as_object().is_some_and(|families| {
        families
            .get(family)
            .is_some_and(|value| coerce_preview_selected_value(Some(value), false))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_preview_patch_from_payload_with_category_lookup(
    connection: &Connection,
    user_id: UserId,
    preview_id: i64,
    object: &Map<String, Value>,
) -> Result<ImportPreviewPatch, ImportV2RouteResponse> {
    let mut patch = build_preview_patch_from_payload(preview_id, object);
    apply_category_id_to_preview_patch(connection, user_id, object, &mut patch)?;
    Ok(patch)
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_category_id_to_preview_patch(
    connection: &Connection,
    user_id: UserId,
    object: &Map<String, Value>,
    patch: &mut ImportPreviewPatch,
) -> Result<(), ImportV2RouteResponse> {
    let Some(value) = first_value(object, &["categoryId", "category_id"]) else {
        return Ok(());
    };

    let Some(category_id) = value_to_i64(value).filter(|value| *value > 0) else {
        clear_category_id_on_preview_patch(patch);
        return Ok(());
    };

    let Some(category) = load_preview_payload_category(connection, user_id, category_id)? else {
        return Err(import_v2_error_response(400, "Invalid category"));
    };

    apply_loaded_category_to_preview_patch(patch, category_id, category);
    Ok(())
}

fn clear_category_id_on_preview_patch(patch: &mut ImportPreviewPatch) {
    patch
        .changes
        .retain(|(field, _)| *field != ImportPreviewPatchField::CategoryId);
    patch
        .changes
        .push((ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Null));
    set_preview_patch_text_change(patch, ImportPreviewPatchField::MainCategory, String::new());
    set_preview_patch_text_change(patch, ImportPreviewPatchField::SubCategory, String::new());
}

fn apply_loaded_category_to_preview_patch(
    patch: &mut ImportPreviewPatch,
    category_id: i64,
    category: PreviewPayloadCategory,
) {
    if let Some(preview_type) = preview_payload_category_type_name(category.type_code) {
        set_preview_patch_text_change(
            patch,
            ImportPreviewPatchField::Type,
            preview_type.to_string(),
        );
    }
    patch
        .changes
        .retain(|(field, _)| *field != ImportPreviewPatchField::CategoryId);
    patch.changes.push((
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(category_id),
    ));
    set_preview_patch_text_change(
        patch,
        ImportPreviewPatchField::MainCategory,
        category.main_category,
    );
    set_preview_patch_text_change(
        patch,
        ImportPreviewPatchField::SubCategory,
        category.sub_category,
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewPayloadCategory {
    type_code: Option<i64>,
    main_category: String,
    sub_category: String,
}

impl From<ImportPreviewCategoryLookup> for PreviewPayloadCategory {
    fn from(category: ImportPreviewCategoryLookup) -> Self {
        Self {
            type_code: category.type_code,
            main_category: category.main_category,
            sub_category: category.sub_category,
        }
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_preview_payload_category(
    connection: &Connection,
    user_id: UserId,
    category_id: i64,
) -> Result<Option<PreviewPayloadCategory>, ImportV2RouteResponse> {
    get_import_preview_category_by_id(connection, user_id, category_id)
        .map(|category| category.map(Into::into))
        .map_err(db_error_response)
}

fn set_preview_patch_text_change(
    patch: &mut ImportPreviewPatch,
    field: ImportPreviewPatchField,
    value: String,
) {
    patch.changes.retain(|(existing_field, _)| *existing_field != field);
    patch
        .changes
        .push((field, ImportPreviewPatchValue::Text(value)));
}

fn preview_payload_category_type_name(type_code: Option<i64>) -> Option<&'static str> {
    match type_code {
        Some(2) => Some("收入"),
        Some(3) => Some("支出"),
        Some(4) => Some("转账"),
        Some(5) => Some("投资"),
        _ => None,
    }
}

#[cfg(test)]
mod preview_mutation_helper_tests {
    use super::*;

    fn change_value(
        patch: &ImportPreviewPatch,
        target: ImportPreviewPatchField,
    ) -> Option<&ImportPreviewPatchValue> {
        patch
            .changes
            .iter()
            .rev()
            .find_map(|(field, value)| (*field == target).then_some(value))
    }

    #[test]
    fn category_id_patch_clear_removes_identity_and_category_names() {
        let mut patch = ImportPreviewPatch::new(7)
            .with_change(
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(99),
            )
            .with_change(
                ImportPreviewPatchField::MainCategory,
                ImportPreviewPatchValue::Text("旧主类".to_string()),
            )
            .with_change(
                ImportPreviewPatchField::SubCategory,
                ImportPreviewPatchValue::Text("旧子类".to_string()),
            );

        clear_category_id_on_preview_patch(&mut patch);

        assert!(matches!(
            change_value(&patch, ImportPreviewPatchField::CategoryId),
            Some(ImportPreviewPatchValue::Null)
        ));
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MainCategory),
            Some(&ImportPreviewPatchValue::Text(String::new()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::SubCategory),
            Some(&ImportPreviewPatchValue::Text(String::new()))
        );
        assert_eq!(
            patch.changes
                .iter()
                .filter(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
                .count(),
            1
        );
    }

    #[test]
    fn category_id_patch_apply_uses_canonical_category_identity() {
        let mut patch = ImportPreviewPatch::new(7)
            .with_change(
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(1),
            )
            .with_change(
                ImportPreviewPatchField::Type,
                ImportPreviewPatchValue::Text("支出".to_string()),
            );
        let category = PreviewPayloadCategory {
            type_code: Some(2),
            main_category: "理财".to_string(),
            sub_category: "理财收益".to_string(),
        };

        apply_loaded_category_to_preview_patch(&mut patch, 42, category);

        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::CategoryId),
            Some(&ImportPreviewPatchValue::Integer(42))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::Type),
            Some(&ImportPreviewPatchValue::Text("收入".to_string()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::MainCategory),
            Some(&ImportPreviewPatchValue::Text("理财".to_string()))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::SubCategory),
            Some(&ImportPreviewPatchValue::Text("理财收益".to_string()))
        );
        assert_eq!(
            patch.changes
                .iter()
                .filter(|(field, _)| *field == ImportPreviewPatchField::CategoryId)
                .count(),
            1
        );
    }

    #[test]
    fn preview_payload_category_maps_from_db_lookup() {
        let category = PreviewPayloadCategory::from(ImportPreviewCategoryLookup {
            type_code: Some(5),
            main_category: "投资".to_string(),
            sub_category: "理财收益".to_string(),
        });

        assert_eq!(category.type_code, Some(5));
        assert_eq!(category.main_category, "投资");
        assert_eq!(category.sub_category, "理财收益");
    }

    #[test]
    fn preview_patch_payload_accepts_explicit_cents_aliases() {
        let object = Map::from_iter([
            ("amountCents".to_string(), json!(12345)),
            ("destinationAmountCents".to_string(), json!("54321")),
        ]);

        let patch = build_preview_patch_from_payload(7, &object);

        assert_eq!(patch.preview_id, 7);
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::Amount),
            Some(&ImportPreviewPatchValue::Integer(12345))
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::DestinationAmount),
            Some(&ImportPreviewPatchValue::Integer(54321))
        );
    }

    #[test]
    fn preview_patch_payload_rejects_non_integer_cents_values() {
        let object = Map::from_iter([
            ("amountCents".to_string(), json!(true)),
            ("destinationAmountCents".to_string(), json!(12.34)),
            ("recurringCandidateCount".to_string(), json!(2)),
        ]);

        let patch = build_preview_patch_from_payload(7, &object);

        assert_eq!(change_value(&patch, ImportPreviewPatchField::Amount), None);
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::DestinationAmount),
            None
        );
        assert_eq!(
            change_value(&patch, ImportPreviewPatchField::RecurringCandidateCount),
            Some(&ImportPreviewPatchValue::Integer(2))
        );
    }

    #[tokio::test]
    async fn category_id_patch_ignores_missing_value_and_clears_invalid_value() {
        let connection =
            PostgresPool::connect_lazy("postgres://localhost/bill_analyser_test").expect("lazy pool");
        let mut patch = ImportPreviewPatch::new(7);
        let object = Map::new();

        apply_category_id_to_preview_patch(
            &connection,
            UserId::new(1).expect("user id"),
            &object,
            &mut patch,
        )
        .expect("missing category id is ignored");
        assert!(patch.changes.is_empty());

        let mut object = Map::new();
        object.insert("categoryId".to_string(), json!(0));
        apply_category_id_to_preview_patch(
            &connection,
            UserId::new(1).expect("user id"),
            &object,
            &mut patch,
        )
        .expect("invalid category id clears category");
        assert!(patch.changes.iter().any(|(field, value)| {
            *field == ImportPreviewPatchField::CategoryId
                && *value == ImportPreviewPatchValue::Null
        }));
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_preview_updates_from_payload(
    runtime: &mut ImportRuntime,
    session_id: &str,
    user_id: UserId,
    payload: &Value,
) -> Result<usize, ImportV2RouteResponse> {
    let update_items = preview_update_items_from_payload(payload)?;
    if update_items.is_empty() {
        return Ok(0);
    }

    let mut patches = Vec::with_capacity(update_items.len());
    for item in update_items {
        let preview_id = preview_id_from_payload(item)?;
        match get_preview_bill_by_id(runtime.connection(), preview_id, user_id) {
            Ok(Some(preview)) if preview.session_id == session_id => {}
            Ok(Some(_)) | Ok(None) => {
                return Err(import_v2_error_response(404, "Preview bill not found"));
            }
            Err(error) => return Err(db_error_response(error)),
        }
        patches.push(build_preview_patch_from_payload_with_category_lookup(
            runtime.connection(),
            user_id,
            preview_id,
            item,
        )?);
    }
    update_preview_bills_batch(runtime.connection_mut(), session_id, user_id, &patches)
        .map_err(db_error_response)
}

fn preview_ids_from_payload(payload: &Value) -> Vec<i64> {
    let Ok(object) = payload_object(payload) else {
        return Vec::new();
    };
    let mut preview_ids = Vec::new();
    if let Some(value) = first_value(object, &["preview_ids", "previewIds"]) {
        match value {
            Value::Array(values) => {
                preview_ids.extend(values.iter().filter_map(value_to_i64).filter(|id| *id > 0));
            }
            other => {
                if let Some(preview_id) = value_to_i64(other).filter(|id| *id > 0) {
                    preview_ids.push(preview_id);
                }
            }
        }
    }
    if preview_ids.is_empty() {
        if let Ok(items) = preview_update_items_from_payload(payload) {
            preview_ids.extend(
                items
                    .iter()
                    .filter_map(|item| preview_id_from_payload(item).ok()),
            );
        }
    }
    preview_ids.sort_unstable();
    preview_ids.dedup();
    preview_ids
}

fn expected_state_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewExpectedState, ImportV2RouteResponse> {
    let expected_state = first_value(object, &["expectedState", "expected_state"])
        .and_then(Value::as_object)
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))?;
    Ok(ImportPreviewExpectedState {
        session_id: first_value(expected_state, &["sessionId", "session_id"])
            .and_then(value_to_text),
        preview_type: first_value(expected_state, &["type", "previewType", "preview_type"])
            .and_then(value_to_text),
        preview_main_category: first_value(
            expected_state,
            &[
                "mainCategory",
                "previewMainCategory",
                "preview_main_category",
            ],
        )
        .and_then(value_to_text),
        preview_sub_category: first_value(
            expected_state,
            &["subCategory", "previewSubCategory", "preview_sub_category"],
        )
        .and_then(value_to_text),
        preview_recurring_id: optional_id_field_from_object(
            expected_state,
            &[
                "recurringTemplateId",
                "recurringId",
                "previewRecurringId",
                "preview_recurring_id",
            ],
        ),
        preview_source_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "sourceAccountId",
                "previewSourceAccountId",
                "preview_source_account_id",
            ],
        ),
        preview_destination_account_id: optional_id_field_from_object(
            expected_state,
            &[
                "destinationAccountId",
                "previewDestinationAccountId",
                "preview_destination_account_id",
            ],
        ),
        preview_matching_feedback: first_value(
            expected_state,
            &[
                "matchingFeedback",
                "previewMatchingFeedback",
                "preview_matching_feedback",
            ],
        )
        .cloned(),
    })
}

fn optional_id_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
) -> Option<Option<i64>> {
    first_value(object, keys).map(|value| match value_to_i64(value) {
        Some(value) if value > 0 => Some(value),
        _ => None,
    })
}

fn decision_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewDecision, ImportV2RouteResponse> {
    let decision = first_value(object, &["decision"])
        .and_then(value_to_text)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    match decision.as_str() {
        "accept" | "accepted" => Ok(ImportPreviewDecision::Accept),
        "reject" | "rejected" => Ok(ImportPreviewDecision::Reject),
        "clear" | "cleared" => Ok(ImportPreviewDecision::Clear),
        _ => Err(import_v2_error_response(400, "Invalid decision")),
    }
}

fn decision_name(decision: ImportPreviewDecision) -> &'static str {
    match decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    }
}

fn recurring_candidate_from_payload(
    object: &Map<String, Value>,
    recurring_id: i64,
) -> Option<ImportPreviewRecurringCandidate> {
    let candidate = first_value(
        object,
        &[
            "candidate",
            "targetCandidate",
            "target_candidate",
            "recurringCandidate",
            "recurring_candidate",
        ],
    )
    .and_then(Value::as_object)?;
    Some(ImportPreviewRecurringCandidate {
        id: first_value(candidate, &["id", "recurringId", "recurring_id"])
            .and_then(value_to_i64)
            .unwrap_or(recurring_id),
        name: first_value(candidate, &["name", "recurringName", "recurring_name"])
            .and_then(value_to_text)
            .unwrap_or_default(),
        match_score: first_value(candidate, &["matchScore", "match_score"])
            .and_then(value_to_f64)
            .unwrap_or_default(),
        match_reasons: match_reasons_from_value(first_value(
            candidate,
            &["matchReasons", "match_reasons"],
        )),
        matched_occurrence_date: first_value(
            candidate,
            &["matchedOccurrenceDate", "matched_occurrence_date"],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
    })
}

fn recurring_candidate_count_from_payload(
    object: &Map<String, Value>,
    target_candidate: Option<&ImportPreviewRecurringCandidate>,
) -> i64 {
    first_value(
        object,
        &[
            "candidateCount",
            "candidate_count",
            "recurringCandidateCount",
            "previewRecurringCandidateCount",
            "preview_recurring_candidate_count",
        ],
    )
    .and_then(value_to_i64)
    .unwrap_or_else(|| i64::from(target_candidate.is_some()))
}

fn match_reasons_from_value(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(value_to_text)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect(),
        Some(value) => value_to_text(value)
            .unwrap_or_default()
            .split(['|', ','])
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
            .collect(),
        None => Vec::new(),
    }
}

fn preview_decision_result_response(
    result: ImportPreviewDecisionResult,
    extra: Value,
) -> ImportV2RouteResponse {
    if result.state_conflict {
        return preview_state_conflict_response();
    }
    if result.invalid_recurring_id {
        return import_v2_error_response(400, "Invalid recurringId");
    }
    let Some(preview) = result.preview else {
        return import_v2_error_response(404, "Preview bill not found");
    };
    let preview_id = preview.id;
    let session_id = preview.session_id.clone();
    let preview_item = preview_row_to_value(preview);
    let mut data = json!({
        "previewId": preview_id,
        "sessionId": session_id,
        "previewItem": preview_item.clone(),
        "preview": [preview_item],
    });
    if let (Some(data), Some(extra)) = (data.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            data.insert(key.clone(), value.clone());
        }
    }
    import_v2_data_response(data)
}

fn llm_suggestion_from_value(value: &Value) -> Option<ImportPreviewLlmSuggestion> {
    let object = value.as_object()?;
    let suggestion = ImportPreviewLlmSuggestion {
        suggested_main_category: first_value(
            object,
            &[
                "suggested_main_category",
                "suggestedMainCategory",
                "mainCategory",
                "main_category",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_sub_category: first_value(
            object,
            &[
                "suggested_sub_category",
                "suggestedSubCategory",
                "subCategory",
                "sub_category",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_source_account: first_value(
            object,
            &[
                "suggested_source_account",
                "suggestedSourceAccount",
                "sourceAccount",
                "source_account",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        suggested_destination_account: first_value(
            object,
            &[
                "suggested_destination_account",
                "suggestedDestinationAccount",
                "destinationAccount",
                "destination_account",
            ],
        )
        .and_then(value_to_text)
        .unwrap_or_default(),
        resolved_source_account_id: first_value(
            object,
            &[
                "resolved_source_account_id",
                "resolvedSourceAccountId",
                "sourceAccountId",
                "source_account_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        resolved_destination_account_id: first_value(
            object,
            &[
                "resolved_destination_account_id",
                "resolvedDestinationAccountId",
                "destinationAccountId",
                "destination_account_id",
            ],
        )
        .and_then(value_to_i64)
        .filter(|value| *value > 0),
        confidence: first_value(object, &["confidence"])
            .and_then(value_to_f64)
            .unwrap_or_default(),
        reason: first_value(object, &["reason"])
            .and_then(value_to_text)
            .unwrap_or_default(),
    };
    Some(suggestion)
}

fn llm_decision_result_response(
    result: ImportPreviewLlmDecisionResult,
    session_id: &str,
    preview_id: i64,
    decision: &str,
) -> ImportV2RouteResponse {
    let Some(preview) = result.preview else {
        return import_v2_error_response(404, "Preview recommendation is no longer available");
    };
    let llm_payload = preview
        .preview_matching_feedback
        .get("llm")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let preview = preview_row_to_value(preview);
    import_v2_data_response(json!({
        "session_id": session_id,
        "preview_id": preview_id,
        "preview": preview,
        "matching": {"llm": llm_payload},
        "event_id": result.event_id,
        "applied_fields": result.applied_fields,
        "decision": decision,
    }))
}

fn first_value<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn push_text_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_text) {
        changes.push((field, ImportPreviewPatchValue::Text(value)));
    }
}

fn push_preview_type_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_preview_type_text) {
        changes.push((
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(value),
        ));
    }
}

fn push_real_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_f64) {
        changes.push((field, ImportPreviewPatchValue::Real(value)));
    }
}

fn push_i64_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_i64) {
        changes.push((field, ImportPreviewPatchValue::Integer(value)));
    }
}

fn push_minor_units_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_minor_units) {
        changes.push((field, ImportPreviewPatchValue::Integer(value)));
    }
}

fn push_nullable_i64_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys) {
        let patch_value = match value_to_i64(value) {
            Some(value) if value > 0 => ImportPreviewPatchValue::Integer(value),
            _ => ImportPreviewPatchValue::Null,
        };
        changes.push((field, patch_value));
    }
}

fn value_to_preview_type_text(value: &Value) -> Option<String> {
    if let Some(label) = value_to_i64(value).and_then(transaction_type_label_from_i64) {
        return Some(label.to_string());
    }
    let text = value_to_text(value)?;
    normalize_transaction_type_text(&text)
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_transaction_type_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = match trimmed.to_ascii_lowercase().as_str() {
        "expense" => "支出",
        "income" => "收入",
        "transfer" => "转账",
        "investment" => "投资",
        _ => trimmed,
    };
    Some(normalized.to_string())
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Bool(_) => None,
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_minor_units(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<f64>().ok()
            }
        }
        Value::Bool(value) => Some(f64::from(u8::from(*value))),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn response_mode_is_preview_item(object: &Map<String, Value>) -> bool {
    first_value(object, &["responseMode", "response_mode"]).is_some_and(|value| {
        value
            .as_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("preview-item"))
    })
}

fn preview_row_to_value(row: ImportPreviewRow) -> Value {
    let mut value = serde_json::to_value(row).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        attach_import_preview_matching_payload(object);
    }
    value
}

fn preview_row_is_categorized(row: &&ImportPreviewRow) -> bool {
    !row.preview_main_category.trim().is_empty() || !row.preview_sub_category.trim().is_empty()
}

fn preview_row_has_account(row: &&ImportPreviewRow) -> bool {
    row.preview_source_account_id.is_some() || row.preview_destination_account_id.is_some()
}
