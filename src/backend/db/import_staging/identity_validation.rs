async fn load_import_identity_maps(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut maps = ImportIdentityMaps::default();
    for row in account_rows {
        maps.active_accounts.insert(row.try_get("id")?);
    }
    for row in category_rows {
        let id = row.try_get::<i64, _>("id")?;
        let category_type = row
            .try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .and_then(preview_category_type_code);
        maps.active_categories.insert(id, category_type);
    }
    Ok(maps)
}

fn apply_identity_validation_to_draft(
    draft: &mut ImportPreviewDraft,
    identity_maps: &ImportIdentityMaps,
) {
    let issues = normalize_identity_values(
        &draft.preview_type,
        &mut draft.category_id,
        &mut draft.preview_source_account_id,
        &mut draft.preview_destination_account_id,
        identity_maps,
    );
    set_identity_validation_feedback(&mut draft.preview_matching_feedback, &issues);
    if !issues.is_empty() {
        draft.preview_selected = false;
    }
}

fn apply_identity_validation_to_preview(
    preview: &mut ImportPreviewRow,
    payload: &mut Value,
    identity_maps: &ImportIdentityMaps,
) {
    let issues = normalize_identity_values(
        &preview.preview_type,
        &mut preview.category_id,
        &mut preview.preview_source_account_id,
        &mut preview.preview_destination_account_id,
        identity_maps,
    );
    payload_set(
        payload,
        "category_id",
        preview.category_id.map_or(Value::Null, Value::from),
    );
    payload_set(
        payload,
        "categoryId",
        preview.category_id.map_or(Value::Null, Value::from),
    );
    payload_set(
        payload,
        "preview_source_account_id",
        preview
            .preview_source_account_id
            .map_or(Value::Null, Value::from),
    );
    payload_set(
        payload,
        "preview_destination_account_id",
        preview
            .preview_destination_account_id
            .map_or(Value::Null, Value::from),
    );
    set_identity_validation_feedback(&mut preview.preview_matching_feedback, &issues);
    if !issues.is_empty() {
        preview.preview_selected = false;
        payload_set(payload, "preview_selected", Value::Bool(false));
    }
}

fn normalize_identity_values(
    preview_type: &str,
    category_id: &mut Option<i64>,
    source_account_id: &mut Option<i64>,
    destination_account_id: &mut Option<i64>,
    identity_maps: &ImportIdentityMaps,
) -> Vec<Value> {
    let mut issues = Vec::new();
    if preview_category_required(preview_type) {
        match *category_id {
            Some(id) if id > 0 => match identity_maps.active_categories.get(&id) {
                Some(category_type)
                    if category_type_matches_preview_type(*category_type, preview_type) => {}
                Some(_) => {
                    issues.push(identity_issue("category_id", "type_mismatch", Some(id)));
                    *category_id = None;
                }
                None => {
                    issues.push(identity_issue(
                        "category_id",
                        "not_active_or_not_found",
                        Some(id),
                    ));
                    *category_id = None;
                }
            },
            Some(id) => {
                issues.push(identity_issue("category_id", "non_positive", Some(id)));
                *category_id = None;
            }
            None => issues.push(identity_issue("category_id", "missing", None)),
        }
    } else if category_id.is_some_and(|id| id <= 0) {
        issues.push(identity_issue("category_id", "non_positive", *category_id));
        *category_id = None;
    }

    match *source_account_id {
        Some(id) if id > 0 && identity_maps.active_accounts.contains(&id) => {}
        Some(id) if id > 0 => {
            issues.push(identity_issue(
                "source_account_id",
                "not_active_or_not_found",
                Some(id),
            ));
            *source_account_id = None;
        }
        Some(id) => {
            issues.push(identity_issue(
                "source_account_id",
                "non_positive",
                Some(id),
            ));
            *source_account_id = None;
        }
        None => issues.push(identity_issue("source_account_id", "missing", None)),
    }

    if preview_destination_account_required(preview_type) {
        match *destination_account_id {
            Some(id) if id > 0 && identity_maps.active_accounts.contains(&id) => {}
            Some(id) if id > 0 => {
                issues.push(identity_issue(
                    "destination_account_id",
                    "not_active_or_not_found",
                    Some(id),
                ));
                *destination_account_id = None;
            }
            Some(id) => {
                issues.push(identity_issue(
                    "destination_account_id",
                    "non_positive",
                    Some(id),
                ));
                *destination_account_id = None;
            }
            None => issues.push(identity_issue("destination_account_id", "missing", None)),
        }
    } else if destination_account_id.is_some() {
        let reason = if destination_account_id.is_some_and(|id| id <= 0) {
            "non_positive"
        } else {
            "not_allowed_for_type"
        };
        issues.push(identity_issue(
            "destination_account_id",
            reason,
            *destination_account_id,
        ));
        *destination_account_id = None;
    }

    if preview_destination_account_required(preview_type)
        && source_account_id.is_some()
        && destination_account_id.is_some()
        && source_account_id == destination_account_id
    {
        issues.push(identity_issue(
            "destination_account_id",
            "same_as_source_account",
            *destination_account_id,
        ));
        *destination_account_id = None;
    }

    issues
}

fn identity_issue(field: &str, reason: &str, value: Option<i64>) -> Value {
    json!({
        "field": field,
        "reason": reason,
        "value": value,
    })
}

fn set_identity_validation_feedback(feedback: &mut Value, issues: &[Value]) {
    if !feedback.is_object() {
        *feedback = json!({});
    }
    let object = feedback.as_object_mut().expect("feedback object");
    if issues.is_empty() {
        object.remove("identity_validation");
        return;
    }
    object.insert(
        "identity_validation".to_string(),
        json!({
            "review_status": "requires_identity_review",
            "issues": issues,
        }),
    );
}

fn category_type_matches_preview_type(category_type: Option<i64>, preview_type: &str) -> bool {
    let Some(category_type) = category_type else {
        return true;
    };
    if matches!(category_type, 0 | 1) {
        return true;
    }
    preview_category_type_code(preview_type)
        .is_none_or(|preview_type| preview_type == category_type)
}

fn preview_identity_error_messages(
    preview: &ImportPreviewRow,
    identity_maps: &ImportIdentityMaps,
) -> Vec<String> {
    let mut category_id = preview.category_id;
    let mut source_account_id = preview.preview_source_account_id;
    let mut destination_account_id = preview.preview_destination_account_id;
    normalize_identity_values(
        &preview.preview_type,
        &mut category_id,
        &mut source_account_id,
        &mut destination_account_id,
        identity_maps,
    )
    .into_iter()
    .map(|issue| {
        let field = issue
            .get("field")
            .and_then(Value::as_str)
            .unwrap_or("identity");
        let reason = issue
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("invalid");
        let value = issue.get("value").cloned().unwrap_or(Value::Null);
        format!("preview {} {field} {reason} value={value}", preview.id)
    })
    .collect()
}
