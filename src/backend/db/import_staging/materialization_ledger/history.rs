/// 清理可重建的 preview materialization 数据，必须在重新 stage2 前执行以避免旧候选残留。
pub fn clear_import_preview_materialization_state(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let feedback_events = sqlx::query(
            r#"
            UPDATE import_learning_feedback_events
            SET suggestion_id = NULL
            WHERE user_id = $1
              AND suggestion_id IN (
                  SELECT id FROM import_learning_suggestions
                  WHERE user_id = $1 AND session_id = $2
              )
            "#,
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let learning = sqlx::query(
            "DELETE FROM import_learning_suggestions WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let matching = sqlx::query(
            "DELETE FROM preview_matching_feedback WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let confirm = sqlx::query(
            "DELETE FROM import_confirm_operations WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let group_members = sqlx::query(
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
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let groups = sqlx::query(
            "DELETE FROM import_decision_groups WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let history = sqlx::query(
            "DELETE FROM import_history_materializations WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        let previews = sqlx::query(
            "DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_db_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(usize::try_from(
            feedback_events
                + learning
                + matching
                + confirm
                + group_members
                + groups
                + history
                + previews,
        )
        .unwrap_or(usize::MAX))
    })
}

/// 批量写入历史重复/转账 materialization 证据，供 preview 与 confirm 审核链路复用。
pub fn insert_import_history_materializations_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportHistoryMaterializationDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        if drafts.is_empty() {
            tx.commit().await?;
            return Ok(0);
        }
        let mut inserted = 0usize;
        for draft in drafts {
            let payload = canonical_history_materialization_payload(&draft.materialized_payload)?;
            let row = sqlx::query(
                r#"
                INSERT INTO import_history_materializations (
                    session_id, user_id, history_bill_id, history_bill_version,
                    materialized_payload, rewrite_reason, created_at
                )
                SELECT $1, $2, bill.id, bill.version, $5::jsonb, $6, now()
                FROM bills bill
                WHERE bill.user_id = $2
                  AND bill.id = $3
                  AND bill.version = $4
                  AND bill.is_deleted = false
                ON CONFLICT (session_id, history_bill_id) DO UPDATE SET
                    history_bill_version = excluded.history_bill_version,
                    materialized_payload = excluded.materialized_payload,
                    rewrite_reason = excluded.rewrite_reason
                RETURNING id
                "#,
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(draft.history_bill_id)
            .bind(draft.history_bill_version)
            .bind(payload.to_string())
            .bind(&draft.rewrite_reason)
            .fetch_optional(&mut *tx)
            .await?;
            if row.is_none() {
                return Err(DbError::InvalidOperation(format!(
                    "history materialization CAS mismatch: bill {} version {}",
                    draft.history_bill_id, draft.history_bill_version
                )));
            }
            inserted += 1;
        }
        refresh_import_session_counters_on_tx(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(inserted)
    })
}

pub fn get_import_history_materializations_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportHistoryMaterializationRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT materialization.*, session.session_key
            FROM import_history_materializations materialization
            JOIN import_sessions session ON session.id = materialization.session_id
            WHERE materialization.session_id = $1 AND materialization.user_id = $2
            ORDER BY materialization.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter()
            .map(|row| {
                Ok(ImportHistoryMaterializationRow {
                    id: row.try_get("id")?,
                    session_id: row.try_get("session_key")?,
                    user_id: row.try_get("user_id")?,
                    history_bill_id: row.try_get("history_bill_id")?,
                    history_bill_version: row.try_get("history_bill_version")?,
                    materialized_payload: row.try_get("materialized_payload")?,
                    rewrite_reason: row.try_get("rewrite_reason")?,
                    created_at: format_pg_time(row.try_get("created_at")?),
                })
            })
            .collect()
    })
}

pub fn get_import_history_candidate_bills_for_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportHistoryBillRow>> {
    block_on_db(async move {
        let _session_db_id = active_session_db_id(pool, session_id, user_id_i64(user_id)?).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT bill.*, category.path AS category_path, account.name AS account_name,
                   target.name AS target_account_name
            FROM bills bill
            LEFT JOIN categories category
              ON category.id = bill.category_id AND category.user_id = bill.user_id
            LEFT JOIN accounts account
              ON account.id = COALESCE(bill.source_account_id, bill.account_id)
             AND account.user_id = bill.user_id
            LEFT JOIN accounts target
              ON target.id = COALESCE(bill.target_account_id, bill.transfer_target_account_id)
             AND target.user_id = bill.user_id
            WHERE bill.user_id = $1 AND bill.is_deleted = false
            ORDER BY bill.occurred_at ASC, bill.id ASC
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter().map(history_bill_from_pg_row).collect()
    })
}

