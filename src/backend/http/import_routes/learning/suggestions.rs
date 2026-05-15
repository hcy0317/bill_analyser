#[derive(Debug, Clone, PartialEq, Eq)]
struct LearningSuggestionSignature {
    suggested_type: Option<String>,
    suggested_category_id: Option<i64>,
    suggested_source_account_id: Option<i64>,
    suggested_destination_account_id: Option<i64>,
}

#[derive(Debug, Clone)]
struct LearningSuggestionCandidate {
    match_features: BTreeMap<String, String>,
    signature: LearningSuggestionSignature,
    sample_count: i64,
    source_session_ids: BTreeSet<String>,
    source_preview_ids: BTreeSet<i64>,
    existing_rule_id: Option<i64>,
}

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
    existing_rule_id: Option<i64>,
    summary: Option<String>,
    created_at: String,
    updated_at: String,
}

enum LearningSuggestionDecision {
    Accepted(Value),
    Conflict(Value),
    NotFound,
}

fn mine_learning_suggestions(
    connection: &mut Connection,
    user_id: UserId,
) -> Result<Value, ImportV2RouteResponse> {
    let user_id_i64 = user_id_i64_value(user_id)?;
    let annotations = load_learning_corpus_annotations(connection, user_id_i64)?;
    if annotations.is_empty() {
        return Ok(empty_learning_suggestion_mining_result());
    }
    let existing_rule_hashes = existing_composite_rule_hashes(connection, user_id_i64)?;
    let (candidates, conflicted_hashes) =
        collect_learning_suggestion_candidates(annotations, &existing_rule_hashes);

    let now = now_text();
    let mut created = 0;
    let mut updated = 0;
    let mut skipped_existing = 0;
    for (composite_hash, candidate) in candidates {
        if candidate.existing_rule_id.is_some() {
            skipped_existing += 1;
            continue;
        }
        let existed =
            learning_suggestion_id_by_key(connection, user_id_i64, "composite", &composite_hash)?
                .is_some();
        let session_ids = candidate.source_session_ids.into_iter().collect::<Vec<_>>();
        let preview_ids = candidate.source_preview_ids.into_iter().collect::<Vec<_>>();
        let match_features_json =
            serde_json::to_string(&candidate.match_features).map_err(|error| {
                import_v2_error_response(
                    500,
                    &format!("Unable to serialize learning features: {error}"),
                )
            })?;
        let session_ids_json = serde_json::to_string(&session_ids).map_err(|error| {
            import_v2_error_response(
                500,
                &format!("Unable to serialize source sessions: {error}"),
            )
        })?;
        let preview_ids_json = serde_json::to_string(&preview_ids).map_err(|error| {
            import_v2_error_response(
                500,
                &format!("Unable to serialize source previews: {error}"),
            )
        })?;
        connection
            .execute(
                "
                INSERT INTO import_learning_suggestions (
                    user_id, match_type, match_value, normalized_match_value,
                    composite_match_hash, match_features_json, suggested_type,
                    suggested_category_id, suggested_source_account_id,
                    suggested_destination_account_id, sample_count,
                    source_session_ids_json, source_preview_ids_json, status,
                    existing_rule_id, summary, created_at, updated_at
                ) VALUES (?1, 'composite', ?2, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    'pending', NULL, ?11, ?12, ?12)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    suggested_type = excluded.suggested_type,
                    suggested_category_id = excluded.suggested_category_id,
                    suggested_source_account_id = excluded.suggested_source_account_id,
                    suggested_destination_account_id = excluded.suggested_destination_account_id,
                    sample_count = excluded.sample_count,
                    source_session_ids_json = excluded.source_session_ids_json,
                    source_preview_ids_json = excluded.source_preview_ids_json,
                    summary = excluded.summary,
                    updated_at = excluded.updated_at
                ",
                params![
                    user_id_i64,
                    composite_hash,
                    match_features_json,
                    candidate.signature.suggested_type,
                    candidate.signature.suggested_category_id,
                    candidate.signature.suggested_source_account_id,
                    candidate.signature.suggested_destination_account_id,
                    candidate.sample_count,
                    session_ids_json,
                    preview_ids_json,
                    summarize_learning_suggestion(&candidate.match_features),
                    now,
                ],
            )
            .map_err(db_error_response)?;
        if existed {
            updated += 1;
        } else {
            created += 1;
        }
    }

    Ok(json!({
        "total_annotations": annotations_total(connection, user_id_i64)?,
        "mined": created + updated + skipped_existing,
        "created": created,
        "updated": updated,
        "skipped_conflict": conflicted_hashes.len(),
        "skipped_existing": skipped_existing,
    }))
}

