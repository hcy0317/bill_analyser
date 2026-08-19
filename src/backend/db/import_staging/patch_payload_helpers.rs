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
    let preview = preview_from_pg_row(&row)?;
    let payload = row
        .try_get::<Value, _>("preview_payload")
        .unwrap_or_else(|_| json!({}));
    let projection = project_preview_patch(preview, payload, patch, identity_maps)?;
    let mut query = build_preview_row_update_query(
        &projection.preview,
        projection.payload.to_string(),
        projection.signal_projection,
        projection.amount_cents,
        projection.direction,
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
                projection.preview.version,
            ));
        }
    } else {
        observe_import_preview_signal_projection_parity(
            tx,
            &[patch.preview_id],
            "preview_patch",
        )
        .await?;
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
    signal_projection: ImportPreviewSignalProjection,
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
    query.push("::jsonb");
    push_import_preview_signal_projection_assignments(&mut query, signal_projection);
    query.push(
        r#",
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
