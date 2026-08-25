#[derive(Debug, Clone, PartialEq)]
struct PreviewPatchProjection {
    preview: ImportPreviewRow,
    payload: Value,
    signal_projection: ImportPreviewSignalProjection,
    amount_cents: i64,
    direction: &'static str,
}

fn project_preview_patch(
    mut preview: ImportPreviewRow,
    mut payload: Value,
    patch: &ImportPreviewPatch,
    identity_maps: &ImportIdentityMaps,
) -> DbResult<PreviewPatchProjection> {
    apply_patch_changes_to_preview(&mut preview, &mut payload, &patch.changes)?;
    if patch.clear_transfer_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "transfer");
    }
    if patch.clear_learning_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "learning");
    }
    if patch.clear_llm_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "llm");
    }
    apply_identity_validation_to_preview(&mut preview, &mut payload, identity_maps);
    payload_set(
        &mut payload,
        "preview_matching_feedback",
        preview.preview_matching_feedback.clone(),
    );
    let amount_cents = preview
        .preview_amount_cents
        .checked_abs()
        .ok_or_else(|| DbError::InvalidOperation("invalid preview amount_cents".to_string()))?;
    let direction = if matches!(preview.preview_type.as_str(), "收入" | "income") {
        "income"
    } else {
        "expense"
    };
    let signal_projection = import_preview_signal_projection_from_payload(&payload)?;

    Ok(PreviewPatchProjection {
        preview,
        payload,
        signal_projection,
        amount_cents,
        direction,
    })
}

fn apply_patch_value_to_preview(
    preview: &mut ImportPreviewRow,
    payload: &mut Value,
    field: ImportPreviewPatchField,
    value: ImportPreviewPatchValue,
) -> DbResult<()> {
    match (field, value) {
        (ImportPreviewPatchField::Date, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_date = value.clone();
            payload_set(payload, "preview_date", json!(value));
        }
        (ImportPreviewPatchField::Type, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_type = value.clone();
            payload_set(payload, "preview_type", json!(value));
        }
        (ImportPreviewPatchField::Amount, ImportPreviewPatchValue::Integer(value)) => {
            let value = value.checked_abs().ok_or_else(|| {
                DbError::InvalidOperation("invalid preview amount_cents".to_string())
            })?;
            preview.preview_amount_cents = value;
            payload_set(payload, "preview_amount_cents", json!(value));
        }
        (ImportPreviewPatchField::DestinationAmount, ImportPreviewPatchValue::Integer(value)) => {
            let value = value.checked_abs().ok_or_else(|| {
                DbError::InvalidOperation(
                    "invalid preview destination_amount_cents".to_string(),
                )
            })?;
            preview.preview_destination_amount_cents = value;
            payload_set(
                payload,
                "preview_destination_amount_cents",
                json!(value),
            );
        }
        (ImportPreviewPatchField::MainCategory, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_main_category = value.clone();
            payload_set(payload, "preview_main_category", json!(value));
        }
        (ImportPreviewPatchField::SubCategory, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_sub_category = value.clone();
            payload_set(payload, "preview_sub_category", json!(value));
        }
        (ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Integer(value)) => {
            preview.category_id = Some(value);
            payload_set(payload, "category_id", json!(value));
            payload_set(payload, "categoryId", json!(value));
        }
        (ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Null) => {
            preview.category_id = None;
            payload_set(payload, "category_id", Value::Null);
            payload_set(payload, "categoryId", Value::Null);
        }
        (ImportPreviewPatchField::SourceAccountId, ImportPreviewPatchValue::Integer(value)) => {
            preview.preview_source_account_id = Some(value);
            payload_set(payload, "preview_source_account_id", json!(value));
        }
        (ImportPreviewPatchField::SourceAccountId, ImportPreviewPatchValue::Null) => {
            preview.preview_source_account_id = None;
            payload_set(payload, "preview_source_account_id", Value::Null);
        }
        (
            ImportPreviewPatchField::DestinationAccountId,
            ImportPreviewPatchValue::Integer(value),
        ) => {
            preview.preview_destination_account_id = Some(value);
            payload_set(payload, "preview_destination_account_id", json!(value));
        }
        (ImportPreviewPatchField::DestinationAccountId, ImportPreviewPatchValue::Null) => {
            preview.preview_destination_account_id = None;
            payload_set(payload, "preview_destination_account_id", Value::Null);
        }
        (ImportPreviewPatchField::Counterparty, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_counterparty = value.clone();
            payload_set(payload, "preview_counterparty", json!(value));
        }
        (ImportPreviewPatchField::PaymentMethod, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_payment_method = value.clone();
            payload_set(payload, "preview_payment_method", json!(value));
        }
        (ImportPreviewPatchField::Description, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_description = value.clone();
            payload_set(payload, "preview_description", json!(value));
        }
        (ImportPreviewPatchField::RecurringId, ImportPreviewPatchValue::Integer(value)) => {
            preview.preview_recurring_id = Some(value);
            payload_set(payload, "preview_recurring_id", json!(value));
        }
        (ImportPreviewPatchField::RecurringId, ImportPreviewPatchValue::Null) => {
            preview.preview_recurring_id = None;
            payload_set(payload, "preview_recurring_id", Value::Null);
        }
        (ImportPreviewPatchField::RecurringName, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_recurring_name = value.clone();
            payload_set(payload, "preview_recurring_name", json!(value));
        }
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(value),
        ) => {
            preview.preview_recurring_candidate_count = value;
            payload_set(payload, "preview_recurring_candidate_count", json!(value));
        }
        (ImportPreviewPatchField::RecurringMatchScore, ImportPreviewPatchValue::Real(value)) => {
            preview.preview_recurring_match_score = value;
            payload_set(payload, "preview_recurring_match_score", json!(value));
        }
        (ImportPreviewPatchField::RecurringMatchReasons, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_recurring_match_reasons = value.clone();
            payload_set(payload, "preview_recurring_match_reasons", json!(value));
        }
        (ImportPreviewPatchField::RecurringMatchedDate, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_recurring_matched_date = value.clone();
            payload_set(payload, "preview_recurring_matched_date", json!(value));
        }
        (ImportPreviewPatchField::Selected, ImportPreviewPatchValue::Bool(value)) => {
            preview.preview_selected = value;
            payload_set(payload, "preview_selected", json!(value));
        }
        (ImportPreviewPatchField::ManualAnnotation, ImportPreviewPatchValue::Bool(true)) => {
            mark_preview_manual_annotation(preview, payload);
        }
        (ImportPreviewPatchField::MatchingFeedback, ImportPreviewPatchValue::Json(value)) => {
            preview.preview_matching_feedback = value.clone();
            payload_set(payload, "preview_matching_feedback", value);
        }
        _ => {
            return Err(DbError::InvalidOperation(format!(
                "invalid preview patch value for {field:?}"
            )))
        }
    }
    Ok(())
}