fn empty_learning_suggestion_mining_result() -> Value {
    json!({
        "total_annotations": 0,
        "mined": 0,
        "created": 0,
        "updated": 0,
        "skipped_conflict": 0,
        "skipped_existing": 0,
    })
}

fn annotations_total(connection: &Connection, user_id: i64) -> Result<i64, ImportV2RouteResponse> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM import_learning_corpus_samples WHERE user_id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .map_err(db_error_response)
}

fn load_learning_corpus_annotations(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<LearningSuggestionCandidateInput>, ImportV2RouteResponse> {
    let mut statement = connection
        .prepare(
            "
            SELECT *
            FROM import_learning_corpus_samples
            WHERE user_id = ?1
            ORDER BY session_id, updated_at ASC, id ASC
            ",
        )
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], learning_corpus_row_to_input)
        .map_err(db_error_response)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)
}

#[derive(Debug, Clone)]
struct LearningSuggestionCandidateInput {
    session_id: String,
    preview_id: i64,
    parser_id: String,
    counterparty: String,
    description: String,
    payment_method: String,
    composite_match_hash: String,
    match_features_json: String,
    signature: LearningSuggestionSignature,
}

fn learning_corpus_row_to_input(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<LearningSuggestionCandidateInput> {
    Ok(LearningSuggestionCandidateInput {
        session_id: row.get::<_, String>("session_id")?,
        preview_id: row.get::<_, i64>("preview_id")?,
        parser_id: row
            .get::<_, Option<String>>("parser_id")?
            .unwrap_or_default(),
        counterparty: row
            .get::<_, Option<String>>("counterparty")?
            .unwrap_or_default(),
        description: row
            .get::<_, Option<String>>("description")?
            .unwrap_or_default(),
        payment_method: row
            .get::<_, Option<String>>("payment_method")?
            .unwrap_or_default(),
        composite_match_hash: row
            .get::<_, Option<String>>("composite_match_hash")?
            .unwrap_or_default(),
        match_features_json: row
            .get::<_, Option<String>>("match_features_json")?
            .unwrap_or_default(),
        signature: LearningSuggestionSignature {
            suggested_type: row.get::<_, Option<String>>("annotated_type")?,
            suggested_category_id: row.get::<_, Option<i64>>("annotated_category_id")?,
            suggested_source_account_id: row
                .get::<_, Option<i64>>("annotated_source_account_id")?,
            suggested_destination_account_id: row
                .get::<_, Option<i64>>("annotated_destination_account_id")?,
        },
    })
}

fn existing_composite_rule_hashes(
    connection: &Connection,
    user_id: i64,
) -> Result<BTreeMap<String, i64>, ImportV2RouteResponse> {
    let mut statement = connection
        .prepare(
            "
            SELECT id, normalized_match_value
            FROM import_learning_rules
            WHERE user_id = ?1 AND match_type = 'composite'
            ",
        )
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], |row| {
            Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?))
        })
        .map_err(db_error_response)?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(db_error_response)
}

fn collect_learning_suggestion_candidates(
    annotations: Vec<LearningSuggestionCandidateInput>,
    existing_rule_hashes: &BTreeMap<String, i64>,
) -> (
    BTreeMap<String, LearningSuggestionCandidate>,
    BTreeSet<String>,
) {
    let mut candidates: BTreeMap<String, LearningSuggestionCandidate> = BTreeMap::new();
    let mut conflicted_hashes = BTreeSet::new();
    for annotation in annotations {
        let Some((composite_hash, match_features)) = resolve_suggestion_match(&annotation) else {
            continue;
        };
        if conflicted_hashes.contains(&composite_hash) {
            continue;
        }
        if let Some(existing) = candidates.get_mut(&composite_hash) {
            if existing.signature != annotation.signature {
                conflicted_hashes.insert(composite_hash.clone());
                candidates.remove(&composite_hash);
                continue;
            }
            existing.sample_count += 1;
            existing.source_session_ids.insert(annotation.session_id);
            existing.source_preview_ids.insert(annotation.preview_id);
            continue;
        }
        candidates.insert(
            composite_hash.clone(),
            LearningSuggestionCandidate {
                match_features,
                signature: annotation.signature,
                sample_count: 1,
                source_session_ids: BTreeSet::from([annotation.session_id]),
                source_preview_ids: BTreeSet::from([annotation.preview_id]),
                existing_rule_id: existing_rule_hashes.get(&composite_hash).copied(),
            },
        );
    }
    (candidates, conflicted_hashes)
}

