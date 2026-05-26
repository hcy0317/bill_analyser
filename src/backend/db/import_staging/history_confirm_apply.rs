struct HistoryRewriteApplyMeta<'a> {
    session_id: &'a str,
    user_id: i64,
    now: &'a str,
    batch_id: &'a str,
}

fn apply_history_rewrite_operation(
    tx: &rusqlite::Transaction<'_>,
    preview: &ImportPreviewRow,
    operation: &PlannedHistoryRewriteOperation,
    meta: &HistoryRewriteApplyMeta<'_>,
    touched_account_ids: &mut Vec<i64>,
) -> DbResult<()> {
    let mut created_bill_id = None;
    let mut deleted_bill_id = None;
    collect_history_bill_account_ids(
        tx,
        meta.user_id,
        operation.history_bill_id,
        touched_account_ids,
    )?;
    if operation.planned_operation == "merge_transfer_history"
        && operation.history_role.eq_ignore_ascii_case("incoming")
    {
        let new_bill_id =
            insert_history_transfer_base_from_preview(tx, meta.user_id, preview, meta.now, meta.batch_id)?;
        copy_bill_tags(tx, operation.history_bill_id, new_bill_id, meta.now)?;
        tx.execute(
            "DELETE FROM bills WHERE id = ?1 AND user_id = ?2 AND COALESCE(import_history_id, 1) = ?3",
            params![
                operation.history_bill_id,
                meta.user_id,
                operation.history_bill_version
            ],
        )?;
        if tx.changes() == 0 {
            return Err(DbError::InvalidOperation(
                "history bill version is stale".to_string(),
            ));
        }
        delete_matching_suppressions_for_bill(tx, meta.user_id, operation.history_bill_id)?;
        created_bill_id = Some(new_bill_id);
        deleted_bill_id = Some(operation.history_bill_id);
        collect_preview_account_ids(preview, touched_account_ids);
    } else {
        update_history_bill_from_preview(
            tx,
            meta.user_id,
            preview,
            operation,
            meta.now,
            meta.batch_id,
        )?;
        collect_preview_account_ids(preview, touched_account_ids);
    }
    store_import_confirm_operation(
        tx,
        meta.session_id,
        meta.user_id,
        operation,
        created_bill_id,
        deleted_bill_id,
        meta.now,
    )?;
    Ok(())
}

fn update_history_bill_from_preview(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    preview: &ImportPreviewRow,
    operation: &PlannedHistoryRewriteOperation,
    now: &str,
    batch_id: &str,
) -> DbResult<()> {
    let bill_type = normalize_confirm_bill_type(&preview.preview_type);
    let preview_date_text = normalize_bill_date_text(&preview.preview_date);
    let amount = confirm_amount_for_type(&bill_type, preview.preview_amount);
    let bill_hash = calculate_import_bill_hash(
        &preview_date_text,
        &bill_type,
        amount,
        &preview.preview_counterparty,
        &preview.preview_description,
    );
    tx.execute(
        "
        UPDATE bills
        SET date = ?1, type = ?2, amount = ?3, counterparty = ?4, description = ?5,
            payment_method = ?6, main_category = ?7, sub_category = ?8,
            source_account_id = ?9, destination_account_id = ?10,
            destination_amount = ?11, batch_id = ?12, hash = ?13,
            import_history_id = ?14, updated_at = ?15
        WHERE id = ?16 AND user_id = ?17 AND COALESCE(import_history_id, 1) = ?18
        ",
        params![
            preview_date_text,
            bill_type,
            amount,
            preview.preview_counterparty,
            preview.preview_description,
            preview.preview_payment_method,
            preview.preview_main_category,
            preview.preview_sub_category,
            preview.preview_source_account_id,
            preview.preview_destination_account_id,
            preview.preview_destination_amount,
            batch_id,
            bill_hash,
            operation.history_bill_version + 1,
            now,
            operation.history_bill_id,
            user_id,
            operation.history_bill_version,
        ],
    )?;
    if tx.changes() == 0 {
        return Err(DbError::InvalidOperation(
            "history bill version is stale".to_string(),
        ));
    }
    Ok(())
}

