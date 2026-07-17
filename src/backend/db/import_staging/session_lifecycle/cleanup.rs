async fn clear_import_session_child_data_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    session_db_id: i64,
    session_key: &str,
) -> DbResult<usize> {
    let annotation_count =
        sqlx::query("DELETE FROM import_annotation_samples WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_key)
            .execute(&mut **tx)
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
    .execute(&mut **tx)
    .await?
    .rows_affected();
    let learning_suggestion_count = sqlx::query(
        "DELETE FROM import_learning_suggestions WHERE user_id = $1 AND session_id = $2",
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(&mut **tx)
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
    .execute(&mut **tx)
    .await?
    .rows_affected();
    let matching_feedback_count =
        sqlx::query("DELETE FROM preview_matching_feedback WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(&mut **tx)
            .await?
            .rows_affected();
    let confirm_count =
        sqlx::query("DELETE FROM import_confirm_operations WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(&mut **tx)
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
    .execute(&mut **tx)
    .await?
    .rows_affected();
    let group_count =
        sqlx::query("DELETE FROM import_decision_groups WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(&mut **tx)
            .await?
            .rows_affected();
    let history_count = sqlx::query(
        "DELETE FROM import_history_materializations WHERE user_id = $1 AND session_id = $2",
    )
    .bind(user_id)
    .bind(session_db_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    let preview_count =
        sqlx::query("DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(&mut **tx)
            .await?
            .rows_affected();
    let parser_count =
        sqlx::query("DELETE FROM import_standard_rows WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(&mut **tx)
            .await?
            .rows_affected();
    let source_count =
        sqlx::query("DELETE FROM import_sources WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(&mut **tx)
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
    let mut tx = pool.begin().await?;
    let session_ids = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM import_sessions WHERE user_id = $1 ORDER BY id ASC FOR UPDATE",
    )
    .bind(user_id)
    .fetch_all(&mut *tx)
    .await?;
    let annotation_count = sqlx::query("DELETE FROM import_annotation_samples WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

    if session_ids.is_empty() {
        tx.commit().await?;
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
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let learning_suggestion_count = sqlx::query(
        "DELETE FROM import_learning_suggestions WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(&mut *tx)
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
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let matching_feedback_count = sqlx::query(
        "DELETE FROM preview_matching_feedback WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let confirm_count = sqlx::query(
        "DELETE FROM import_confirm_operations WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(&mut *tx)
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
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let group_count = sqlx::query(
        "DELETE FROM import_decision_groups WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let history_count = sqlx::query(
        "DELETE FROM import_history_materializations WHERE user_id = $1 AND session_id = ANY($2)",
    )
    .bind(user_id)
    .bind(&session_ids)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    let preview_count =
        sqlx::query("DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    let parser_count =
        sqlx::query("DELETE FROM import_standard_rows WHERE user_id = $1 AND session_id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    let source_count =
        sqlx::query("DELETE FROM import_sources WHERE user_id = $1 AND session_id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();
    let session_count =
        sqlx::query("DELETE FROM import_sessions WHERE user_id = $1 AND id = ANY($2)")
            .bind(user_id)
            .bind(&session_ids)
            .execute(&mut *tx)
            .await?
            .rows_affected();

    let cleared = usize::try_from(
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
    .unwrap_or(usize::MAX);
    tx.commit().await?;
    Ok(cleared)
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

async fn active_session_db_id(
    pool: &PostgresPool,
    session_id: &str,
    user_id: i64,
) -> DbResult<i64> {
    let row = sqlx::query(
        "SELECT id, status FROM import_sessions WHERE session_key = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))?;
    if row.try_get::<String, _>("status")? == "confirmed" {
        return Err(DbError::InvalidOperation(
            "confirmed import session is terminal".to_string(),
        ));
    }
    row.try_get("id").map_err(DbError::from)
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}