fn resolve_suggestion_match(
    annotation: &LearningSuggestionCandidateInput,
) -> Option<(String, BTreeMap<String, String>)> {
    let parsed_features =
        serde_json::from_str::<BTreeMap<String, String>>(annotation.match_features_json.trim())
            .ok()
            .map(|features| {
                features
                    .into_iter()
                    .map(|(key, value)| (key, normalize_learning_match_value(&value)))
                    .filter(|(_, value)| !value.is_empty())
                    .collect::<BTreeMap<_, _>>()
            })
            .filter(|features| features.len() >= 2);
    let match_features = parsed_features.or_else(|| {
        build_composite_match_features(
            &annotation.parser_id,
            &annotation.counterparty,
            &annotation.description,
            &annotation.payment_method,
        )
    })?;
    let composite_hash = if annotation.composite_match_hash.trim().is_empty() {
        composite_hash_from_features(&match_features)
    } else {
        annotation.composite_match_hash.trim().to_string()
    };
    (!composite_hash.is_empty()).then_some((composite_hash, match_features))
}

fn summarize_learning_suggestion(match_features: &BTreeMap<String, String>) -> String {
    let parts = [
        ("counterparty", "交易方"),
        ("description", "描述"),
        ("payment_method", "支付方式"),
    ]
    .into_iter()
    .filter_map(|(key, label)| {
        match_features
            .get(key)
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!("{label}: {value}"))
    })
    .collect::<Vec<_>>();
    if parts.is_empty() {
        composite_hash_from_features(match_features)
    } else {
        parts.join(" | ")
    }
}

fn learning_suggestion_id_by_key(
    connection: &Connection,
    user_id: i64,
    match_type: &str,
    normalized_match_value: &str,
) -> Result<Option<i64>, ImportV2RouteResponse> {
    connection
        .query_row(
            "
            SELECT id FROM import_learning_suggestions
            WHERE user_id = ?1 AND match_type = ?2 AND normalized_match_value = ?3
            LIMIT 1
            ",
            params![user_id, match_type, normalized_match_value],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error_response)
}

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
        existing_rule_id: row.get("existing_rule_id")?,
        summary: row.get("summary")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

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

fn accept_learning_suggestion(
    connection: &mut Connection,
    suggestion_id: i64,
    user_id: UserId,
) -> Result<LearningSuggestionDecision, ImportV2RouteResponse> {
    let Some(suggestion) = get_learning_suggestion(connection, suggestion_id, user_id)? else {
        return Ok(LearningSuggestionDecision::NotFound);
    };
    if suggestion.status != "pending" {
        return Ok(LearningSuggestionDecision::Conflict(json!({
            "error": "suggestion_not_pending",
            "current_status": suggestion.status,
        })));
    }
    let user_id_i64 = user_id_i64_value(user_id)?;
    let now = now_text();
    let match_features = serde_json::from_str::<BTreeMap<String, String>>(
        suggestion.match_features_json.as_deref().unwrap_or("{}"),
    )
    .unwrap_or_default();
    connection
        .execute(
            "
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id, learned_source_account_id,
                learned_destination_account_id, enabled, parser_id,
                composite_match_hash, match_features_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?11, ?12, ?12)
            ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                learned_type = excluded.learned_type,
                learned_category_id = excluded.learned_category_id,
                learned_source_account_id = excluded.learned_source_account_id,
                learned_destination_account_id = excluded.learned_destination_account_id,
                enabled = 1,
                parser_id = excluded.parser_id,
                composite_match_hash = excluded.composite_match_hash,
                match_features_json = excluded.match_features_json,
                updated_at = excluded.updated_at
            ",
            params![
                user_id_i64,
                &suggestion.match_type,
                &suggestion.match_value,
                &suggestion.normalized_match_value,
                suggestion.suggested_type.as_deref(),
                suggestion.suggested_category_id,
                suggestion.suggested_source_account_id,
                suggestion.suggested_destination_account_id,
                match_features.get("parser_id").cloned().unwrap_or_default(),
                suggestion.composite_match_hash.as_deref(),
                suggestion.match_features_json.as_deref(),
                now,
            ],
        )
        .map_err(db_error_response)?;
    let rule_id = connection
        .query_row(
            "
            SELECT id FROM import_learning_rules
            WHERE user_id = ?1 AND match_type = ?2 AND normalized_match_value = ?3
            LIMIT 1
            ",
            params![
                user_id_i64,
                &suggestion.match_type,
                &suggestion.normalized_match_value
            ],
            |row| row.get::<_, i64>(0),
        )
        .map_err(db_error_response)?;
    connection
        .execute(
            "
            UPDATE import_learning_suggestions
            SET status = 'accepted', existing_rule_id = ?1, updated_at = ?2
            WHERE id = ?3 AND user_id = ?4
            ",
            params![rule_id, now, suggestion_id, user_id_i64],
        )
        .map_err(db_error_response)?;
    record_learning_rule_log(
        connection,
        rule_id,
        user_id_i64,
        "created_from_suggestion",
        Some(&suggestion.match_type),
        Some(&suggestion.match_value),
        Some(&suggestion.normalized_match_value),
        None,
        None,
        Some(json!({"suggestion_id": suggestion_id})),
    )?;
    record_learning_feedback_event(
        connection,
        user_id_i64,
        "suggestion_accept",
        Some(rule_id),
        Some(suggestion_id),
        Some(json!({"status": "accepted"})),
    )?;
    Ok(LearningSuggestionDecision::Accepted(json!({
        "suggestion_id": suggestion_id,
        "rule_id": rule_id,
        "status": "accepted",
    })))
}

