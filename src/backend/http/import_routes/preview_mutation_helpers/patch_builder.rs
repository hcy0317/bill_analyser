/// 从前端 preview update payload 构建 DB patch，并保留 manual edit 与 actionable suggestion 清理语义。
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
    if bool_field_from_object(
        object,
        &[
            "isManuallyAnnotated",
            "is_manually_annotated",
            "preview_is_manually_annotated",
            "previewIsManuallyAnnotated",
        ],
    )
    .is_some_and(|value| value)
        && payload_contains_manual_edit_field(object)
    {
        changes.push((
            ImportPreviewPatchField::ManualAnnotation,
            ImportPreviewPatchValue::Bool(true),
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

fn payload_contains_manual_edit_field(object: &Map<String, Value>) -> bool {
    const MANUAL_EDIT_KEYS: &[&str] = &[
        "type",
        "preview_type",
        "previewType",
        "amountCents",
        "amount_cents",
        "previewAmountCents",
        "preview_amount_cents",
        "sourceAmountCents",
        "destinationAmountCents",
        "destination_amount_cents",
        "previewDestinationAmountCents",
        "preview_destination_amount_cents",
        "mainCategory",
        "preview_main_category",
        "previewMainCategory",
        "subCategory",
        "preview_sub_category",
        "previewSubCategory",
        "categoryId",
        "category_id",
        "sourceAccountId",
        "preview_source_account_id",
        "previewSourceAccountId",
        "destinationAccountId",
        "preview_destination_account_id",
        "previewDestinationAccountId",
        "counterparty",
        "preview_counterparty",
        "previewCounterparty",
        "paymentMethod",
        "preview_payment_method",
        "previewPaymentMethod",
        "description",
        "preview_description",
        "previewDescription",
    ];
    MANUAL_EDIT_KEYS.iter().any(|key| object.contains_key(*key))
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

/// 构建带分类 lookup 的 preview patch，保证 category_id 是 canonical 身份来源。
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
