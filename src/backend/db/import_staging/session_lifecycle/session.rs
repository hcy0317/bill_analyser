#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearSessionDataResult {
    pub parser_count: usize,
    pub preview_count: usize,
    pub annotation_count: usize,
    pub session_count: usize,
}

const INCOMPLETE_IMPORT_SESSION_RETENTION_LIMIT: i64 = 2;

/// 当前 Postgres 运行态的导入 staging schema 由迁移管理，这里保留初始化入口以维持旧调用方合同。
#[tracing::instrument(level = "debug", skip_all)]
pub fn init_import_staging_schema(_pool: &PostgresPool) -> DbResult<()> {
    Ok(())
}

/// 创建或重建一次导入 session，并通过外部 session key 绑定当前用户的 Postgres session id。
#[tracing::instrument(level = "debug", skip_all)]
pub fn create_import_session(pool: &PostgresPool, draft: &ImportSessionDraft) -> DbResult<i64> {
    block_on_db(create_import_session_async(pool, draft))
}

/// 清理指定用户的全部导入 staging 数据，必须保持 user-scope，避免跨用户删除。
#[tracing::instrument(level = "debug", skip_all)]
pub fn clear_user_import_staging_data(pool: &PostgresPool, user_id: i64) -> DbResult<usize> {
    block_on_db(clear_user_import_staging_data_async(pool, user_id))
}

/// 更新导入 session 的阶段统计与版本号，用于 parse、preview 和 confirm 之间的状态推进。
#[tracing::instrument(level = "debug", skip_all)]
pub fn update_import_session_status(
    pool: &PostgresPool,
    update: &ImportSessionStatusUpdate,
) -> DbResult<bool> {
    block_on_db(async move {
        let user_id = user_id_i64(update.user_id)?;
        let mut tx = pool.begin().await?;
        let (session_db_id, status) =
            lock_import_session_on_tx(&mut tx, &update.session_id, user_id).await?;
        if status == "confirmed" {
            tx.rollback().await?;
            return Ok(false);
        }
        if update.status == "confirmed" {
            return Err(DbError::InvalidOperation(
                "confirmed status requires the canonical confirm command".to_string(),
            ));
        }
        let changed = sqlx::query(
            r#"
            UPDATE import_sessions
            SET status = $1,
                updated_at = now(),
                total_parsed = COALESCE($2, total_parsed),
                total_preview = COALESCE($3, total_preview),
                total_confirmed = COALESCE($4, total_confirmed),
                row_count = COALESCE($2, row_count),
                version = version + 1
            WHERE id = $5 AND user_id = $6
            "#,
        )
        .bind(&update.status)
        .bind(update.total_parsed)
        .bind(update.total_preview)
        .bind(update.total_confirmed)
        .bind(session_db_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(changed > 0)
    })
}

/// 按当前用户读取导入 session 摘要；调用方依赖 None 区分 session 不存在和数据库错误。
#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Option<ImportSessionRow>> {
    block_on_db(get_import_session_async(pool, session_id, user_id))
}

/// 返回当前用户仍可恢复的预览会话，按最近更新时间倒序排列。
#[tracing::instrument(level = "debug", skip_all)]
pub fn list_recoverable_import_sessions(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<ImportSessionRow>> {
    block_on_db(list_recoverable_import_sessions_async(pool, user_id))
}

/// 按 session 清理导入子表数据并返回各子表删除数量，用于取消导入和测试生命周期。
pub fn clear_session_data(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ClearSessionDataResult> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let (session_db_id, status) =
            lock_import_session_on_tx(&mut tx, session_id, user_id).await?;
        if status == "confirmed" {
            tx.rollback().await?;
            return Ok(ClearSessionDataResult {
                parser_count: 0,
                preview_count: 0,
                annotation_count: 0,
                session_count: 0,
            });
        }
        let annotation_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM import_annotation_samples WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?;
        let preview_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .fetch_one(&mut *tx)
        .await?;
        let parser_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM import_standard_rows WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .fetch_one(&mut *tx)
        .await?;
        clear_import_session_child_data_on_tx(&mut tx, user_id, session_db_id, session_id).await?;
        let session_count = sqlx::query(
            "DELETE FROM import_sessions WHERE user_id = $1 AND id = $2 AND status <> 'confirmed'",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok(ClearSessionDataResult {
            parser_count: usize::try_from(parser_count).unwrap_or(usize::MAX),
            preview_count: usize::try_from(preview_count).unwrap_or(usize::MAX),
            annotation_count: usize::try_from(annotation_count).unwrap_or(usize::MAX),
            session_count: usize::try_from(session_count).unwrap_or(usize::MAX),
        })
    })
}

fn block_on_db<T, F>(future: F) -> DbResult<T>
where
    F: Future<Output = DbResult<T>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(DbError::from)?
            .block_on(future)
    }
}

async fn create_import_session_async(
    pool: &PostgresPool,
    draft: &ImportSessionDraft,
) -> DbResult<i64> {
    let mut tx = pool.begin().await?;
    let session_db_id = prepare_import_session_with_retention_on_tx(&mut tx, draft, false)
        .await?
        .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))?;
    tx.commit().await?;
    Ok(session_db_id)
}