fn canonical_history_materialization_payload(payload: &Value) -> DbResult<Value> {
    let mut payload = payload.clone();
    let Some(object) = payload.as_object_mut() else {
        return Err(DbError::InvalidOperation(
            "history materialization payload must be an object".to_string(),
        ));
    };
    let operation_key = if object.contains_key("planned_operation") {
        "planned_operation"
    } else if object.contains_key("operation") {
        "operation"
    } else {
        return Err(DbError::InvalidOperation(
            "history materialization operation is required".to_string(),
        ));
    };
    let raw_operation = object
        .get(operation_key)
        .and_then(Value::as_str)
        .unwrap_or_default();
    let operation = normalize_history_operation(raw_operation).ok_or_else(|| {
        DbError::InvalidOperation("invalid history materialization operation".to_string())
    })?;
    object.insert(
        operation_key.to_string(),
        Value::String(operation.as_str().to_string()),
    );
    Ok(payload)
}

fn history_bill_from_pg_row(row: &PgRow) -> DbResult<ImportHistoryBillRow> {
    let history_bill_id = row.try_get::<i64, _>("id")?;
    let history_bill_version = row.try_get::<i64, _>("version")?;
    let amount_cents = row.try_get::<i64, _>("amount_cents")?;
    let direction = row.try_get::<String, _>("direction")?;
    let signed_amount_cents = if direction == "expense" {
        -amount_cents.abs()
    } else {
        amount_cents.abs()
    };
    let category_path = row
        .try_get::<Option<String>, _>("category_path")?
        .unwrap_or_default();
    let (main_category, sub_category) = category_path
        .split_once('/')
        .map(|(main, sub)| (main.to_string(), sub.to_string()))
        .unwrap_or_else(|| (category_path.clone(), String::new()));
    let source_account_id = row
        .try_get::<Option<i64>, _>("source_account_id")?
        .or(row.try_get::<Option<i64>, _>("account_id")?);
    let destination_account_id = row
        .try_get::<Option<i64>, _>("target_account_id")?
        .or(row.try_get::<Option<i64>, _>("transfer_target_account_id")?);
    let snapshot = json!({
        "id": history_bill_id,
        "version": history_bill_version,
        "occurred_at": format_pg_time(row.try_get("occurred_at")?),
        "amount_cents": amount_cents,
        "direction": direction,
        "transaction_type": row.try_get::<String, _>("transaction_type")?,
        "source_account_id": source_account_id,
        "destination_account_id": destination_account_id,
        "category_id": row.try_get::<Option<i64>, _>("category_id")?,
        "merchant": row.try_get::<Option<String>, _>("merchant")?,
        "payment_method": row.try_get::<Option<String>, _>("payment_method")?,
        "description": row.try_get::<Option<String>, _>("description")?,
    });
    Ok(ImportHistoryBillRow {
        history_bill_id,
        history_bill_version,
        bill: DedupBill {
            id: Some(history_bill_id.to_string()),
            date: snapshot["occurred_at"].as_str().unwrap_or_default().to_string(),
            amount: Money::from_cents(signed_amount_cents),
            transaction_type: snapshot["transaction_type"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            source_account_id: source_account_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            destination_account_id: destination_account_id.map(|value| value.to_string()),
            parser_id: row
                .try_get::<Option<String>, _>("parser_name")?
                .unwrap_or_else(|| "history_db".to_string()),
            source: "postgres".to_string(),
            counterparty: row
                .try_get::<Option<String>, _>("merchant")?
                .unwrap_or_default(),
            payment_method: row
                .try_get::<Option<String>, _>("payment_method")?
                .unwrap_or_default(),
            description: row
                .try_get::<Option<String>, _>("description")?
                .unwrap_or_default(),
            main_category,
            sub_category,
            account_name: row
                .try_get::<Option<String>, _>("account_name")?
                .unwrap_or_default(),
            destination_account_name: row
                .try_get::<Option<String>, _>("target_account_name")?
                .unwrap_or_default(),
            ..DedupBill::default()
        },
        snapshot,
    })
}
