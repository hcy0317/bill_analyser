/// 插入单条 preview draft 并立即执行身份校验，主要用于测试和小批量兼容入口。
#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_preview_bill(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let identity_maps = load_import_identity_maps_on_tx(&mut tx, user_id).await?;
        let id =
            insert_preview_row_async(&mut tx, session_db_id, user_id, draft, &identity_maps)
                .await?;
        update_session_preview_count(&mut tx, session_db_id, user_id).await?;
        tx.commit().await?;
        Ok(id)
    })
}

/// 批量插入 preview drafts 并刷新 session preview 计数，是 stage2 materialization 的主写入入口。
#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_preview_bills_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let total_started_at = std::time::Instant::now();
        let user_id = user_id_i64(user_id)?;
        let transaction_begin_started_at = std::time::Instant::now();
        let mut tx = pool.begin().await?;
        let elapsed_transaction_begin_ms = transaction_begin_started_at.elapsed().as_millis();
        let session_lock_started_at = std::time::Instant::now();
        let session_db_id =
            lock_active_import_session_on_tx(&mut tx, session_id, user_id).await?;
        let elapsed_session_lock_ms = session_lock_started_at.elapsed().as_millis();
        if drafts.is_empty() {
            tx.commit().await?;
            return Ok(0);
        }
        let write_timing =
            insert_preview_rows_batch_async(&mut tx, session_db_id, user_id, drafts).await?;
        let counter_refresh_started_at = std::time::Instant::now();
        update_session_preview_count(&mut tx, session_db_id, user_id).await?;
        let elapsed_counter_refresh_ms = counter_refresh_started_at.elapsed().as_millis();
        let commit_started_at = std::time::Instant::now();
        tx.commit().await?;
        let elapsed_commit_ms = commit_started_at.elapsed().as_millis();
        tracing::info!(
            target: "bill_analyser::import_preview_write",
            operation = "insert_preview_bills_batch",
            preview_rows = write_timing.preview_rows,
            query_chunks = write_timing.query_chunks,
            elapsed_transaction_begin_ms,
            elapsed_session_lock_ms,
            elapsed_identity_load_ms = write_timing.elapsed_identity_load_ms,
            elapsed_draft_clone_ms = write_timing.elapsed_draft_clone_ms,
            elapsed_identity_validation_ms = write_timing.elapsed_identity_validation_ms,
            elapsed_row_encode_ms = write_timing.elapsed_row_encode_ms,
            elapsed_query_build_ms = write_timing.elapsed_query_build_ms,
            elapsed_query_execute_ms = write_timing.elapsed_query_execute_ms,
            elapsed_signal_shadow_ms = write_timing.elapsed_signal_shadow_ms,
            elapsed_counter_refresh_ms,
            elapsed_commit_ms,
            elapsed_total_ms = total_started_at.elapsed().as_millis(),
            "preview batch insert complete"
        );
        Ok(drafts.len())
    })
}

async fn insert_preview_row_async(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    draft: &ImportPreviewDraft,
    identity_maps: &ImportIdentityMaps,
) -> DbResult<i64> {
    let mut draft = draft.clone();
    apply_identity_validation_to_draft(&mut draft, identity_maps);
    let payload = preview_payload_from_draft(&draft);
    let amount_cents = draft.preview_amount_cents.abs();
    let direction = if draft.preview_type == "收入" || draft.preview_type == "income" {
        "income"
    } else {
        "expense"
    };
    let signal_projection = import_preview_signal_projection_from_payload(&payload)?;
    let mut query = build_preview_row_insert_returning_query(
        session_db_id,
        user_id,
        &draft,
        payload.to_string(),
        signal_projection,
        amount_cents,
        direction,
    );
    let id = query
        .build()
        .fetch_one(&mut **tx)
        .await?
        .try_get("id")?;
    observe_import_preview_signal_projection_parity(tx, &[id], "preview_insert_single").await?;
    Ok(id)
}

