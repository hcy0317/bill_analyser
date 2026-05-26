// 中文导读：导入学习推荐生命周期持久化。
// 维护重点：Postgres 是最终权威；SQLite 运行态先提供同名 lifecycle/event/suppression 语义，便于当前导入链路可运行。
// 不变式：recommendation_key 必须 user-scoped 且稳定；反馈事件必须和生命周期计数在同一事务内更新。

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_learning_lifecycle_view(
    connection: &Connection,
    user_id: i64,
    recommendation_key: &str,
    recommendation_type: &str,
) -> DbResult<ImportLearningLifecycleView> {
    if !import_learning_table_exists(connection, "import_learning_lifecycle")? {
        return Ok(default_import_learning_lifecycle_view(
            recommendation_key,
            recommendation_type,
        ));
    }
    let row = connection
        .query_row(
            "
            SELECT status, accepted_count, rejected_count, auto_applied_count
            FROM import_learning_lifecycle
            WHERE user_id = ?1 AND recommendation_key = ?2
            LIMIT 1
            ",
            params![user_id, recommendation_key],
            |row| {
                Ok(ImportLearningLifecycleState {
                    status: row
                        .get::<_, Option<String>>("status")?
                        .unwrap_or_else(|| "yellow".to_string()),
                    accepted_count: row.get::<_, Option<i64>>("accepted_count")?.unwrap_or(0),
                    rejected_count: row.get::<_, Option<i64>>("rejected_count")?.unwrap_or(0),
                    auto_applied_count: row
                        .get::<_, Option<i64>>("auto_applied_count")?
                        .unwrap_or(0),
                })
            },
        )
        .optional()?;
    let state = row.unwrap_or_default();
    Ok(import_learning_lifecycle_view_from_state(
        recommendation_key,
        recommendation_type,
        state,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn record_import_learning_lifecycle_feedback(
    connection: &mut Connection,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<ImportLearningLifecycleView> {
    run_transaction(connection, |tx| {
        record_import_learning_lifecycle_feedback_on_tx(tx, user_id, input)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn record_import_learning_lifecycle_feedback_on_tx(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<ImportLearningLifecycleView> {
    if input.recommendation_key.trim().is_empty()
        || !import_staging_table_exists_tx(tx, "import_learning_lifecycle")?
    {
        return Ok(default_import_learning_lifecycle_view(
            &input.recommendation_key,
            &input.recommendation_type,
        ));
    }

    let current = tx
        .query_row(
            "
            SELECT status, accepted_count, rejected_count, auto_applied_count
            FROM import_learning_lifecycle
            WHERE user_id = ?1 AND recommendation_key = ?2
            LIMIT 1
            ",
            params![user_id, &input.recommendation_key],
            |row| {
                Ok(ImportLearningLifecycleState {
                    status: row
                        .get::<_, Option<String>>("status")?
                        .unwrap_or_else(|| "yellow".to_string()),
                    accepted_count: row.get::<_, Option<i64>>("accepted_count")?.unwrap_or(0),
                    rejected_count: row.get::<_, Option<i64>>("rejected_count")?.unwrap_or(0),
                    auto_applied_count: row
                        .get::<_, Option<i64>>("auto_applied_count")?
                        .unwrap_or(0),
                })
            },
        )
        .optional()?
        .unwrap_or_default();
    let transition = transition_import_learning_lifecycle(&current, &input.feedback);
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "
        INSERT INTO import_learning_lifecycle (
            user_id, recommendation_key, recommendation_type, status,
            accepted_count, rejected_count, auto_applied_count,
            auto_apply_enabled, last_feedback_at, metadata_json, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?9)
        ON CONFLICT(user_id, recommendation_key) DO UPDATE SET
            recommendation_type = excluded.recommendation_type,
            status = excluded.status,
            accepted_count = excluded.accepted_count,
            rejected_count = excluded.rejected_count,
            auto_applied_count = excluded.auto_applied_count,
            auto_apply_enabled = excluded.auto_apply_enabled,
            last_feedback_at = excluded.last_feedback_at,
            metadata_json = excluded.metadata_json,
            updated_at = excluded.updated_at
        ",
        params![
            user_id,
            &input.recommendation_key,
            &input.recommendation_type,
            &transition.next_status,
            transition.accepted_count,
            transition.rejected_count,
            transition.auto_applied_count,
            i64::from(transition.auto_apply_enabled),
            &now,
            learning_lifecycle_metadata_json(&transition),
        ],
    )?;
    let lifecycle_id = tx.query_row(
        "
        SELECT id FROM import_learning_lifecycle
        WHERE user_id = ?1 AND recommendation_key = ?2
        LIMIT 1
        ",
        params![user_id, &input.recommendation_key],
        |row| row.get::<_, i64>(0),
    )?;
    insert_learning_feedback_event(tx, user_id, input, &transition, lifecycle_id, &now)?;
    if transition.suppressed {
        upsert_learning_suppression(tx, user_id, input, &now)?;
    }
    Ok(ImportLearningLifecycleView {
        recommendation_key: input.recommendation_key.clone(),
        recommendation_type: input.recommendation_type.clone(),
        status: transition.next_status,
        signal_state: transition.signal_state,
        accepted_count: transition.accepted_count,
        rejected_count: transition.rejected_count,
        auto_applied_count: transition.auto_applied_count,
        auto_apply_enabled: transition.auto_apply_enabled,
        suppressed: transition.suppressed,
    })
}

fn insert_learning_feedback_event(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
    transition: &bill_analyser_core::ImportLearningLifecycleTransition,
    lifecycle_id: i64,
    now: &str,
) -> DbResult<()> {
    if !import_staging_table_exists_tx(tx, "import_learning_feedback_events")? {
        return Ok(());
    }
    let mut columns = vec![
        "user_id".to_string(),
        "event_type".to_string(),
        "rule_id".to_string(),
        "suggestion_id".to_string(),
        "session_id".to_string(),
        "preview_id".to_string(),
        "bill_id".to_string(),
        "candidate_id".to_string(),
        "payload_json".to_string(),
        "created_at".to_string(),
    ];
    let mut values = vec![
        SqlValue::Integer(user_id),
        SqlValue::Text(transition.event_type.clone()),
        optional_i64_sql_value(input.rule_id),
        optional_i64_sql_value(input.suggestion_id),
        optional_text_sql_value(input.session_id.as_deref()),
        optional_i64_sql_value(input.preview_id),
        optional_i64_sql_value(input.bill_id),
        optional_text_sql_value(input.candidate_id.as_deref()),
        optional_text_sql_value(input.payload_json.as_deref()),
        SqlValue::Text(now.to_string()),
    ];
    for (column, value) in [
        (
            "recommendation_key",
            SqlValue::Text(input.recommendation_key.clone()),
        ),
        ("lifecycle_id", SqlValue::Integer(lifecycle_id)),
        (
            "previous_signal_state",
            SqlValue::Text(learning_lifecycle_signal_state(&transition.previous_status).to_string()),
        ),
        (
            "next_signal_state",
            SqlValue::Text(transition.signal_state.clone()),
        ),
    ] {
        if import_learning_column_exists_tx(tx, "import_learning_feedback_events", column)? {
            columns.push(column.to_string());
            values.push(value);
        }
    }
    let placeholders = std::iter::repeat_n("?", columns.len())
        .collect::<Vec<_>>()
        .join(", ");
    tx.execute(
        &format!(
            "INSERT INTO import_learning_feedback_events ({}) VALUES ({})",
            columns.join(", "),
            placeholders
        ),
        params_from_iter(values),
    )?;
    Ok(())
}

fn upsert_learning_suppression(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
    now: &str,
) -> DbResult<()> {
    if !import_staging_table_exists_tx(tx, "import_learning_suppressions")? {
        return Ok(());
    }
    tx.execute(
        "
        INSERT INTO import_learning_suppressions (
            user_id, recommendation_key, suppression_reason, metadata_json, created_at, updated_at
        ) VALUES (?1, ?2, 'yellow_reject_threshold', ?3, ?4, ?4)
        ON CONFLICT(user_id, recommendation_key) DO UPDATE SET
            suppression_reason = excluded.suppression_reason,
            metadata_json = excluded.metadata_json,
            updated_at = excluded.updated_at
        ",
        params![
            user_id,
            &input.recommendation_key,
            input.payload_json.as_deref().unwrap_or("{}"),
            now,
        ],
    )?;
    Ok(())
}

fn import_learning_lifecycle_view_from_state(
    recommendation_key: &str,
    recommendation_type: &str,
    state: ImportLearningLifecycleState,
) -> ImportLearningLifecycleView {
    let status = bill_analyser_core::normalize_learning_lifecycle_status(&state.status);
    ImportLearningLifecycleView {
        recommendation_key: recommendation_key.to_string(),
        recommendation_type: recommendation_type.to_string(),
        signal_state: learning_lifecycle_signal_state(&status).to_string(),
        auto_apply_enabled: learning_lifecycle_is_auto_eligible(&status),
        suppressed: status == "suppressed",
        status,
        accepted_count: state.accepted_count.max(0),
        rejected_count: state.rejected_count.max(0),
        auto_applied_count: state.auto_applied_count.max(0),
    }
}

fn default_import_learning_lifecycle_view(
    recommendation_key: &str,
    recommendation_type: &str,
) -> ImportLearningLifecycleView {
    import_learning_lifecycle_view_from_state(
        recommendation_key,
        recommendation_type,
        ImportLearningLifecycleState::default(),
    )
}

fn learning_lifecycle_metadata_json(
    transition: &bill_analyser_core::ImportLearningLifecycleTransition,
) -> String {
    serde_json::json!({
        "previous_status": transition.previous_status,
        "next_status": transition.next_status,
        "signal_state": transition.signal_state,
        "event_type": transition.event_type,
    })
    .to_string()
}

fn optional_i64_sql_value(value: Option<i64>) -> SqlValue {
    value.map(SqlValue::Integer).unwrap_or(SqlValue::Null)
}

fn optional_text_sql_value(value: Option<&str>) -> SqlValue {
    value
        .map(|value| SqlValue::Text(value.to_string()))
        .unwrap_or(SqlValue::Null)
}

fn import_learning_table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    Ok(connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn import_learning_column_exists_tx(
    tx: &rusqlite::Transaction<'_>,
    table_name: &str,
    column_name: &str,
) -> DbResult<bool> {
    let mut statement = tx.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn learning_recommendation_key_from_feedback(
    user_id: i64,
    preview: &ImportPreviewRow,
    learning_feedback: &Value,
    applied_result: Option<&ImportPreviewLearningApply>,
) -> String {
    for key in ["recommendation_key", "recommendationKey"] {
        if let Some(value) = learning_feedback
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return value.to_string();
        }
    }
    let applied_preview = learning_feedback
        .get("applied_preview")
        .or_else(|| learning_feedback.get("appliedPreview"));
    let recommended_type = applied_result
        .and_then(|result| result.preview_type.clone())
        .or_else(|| learning_feedback.get("recommended_type").and_then(json_value_to_string))
        .or_else(|| {
            applied_preview
                .and_then(|value| value.get("preview_type"))
                .and_then(json_value_to_string)
        })
        .unwrap_or_else(|| preview.preview_type.clone());
    let category_id = learning_feedback
        .get("category_id")
        .and_then(value_to_positive_i64);
    let source_account_id = applied_result
        .and_then(|result| result.preview_source_account_id.flatten())
        .or_else(|| {
            applied_preview
                .and_then(|value| value.get("preview_source_account_id"))
                .and_then(value_to_positive_i64)
        })
        .or(preview.preview_source_account_id);
    let destination_account_id = applied_result
        .and_then(|result| result.preview_destination_account_id.flatten())
        .or_else(|| {
            applied_preview
                .and_then(|value| value.get("preview_destination_account_id"))
                .and_then(value_to_positive_i64)
        })
        .or(preview.preview_destination_account_id);
    build_import_learning_recommendation_key(&ImportLearningRecommendationKeyInput {
        user_id,
        recommended_type,
        recommended_category_id: category_id,
        recommended_source_account_id: source_account_id,
        recommended_destination_account_id: destination_account_id,
        transaction_type_scope: preview.preview_type.clone(),
        parser_bucket: preview.preview_parser_id.clone(),
        counterparty_bucket: preview.preview_counterparty.clone(),
        payment_bucket: preview.preview_payment_method.clone(),
        description_bucket: preview.preview_description.clone(),
        amount_bucket: Some(amount_bucket(Some(&serde_json::json!(preview.preview_amount))).to_string()),
        transfer_protected: preview
            .dedup_type
            .trim()
            .to_ascii_lowercase()
            .contains("transfer")
            || preview.preview_matching_feedback.get("transfer").is_some(),
        ..ImportLearningRecommendationKeyInput::default()
    })
}

fn json_value_to_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_f64().map(|number| number.to_string()))
}
