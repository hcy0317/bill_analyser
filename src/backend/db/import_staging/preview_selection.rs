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
        .map(ImportPreviewFilterIndexRow::from)
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
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let changed =
            apply_preview_patches_on_tx(&mut tx, session_db_id, user_id, patches).await?;
        tx.commit().await?;
        Ok(changed)
    })
}

async fn apply_preview_patches_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    let identity_maps = load_import_identity_maps_for_confirm(tx, user_id).await?;
    let mut changed = 0usize;
    for patch in patches {
        if apply_preview_patch_on_tx(tx, session_db_id, user_id, patch, &identity_maps).await? {
            changed += 1;
        }
    }
    Ok(changed)
}

async fn load_preview_bill_by_id_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    preview_id: i64,
    user_id: i64,
) -> DbResult<Option<ImportPreviewRow>> {
    let row = sqlx::query(
        r#"
        SELECT p.*, s.session_key
        FROM import_preview_rows p
        JOIN import_sessions s ON s.id = p.session_id
        WHERE p.id = $1 AND p.session_id = $2 AND p.user_id = $3
        FOR UPDATE OF p
        "#,
    )
    .bind(preview_id)
    .bind(session_db_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(preview_from_pg_row).transpose()
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

/// 在同一 session 锁事务内校验选择快照、应用行级 CAS 草稿并读取权威选中行。
pub fn apply_preview_patches_and_load_selected_if_current(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
    expected_selection_hash: &str,
) -> DbResult<ImportPreviewActionPreflushResult> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let selected_snapshot = validate_expected_selection_hash_on_tx(
            &mut tx,
            session_db_id,
            user_id,
            Some(expected_selection_hash),
        )
        .await?;
        let selected_ids = selected_snapshot.into_iter().collect::<BTreeSet<_>>();
        if patches
            .iter()
            .any(|patch| !selected_ids.contains(&patch.preview_id))
        {
            return Err(DbError::PreviewSelectionTargetMismatch {
                session_id: session_id.to_string(),
            });
        }
        let applied_preview_updates =
            apply_preview_patches_on_tx(&mut tx, session_db_id, user_id, patches).await?;
        let rows = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.session_id = $1 AND p.user_id = $2 AND p.selected = true
            ORDER BY p.occurred_at ASC, p.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        let selected_rows = rows
            .iter()
            .map(preview_from_pg_row)
            .collect::<DbResult<Vec<_>>>()?;
        tx.commit().await?;
        Ok(ImportPreviewActionPreflushResult {
            applied_preview_updates,
            selected_rows,
        })
    })
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
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let Some(session_db_id) =
            lock_active_preview_parent_for_ids_on_tx(&mut tx, preview_ids, user_id).await?
        else {
            tx.commit().await?;
            return Ok(0);
        };
        let mut query = QueryBuilder::<Postgres>::new("UPDATE import_preview_rows SET selected = ");
        query.push_bind(selected);
        query.push(", preview_payload = jsonb_set(preview_payload, '{preview_selected}', ");
        query.push_bind(Value::Bool(selected));
        query.push("::jsonb, true), updated_at = now() WHERE user_id = ");
        query.push_bind(user_id);
        query.push(" AND session_id = ");
        query.push_bind(session_db_id);
        query.push(" AND id IN (");
        let mut separated = query.separated(", ");
        for id in preview_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let changed = query.build().execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