fn build_preview_row_insert_returning_query(
    session_db_id: i64,
    user_id: i64,
    draft: &ImportPreviewDraft,
    preview_payload: String,
    signal_projection: ImportPreviewSignalProjection,
    amount_cents: i64,
    direction: &str,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id, category_id,
            merchant, payment_method, description, preview_payload,
            signal_parser, signal_platform_duplicate, signal_transfer, signal_history,
            signal_learning, signal_llm, signal_projection_version, created_at, updated_at
        ) VALUES (
        "#,
    );
    query.push_bind(session_db_id);
    query.push(", ");
    query.push_bind(user_id);
    query.push(", ");
    query.push_bind(format!(
        "{}:{}",
        normalize_bill_date_text(&draft.preview_date),
        draft.preview_counterparty
    ));
    query.push(", 'insert', ");
    query.push_bind(draft.preview_selected);
    query.push(", '[]'::jsonb, ");
    query.push_bind(draft.dedup_source_ids.clone());
    query.push(", ");
    query.push_bind(normalize_bill_date_text(&draft.preview_date));
    query.push("::timestamptz, ");
    query.push_bind(amount_cents);
    query.push(", ");
    query.push_bind(direction.to_string());
    query.push(", ");
    query.push_bind(draft.preview_type.clone());
    query.push(", ");
    query.push_bind(draft.preview_source_account_id);
    query.push(", ");
    query.push_bind(draft.preview_destination_account_id);
    query.push(", ");
    query.push_bind(draft.category_id);
    query.push(", ");
    query.push_bind(draft.preview_counterparty.clone());
    query.push(", ");
    query.push_bind(draft.preview_payment_method.clone());
    query.push(", ");
    query.push_bind(draft.preview_description.clone());
    query.push(", ");
    query.push_bind(preview_payload);
    query.push("::jsonb, ");
    query.push_bind(signal_projection.parser);
    query.push(", ");
    query.push_bind(signal_projection.platform_duplicate);
    query.push(", ");
    query.push_bind(signal_projection.transfer);
    query.push(", ");
    query.push_bind(signal_projection.history);
    query.push(", ");
    query.push_bind(signal_projection.learning);
    query.push(", ");
    query.push_bind(signal_projection.llm);
    query.push(", ");
    query.push_bind(signal_projection.version);
    query.push(", now(), now()) RETURNING id");
    query
}

struct PreviewRowBatchValue {
    page_sort_key: String,
    selected: bool,
    merged_source_ids: Vec<i64>,
    occurred_at: String,
    amount_cents: i64,
    direction: String,
    transaction_type: String,
    account_id: Option<i64>,
    transfer_target_account_id: Option<i64>,
    category_id: Option<i64>,
    merchant: String,
    payment_method: String,
    description: String,
    preview_payload: String,
    signal_projection: ImportPreviewSignalProjection,
}

fn preview_row_batch_value_from_draft(
    draft: &ImportPreviewDraft,
) -> DbResult<PreviewRowBatchValue> {
    let payload = preview_payload_from_draft(draft);
    let signal_projection = import_preview_signal_projection_from_payload(&payload)?;
    let amount_cents = draft.preview_amount_cents.abs();
    let occurred_at = normalize_bill_date_text(&draft.preview_date);
    Ok(PreviewRowBatchValue {
        page_sort_key: format!("{}:{}", occurred_at, draft.preview_counterparty),
        selected: draft.preview_selected,
        merged_source_ids: draft.dedup_source_ids.clone(),
        occurred_at,
        amount_cents,
        direction: if draft.preview_type == "收入" || draft.preview_type == "income" {
            "income".to_string()
        } else {
            "expense".to_string()
        },
        transaction_type: draft.preview_type.clone(),
        account_id: draft.preview_source_account_id,
        transfer_target_account_id: draft.preview_destination_account_id,
        category_id: draft.category_id,
        merchant: draft.preview_counterparty.clone(),
        payment_method: draft.preview_payment_method.clone(),
        description: draft.preview_description.clone(),
        preview_payload: payload.to_string(),
        signal_projection,
    })
}