fn reject_learning_suggestion(
    connection: &mut Connection,
    suggestion_id: i64,
    user_id: UserId,
) -> Result<bool, ImportV2RouteResponse> {
    let Some(suggestion) = get_learning_suggestion(connection, suggestion_id, user_id)? else {
        return Ok(false);
    };
    if suggestion.status != "pending" {
        return Ok(false);
    }
    let user_id_i64 = user_id_i64_value(user_id)?;
    let now = now_text();
    connection
        .execute(
            "
            UPDATE import_learning_suggestions
            SET status = 'rejected', updated_at = ?1
            WHERE id = ?2 AND user_id = ?3
            ",
            params![now, suggestion_id, user_id_i64],
        )
        .map_err(db_error_response)?;
    record_learning_feedback_event(
        connection,
        user_id_i64,
        "suggestion_reject",
        None,
        Some(suggestion_id),
        Some(json!({"status": "rejected"})),
    )?;
    Ok(true)
}

fn set_import_learning_rule_enabled(
    connection: &mut Connection,
    rule_id: i64,
    user_id: UserId,
    enabled: bool,
) -> Result<bool, ImportV2RouteResponse> {
    let Some(existing) = get_import_learning_rule(connection, rule_id, user_id)? else {
        return Ok(false);
    };
    let user_id_i64 = user_id_i64_value(user_id)?;
    connection
        .execute(
            "
            UPDATE import_learning_rules
            SET enabled = ?1, updated_at = ?2
            WHERE id = ?3 AND user_id = ?4
            ",
            params![enabled, now_text(), rule_id, user_id_i64],
        )
        .map_err(db_error_response)?;
    record_learning_rule_log(
        connection,
        rule_id,
        user_id_i64,
        if enabled { "enabled" } else { "disabled" },
        existing["matchType"].as_str(),
        existing["matchValue"].as_str(),
        existing["normalizedMatchValue"].as_str(),
        existing["sourceSessionId"].as_str(),
        existing["sourcePreviewId"].as_i64(),
        Some(json!({"enabled": enabled})),
    )?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
fn record_learning_rule_log(
    connection: &Connection,
    rule_id: i64,
    user_id: i64,
    action: &str,
    match_type: Option<&str>,
    match_value: Option<&str>,
    normalized_match_value: Option<&str>,
    session_id: Option<&str>,
    preview_id: Option<i64>,
    payload: Option<Value>,
) -> Result<(), ImportV2RouteResponse> {
    let payload_json = payload.map(|value| value.to_string());
    connection
        .execute(
            "
            INSERT INTO import_learning_rule_logs (
                user_id, rule_id, action, match_type, match_value,
                normalized_match_value, session_id, preview_id, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ",
            params![
                user_id,
                rule_id,
                action,
                match_type,
                match_value,
                normalized_match_value,
                session_id,
                preview_id,
                payload_json,
                now_text(),
            ],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

