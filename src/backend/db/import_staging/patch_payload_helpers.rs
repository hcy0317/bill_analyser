async fn apply_preview_patch_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    patch: &ImportPreviewPatch,
    identity_maps: &ImportIdentityMaps,
) -> DbResult<bool> {
    let Some(row) = sqlx::query(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.id = $1 AND p.session_id = $2 AND p.user_id = $3 FOR UPDATE OF p",
    )
    .bind(patch.preview_id)
    .bind(session_db_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    else {
        return Ok(false);
    };
    let mut preview = preview_from_pg_row(&row)?;
    let mut payload = row
        .try_get::<Value, _>("preview_payload")
        .unwrap_or_else(|_| json!({}));
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
    let direction = if preview.preview_type == "收入" || preview.preview_type == "income" {
        "income"
    } else {
        "expense"
    };
    let mut query = build_preview_row_update_query(
        &preview,
        payload.to_string(),
        amount_cents,
        direction,
        PreviewRowUpdateTarget {
            preview_id: patch.preview_id,
            session_db_id,
            user_id,
            expected_row_version: patch.expected_row_version,
        },
    );
    let changed = query.build().execute(&mut **tx).await?.rows_affected();
    if changed == 0 {
        if let Some(expected) = patch.expected_row_version {
            return Err(DbError::preview_version_conflict(
                patch.preview_id,
                expected,
                preview.version,
            ));
        }
    }
    Ok(changed > 0)
}

#[derive(Debug, Clone, Copy)]
struct PreviewRowUpdateTarget {
    preview_id: i64,
    session_db_id: i64,
    user_id: i64,
    expected_row_version: Option<i64>,
}

fn build_preview_row_update_query(
    preview: &ImportPreviewRow,
    preview_payload: String,
    amount_cents: i64,
    direction: &str,
    target: PreviewRowUpdateTarget,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        UPDATE import_preview_rows
        SET selected =
        "#,
    );
    query.push_bind(preview.preview_selected);
    query.push(", occurred_at = ");
    query.push_bind(normalize_bill_date_text(&preview.preview_date));
    query.push("::timestamptz, amount_cents = ");
    query.push_bind(amount_cents);
    query.push(", direction = ");
    query.push_bind(direction.to_string());
    query.push(", transaction_type = ");
    query.push_bind(preview.preview_type.clone());
    query.push(", account_id = ");
    query.push_bind(preview.preview_source_account_id);
    query.push(", transfer_target_account_id = ");
    query.push_bind(preview.preview_destination_account_id);
    query.push(", category_id = ");
    query.push_bind(preview.category_id);
    query.push(", merchant = ");
    query.push_bind(preview.preview_counterparty.clone());
    query.push(", payment_method = ");
    query.push_bind(preview.preview_payment_method.clone());
    query.push(", description = ");
    query.push_bind(preview.preview_description.clone());
    if preview.preview_type != "转账" && preview.preview_type != "transfer" {
        query.push(", hidden_transfer_payload = '{}'::jsonb");
    }
    query.push(", preview_payload = ");
    query.push_bind(preview_payload);
    query.push(
        r#"::jsonb,
            updated_at = now(),
            version = version + 1
        "#,
    );
    query.push(" WHERE id = ");
    query.push_bind(target.preview_id);
    query.push(" AND session_id = ");
    query.push_bind(target.session_db_id);
    query.push(" AND user_id = ");
    query.push_bind(target.user_id);
    if let Some(expected_row_version) = target.expected_row_version {
        query.push(" AND version = ");
        query.push_bind(expected_row_version);
    }
    query
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
        _ => {}
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
    let edited_fields = changes.iter().filter_map(|(field, _)| match field {
        ImportPreviewPatchField::CategoryId => Some("category_id"),
        ImportPreviewPatchField::SourceAccountId => Some("source_account_id"),
        ImportPreviewPatchField::DestinationAccountId => Some("destination_account_id"),
        _ => None,
    });
    mark_manual_identity_ownership(preview, payload, edited_fields);
    Ok(())
}

fn mark_manual_identity_ownership<'a>(
    preview: &mut ImportPreviewRow,
    payload: &mut Value,
    edited_fields: impl Iterator<Item = &'a str>,
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
    for field in &edited_fields {
        manual_fields
            .as_object_mut()
            .expect("manual fields object")
            .insert((*field).to_string(), json!(true));
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
        for field in edited_fields {
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

fn set_feedback_review_status(mut feedback: Value, key: &str, status: &str) -> Value {
    if !feedback.is_object() {
        feedback = json!({});
    }
    let object = feedback.as_object_mut().expect("feedback object");
    let mut child = object.get(key).cloned().unwrap_or_else(|| json!({}));
    if !child.is_object() {
        child = json!({});
    }
    child
        .as_object_mut()
        .expect("child object")
        .insert("review_status".to_string(), json!(status));
    object.insert(key.to_string(), child);
    feedback
}

fn clear_feedback_key(feedback: &mut Value, key: &str) {
    if let Some(object) = feedback.as_object_mut() {
        object.remove(key);
    }
}

fn payload_set(payload: &mut Value, key: &str, value: Value) {
    if !payload.is_object() {
        *payload = json!({});
    }
    payload
        .as_object_mut()
        .expect("payload object")
        .insert(key.to_string(), value);
}

fn payload_text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(value_string)
}

fn payload_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

fn payload_f64(payload: &Value, key: &str) -> Option<f64> {
    payload.get(key).and_then(Value::as_f64)
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn payload_array_strings(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(value_string).collect())
        .unwrap_or_default()
}

fn value_string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

fn parse_json_array_strings(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn format_pg_time(value: DateTime<Utc>) -> String {
    value.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn truncate_text_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_string()
}

fn is_transfer_type(bill_type: &str) -> bool {
    matches!(
        bill_type.trim().to_ascii_lowercase().as_str(),
        "转账" | "transfer" | "4"
    )
}

fn is_investment_type(bill_type: &str) -> bool {
    matches!(
        bill_type.trim().to_ascii_lowercase().as_str(),
        "投资" | "investment" | "5"
    )
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn first_non_empty(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    values
        .into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}
