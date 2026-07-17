async fn load_import_identity_maps_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(&mut **transaction)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
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

async fn load_import_learning_lifecycle_transition_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<bill_analyser_core::ImportLearningLifecycleTransition> {
    let feedback = bill_analyser_core::import_learning_lifecycle::normalize_import_learning_lifecycle_feedback(
        &input.feedback,
    )
    .ok_or_else(|| DbError::InvalidOperation("Invalid learning lifecycle feedback".to_string()))?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, $2))")
        .bind(&input.recommendation_key)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
    let row = sqlx::query(
        r#"
        SELECT status, accepted_count, rejected_count, auto_applied_count
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND recommendation_key = $2
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(&input.recommendation_key)
    .fetch_optional(&mut **transaction)
    .await?;
    let state = match row {
        Some(row) => ImportLearningLifecycleState {
            status: row.try_get("status")?,
            accepted_count: row.try_get::<i32, _>("accepted_count")? as i64,
            rejected_count: row.try_get::<i32, _>("rejected_count")? as i64,
            auto_applied_count: row.try_get::<i32, _>("auto_applied_count")? as i64,
        },
        None => ImportLearningLifecycleState::default(),
    };
    Ok(transition_import_learning_lifecycle(&state, feedback))
}

async fn persist_import_learning_lifecycle_transition_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    input: &ImportLearningLifecycleRecordInput,
    next: &bill_analyser_core::ImportLearningLifecycleTransition,
) -> DbResult<()> {
    sqlx::query(
        r#"
        INSERT INTO import_learning_lifecycle (
            user_id, recommendation_key, recommendation_type, status,
            accepted_count, rejected_count, auto_applied_count, auto_apply_enabled,
            metadata, last_feedback_at, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, now(), now(), now())
        ON CONFLICT (user_id, recommendation_key) DO UPDATE SET
            recommendation_type = excluded.recommendation_type,
            status = excluded.status,
            accepted_count = excluded.accepted_count,
            rejected_count = excluded.rejected_count,
            auto_applied_count = excluded.auto_applied_count,
            auto_apply_enabled = excluded.auto_apply_enabled,
            last_feedback_at = now(),
            updated_at = now(),
            version = import_learning_lifecycle.version + 1
        "#,
    )
    .bind(user_id)
    .bind(&input.recommendation_key)
    .bind(&input.recommendation_type)
    .bind(&next.next_status)
    .bind(next.accepted_count)
    .bind(next.rejected_count)
    .bind(next.auto_applied_count)
    .bind(next.auto_apply_enabled)
    .execute(&mut **transaction)
    .await?;
    let lifecycle_id = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT id FROM import_learning_lifecycle WHERE user_id = $1 AND recommendation_key = $2",
    )
    .bind(user_id)
    .bind(&input.recommendation_key)
    .fetch_one(&mut **transaction)
    .await?
    .unwrap_or_default();
    sqlx::query(
        r#"
        INSERT INTO import_learning_feedback_events (
            user_id, rule_id, suggestion_id, lifecycle_id, recommendation_key,
            event_type, session_id, preview_id, bill_id, candidate_id,
            previous_signal_state, next_signal_state, payload, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::jsonb,now())
        "#,
    )
    .bind(user_id)
    .bind(input.rule_id)
    .bind(input.suggestion_id)
    .bind(lifecycle_id)
    .bind(&input.recommendation_key)
    .bind(&next.event_type)
    .bind(&input.session_id)
    .bind(input.preview_id)
    .bind(input.bill_id)
    .bind(&input.candidate_id)
    .bind(&next.previous_status)
    .bind(&next.signal_state)
    .bind(input.payload_json.as_deref().unwrap_or("{}"))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