#[derive(Debug, Clone, Default)]
struct ImportIdentityMaps {
    active_accounts: BTreeSet<i64>,
    active_categories: BTreeMap<i64, Option<i64>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PreviewRowsBatchWriteTiming {
    preview_rows: usize,
    query_chunks: usize,
    elapsed_identity_load_ms: u128,
    elapsed_draft_clone_ms: u128,
    elapsed_identity_validation_ms: u128,
    elapsed_row_encode_ms: u128,
    elapsed_query_build_ms: u128,
    elapsed_query_execute_ms: u128,
    elapsed_signal_shadow_ms: u128,
}

async fn insert_preview_rows_batch_async(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    drafts: &[ImportPreviewDraft],
) -> DbResult<PreviewRowsBatchWriteTiming> {
    let identity_load_started_at = std::time::Instant::now();
    let identity_maps = load_import_identity_maps_on_tx(tx, user_id).await?;
    let elapsed_identity_load_ms = identity_load_started_at.elapsed().as_millis();
    let draft_clone_started_at = std::time::Instant::now();
    let mut drafts = drafts.to_vec();
    let elapsed_draft_clone_ms = draft_clone_started_at.elapsed().as_millis();
    let identity_validation_started_at = std::time::Instant::now();
    for draft in &mut drafts {
        apply_identity_validation_to_draft(draft, &identity_maps);
    }
    let elapsed_identity_validation_ms = identity_validation_started_at.elapsed().as_millis();
    let row_encode_started_at = std::time::Instant::now();
    let rows = drafts
        .iter()
        .map(preview_row_batch_value_from_draft)
        .collect::<DbResult<Vec<_>>>()?;
    let elapsed_row_encode_ms = row_encode_started_at.elapsed().as_millis();
    let mut shadow_sample_ids = Vec::with_capacity(
        rows.len()
            .min(IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE),
    );
    let mut query_chunks = 0usize;
    let mut elapsed_query_build_ms = 0u128;
    let mut elapsed_query_execute_ms = 0u128;
    for chunk in rows.chunks(IMPORT_PREVIEW_BULK_INSERT_CHUNK_SIZE) {
        query_chunks = query_chunks.saturating_add(1);
        let capture_shadow_ids =
            shadow_sample_ids.len() < IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE;
        let query_build_started_at = std::time::Instant::now();
        let mut query = build_preview_rows_insert_query(
            session_db_id,
            user_id,
            chunk,
            capture_shadow_ids,
        );
        elapsed_query_build_ms = elapsed_query_build_ms
            .saturating_add(query_build_started_at.elapsed().as_millis());
        let query_execute_started_at = std::time::Instant::now();
        if capture_shadow_ids {
            let inserted = query.build().fetch_all(&mut **tx).await?;
            for row in inserted.iter().take(
                IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE.saturating_sub(shadow_sample_ids.len()),
            ) {
                shadow_sample_ids.push(row.try_get::<i64, _>("id")?);
            }
        } else {
            query.build().execute(&mut **tx).await?;
        }
        elapsed_query_execute_ms = elapsed_query_execute_ms
            .saturating_add(query_execute_started_at.elapsed().as_millis());
    }
    let signal_shadow_started_at = std::time::Instant::now();
    observe_import_preview_signal_projection_parity(
        tx,
        &shadow_sample_ids,
        "preview_insert_batch",
    )
    .await?;
    let elapsed_signal_shadow_ms = signal_shadow_started_at.elapsed().as_millis();
    Ok(PreviewRowsBatchWriteTiming {
        preview_rows: rows.len(),
        query_chunks,
        elapsed_identity_load_ms,
        elapsed_draft_clone_ms,
        elapsed_identity_validation_ms,
        elapsed_row_encode_ms,
        elapsed_query_build_ms,
        elapsed_query_execute_ms,
        elapsed_signal_shadow_ms,
    })
}

fn build_preview_rows_insert_query<'a>(
    session_db_id: i64,
    user_id: i64,
    rows: &'a [PreviewRowBatchValue],
    capture_shadow_ids: bool,
) -> QueryBuilder<'a, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(if capture_shadow_ids {
        "WITH inserted AS ("
    } else {
        ""
    });
    query.push(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id, category_id,
            merchant, payment_method, description, preview_payload,
            signal_parser, signal_platform_duplicate, signal_transfer, signal_history,
            signal_learning, signal_llm, signal_projection_version, created_at, updated_at
        )
        "#,
    );
    query.push_values(rows, |mut row, value| {
        row.push_bind(session_db_id)
            .push_bind(user_id)
            .push_bind(&value.page_sort_key)
            .push_bind("insert")
            .push_bind(value.selected)
            .push("'[]'::jsonb")
            .push_bind(&value.merged_source_ids)
            .push_bind(&value.occurred_at)
            .push_unseparated("::timestamptz")
            .push_bind(value.amount_cents)
            .push_bind(&value.direction)
            .push_bind(&value.transaction_type)
            .push_bind(value.account_id)
            .push_bind(value.transfer_target_account_id)
            .push_bind(value.category_id)
            .push_bind(&value.merchant)
            .push_bind(&value.payment_method)
            .push_bind(&value.description)
            .push_bind(&value.preview_payload)
            .push_unseparated("::jsonb")
            .push_bind(value.signal_projection.parser)
            .push_bind(value.signal_projection.platform_duplicate)
            .push_bind(value.signal_projection.transfer)
            .push_bind(value.signal_projection.history)
            .push_bind(value.signal_projection.learning)
            .push_bind(value.signal_projection.llm)
            .push_bind(value.signal_projection.version)
            .push("now()")
            .push("now()");
    });
    if capture_shadow_ids {
        query.push(" RETURNING id) SELECT id FROM inserted ORDER BY id ASC LIMIT ");
        query.push(IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE);
    }
    query
}

