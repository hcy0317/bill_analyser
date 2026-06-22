/// 写入 LLM memory 事件并截断大文本，避免 prompt 原文无限扩大数据库负载。
pub fn create_llm_memory_event(pool: &PostgresPool, draft: &LlmMemoryEventDraft) -> DbResult<i64> {
    block_on_db(async move {
        let prompt_text = draft
            .prompt_text
            .as_deref()
            .map(|value| truncate_text_bytes(value, LLM_MEMORY_PROMPT_TEXT_MAX_BYTES));
        let id = sqlx::query(
            r#"
            INSERT INTO llm_memory_events (
                user_id, session_id, preview_id, event_type, decision, prompt_text,
                llm_response_raw, llm_provider, llm_model, suggested_main_category,
                suggested_sub_category, suggested_source_account, suggested_destination_account,
                confidence, user_correction_category, user_correction_account,
                snapshot_before, snapshot_after, metadata, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17::jsonb,$18::jsonb,$19::jsonb,now())
            RETURNING id
            "#,
        )
        .bind(user_id_i64(draft.user_id)?)
        .bind(&draft.session_id)
        .bind(draft.preview_id)
        .bind(&draft.event_type)
        .bind(&draft.decision)
        .bind(&prompt_text)
        .bind(&draft.llm_response_raw)
        .bind(&draft.llm_provider)
        .bind(&draft.llm_model)
        .bind(&draft.suggested_main_category)
        .bind(&draft.suggested_sub_category)
        .bind(&draft.suggested_source_account)
        .bind(&draft.suggested_destination_account)
        .bind(draft.confidence)
        .bind(&draft.user_correction_category)
        .bind(&draft.user_correction_account)
        .bind(draft.snapshot_before.as_ref().unwrap_or(&Value::Null).to_string())
        .bind(draft.snapshot_after.as_ref().unwrap_or(&Value::Null).to_string())
        .bind(draft.metadata.as_ref().unwrap_or(&json!({})).to_string())
        .fetch_one(pool)
        .await?
        .try_get("id")?;
        Ok(id)
    })
}

/// 按用户和可选 session 读取最新 LLM memory 事件，供 prompt 上下文和学习中心展示使用。
pub fn get_llm_memory_events(
    pool: &PostgresPool,
    user_id: UserId,
    session_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<LlmMemoryEventRow>> {
    block_on_db(async move {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM llm_memory_events WHERE user_id = ");
        builder.push_bind(user_id_i64(user_id)?);
        if let Some(session_id) = session_id {
            builder.push(" AND session_id = ");
            builder.push_bind(session_id);
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(i64::try_from(limit.clamp(1, 500)).unwrap_or(500));
        let rows = builder.build().fetch_all(pool).await?;
        rows.iter().map(llm_memory_event_from_pg_row).collect()
    })
}

/// 保存导入标注样本，供后续 LLM/learning 召回使用，必须保持 user-scope。
pub fn save_import_annotation_samples(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportAnnotationSampleDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut changed = 0usize;
        for draft in drafts {
            let payload = json!({
                "annotated_type": draft.annotated_type,
                "annotated_category_id": draft.annotated_category_id,
                "annotated_source_account_id": draft.annotated_source_account_id,
                "annotated_destination_account_id": draft.annotated_destination_account_id,
            });
            let annotation_type = draft.annotated_type.as_deref().unwrap_or("classification");
            let key = format!("{}:{}", draft.preview_id, annotation_type);
            sqlx::query(
                r#"
                INSERT INTO import_annotation_samples (
                    user_id, session_id, preview_id, annotation_type, annotation_key,
                    sample_payload, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6::jsonb,now())
                ON CONFLICT (user_id, session_id, annotation_type, annotation_key)
                DO UPDATE SET sample_payload = excluded.sample_payload,
                              preview_id = excluded.preview_id,
                              created_at = now(),
                              version = import_annotation_samples.version + 1
                "#,
            )
            .bind(user_id)
            .bind(session_id)
            .bind(draft.preview_id)
            .bind(annotation_type)
            .bind(key)
            .bind(payload.to_string())
            .execute(pool)
            .await?;
            changed += 1;
        }
        Ok(changed)
    })
}

/// 读取指定导入 session 的人工标注样本，供 learning/LLM 召回和训练样本生成使用。
pub fn get_import_annotation_samples(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportAnnotationSampleRow>> {
    block_on_db(async move {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, session_id, preview_id, annotation_type, sample_payload, created_at
            FROM import_annotation_samples
            WHERE user_id = $1 AND session_id = $2
            ORDER BY id ASC
            "#,
        )
        .bind(user_id_i64(user_id)?)
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        rows.iter()
            .map(|row| {
                let payload: Value = row.try_get("sample_payload")?;
                let created_at = format_pg_time(row.try_get("created_at")?);
                Ok(ImportAnnotationSampleRow {
                    id: row.try_get("id")?,
                    session_id: row.try_get("session_id")?,
                    user_id: row.try_get("user_id")?,
                    preview_id: row.try_get("preview_id")?,
                    annotated_type: payload_text(&payload, "annotated_type"),
                    annotated_category_id: payload_i64(&payload, "annotated_category_id"),
                    annotated_source_account_id: payload_i64(
                        &payload,
                        "annotated_source_account_id",
                    ),
                    annotated_destination_account_id: payload_i64(
                        &payload,
                        "annotated_destination_account_id",
                    ),
                    created_at: created_at.clone(),
                    updated_at: created_at,
                })
            })
            .collect()
    })
}
