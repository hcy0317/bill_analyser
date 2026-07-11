/// 写入 parser/source/standard-row staging 数据；这是 parse 阶段进入 DB 的主入口，金额仍保留 parser 边界语义。
#[tracing::instrument(level = "debug", skip_all)]
pub fn stage_import_parser_templates_with_sources(
    pool: &PostgresPool,
    draft: &ImportSessionDraft,
    parser_drafts: &[ImportParserTemplateDraft],
    source_drafts: &[ImportSourceDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    require_existing_session: bool,
) -> DbResult<ImportParseStagingResult> {
    block_on_db(async move {
        let user_id = user_id_i64(draft.user_id)?;
        let mut tx = pool.begin().await?;
        let session =
            prepare_import_session_for_staging_on_tx(&mut tx, draft, require_existing_session)
                .await?;
        let Some(session_db_id) = session else {
            tx.commit().await?;
            return Ok(ImportParseStagingResult {
                inserted_count: 0,
                total_parsed: 0,
                session_found: false,
            });
        };
        let sources = if source_drafts.is_empty() {
            vec![ImportSourceDraft {
                source_index: 0,
                original_file_name: "inline-standard-bills".to_string(),
                parser_id: parser_drafts
                    .first()
                    .map(|draft| draft.parser_id.clone())
                    .unwrap_or_else(|| "auto".to_string()),
                parser_name: "auto".to_string(),
                parser_signal: "provided".to_string(),
                parser_confidence: 1.0,
                feature_signature: format!("{}:inline", draft.session_id),
                metadata: json!({}),
            }]
        } else {
            source_drafts.to_vec()
        };
        let source_ids =
            upsert_import_sources_on_tx(&mut tx, session_db_id, user_id, &sources).await?;
        let inserted = insert_standard_rows_and_parser_payloads_on_tx(
            &mut tx,
            session_db_id,
            user_id,
            parser_drafts,
            standard_row_drafts,
            &source_ids,
        )
        .await?;
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        let total_parsed = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM import_standard_rows WHERE session_id = $1 AND user_id = $2",
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ImportParseStagingResult {
            inserted_count: inserted,
            total_parsed,
            session_found: true,
        })
    })
}

/// 兼容仅传 parser template 的 staging 入口，内部补默认 source 后复用完整 source 写入链路。
#[tracing::instrument(level = "debug", skip_all)]
pub fn stage_import_parser_templates(
    pool: &PostgresPool,
    draft: &ImportSessionDraft,
    parser_drafts: &[ImportParserTemplateDraft],
    require_existing_session: bool,
) -> DbResult<ImportParseStagingResult> {
    stage_import_parser_templates_with_sources(
        pool,
        draft,
        parser_drafts,
        &[],
        &[],
        require_existing_session,
    )
}

/// 读取尚未进入 dedup/stage2 的标准行，结果顺序决定后续预览基底的稳定性。
#[tracing::instrument(level = "debug", skip_all)]
pub fn get_unprocessed_templates_for_dedup(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let session_db_id = active_session_db_id(pool, session_id, user_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT r.*, s.session_key, src.parser_id AS source_parser_id
            FROM import_standard_rows r
            JOIN import_sessions s ON s.id = r.session_id
            LEFT JOIN import_sources src ON src.id = r.source_id
            WHERE r.session_id = $1
              AND r.user_id = $2
              AND COALESCE((r.parser_payload->>'parser_is_processed')::boolean, false) = false
            ORDER BY r.occurred_at ASC, r.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter().map(parser_template_from_pg_row).collect()
    })
}

/// 在 preview materialization 成功后批量标记 template 已处理，避免重复 stage2。
#[tracing::instrument(level = "debug", skip_all)]
pub fn mark_unprocessed_parser_templates_processed_for_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let changed = sqlx::query(
            r#"
            UPDATE import_standard_rows
            SET parser_payload = jsonb_set(parser_payload, '{parser_is_processed}', 'true'::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE session_id = $1 AND user_id = $2
              AND COALESCE((parser_payload->>'parser_is_processed')::boolean, false) = false
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_parser_template_status(
    pool: &PostgresPool,
    template_id: i64,
    user_id: UserId,
    processed: bool,
) -> DbResult<bool> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session = sqlx::query(
            r#"
            SELECT session.id, session.status
            FROM import_sessions session
            JOIN import_standard_rows row ON row.session_id = session.id
            WHERE row.id = $1 AND row.user_id = $2 AND session.user_id = $2
            FOR UPDATE OF session
            "#,
        )
        .bind(template_id)
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(session) = session else {
            tx.commit().await?;
            return Ok(false);
        };
        if session.try_get::<String, _>("status")? == "confirmed" {
            return Err(DbError::InvalidOperation(
                "confirmed import session is terminal".to_string(),
            ));
        }
        let session_db_id = session.try_get::<i64, _>("id")?;
        let changed = sqlx::query(
            r#"
            UPDATE import_standard_rows
            SET parser_payload = jsonb_set(parser_payload, '{parser_is_processed}', $1::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(Value::Bool(processed).to_string())
        .bind(template_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(changed > 0)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_parser_templates_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT r.*, s.session_key, src.parser_id AS source_parser_id
            FROM import_standard_rows r
            JOIN import_sessions s ON s.id = r.session_id
            LEFT JOIN import_sources src ON src.id = r.source_id
            WHERE r.session_id = $1 AND r.user_id = $2
            ORDER BY r.occurred_at ASC, r.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter().map(parser_template_from_pg_row).collect()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_parser_template(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    draft: &ImportParserTemplateDraft,
) -> DbResult<i64> {
    block_on_db(async move {
        let user_id_i64 = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id_i64).await?;
        let source_id = ensure_default_source_on_tx(
            &mut tx,
            session_db_id,
            user_id_i64,
            &draft.parser_id,
        )
        .await?;
        let row = standard_row_batch_value_from_parser_template(source_id, 0, draft);
        insert_standard_rows_batch_on_tx(
            &mut tx,
            session_db_id,
            user_id_i64,
            std::slice::from_ref(&row),
        )
        .await?;
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM import_standard_rows WHERE user_id = $1 AND source_id = $2 AND source_row_index = 0",
        )
        .bind(user_id_i64)
        .bind(source_id)
        .fetch_one(&mut *tx)
        .await?;
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id_i64).await?;
        tx.commit().await?;
        Ok(id)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_parser_templates_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportParserTemplateDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id_i64 = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id_i64).await?;
        if drafts.is_empty() {
            tx.commit().await?;
            return Ok(0);
        }
        let source_id = ensure_default_source_on_tx(
            &mut tx,
            session_db_id,
            user_id_i64,
            drafts
                .first()
                .map(|draft| draft.parser_id.as_str())
                .unwrap_or("auto"),
        )
        .await?;
        let rows = standard_row_batch_values_from_parser_templates(source_id, drafts);
        insert_standard_rows_batch_on_tx(&mut tx, session_db_id, user_id_i64, &rows).await?;
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id_i64).await?;
        tx.commit().await?;
        Ok(drafts.len())
    })
}

