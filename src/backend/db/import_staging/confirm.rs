/// 兼容无历史改写 ack 的 confirm 入口，实际写入委托给带 ack 的事务实现。
pub fn confirm_preview_to_bills(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ConfirmPreviewResult> {
    confirm_preview_to_bills_with_ack(pool, session_id, user_id, None)
}

/// 在单个事务内锁定 selected preview、校验身份/ack 并创建正式账单，是导入链路最终提交点。
pub fn confirm_preview_to_bills_with_ack(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    _history_acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> DbResult<ConfirmPreviewResult> {
    block_on_db(async move {
        let user_id_i64 = user_id_i64(user_id)?;
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let mut tx = pool.begin().await?;
        let previews =
            load_selected_preview_rows_for_confirm(&mut tx, session_db_id, user_id_i64).await?;
        let mut result = ConfirmPreviewResult {
            confirmed_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            errors: Vec::new(),
        };
        let identity_maps = load_import_identity_maps_for_confirm(&mut tx, user_id_i64).await?;
        let identity_errors = previews
            .iter()
            .flat_map(|preview| preview_identity_error_messages(preview, &identity_maps))
            .collect::<Vec<_>>();
        if !identity_errors.is_empty() {
            return Err(DbError::InvalidOperation(format!(
                "import preview identity validation failed: {}",
                identity_errors.join("; ")
            )));
        }
        let review_errors = previews
            .iter()
            .filter(|preview| preview_requires_review(preview))
            .map(|preview| format!("preview {} requires review before confirm", preview.id))
            .collect::<Vec<_>>();
        if !review_errors.is_empty() {
            return Err(DbError::InvalidOperation(format!(
                "import preview requires review: {}",
                review_errors.join("; ")
            )));
        }
        let drafts = previews
            .iter()
            .map(|preview| BillCreateDraft {
                fields: bill_create_fields_from_preview(preview),
                tag_ids: Vec::new(),
            })
            .collect::<Vec<_>>();
        let created_bill_ids =
            batch_create_postgres_bills_in_transaction(pool, &mut tx, user_id_i64, &drafts).await?;
        result.confirmed_count = created_bill_ids.len();
        let session_updated = sqlx::query(
            r#"
            UPDATE import_sessions
            SET status = $3,
                total_confirmed = $4,
                updated_at = now(),
                version = version + 1
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64)
        .bind("confirmed")
        .bind(i64::try_from(result.confirmed_count).unwrap_or(i64::MAX))
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if session_updated != 1 {
            return Err(DbError::InvalidOperation(format!(
                "import session not found: {session_id}"
            )));
        }
        tx.commit().await?;
        Ok(result)
    })
}

async fn load_selected_preview_rows_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<Vec<ImportPreviewRow>> {
    let rows = sqlx::query(
        r#"
        SELECT p.*, s.session_key
        FROM import_preview_rows p
        JOIN import_sessions s ON s.id = p.session_id
        WHERE p.session_id = $1 AND p.user_id = $2 AND p.selected = true
        ORDER BY p.occurred_at ASC, p.id ASC
        FOR UPDATE OF p
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(preview_from_pg_row).collect()
}

async fn load_import_identity_maps_for_confirm(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(&mut **tx)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut maps = ImportIdentityMaps::default();
    for row in account_rows {
        maps.active_accounts.insert(row.try_get("id")?);
    }
    for row in category_rows {
        let id = row.try_get::<i64, _>("id")?;
        let category_type = row
            .try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .and_then(preview_category_type_code);
        maps.active_categories.insert(id, category_type);
    }
    Ok(maps)
}

fn bill_create_fields_from_preview(preview: &ImportPreviewRow) -> BillRecord {
    let mut fields = Map::new();
    fields.insert("date".to_string(), json!(preview.preview_date));
    fields.insert("type".to_string(), json!(preview.preview_type));
    fields.insert(
        "amount_cents".to_string(),
        json!(preview.preview_amount_cents),
    );
    fields.insert(
        "destination_amount_cents".to_string(),
        json!(preview.preview_destination_amount_cents),
    );
    fields.insert(
        "counterparty".to_string(),
        json!(preview.preview_counterparty),
    );
    fields.insert(
        "description".to_string(),
        json!(preview.preview_description),
    );
    fields.insert(
        "payment_method".to_string(),
        json!(preview.preview_payment_method),
    );
    if let Some(value) = preview.category_id {
        fields.insert("category_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_source_account_id {
        fields.insert("source_account_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_destination_account_id {
        fields.insert("destination_account_id".to_string(), json!(value));
    }
    fields
}
