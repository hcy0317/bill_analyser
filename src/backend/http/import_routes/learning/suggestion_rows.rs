// 中文导读：长期学习 suggestion 行投影。
// 维护重点：旧 SQLite 表可能缺少 lifecycle 新列，读取时必须容错并给前端稳定默认值。
// 不变式：列表、accept/reject 和 batch accept 共享同一行解析，不重复猜测字段形状。

#[derive(Debug, Clone)]
struct LearningSuggestionRow {
    id: i64,
    user_id: i64,
    match_type: String,
    match_value: String,
    normalized_match_value: String,
    composite_match_hash: Option<String>,
    match_features_json: Option<String>,
    suggested_type: Option<String>,
    suggested_category_id: Option<i64>,
    suggested_source_account_id: Option<i64>,
    suggested_destination_account_id: Option<i64>,
    sample_count: i64,
    source_session_ids_json: Option<String>,
    source_preview_ids_json: Option<String>,
    status: String,
    recommendation_key: Option<String>,
    signal_state: Option<String>,
    existing_rule_id: Option<i64>,
    summary: Option<String>,
    created_at: String,
    updated_at: String,
}

fn learning_suggestion_row_to_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let row = learning_suggestion_row(row)?;
    Ok(json!({
        "id": row.id,
        "user_id": row.user_id,
        "match_type": row.match_type,
        "match_value": row.match_value,
        "normalized_match_value": row.normalized_match_value,
        "composite_match_hash": row.composite_match_hash,
        "match_features_json": row.match_features_json.unwrap_or_default(),
        "suggested_type": row.suggested_type,
        "suggested_category_id": row.suggested_category_id,
        "suggested_source_account_id": row.suggested_source_account_id,
        "suggested_destination_account_id": row.suggested_destination_account_id,
        "sample_count": row.sample_count,
        "source_session_ids_json": row.source_session_ids_json.unwrap_or_default(),
        "source_preview_ids_json": row.source_preview_ids_json.unwrap_or_default(),
        "status": row.status,
        "recommendation_key": row.recommendation_key.unwrap_or_default(),
        "signal_state": row.signal_state.unwrap_or_else(|| "yellow".to_string()),
        "existing_rule_id": row.existing_rule_id,
        "summary": row.summary.unwrap_or_default(),
        "created_at": row.created_at,
        "updated_at": row.updated_at,
    }))
}

fn learning_suggestion_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LearningSuggestionRow> {
    Ok(LearningSuggestionRow {
        id: row.get("id")?,
        user_id: row.get("user_id")?,
        match_type: row.get("match_type")?,
        match_value: row.get("match_value")?,
        normalized_match_value: row.get("normalized_match_value")?,
        composite_match_hash: row.get("composite_match_hash")?,
        match_features_json: row.get("match_features_json")?,
        suggested_type: row.get("suggested_type")?,
        suggested_category_id: row.get("suggested_category_id")?,
        suggested_source_account_id: row.get("suggested_source_account_id")?,
        suggested_destination_account_id: row.get("suggested_destination_account_id")?,
        sample_count: row.get("sample_count")?,
        source_session_ids_json: row.get("source_session_ids_json")?,
        source_preview_ids_json: row.get("source_preview_ids_json")?,
        status: row.get("status")?,
        recommendation_key: row.get("recommendation_key").ok().flatten(),
        signal_state: row.get("signal_state").ok().flatten(),
        existing_rule_id: row.get("existing_rule_id")?,
        summary: row.get("summary")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn count_learning_suggestions(
    connection: &Connection,
    user_id: UserId,
    status: Option<&str>,
) -> Result<i64, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    if let Some(status) = status {
        connection
            .query_row(
                "SELECT COUNT(*) FROM import_learning_suggestions WHERE user_id = ?1 AND status = ?2",
                params![user_id, status],
                |row| row.get(0),
            )
            .map_err(db_error_response)
    } else {
        connection
            .query_row(
                "SELECT COUNT(*) FROM import_learning_suggestions WHERE user_id = ?1",
                params![user_id],
                |row| row.get(0),
            )
            .map_err(db_error_response)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_learning_suggestions(
    connection: &Connection,
    user_id: UserId,
    status: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    if let Some(status) = status {
        let mut statement = connection
            .prepare(
                "
            SELECT * FROM import_learning_suggestions
            WHERE user_id = ?1 AND status = ?2
            ORDER BY sample_count DESC, updated_at DESC, id DESC
            LIMIT ?3 OFFSET ?4
            ",
            )
            .map_err(db_error_response)?;
        let rows = statement
            .query_map(
                params![user_id, status, usize_to_i64(limit), usize_to_i64(offset)],
                learning_suggestion_row_to_value,
            )
            .map_err(db_error_response)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_error_response)
    } else {
        let mut statement = connection
            .prepare(
                "
            SELECT * FROM import_learning_suggestions
            WHERE user_id = ?1
            ORDER BY sample_count DESC, updated_at DESC, id DESC
            LIMIT ?2 OFFSET ?3
            ",
            )
            .map_err(db_error_response)?;
        let rows = statement
            .query_map(
                params![user_id, usize_to_i64(limit), usize_to_i64(offset)],
                learning_suggestion_row_to_value,
            )
            .map_err(db_error_response)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(db_error_response)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_learning_suggestion(
    connection: &Connection,
    suggestion_id: i64,
    user_id: UserId,
) -> Result<Option<LearningSuggestionRow>, ImportV2RouteResponse> {
    connection
        .query_row(
            "
            SELECT * FROM import_learning_suggestions
            WHERE id = ?1 AND user_id = ?2
            LIMIT 1
            ",
            params![suggestion_id, user_id_i64_value(user_id)?],
            learning_suggestion_row,
        )
        .optional()
        .map_err(db_error_response)
}