async fn prepare_import_session_with_retention_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    draft: &ImportSessionDraft,
    require_existing_session: bool,
) -> DbResult<Option<i64>> {
    let user_id = user_id_i64(draft.user_id)?;
    lock_import_session_retention_owner_on_tx(tx, user_id).await?;
    let session =
        prepare_import_session_for_staging_on_tx(tx, draft, require_existing_session).await?;
    if !require_existing_session {
        prune_incomplete_import_sessions_on_tx(
            tx,
            user_id,
            INCOMPLETE_IMPORT_SESSION_RETENTION_LIMIT,
        )
        .await?;
    }
    Ok(session)
}

async fn lock_import_session_retention_owner_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<()> {
    let owner = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
    if owner.is_none() {
        return Err(DbError::InvalidOperation(
            "import session user not found".to_string(),
        ));
    }
    Ok(())
}

async fn prune_incomplete_import_sessions_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    retained_count: i64,
) -> DbResult<usize> {
    let stale_sessions = sqlx::query(
        r#"
        SELECT id, session_key
        FROM import_sessions
        WHERE user_id = $1 AND status <> 'confirmed'
        ORDER BY updated_at DESC, id DESC
        OFFSET $2
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .bind(retained_count)
    .fetch_all(&mut **tx)
    .await?;

    let mut deleted_count = 0usize;
    for session in stale_sessions {
        let session_db_id = session.try_get::<i64, _>("id")?;
        let session_key = session.try_get::<String, _>("session_key")?;
        clear_import_session_child_data_on_tx(tx, user_id, session_db_id, &session_key).await?;
        let rows_affected = sqlx::query(
            "DELETE FROM import_sessions WHERE id = $1 AND user_id = $2 AND status <> 'confirmed'",
        )
        .bind(session_db_id)
        .bind(user_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
        deleted_count = deleted_count.saturating_add(
            usize::try_from(rows_affected).unwrap_or(usize::MAX),
        );
    }
    Ok(deleted_count)
}

async fn prepare_import_session_for_staging_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    draft: &ImportSessionDraft,
    require_existing_session: bool,
) -> DbResult<Option<i64>> {
    let user_id = user_id_i64(draft.user_id)?;
    let existing = sqlx::query(
        "SELECT id, status FROM import_sessions WHERE session_key = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(&draft.session_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let session_db_id = if let Some(row) = existing {
        if row.try_get::<String, _>("status")? == "confirmed" {
            return Err(DbError::InvalidOperation(
                "confirmed import session cannot be restaged".to_string(),
            ));
        }
        let session_db_id = row.try_get::<i64, _>("id")?;
        if !require_existing_session {
            clear_import_session_child_data_on_tx(
                tx,
                user_id,
                session_db_id,
                &draft.session_id,
            )
            .await?;
            sqlx::query(
                r#"
                UPDATE import_sessions
                SET status = 'parsing',
                    source_count = $3,
                    row_count = 0,
                    file_count = $3,
                    total_parsed = 0,
                    total_preview = 0,
                    total_confirmed = 0,
                    metadata = '{}'::jsonb,
                    updated_at = now(),
                    version = version + 1
                WHERE id = $1 AND user_id = $2
                "#,
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(draft.file_count)
            .execute(&mut **tx)
            .await?;
        }
        session_db_id
    } else if require_existing_session {
        return Ok(None);
    } else {
        sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO import_sessions (
                user_id, session_key, status, import_mode, source_count, row_count,
                file_count, total_parsed, total_preview, total_confirmed, metadata,
                created_at, updated_at
            ) VALUES ($1,$2,'parsing','preview',$3,0,$3,0,0,0,'{}'::jsonb,now(),now())
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(&draft.session_id)
        .bind(draft.file_count)
        .fetch_one(&mut **tx)
        .await?
    };
    Ok(Some(session_db_id))
}

async fn lock_active_import_session_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_id: &str,
    user_id: i64,
) -> DbResult<i64> {
    let (session_db_id, status) = lock_import_session_on_tx(tx, session_id, user_id).await?;
    if status == "confirmed" {
        return Err(DbError::InvalidOperation(
            "confirmed import session is terminal".to_string(),
        ));
    }
    Ok(session_db_id)
}

async fn lock_import_session_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_id: &str,
    user_id: i64,
) -> DbResult<(i64, String)> {
    let row = sqlx::query(
        "SELECT id, status FROM import_sessions WHERE session_key = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))?;
    Ok((row.try_get("id")?, row.try_get("status")?))
}

async fn refresh_import_session_counters_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<()> {
    let changed = sqlx::query(
        r#"
        UPDATE import_sessions
        SET row_count = (
                SELECT COUNT(*)::BIGINT FROM import_standard_rows
                WHERE session_id = $1 AND user_id = $2
            ),
            total_parsed = (
                SELECT COUNT(*)::BIGINT FROM import_standard_rows
                WHERE session_id = $1 AND user_id = $2
            ),
            total_preview = (
                SELECT COUNT(*)::BIGINT FROM import_preview_rows
                WHERE session_id = $1 AND user_id = $2
            ),
            updated_at = now(),
            version = version + 1
        WHERE id = $1 AND user_id = $2 AND status <> 'confirmed'
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(DbError::InvalidOperation(
            "confirmed import session is terminal".to_string(),
        ));
    }
    Ok(())
}