/// 在同一个 session 锁事务内校验集合快照并应用显式 selection patch。
pub fn patch_preview_selection(
    pool: &PostgresPool,
    session_id: &str,
    selected_ids: &[i64],
    deselected_ids: &[i64],
    expected_selection_hash: Option<&str>,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        validate_expected_selection_hash_on_tx(
            &mut tx,
            session_db_id,
            user_id,
            expected_selection_hash,
        )
        .await?;

        let target_ids = selected_ids
            .iter()
            .chain(deselected_ids.iter())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !target_ids.is_empty() {
            let scoped_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM import_preview_rows WHERE session_id = $1 AND user_id = $2 AND id = ANY($3)",
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(&target_ids)
            .fetch_one(&mut *tx)
            .await?;
            if usize::try_from(scoped_count).unwrap_or(usize::MAX) != target_ids.len() {
                return Err(DbError::PreviewSelectionTargetMismatch {
                    session_id: session_id.to_string(),
                });
            }
        }

        let mut updated = 0_u64;
        for (ids, selected) in [(selected_ids, true), (deselected_ids, false)] {
            if ids.is_empty() {
                continue;
            }
            updated += sqlx::query(
                r#"
                UPDATE import_preview_rows
                SET selected = $1,
                    preview_payload = jsonb_set(preview_payload, '{preview_selected}', to_jsonb($1::boolean), true),
                    updated_at = now()
                WHERE session_id = $2 AND user_id = $3 AND id = ANY($4)
                "#,
            )
            .bind(selected)
            .bind(session_db_id)
            .bind(user_id)
            .bind(ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        }
        tx.commit().await?;
        Ok(usize::try_from(updated).unwrap_or(usize::MAX))
    })
}

async fn validate_expected_selection_hash_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    expected_selection_hash: Option<&str>,
) -> DbResult<Vec<i64>> {
    let Some(expected) = expected_selection_hash else {
        return Ok(Vec::new());
    };
    let selected_snapshot = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM import_preview_rows WHERE session_id = $1 AND user_id = $2 AND selected = true ORDER BY id ASC",
    )
    .bind(session_db_id)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;
    let actual_selection_hash = preview_id_snapshot_hash(&selected_snapshot);
    if expected != actual_selection_hash {
        return Err(DbError::preview_selection_conflict(
            expected,
            &actual_selection_hash,
        ));
    }
    Ok(selected_snapshot)
}

async fn lock_active_preview_parent_for_ids_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    preview_ids: &[i64],
    user_id: i64,
) -> DbResult<Option<i64>> {
    let rows = sqlx::query(
        r#"
        SELECT session.id, session.status
        FROM import_sessions session
        WHERE session.user_id = $1
          AND session.id IN (
              SELECT preview.session_id
              FROM import_preview_rows preview
              WHERE preview.user_id = $1 AND preview.id = ANY($2)
          )
        ORDER BY session.id ASC
        FOR UPDATE OF session
        "#,
    )
    .bind(user_id)
    .bind(preview_ids.to_vec())
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 1 {
        return Err(DbError::InvalidOperation(
            "preview ids span multiple import sessions".to_string(),
        ));
    }
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    if row.try_get::<String, _>("status")? == "confirmed" {
        return Err(DbError::InvalidOperation(
            "confirmed import session is terminal".to_string(),
        ));
    }
    Ok(Some(row.try_get("id")?))
}

pub fn reset_session_preview_selection(
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
            UPDATE import_preview_rows
            SET selected = false,
                preview_payload = jsonb_set(preview_payload, '{preview_selected}', 'false'::jsonb, true),
                updated_at = now()
            WHERE session_id = $1 AND user_id = $2
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
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
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let mut query =
            build_preview_selection_update_query(session_db_id, user_id, mode, target, request);
        let changed = query.build().execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

/// 在同一 session 锁事务内应用预览草稿并执行条件选择，避免两阶段部分提交。
pub fn apply_preview_patches_and_update_selection_by_query(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
    command: ImportPreviewConditionalSelectionCommand<'_>,
) -> DbResult<ImportPreviewSelectionMutationResult> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        validate_expected_selection_hash_on_tx(
            &mut tx,
            session_db_id,
            user_id,
            command.expected_selection_hash,
        )
        .await?;
        let applied_preview_updates =
            apply_preview_patches_on_tx(&mut tx, session_db_id, user_id, patches).await?;
        if applied_preview_updates != patches.len() {
            return Err(DbError::InvalidOperation(
                "preview patch is outside this session".to_string(),
            ));
        }
        let mut query = build_preview_selection_update_query(
            session_db_id,
            user_id,
            command.mode,
            command.target,
            command.request,
        );
        let updated_selection = query.build().execute(&mut *tx).await?.rows_affected();
        tx.commit().await?;
        Ok(ImportPreviewSelectionMutationResult {
            applied_preview_updates,
            updated_selection: usize::try_from(updated_selection).unwrap_or(usize::MAX),
        })
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
    query.push(", updated_at = now() WHERE p.session_id = ");
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