async fn load_preview_rows(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    let selected_clause = if selected_only {
        " AND p.selected = true"
    } else {
        ""
    };
    let rows = sqlx::query(&format!(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = $1 AND p.user_id = $2{selected_clause} ORDER BY p.occurred_at ASC, p.id ASC"
    ))
    .bind(session_db_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(preview_from_pg_row).collect()
}

async fn update_session_preview_count(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<()> {
    refresh_import_session_counters_on_tx(tx, session_db_id, user_id).await
}

fn preview_payload_from_draft(draft: &ImportPreviewDraft) -> Value {
    json!({
        "preview_date": normalize_bill_date_text(&draft.preview_date),
        "preview_type": draft.preview_type,
        "preview_amount_cents": draft.preview_amount_cents,
        "preview_destination_amount_cents": draft.preview_destination_amount_cents,
        "category_id": draft.category_id,
        "categoryId": draft.category_id,
        "preview_main_category": draft.preview_main_category,
        "preview_sub_category": draft.preview_sub_category,
        "preview_source_account_id": draft.preview_source_account_id,
        "preview_destination_account_id": draft.preview_destination_account_id,
        "preview_counterparty": draft.preview_counterparty,
        "preview_payment_method": draft.preview_payment_method,
        "preview_description": draft.preview_description,
        "preview_parser_id": draft.preview_parser_id,
        "preview_parser_tags": draft.preview_parser_tags.clone().unwrap_or_else(|| json!([])),
        "preview_recurring_id": draft.preview_recurring_id,
        "preview_recurring_name": draft.preview_recurring_name,
        "preview_recurring_candidate_count": draft.preview_recurring_candidate_count,
        "preview_recurring_match_score": draft.preview_recurring_match_score,
        "preview_recurring_match_reasons": draft.preview_recurring_match_reasons,
        "preview_recurring_matched_date": draft.preview_recurring_matched_date,
        "preview_selected": draft.preview_selected,
        "dedup_type": draft.dedup_type,
        "dedup_source_ids": draft.dedup_source_ids,
        "preview_matching_feedback": draft.preview_matching_feedback,
    })
}
