// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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
        let recommendation_key = build_import_learning_recommendation_key(
            &ImportLearningRecommendationKeyInput {
                user_id: user_id_i64,
                recommendation_type: "learning_center".to_string(),
                recommended_type: candidate
                    .signature
                    .suggested_type
                    .clone()
                    .unwrap_or_default(),
                recommended_category_id: candidate.signature.suggested_category_id,
                recommended_source_account_id: candidate.signature.suggested_source_account_id,
                recommended_destination_account_id: candidate
                    .signature
                    .suggested_destination_account_id,
                transaction_type_scope: candidate
                    .signature
                    .suggested_type
                    .clone()
                    .unwrap_or_default(),
                parser_bucket: candidate
                    .match_features
                    .get("parser_id")
                    .cloned()
                    .unwrap_or_default(),
                counterparty_bucket: candidate
                    .match_features
                    .get("counterparty")
                    .cloned()
                    .unwrap_or_default(),
                payment_bucket: candidate
                    .match_features
                    .get("payment_method")
                    .cloned()
                    .unwrap_or_default(),
                description_bucket: candidate
                    .match_features
                    .get("description")
                    .cloned()
                    .unwrap_or_default(),
                suppression_scope: "learning_center".to_string(),
                ..ImportLearningRecommendationKeyInput::default()
            },
        );
        connection
            .execute(
                "
                INSERT INTO import_learning_suggestions (
                    user_id, match_type, match_value, normalized_match_value,
                    composite_match_hash, match_features_json, suggested_type,
                    suggested_category_id, suggested_source_account_id,
                    suggested_destination_account_id, sample_count,
                    source_session_ids_json, source_preview_ids_json, status,
                    recommendation_key, signal_state, existing_rule_id, summary, created_at, updated_at
                ) VALUES (?1, 'composite', ?2, ?2, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                    'pending', ?11, 'yellow', NULL, ?12, ?13, ?13)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    suggested_type = excluded.suggested_type,
                    suggested_category_id = excluded.suggested_category_id,
                    suggested_source_account_id = excluded.suggested_source_account_id,
                    suggested_destination_account_id = excluded.suggested_destination_account_id,
                    sample_count = excluded.sample_count,
                    source_session_ids_json = excluded.source_session_ids_json,
                    source_preview_ids_json = excluded.source_preview_ids_json,
                    recommendation_key = excluded.recommendation_key,
                    signal_state = excluded.signal_state,
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
                    recommendation_key,
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
    if let Some(recommendation_key) = suggestion
        .recommendation_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record_import_learning_lifecycle_feedback(
            connection,
            user_id_i64,
            &ImportLearningLifecycleRecordInput {
                recommendation_key: recommendation_key.to_string(),
                recommendation_type: "learning_center".to_string(),
                feedback: "accept".to_string(),
                rule_id: Some(rule_id),
                suggestion_id: Some(suggestion_id),
                session_id: None,
                preview_id: None,
                bill_id: None,
                candidate_id: None,
                payload_json: Some(json!({"status": "accepted"}).to_string()),
            },
        )
        .map_err(db_error_response)?;
    }
    Ok(LearningSuggestionDecision::Accepted(json!({
        "suggestion_id": suggestion_id,
        "rule_id": rule_id,
        "status": "accepted",
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
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
    if let Some(recommendation_key) = suggestion
        .recommendation_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        record_import_learning_lifecycle_feedback(
            connection,
            user_id_i64,
            &ImportLearningLifecycleRecordInput {
                recommendation_key: recommendation_key.to_string(),
                recommendation_type: "learning_center".to_string(),
                feedback: "reject".to_string(),
                rule_id: None,
                suggestion_id: Some(suggestion_id),
                session_id: None,
                preview_id: None,
                bill_id: None,
                candidate_id: None,
                payload_json: Some(json!({"status": "rejected"}).to_string()),
            },
        )
        .map_err(db_error_response)?;
    }
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