async fn upsert_import_sources_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    drafts: &[ImportSourceDraft],
) -> DbResult<BTreeMap<i64, i64>> {
    let mut ids = BTreeMap::new();
    for draft in drafts {
        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO import_sources (
                session_id, user_id, source_index, original_file_name,
                parser_id, parser_name, parser_signal, parser_confidence,
                feature_signature, metadata, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::jsonb,now(),now())
            ON CONFLICT (session_id, source_index) DO UPDATE SET
                original_file_name = excluded.original_file_name,
                parser_id = excluded.parser_id,
                parser_name = excluded.parser_name,
                parser_signal = excluded.parser_signal,
                parser_confidence = excluded.parser_confidence,
                feature_signature = excluded.feature_signature,
                metadata = excluded.metadata,
                updated_at = now(),
                version = import_sources.version + 1
            RETURNING id
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(draft.source_index)
        .bind(&draft.original_file_name)
        .bind(&draft.parser_id)
        .bind(&draft.parser_name)
        .bind(&draft.parser_signal)
        .bind(draft.parser_confidence)
        .bind(&draft.feature_signature)
        .bind(draft.metadata.to_string())
        .fetch_one(&mut **tx)
        .await?;
        ids.insert(draft.source_index, id);
    }
    Ok(ids)
}

async fn ensure_default_source_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    parser_id: &str,
) -> DbResult<i64> {
    let source_ids = upsert_import_sources_on_tx(
        tx,
        session_db_id,
        user_id,
        &[ImportSourceDraft {
            source_index: 0,
            original_file_name: "inline-standard-bills".to_string(),
            parser_id: parser_id.to_string(),
            parser_name: parser_source_label(parser_id).to_string(),
            parser_signal: "provided".to_string(),
            parser_confidence: 1.0,
            feature_signature: format!("{session_db_id}:default"),
            metadata: json!({}),
        }],
    )
    .await?;
    source_ids.get(&0).copied().ok_or_else(|| {
        DbError::InvalidOperation("default import source was not created".to_string())
    })
}

async fn insert_standard_rows_and_parser_payloads_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    parser_drafts: &[ImportParserTemplateDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    source_ids: &BTreeMap<i64, i64>,
) -> DbResult<usize> {
    let rows = if standard_row_drafts.is_empty() {
        let source_id = source_ids.values().next().copied().ok_or_else(|| {
            DbError::InvalidOperation("import source missing".to_string())
        })?;
        standard_row_batch_values_from_parser_templates(source_id, parser_drafts)
    } else {
        standard_row_batch_values_from_standard_row_drafts(
            parser_drafts,
            standard_row_drafts,
            source_ids,
        )?
    };
    insert_standard_rows_batch_on_tx(tx, session_db_id, user_id, &rows).await?;
    Ok(rows.len())
}

async fn insert_standard_rows_batch_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    rows: &[StandardRowBatchValue],
) -> DbResult<()> {
    for chunk in rows.chunks(IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE) {
        let mut query = build_standard_rows_insert_query(session_db_id, user_id, chunk);
        query.build().execute(&mut **tx).await?;
    }
    Ok(())
}

#[allow(dead_code)]
fn retain_legacy_pool_parser_helpers_for_include_contract() {
    let _ = upsert_import_sources;
    let _ = ensure_default_source;
    let _ = insert_standard_rows_and_parser_payloads;
    let _ = insert_standard_rows_batch_async;
    let _ = insert_standard_row;
    let _ = insert_standard_row_from_parser_template;
}

fn standard_row_batch_values_from_parser_templates(
    source_id: i64,
    drafts: &[ImportParserTemplateDraft],
) -> Vec<StandardRowBatchValue> {
    drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            standard_row_batch_value_from_parser_template(
                source_id,
                i64::try_from(index).unwrap_or(i64::MAX),
                draft,
            )
        })
        .collect()
}