fn insert_history_transfer_base_from_preview(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    preview: &ImportPreviewRow,
    now: &str,
    batch_id: &str,
) -> DbResult<i64> {
    let bill_type = normalize_confirm_bill_type(&preview.preview_type);
    let preview_date_text = normalize_bill_date_text(&preview.preview_date);
    let amount = confirm_amount_for_type(&bill_type, preview.preview_amount);
    let bill_hash = calculate_import_bill_hash(
        &preview_date_text,
        &bill_type,
        amount,
        &preview.preview_counterparty,
        &preview.preview_description,
    );
    tx.execute(
        "
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category,
            source_account_id, destination_account_id, destination_amount,
            batch_id, hash, created_from_recurring, created_at, updated_at,
            import_history_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
        ",
        params![
            user_id,
            preview_date_text,
            bill_type,
            amount,
            preview.preview_counterparty,
            preview.preview_description,
            preview.preview_payment_method,
            preview.preview_main_category,
            preview.preview_sub_category,
            preview.preview_source_account_id,
            preview.preview_destination_account_id,
            preview.preview_destination_amount,
            batch_id,
            bill_hash,
            preview.preview_recurring_id,
            now,
            now,
            1_i64,
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

fn store_import_confirm_operation(
    tx: &rusqlite::Transaction<'_>,
    session_id: &str,
    user_id: i64,
    operation: &PlannedHistoryRewriteOperation,
    created_bill_id: Option<i64>,
    deleted_bill_id: Option<i64>,
    now: &str,
) -> DbResult<()> {
    let payload_json = serde_json::to_string(&serde_json::json!({
        "operation_id": operation.operation_id,
        "acknowledgement_token": operation.acknowledgement_token,
        "history_bill_version": operation.history_bill_version,
        "history_role": operation.history_role,
        "materialization": operation.materialization.materialized_payload,
        "rewrite_reason": operation.materialization.rewrite_reason,
    }))
    .unwrap_or_else(|_| "{}".to_string());
    tx.execute(
        "
        INSERT INTO import_confirm_operations (
            session_id, user_id, operation_kind, preview_row_id, history_bill_id,
            created_bill_id, deleted_bill_id, status, payload_json, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'applied', ?8, ?9, ?9)
        ON CONFLICT(session_id, user_id, operation_kind, preview_row_id, history_bill_id)
        DO UPDATE SET
            created_bill_id = excluded.created_bill_id,
            deleted_bill_id = excluded.deleted_bill_id,
            status = excluded.status,
            payload_json = excluded.payload_json,
            updated_at = excluded.updated_at
        ",
        params![
            session_id,
            user_id,
            operation.planned_operation,
            operation.preview_id,
            operation.history_bill_id,
            created_bill_id,
            deleted_bill_id,
            payload_json,
            now,
        ],
    )?;
    Ok(())
}

fn collect_preview_account_ids(preview: &ImportPreviewRow, account_ids: &mut Vec<i64>) {
    for id in [
        preview.preview_source_account_id,
        preview.preview_destination_account_id,
    ]
    .into_iter()
    .flatten()
    .filter(|id| *id > 0)
    {
        if !account_ids.contains(&id) {
            account_ids.push(id);
        }
    }
}

fn collect_history_bill_account_ids(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    bill_id: i64,
    account_ids: &mut Vec<i64>,
) -> DbResult<()> {
    let row = tx
        .query_row(
            "SELECT source_account_id, destination_account_id FROM bills WHERE id = ?1 AND user_id = ?2",
            params![bill_id, user_id],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()?;
    if let Some((source, destination)) = row {
        for id in [source, destination].into_iter().flatten().filter(|id| *id > 0) {
            if !account_ids.contains(&id) {
                account_ids.push(id);
            }
        }
    }
    Ok(())
}

fn copy_bill_tags(
    tx: &rusqlite::Transaction<'_>,
    old_bill_id: i64,
    new_bill_id: i64,
    now: &str,
) -> DbResult<()> {
    if !confirm_table_exists(tx, "bill_tags")? {
        return Ok(());
    }
    tx.execute(
        "
        INSERT OR IGNORE INTO bill_tags(bill_id, tag_id, created_at)
        SELECT ?1, tag_id, ?2 FROM bill_tags WHERE bill_id = ?3
        ",
        params![new_bill_id, now, old_bill_id],
    )?;
    Ok(())
}

fn delete_matching_suppressions_for_bill(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    bill_id: i64,
) -> DbResult<()> {
    for table in [
        "bill_pair_links",
        "bill_transfer_pair_suppressions",
        "bill_investment_pair_suppressions",
        "bill_duplicate_pair_suppressions",
    ] {
        if confirm_table_exists(tx, table)? {
            tx.execute(
                &format!(
                    "DELETE FROM {table} WHERE user_id = ?1 AND (left_bill_id = ?2 OR right_bill_id = ?2)"
                ),
                params![user_id, bill_id],
            )?;
        }
    }
    if confirm_table_exists(tx, "bill_learning_rule_suppressions")? {
        tx.execute(
            "DELETE FROM bill_learning_rule_suppressions WHERE user_id = ?1 AND bill_id = ?2",
            params![user_id, bill_id],
        )?;
    }
    Ok(())
}

fn sync_confirm_account_balances(
    tx: &rusqlite::Transaction<'_>,
    user_id: i64,
    account_ids: &[i64],
    now: &str,
) -> DbResult<()> {
    if account_ids.is_empty()
        || !confirm_table_exists(tx, "accounts")?
        || !confirm_column_exists(tx, "accounts", "balance")?
        || !confirm_column_exists(tx, "accounts", "initial_balance")?
    {
        return Ok(());
    }
    let mut unique_ids = Vec::<i64>::new();
    for id in account_ids.iter().copied().filter(|id| *id > 0) {
        if !unique_ids.contains(&id) {
            unique_ids.push(id);
        }
    }
    for account_id in unique_ids {
        let initial_balance: f64 = tx
            .query_row(
                "SELECT COALESCE(initial_balance, 0) FROM accounts WHERE id = ?1 AND user_id = ?2",
                params![account_id, user_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or_default();
        let bill_delta: f64 = tx.query_row(
            "
            SELECT COALESCE(SUM(
                CASE
                    WHEN source_account_id = ?1 AND lower(type) IN ('转账', 'transfer', '投资', 'investment')
                        THEN -ABS(amount)
                    WHEN source_account_id = ?1 THEN amount
                    ELSE 0
                END
                +
                CASE
                    WHEN destination_account_id = ?1 AND lower(type) IN ('转账', 'transfer', '投资', 'investment')
                        THEN ABS(CASE WHEN COALESCE(destination_amount, 0) = 0 THEN amount ELSE destination_amount END)
                    ELSE 0
                END
            ), 0)
            FROM bills
            WHERE user_id = ?2 AND (source_account_id = ?1 OR destination_account_id = ?1)
            ",
            params![account_id, user_id],
            |row| row.get(0),
        )?;
        tx.execute(
            "UPDATE accounts SET balance = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            params![initial_balance + bill_delta, now, account_id, user_id],
        )?;
    }
    Ok(())
}

fn confirm_table_exists(tx: &rusqlite::Transaction<'_>, table_name: &str) -> DbResult<bool> {
    tx.query_row(
        "SELECT 1 FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1 LIMIT 1",
        params![table_name],
        |_| Ok(()),
    )
    .optional()
    .map(|value| value.is_some())
    .map_err(DbError::from)
}

fn confirm_column_exists(
    tx: &rusqlite::Transaction<'_>,
    table_name: &str,
    column_name: &str,
) -> DbResult<bool> {
    let mut statement = tx.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}
