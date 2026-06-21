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
        let session = if require_existing_session {
            get_import_session_async(pool, &draft.session_id, draft.user_id)
                .await?
                .map(|session| session.id)
        } else {
            Some(create_import_session_async(pool, draft).await?)
        };
        let Some(session_db_id) = session else {
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
        let source_ids = upsert_import_sources(pool, session_db_id, user_id, &sources).await?;
        let inserted = insert_standard_rows_and_parser_payloads(
            pool,
            session_db_id,
            user_id,
            parser_drafts,
            standard_row_drafts,
            &source_ids,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE import_sessions
            SET row_count = row_count + $1,
                total_parsed = total_parsed + $1,
                total_preview = total_preview,
                updated_at = now(),
                version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64::try_from(inserted).unwrap_or(i64::MAX))
        .bind(session_db_id)
        .bind(user_id)
        .execute(pool)
        .await?;
        let total_parsed = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM import_standard_rows WHERE session_id = $1 AND user_id = $2",
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
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
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
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
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
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
        .execute(pool)
        .await?
        .rows_affected();
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
        .bind(user_id_i64(user_id)?)
        .execute(pool)
        .await?
        .rows_affected();
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
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let source_id =
            ensure_default_source(pool, session_db_id, user_id_i64, &draft.parser_id).await?;
        insert_standard_row_from_parser_template(
            pool,
            session_db_id,
            source_id,
            user_id_i64,
            0,
            draft,
        )
        .await
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
        if drafts.is_empty() {
            return Ok(0);
        }
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let source_id = ensure_default_source(
            pool,
            session_db_id,
            user_id_i64,
            drafts
                .first()
                .map(|draft| draft.parser_id.as_str())
                .unwrap_or("auto"),
        )
        .await?;
        let rows = standard_row_batch_values_from_parser_templates(source_id, drafts);
        insert_standard_rows_batch_async(pool, session_db_id, user_id_i64, &rows).await?;
        Ok(drafts.len())
    })
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
