#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearSessionDataResult {
    pub parser_count: usize,
    pub preview_count: usize,
    pub annotation_count: usize,
    pub session_count: usize,
}

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
            WHERE session_key = $5 AND user_id = $6
            "#,
        )
        .bind(&update.status)
        .bind(update.total_parsed)
        .bind(update.total_preview)
        .bind(update.total_confirmed)
        .bind(&update.session_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
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

/// 按 session 清理导入子表数据并返回各子表删除数量，用于取消导入和测试生命周期。
pub fn clear_session_data(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ClearSessionDataResult> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let annotation_count = sqlx::query(
            "DELETE FROM import_annotation_samples WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_id)
        .execute(pool)
        .await?
        .rows_affected();
        let preview_count =
            sqlx::query("DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = $2")
                .bind(user_id)
                .bind(session_db_id)
                .execute(pool)
                .await?
                .rows_affected();
        let parser_count =
            sqlx::query("DELETE FROM import_standard_rows WHERE user_id = $1 AND session_id = $2")
                .bind(user_id)
                .bind(session_db_id)
                .execute(pool)
                .await?
                .rows_affected();
        sqlx::query("DELETE FROM import_sources WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?;
        let session_count =
            sqlx::query("DELETE FROM import_sessions WHERE user_id = $1 AND id = $2")
                .bind(user_id)
                .bind(session_db_id)
                .execute(pool)
                .await?
                .rows_affected();
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
    let user_id = user_id_i64(draft.user_id)?;
    let row = sqlx::query(
        r#"
        INSERT INTO import_sessions (
            user_id, session_key, status, import_mode, source_count, row_count,
            file_count, total_parsed, total_preview, total_confirmed, metadata,
            created_at, updated_at
        ) VALUES ($1,$2,'parsing','preview',$3,0,$3,0,0,0,'{}'::jsonb,now(),now())
        ON CONFLICT (user_id, session_key) DO UPDATE SET
            status = 'parsing',
            source_count = excluded.source_count,
            row_count = 0,
            file_count = excluded.file_count,
            total_parsed = 0,
            total_preview = 0,
            total_confirmed = 0,
            metadata = '{}'::jsonb,
            updated_at = now(),
            version = import_sessions.version + 1
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&draft.session_id)
    .bind(draft.file_count)
    .fetch_one(pool)
    .await?;
    let session_db_id = row.try_get("id").map_err(DbError::from)?;
    clear_import_session_child_data_async(pool, user_id, session_db_id, &draft.session_id).await?;
    Ok(session_db_id)
}

async fn clear_import_session_child_data_async(
    pool: &PostgresPool,
    user_id: i64,
    session_db_id: i64,
    session_key: &str,
) -> DbResult<usize> {
    let annotation_count =
        sqlx::query("DELETE FROM import_annotation_samples WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_key)
            .execute(pool)
            .await?
            .rows_affected();
    let feedback_event_count = sqlx::query(
        r#"
        UPDATE import_learning_feedback_events
        SET suggestion_id = NULL
        WHERE user_id = $1
          AND suggestion_id IN (
              SELECT id
              FROM import_learning_suggestions
              WHERE user_id = $1 AND session_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(pool)
    .await?
    .rows_affected();
    let learning_suggestion_count = sqlx::query(
        "DELETE FROM import_learning_suggestions WHERE user_id = $1 AND session_id = $2",
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(pool)
    .await?
    .rows_affected();
    let learning_sample_count = sqlx::query(
        r#"
        UPDATE import_learning_samples
        SET preview_row_id = NULL,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND preview_row_id IN (
              SELECT id
              FROM import_preview_rows
              WHERE user_id = $1 AND session_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(pool)
    .await?
    .rows_affected();
    let matching_feedback_count =
        sqlx::query("DELETE FROM preview_matching_feedback WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?
            .rows_affected();
    let confirm_count =
        sqlx::query("DELETE FROM import_confirm_operations WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?
            .rows_affected();
    let group_member_count = sqlx::query(
        r#"
        DELETE FROM import_decision_group_members members
        USING import_decision_groups groups
        WHERE members.group_id = groups.id
          AND groups.user_id = $1
          AND groups.session_id = $2
        "#,
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(pool)
    .await?
    .rows_affected();
    let group_count =
        sqlx::query("DELETE FROM import_decision_groups WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?
            .rows_affected();
    let history_count = sqlx::query(
        "DELETE FROM import_history_materializations WHERE user_id = $1 AND session_id = $2",
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(pool)
    .await?
    .rows_affected();
    let preview_count =
        sqlx::query("DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?
            .rows_affected();
    let parser_count =
        sqlx::query("DELETE FROM import_standard_rows WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?
            .rows_affected();
    let source_count =
        sqlx::query("DELETE FROM import_sources WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?
            .rows_affected();

    Ok(usize::try_from(
        annotation_count
            + feedback_event_count
            + learning_suggestion_count
            + learning_sample_count
            + matching_feedback_count
            + confirm_count
            + group_member_count
            + group_count
            + history_count
            + preview_count
            + parser_count
            + source_count,
    )
    .unwrap_or(usize::MAX))
}

async fn clear_user_import_staging_data_async(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<usize> {
    let session_ids = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM import_sessions WHERE user_id = $1 ORDER BY id ASC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let annotation_count = sqlx::query("DELETE FROM import_annotation_samples WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();

    if session_ids.is_empty() {
        return Ok(usize::try_from(annotation_count).unwrap_or(usize::MAX));
    }

    let feedback_event_count = sqlx::query(
        r#"
        UPDATE import_learning_feedback_events
        SET suggestion_id = NULL
        WHERE user_id = $1
          AND suggestion_id IN (
              SELECT id
              FROM import_learning_suggestions
              WHERE user_id = $1 AND session_id = ANY($2)
          )
        "#,
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let learning_suggestion_count = sqlx::query(
        "DELETE FROM import_learning_suggestions WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let learning_sample_count = sqlx::query(
        r#"
        UPDATE import_learning_samples
        SET preview_row_id = NULL,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND preview_row_id IN (
              SELECT id
              FROM import_preview_rows
              WHERE user_id = $1 AND session_id = ANY($2)
          )
        "#,
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let matching_feedback_count = sqlx::query(
        "DELETE FROM preview_matching_feedback WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let confirm_count = sqlx::query(
        "DELETE FROM import_confirm_operations WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let group_member_count = sqlx::query(
        r#"
        DELETE FROM import_decision_group_members members
        USING import_decision_groups groups
        WHERE members.group_id = groups.id
          AND groups.user_id = $1
          AND groups.session_id = ANY($2)
        "#,
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let group_count = sqlx::query(
        "DELETE FROM import_decision_groups WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let history_count = sqlx::query(
        "DELETE FROM import_history_materializations WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(pool)
    .await?
    .rows_affected();
    let preview_count =
        sqlx::query("DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(pool)
            .await?
            .rows_affected();
    let parser_count =
        sqlx::query("DELETE FROM import_standard_rows WHERE user_id = $1 AND session_id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(pool)
            .await?
            .rows_affected();
    let source_count =
        sqlx::query("DELETE FROM import_sources WHERE user_id = $1 AND session_id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(pool)
            .await?
            .rows_affected();
    let session_count =
        sqlx::query("DELETE FROM import_sessions WHERE user_id = $1 AND id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(pool)
            .await?
            .rows_affected();

    Ok(usize::try_from(
        annotation_count
            + feedback_event_count
            + learning_suggestion_count
            + learning_sample_count
            + matching_feedback_count
            + confirm_count
            + group_member_count
            + group_count
            + history_count
            + preview_count
            + parser_count
            + source_count
            + session_count,
    )
    .unwrap_or(usize::MAX))
}

async fn get_import_session_async(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Option<ImportSessionRow>> {
    let row = sqlx::query(
        r#"
        SELECT *
        FROM import_sessions
        WHERE session_key = $1 AND user_id = $2
        "#,
    )
    .bind(session_id)
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await?;
    row.as_ref().map(import_session_from_pg_row).transpose()
}

async fn session_db_id(pool: &PostgresPool, session_id: &str, user_id: UserId) -> DbResult<i64> {
    sqlx::query_scalar::<_, Option<i64>>(
        "SELECT id FROM import_sessions WHERE session_key = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(user_id_i64(user_id)?)
    .fetch_one(pool)
    .await?
    .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}
