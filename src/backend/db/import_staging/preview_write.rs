/// 插入单条 preview draft 并立即执行身份校验，主要用于测试和小批量兼容入口。
#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_preview_bill(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let identity_maps = load_import_identity_maps(pool, user_id).await?;
        insert_preview_row_async(pool, session_db_id, user_id, draft, &identity_maps).await
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
        if drafts.is_empty() {
            return Ok(0);
        }
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        insert_preview_rows_batch_async(pool, session_db_id, user_id, drafts).await?;
        update_session_preview_count(pool, session_db_id, user_id).await?;
        Ok(drafts.len())
    })
}

async fn insert_preview_row_async(
    pool: &PostgresPool,
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
    let mut query = build_preview_row_insert_returning_query(
        session_db_id,
        user_id,
        &draft,
        payload.to_string(),
        amount_cents,
        direction,
    );
    let id = query.build().fetch_one(pool).await?.try_get("id")?;
    Ok(id)
}

fn build_preview_row_insert_returning_query(
    session_db_id: i64,
    user_id: i64,
    draft: &ImportPreviewDraft,
    preview_payload: String,
    amount_cents: i64,
    direction: &str,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id, category_id,
            merchant, payment_method, description, preview_payload, created_at, updated_at
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
    query.push("::jsonb, now(), now()) RETURNING id");
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
}

fn preview_row_batch_value_from_draft(draft: &ImportPreviewDraft) -> PreviewRowBatchValue {
    let payload = preview_payload_from_draft(draft);
    let amount_cents = draft.preview_amount_cents.abs();
    let occurred_at = normalize_bill_date_text(&draft.preview_date);
    PreviewRowBatchValue {
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
    }
}

#[derive(Debug, Clone, Default)]
struct ImportIdentityMaps {
    active_accounts: BTreeSet<i64>,
    active_categories: BTreeMap<i64, Option<i64>>,
}

async fn insert_preview_rows_batch_async(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    let identity_maps = load_import_identity_maps(pool, user_id).await?;
    let mut drafts = drafts.to_vec();
    for draft in &mut drafts {
        apply_identity_validation_to_draft(draft, &identity_maps);
    }
    let rows = drafts
        .iter()
        .map(preview_row_batch_value_from_draft)
        .collect::<Vec<_>>();
    for chunk in rows.chunks(IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE) {
        let mut query = build_preview_rows_insert_query(session_db_id, user_id, chunk);
        query.build().execute(pool).await?;
    }
    Ok(rows.len())
}

fn build_preview_rows_insert_query<'a>(
    session_db_id: i64,
    user_id: i64,
    rows: &'a [PreviewRowBatchValue],
) -> QueryBuilder<'a, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id, category_id,
            merchant, payment_method, description, preview_payload, created_at, updated_at
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
            .push("now()")
            .push("now()");
    });
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
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE import_sessions
        SET total_preview = (
                SELECT COUNT(*)::BIGINT FROM import_preview_rows
                WHERE session_id = $1 AND user_id = $2
            ),
            updated_at = now(),
            version = version + 1
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
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
