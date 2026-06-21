/// 按 preview id 列表读取当前用户可见行，保持输入顺序无关、输出按数据库稳定排序。
pub fn get_preview_by_ids(
    pool: &PostgresPool,
    session_id: &str,
    preview_ids: &[i64],
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewRow>> {
    block_on_db(async move {
        if preview_ids.is_empty() {
            return Ok(Vec::new());
        }
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = ",
        );
        query.push_bind(session_db_id);
        query.push(" AND p.user_id = ");
        query.push_bind(user_id_i64(user_id)?);
        query.push(" AND p.id IN (");
        let mut separated = query.separated(", ");
        for id in preview_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(") ORDER BY p.occurred_at ASC, p.id ASC");
        let rows = query.build().fetch_all(pool).await?;
        rows.iter().map(preview_from_pg_row).collect()
    })
}

/// 读取单条 preview 行并强制 user-scope，用于更新、reclassify 和决策接口。
pub fn get_preview_bill_by_id(
    pool: &PostgresPool,
    preview_id: i64,
    user_id: UserId,
) -> DbResult<Option<ImportPreviewRow>> {
    block_on_db(async move {
        let row = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            "#,
        )
        .bind(preview_id)
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await?;
        row.as_ref().map(preview_from_pg_row).transpose()
    })
}

/// 读取轻量 filter index 数据，供前端在 server-paged 模式下构建全局索引。
pub fn get_preview_filter_index_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewFilterIndexRow>> {
    Ok(get_preview_by_session(pool, session_id, user_id, false)?
        .into_iter()
        .map(|row| ImportPreviewFilterIndexRow {
            id: row.id,
            preview_date: row.preview_date,
            preview_type: row.preview_type,
            preview_amount_cents: row.preview_amount_cents,
            category_id: row.category_id,
            preview_main_category: row.preview_main_category,
            preview_sub_category: row.preview_sub_category,
            preview_source_account_id: row.preview_source_account_id,
            preview_destination_account_id: row.preview_destination_account_id,
            preview_counterparty: row.preview_counterparty,
            preview_payment_method: row.preview_payment_method,
            preview_description: row.preview_description,
            preview_parser_id: row.preview_parser_id,
            preview_parser_tags: row.preview_parser_tags,
            preview_recurring_id: row.preview_recurring_id,
            preview_recurring_candidate_count: row.preview_recurring_candidate_count,
            preview_recurring_match_reasons: row.preview_recurring_match_reasons,
            preview_recurring_matched_date: row.preview_recurring_matched_date,
            preview_selected: row.preview_selected,
            dedup_type: row.dedup_type,
            dedup_source_ids: row.dedup_source_ids,
            preview_matching_feedback: row.preview_matching_feedback,
        })
        .collect())
}

pub fn update_preview_bill(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<bool> {
    replace_preview_selection_with_patches(pool, session_id, user_id, std::slice::from_ref(patch))
        .map(|count| count > 0)
}

pub fn update_preview_bills_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    replace_preview_selection_with_patches(pool, session_id, user_id, patches)
}

/// 用前端草稿 patch 覆盖选择状态，并同步执行身份校验，保证 confirm 只消费落库状态。
pub fn replace_preview_selection_with_patches(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let identity_maps = load_import_identity_maps(pool, user_id_i64).await?;
        let mut changed = 0usize;
        for patch in patches {
            if apply_preview_patch_async(pool, session_db_id, user_id_i64, patch, &identity_maps)
                .await?
            {
                changed += 1;
            }
        }
        Ok(changed)
    })
}

/// 批量应用 preview patch 但保留现有 selection，用于 update/reclassify 不意外改变跨页选择。
pub fn apply_preview_patches_preserving_selection(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    replace_preview_selection_with_patches(pool, session_id, user_id, patches)
}

pub fn update_preview_selection(
    pool: &PostgresPool,
    preview_ids: &[i64],
    selected: bool,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        if preview_ids.is_empty() {
            return Ok(0);
        }
        let mut query = QueryBuilder::<Postgres>::new("UPDATE import_preview_rows SET selected = ");
        query.push_bind(selected);
        query.push(", preview_payload = jsonb_set(preview_payload, '{preview_selected}', ");
        query.push_bind(Value::Bool(selected));
        query.push("::jsonb, true), updated_at = now(), version = version + 1 WHERE user_id = ");
        query.push_bind(user_id_i64(user_id)?);
        query.push(" AND id IN (");
        let mut separated = query.separated(", ");
        for id in preview_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let changed = query.build().execute(pool).await?.rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

pub fn reset_session_preview_selection(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let changed = sqlx::query(
            r#"
            UPDATE import_preview_rows
            SET selected = false,
                preview_payload = jsonb_set(preview_payload, '{preview_selected}', 'false'::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE session_id = $1 AND user_id = $2
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64(user_id)?)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

/// 按服务端查询条件更新跨页选择集合，必须保留 all/valid/needs-review/invert 语义。
pub fn update_session_preview_selection_by_query(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    mode: ImportPreviewSelectionMode,
    target: ImportPreviewSelectionTarget,
    request: &ImportPreviewPageRequest,
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let mut query =
            build_preview_selection_update_query(session_db_id, user_id_i64, mode, target, request);
        let changed = query.build().execute(pool).await?.rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

fn build_preview_selection_update_query(
    session_db_id: i64,
    user_id: i64,
    mode: ImportPreviewSelectionMode,
    target: ImportPreviewSelectionTarget,
    request: &ImportPreviewPageRequest,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new("UPDATE import_preview_rows p SET selected = ");
    match mode {
        ImportPreviewSelectionMode::Select => {
            query.push_bind(true);
            query.push(
                ", preview_payload = jsonb_set(p.preview_payload, '{preview_selected}', to_jsonb(",
            );
            query.push_bind(true);
            query.push("::boolean), true)");
        }
        ImportPreviewSelectionMode::Deselect => {
            query.push_bind(false);
            query.push(
                ", preview_payload = jsonb_set(p.preview_payload, '{preview_selected}', to_jsonb(",
            );
            query.push_bind(false);
            query.push("::boolean), true)");
        }
        ImportPreviewSelectionMode::Invert => {
            query.push("NOT p.selected, preview_payload = jsonb_set(p.preview_payload, '{preview_selected}', to_jsonb(NOT p.selected), true)");
        }
    }
    query.push(", updated_at = now(), version = version + 1 WHERE p.session_id = ");
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates(&mut query, &request.filters, "p");
    push_preview_selection_target_predicates(&mut query, target, "p");
    if !request.preview_ids.is_empty() {
        query.push(" AND p.id IN (");
        let mut separated = query.separated(", ");
        for id in &request.preview_ids {
            separated.push_bind(*id);
        }
        separated.push_unseparated(")");
    }
    query
}