fn mark_preview_manual_annotation(preview: &mut ImportPreviewRow, payload: &mut Value) {
    if !preview.preview_matching_feedback.is_object() {
        preview.preview_matching_feedback = json!({});
    }
    let Some(feedback) = preview.preview_matching_feedback.as_object_mut() else {
        return;
    };
    let annotation = feedback
        .entry("annotation".to_string())
        .or_insert_with(|| json!({}));
    if !annotation.is_object() {
        *annotation = json!({});
    }
    if let Some(annotation) = annotation.as_object_mut() {
        annotation.insert("is_manually_annotated".to_string(), json!(true));
    }
    payload_set(
        payload,
        "preview_matching_feedback",
        preview.preview_matching_feedback.clone(),
    );
}

fn apply_patch_changes_to_preview(
    preview: &mut ImportPreviewRow,
    payload: &mut Value,
    changes: &[(ImportPreviewPatchField, ImportPreviewPatchValue)],
) -> DbResult<()> {
    for (field, value) in changes {
        apply_patch_value_to_preview(preview, payload, *field, value.clone())?;
    }
    let is_manual = changes.iter().any(|(field, value)| {
        *field == ImportPreviewPatchField::ManualAnnotation
            && *value == ImportPreviewPatchValue::Bool(true)
    });
    if !is_manual {
        return Ok(());
    }
    let edited_fields = changes.iter().filter_map(|(field, value)| match field {
        ImportPreviewPatchField::CategoryId => Some((
            "category_id",
            matches!(value, ImportPreviewPatchValue::Integer(id) if *id > 0),
        )),
        ImportPreviewPatchField::SourceAccountId => Some((
            "source_account_id",
            matches!(value, ImportPreviewPatchValue::Integer(id) if *id > 0),
        )),
        ImportPreviewPatchField::DestinationAccountId => Some((
            "destination_account_id",
            matches!(value, ImportPreviewPatchValue::Integer(id) if *id > 0),
        )),
        _ => None,
    });
    mark_manual_identity_ownership(preview, payload, edited_fields);
    Ok(())
}

fn mark_manual_identity_ownership<'a>(
    preview: &mut ImportPreviewRow,
    payload: &mut Value,
    edited_fields: impl Iterator<Item = (&'a str, bool)>,
) {
    let edited_fields = edited_fields.collect::<Vec<_>>();
    let feedback = preview
        .preview_matching_feedback
        .as_object_mut()
        .expect("manual annotation creates feedback object");
    let annotation = feedback
        .entry("annotation".to_string())
        .or_insert_with(|| json!({}));
    let manual_fields = annotation
        .as_object_mut()
        .expect("manual annotation object")
        .entry("manual_fields".to_string())
        .or_insert_with(|| json!({}));
    if !manual_fields.is_object() {
        *manual_fields = json!({});
    }
    for field in ["category_id", "source_account_id", "destination_account_id"] {
        manual_fields
            .as_object_mut()
            .expect("manual fields object")
            .entry(field.to_string())
            .or_insert_with(|| json!(false));
    }
    for (field, owned) in &edited_fields {
        manual_fields
            .as_object_mut()
            .expect("manual fields object")
            .insert((*field).to_string(), json!(owned));
    }
    if let Some(transfer) = feedback.get_mut("transfer").and_then(Value::as_object_mut) {
        let owned_fields = transfer
            .entry("owned_fields".to_string())
            .or_insert_with(|| json!({}));
        if !owned_fields.is_object() {
            let prior = owned_fields
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .map(|field| (field, json!(true)))
                .collect();
            *owned_fields = Value::Object(prior);
        }
        for (field, _) in edited_fields {
            owned_fields
                .as_object_mut()
                .expect("owned fields object")
                .insert(field.to_string(), json!(false));
        }
    }
    payload_set(
        payload,
        "preview_matching_feedback",
        preview.preview_matching_feedback.clone(),
    );
}
