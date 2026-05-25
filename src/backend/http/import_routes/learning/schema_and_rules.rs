// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。


fn init_import_learning_runtime_schema(
    runtime: &SqliteRuntime,
) -> Result<(), ImportV2RouteResponse> {
    init_import_runtime_schema(runtime)?;
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_learning_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                learned_type TEXT,
                learned_category_id INTEGER,
                learned_source_account_id INTEGER,
                learned_destination_account_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                source_session_id TEXT,
                source_preview_id INTEGER,
                parser_id TEXT,
                composite_match_hash TEXT,
                match_features_json TEXT,
                applied_count INTEGER NOT NULL DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_rules_user_enabled
                ON import_learning_rules(user_id, enabled);
            CREATE TABLE IF NOT EXISTS import_learning_rule_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                rule_id INTEGER,
                session_id TEXT,
                preview_id INTEGER,
                action TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            ",
        )
        .map_err(db_error_response)
}

fn init_global_learning_runtime_schema(
    runtime: &SqliteRuntime,
) -> Result<(), ImportV2RouteResponse> {
    init_import_learning_runtime_schema(runtime)?;
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_learning_corpus_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                session_id TEXT NOT NULL,
                preview_id INTEGER NOT NULL,
                parser_id TEXT,
                counterparty TEXT,
                description TEXT,
                payment_method TEXT,
                composite_match_hash TEXT,
                match_features_json TEXT,
                annotated_type TEXT,
                annotated_category_id INTEGER,
                annotated_source_account_id INTEGER,
                annotated_destination_account_id INTEGER,
                source_snapshot_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, session_id, preview_id)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_user
                ON import_learning_corpus_samples(user_id, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_hash
                ON import_learning_corpus_samples(user_id, composite_match_hash);
            CREATE TABLE IF NOT EXISTS import_learning_feedback_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                event_type TEXT NOT NULL,
                rule_id INTEGER,
                suggestion_id INTEGER,
                session_id TEXT,
                preview_id INTEGER,
                bill_id INTEGER,
                candidate_id TEXT,
                payload_json TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_user
                ON import_learning_feedback_events(user_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_type
                ON import_learning_feedback_events(user_id, event_type);
            CREATE TABLE IF NOT EXISTS import_learning_concept_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                concept_key TEXT NOT NULL,
                concept_type TEXT NOT NULL,
                sample_count INTEGER NOT NULL DEFAULT 0,
                accepted_count INTEGER NOT NULL DEFAULT 0,
                rejected_count INTEGER NOT NULL DEFAULT 0,
                auto_applied_count INTEGER NOT NULL DEFAULT 0,
                rollback_count INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, concept_key, concept_type)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_concept_stats_user
                ON import_learning_concept_stats(user_id, concept_type);
            CREATE TABLE IF NOT EXISTS import_learning_suggestions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                composite_match_hash TEXT,
                match_features_json TEXT,
                suggested_type TEXT,
                suggested_category_id INTEGER,
                suggested_source_account_id INTEGER,
                suggested_destination_account_id INTEGER,
                sample_count INTEGER NOT NULL DEFAULT 1,
                source_session_ids_json TEXT,
                source_preview_ids_json TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                existing_rule_id INTEGER,
                summary TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            );
            CREATE INDEX IF NOT EXISTS idx_learning_suggestions_user_status
                ON import_learning_suggestions(user_id, status);
            ",
        )
        .map_err(db_error_response)?;
    for (table_name, column_name, definition) in [
        ("import_learning_rules", "parser_id", "TEXT"),
        ("import_learning_rules", "composite_match_hash", "TEXT"),
        ("import_learning_rules", "match_features_json", "TEXT"),
        ("import_learning_rule_logs", "match_type", "TEXT"),
        ("import_learning_rule_logs", "match_value", "TEXT"),
        (
            "import_learning_rule_logs",
            "normalized_match_value",
            "TEXT",
        ),
        ("import_learning_rule_logs", "payload_json", "TEXT"),
    ] {
        ensure_table_column(runtime.connection(), table_name, column_name, definition)?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_table_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<(), ImportV2RouteResponse> {
    if table_has_column(connection, table_name, column_name).map_err(db_error_response)? {
        return Ok(());
    }
    connection
        .execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"),
            [],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn annotation_samples_from_payload(payload: &Value) -> Vec<ImportAnnotationSampleDraft> {
    let Ok(items) = preview_update_items_from_payload(payload) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let preview_id = preview_id_from_payload(item).ok()?;
            Some(ImportAnnotationSampleDraft {
                preview_id,
                annotated_type: first_value(
                    item,
                    &[
                        "annotated_type",
                        "annotatedType",
                        "preview_type",
                        "previewType",
                        "type",
                    ],
                )
                .and_then(value_to_preview_type_text),
                annotated_category_id: first_value(
                    item,
                    &[
                        "annotated_category_id",
                        "annotatedCategoryId",
                        "category_id",
                        "categoryId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
                annotated_source_account_id: first_value(
                    item,
                    &[
                        "annotated_source_account_id",
                        "annotatedSourceAccountId",
                        "preview_source_account_id",
                        "previewSourceAccountId",
                        "sourceAccountId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
                annotated_destination_account_id: first_value(
                    item,
                    &[
                        "annotated_destination_account_id",
                        "annotatedDestinationAccountId",
                        "preview_destination_account_id",
                        "previewDestinationAccountId",
                        "destinationAccountId",
                    ],
                )
                .and_then(value_to_i64)
                .filter(|value| *value > 0),
            })
        })
        .collect()
}

#[derive(Debug, Default)]
struct ImportLearningPromotionResult {
    rules_total: i64,
    created: i64,
    updated: i64,
}

fn promote_import_learning_rules(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    preview_ids: &[i64],
) -> Result<ImportLearningPromotionResult, ImportV2RouteResponse> {
    let user_id_i64 = user_id_i64_value(user_id)?;
    let selected_ids = preview_ids
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    if selected_ids.is_empty() {
        let rules_total = count_import_learning_rules(connection, user_id, None)?;
        return Ok(ImportLearningPromotionResult {
            rules_total,
            ..ImportLearningPromotionResult::default()
        });
    }
    let samples = get_import_annotation_samples(connection, session_id, user_id)
        .map_err(db_error_response)?
        .into_iter()
        .map(|sample| (sample.preview_id, sample))
        .collect::<BTreeMap<_, _>>();
    let mut result = ImportLearningPromotionResult::default();
    for preview_id in selected_ids {
        let Some(preview) =
            get_preview_bill_by_id(connection, preview_id, user_id).map_err(db_error_response)?
        else {
            continue;
        };
        if preview.session_id != session_id {
            continue;
        }
        let Some((features, composite_hash)) = composite_match_features_for_preview(&preview)
        else {
            continue;
        };
        let sample = samples.get(&preview_id);
        let learned_type = sample
            .and_then(|sample| sample.annotated_type.clone())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| preview.preview_type.clone());
        let learned_category_id = sample.and_then(|sample| sample.annotated_category_id);
        let learned_source_account_id = sample
            .and_then(|sample| sample.annotated_source_account_id)
            .or(preview.preview_source_account_id);
        let learned_destination_account_id = sample
            .and_then(|sample| sample.annotated_destination_account_id)
            .or(preview.preview_destination_account_id);
        let now = now_text();
        let existing: Option<i64> = connection
            .query_row(
                "
                SELECT id FROM import_learning_rules
                WHERE user_id = ?1 AND match_type = 'composite' AND normalized_match_value = ?2
                ",
                params![user_id_i64, composite_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error_response)?;
        connection
            .execute(
                "
                INSERT INTO import_learning_rules (
                    user_id, match_type, match_value, normalized_match_value,
                    learned_type, learned_category_id, learned_source_account_id,
                    learned_destination_account_id, enabled, source_session_id,
                    source_preview_id, parser_id, composite_match_hash,
                    match_features_json, created_at, updated_at
                ) VALUES (?1, 'composite', ?2, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?2, ?10, ?11, ?11)
                ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                    learned_type = excluded.learned_type,
                    learned_category_id = excluded.learned_category_id,
                    learned_source_account_id = excluded.learned_source_account_id,
                    learned_destination_account_id = excluded.learned_destination_account_id,
                    source_session_id = excluded.source_session_id,
                    source_preview_id = excluded.source_preview_id,
                    parser_id = excluded.parser_id,
                    composite_match_hash = excluded.composite_match_hash,
                    match_features_json = excluded.match_features_json,
                    updated_at = excluded.updated_at
                ",
                params![
                    user_id_i64,
                    composite_hash,
                    learned_type,
                    learned_category_id,
                    learned_source_account_id,
                    learned_destination_account_id,
                    session_id,
                    preview_id,
                    preview.preview_parser_id,
                    Value::Object(features).to_string(),
                    now,
                ],
            )
            .map_err(db_error_response)?;
        let rule_id = match existing {
            Some(rule_id) => {
                result.updated += 1;
                rule_id
            }
            None => {
                result.created += 1;
                connection.last_insert_rowid()
            }
        };
        connection
            .execute(
                "
                INSERT INTO import_learning_rule_logs (
                    user_id, rule_id, session_id, preview_id, action, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                ",
                params![
                    user_id_i64,
                    rule_id,
                    session_id,
                    preview_id,
                    if existing.is_some() {
                        "updated"
                    } else {
                        "created"
                    },
                    now,
                ],
            )
            .map_err(db_error_response)?;
    }
    result.rules_total = count_import_learning_rules(connection, user_id, None)?;
    Ok(result)
}

fn composite_match_features_for_preview(
    preview: &ImportPreviewRow,
) -> Option<(Map<String, Value>, String)> {
    let mut values = BTreeMap::new();
    for (key, raw_value) in [
        ("counterparty", preview.preview_counterparty.as_str()),
        ("description", preview.preview_description.as_str()),
        ("parser_id", preview.preview_parser_id.as_str()),
        ("payment_method", preview.preview_payment_method.as_str()),
    ] {
        let normalized = normalize_learning_match_value(raw_value);
        if !normalized.is_empty() {
            values.insert(key, normalized);
        }
    }
    if values.len() < 2 {
        return None;
    }
    let mut features = Map::new();
    let hash = values
        .iter()
        .map(|(key, value)| {
            features.insert((*key).to_string(), json!(value));
            let alias = match *key {
                "counterparty" => "c",
                "description" => "d",
                "payment_method" => "m",
                "parser_id" => "p",
                _ => key,
            };
            format!("{alias}={value}")
        })
        .collect::<Vec<_>>()
        .join("|");
    Some((features, hash))
}

#[tracing::instrument(level = "debug", skip_all)]
fn count_import_learning_rules(
    connection: &Connection,
    user_id: UserId,
    enabled_only: Option<bool>,
) -> Result<i64, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    let sql = if enabled_only == Some(true) {
        "SELECT COUNT(*) FROM import_learning_rules WHERE user_id = ?1 AND enabled = 1"
    } else {
        "SELECT COUNT(*) FROM import_learning_rules WHERE user_id = ?1"
    };
    connection
        .query_row(sql, params![user_id], |row| row.get(0))
        .map_err(db_error_response)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_import_learning_rules(
    connection: &Connection,
    user_id: UserId,
    enabled_only: Option<bool>,
    limit: usize,
    offset: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    let sql = if enabled_only == Some(true) {
        "
        SELECT * FROM import_learning_rules
        WHERE user_id = ?1 AND enabled = 1
        ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3
        "
    } else {
        "
        SELECT * FROM import_learning_rules
        WHERE user_id = ?1
        ORDER BY updated_at DESC, id DESC LIMIT ?2 OFFSET ?3
        "
    };
    let mut statement = connection.prepare(sql).map_err(db_error_response)?;
    let rows = statement
        .query_map(
            params![user_id, usize_to_i64(limit), usize_to_i64(offset)],
            learning_rule_row_to_value,
        )
        .map_err(db_error_response)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)
}

#[tracing::instrument(level = "debug", skip_all)]
fn get_import_learning_rule(
    connection: &Connection,
    rule_id: i64,
    user_id: UserId,
) -> Result<Option<Value>, ImportV2RouteResponse> {
    let user_id = user_id_i64_value(user_id)?;
    connection
        .query_row(
            "SELECT * FROM import_learning_rules WHERE id = ?1 AND user_id = ?2",
            params![rule_id, user_id],
            learning_rule_row_to_value,
        )
        .optional()
        .map_err(db_error_response)
}

#[tracing::instrument(level = "debug", skip_all)]
fn update_import_learning_rule(
    connection: &Connection,
    rule_id: i64,
    user_id: UserId,
    object: &Map<String, Value>,
) -> Result<(), ImportV2RouteResponse> {
    let Some(existing) = get_import_learning_rule(connection, rule_id, user_id)? else {
        return Err(import_v2_error_response(404, "Rule not found"));
    };
    let has_edit = first_value(object, &["matchValue", "match_value"]).is_some()
        || first_value(object, &["learnedType", "learned_type"]).is_some()
        || first_value(object, &["learnedCategoryId", "learned_category_id"]).is_some()
        || first_value(object, &["enabled"]).is_some();
    if !has_edit {
        return Err(import_v2_error_response(
            400,
            "No editable rule fields provided",
        ));
    }
    let match_value = first_text_from_object(object, &["matchValue", "match_value"])
        .unwrap_or_else(|| {
            existing["matchValue"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        });
    let normalized_match_value = normalize_learning_match_value(&match_value);
    let learned_type = if first_value(object, &["learnedType", "learned_type"]).is_some() {
        first_text_from_object(object, &["learnedType", "learned_type"])
    } else {
        existing["learnedType"].as_str().map(str::to_string)
    };
    let learned_category_id =
        if first_value(object, &["learnedCategoryId", "learned_category_id"]).is_some() {
            first_value(object, &["learnedCategoryId", "learned_category_id"])
                .and_then(value_to_i64)
                .filter(|value| *value > 0)
        } else {
            existing["learnedCategoryId"].as_i64()
        };
    let enabled = if first_value(object, &["enabled"]).is_some() {
        first_value(object, &["enabled"])
            .and_then(value_to_bool)
            .unwrap_or(true)
    } else {
        existing["enabled"].as_bool().unwrap_or(true)
    };
    let user_id_i64 = user_id_i64_value(user_id)?;
    let changed = connection
        .execute(
            "
            UPDATE import_learning_rules
            SET match_value = ?1,
                normalized_match_value = ?2,
                learned_type = ?3,
                learned_category_id = ?4,
                enabled = ?5,
                updated_at = ?6
            WHERE id = ?7 AND user_id = ?8
            ",
            params![
                match_value,
                normalized_match_value,
                learned_type,
                learned_category_id,
                enabled,
                now_text(),
                rule_id,
                user_id_i64,
            ],
        )
        .map_err(db_error_response)?;
    if changed == 0 {
        Err(import_v2_error_response(404, "Rule not found"))
    } else {
        Ok(())
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn delete_import_learning_rule(
    connection: &Connection,
    rule_id: i64,
    user_id: UserId,
) -> Result<bool, ImportV2RouteResponse> {
    let changed = connection
        .execute(
            "DELETE FROM import_learning_rules WHERE id = ?1 AND user_id = ?2",
            params![rule_id, user_id_i64_value(user_id)?],
        )
        .map_err(db_error_response)?;
    Ok(changed > 0)
}

fn learning_rule_row_to_value(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let match_features: Option<String> = row.get("match_features_json")?;
    let match_features = match_features
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .unwrap_or_else(|| json!({}));
    Ok(json!({
        "id": row.get::<_, i64>("id")?,
        "userId": row.get::<_, i64>("user_id")?,
        "matchType": row.get::<_, String>("match_type")?,
        "matchValue": row.get::<_, String>("match_value")?,
        "normalizedMatchValue": row.get::<_, String>("normalized_match_value")?,
        "learnedType": row.get::<_, Option<String>>("learned_type")?,
        "learnedCategoryId": row.get::<_, Option<i64>>("learned_category_id")?,
        "learnedSourceAccountId": row.get::<_, Option<i64>>("learned_source_account_id")?,
        "learnedDestinationAccountId": row.get::<_, Option<i64>>("learned_destination_account_id")?,
        "enabled": row.get::<_, bool>("enabled")?,
        "sourceSessionId": row.get::<_, Option<String>>("source_session_id")?,
        "sourcePreviewId": row.get::<_, Option<i64>>("source_preview_id")?,
        "parserId": row.get::<_, Option<String>>("parser_id")?,
        "compositeMatchHash": row.get::<_, Option<String>>("composite_match_hash")?,
        "matchFeatures": match_features,
        "appliedCount": row.get::<_, i64>("applied_count")?,
        "lastAppliedAt": row.get::<_, Option<String>>("last_applied_at")?,
        "createdAt": row.get::<_, String>("created_at")?,
        "updatedAt": row.get::<_, String>("updated_at")?,
    }))
}

fn learning_data_response(data: Value) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({"success": true, "data": data}),
    }
}

fn learning_error_response(status_code: u16, error: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({"success": false, "error": error}),
    }
}

fn learning_rule_camel_to_snake(rule: &Value) -> Value {
    json!({
        "id": rule["id"],
        "user_id": rule["userId"],
        "match_type": rule["matchType"],
        "match_value": rule["matchValue"],
        "normalized_match_value": rule["normalizedMatchValue"],
        "learned_type": rule["learnedType"],
        "learned_category_id": rule["learnedCategoryId"],
        "learned_source_account_id": rule["learnedSourceAccountId"],
        "learned_destination_account_id": rule["learnedDestinationAccountId"],
        "enabled": rule["enabled"],
        "source_session_id": rule["sourceSessionId"],
        "source_preview_id": rule["sourcePreviewId"],
        "parser_id": rule["parserId"],
        "composite_match_hash": rule["compositeMatchHash"],
        "match_features_json": rule["matchFeatures"].to_string(),
        "applied_count": rule["appliedCount"],
        "last_applied_at": rule["lastAppliedAt"],
        "created_at": rule["createdAt"],
        "updated_at": rule["updatedAt"],
    })
}

