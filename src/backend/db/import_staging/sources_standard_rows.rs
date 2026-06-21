#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_sources_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportSourceRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT src.*, s.session_key
            FROM import_sources src
            JOIN import_sessions s ON s.id = src.session_id
            WHERE src.session_id = $1 AND src.user_id = $2
            ORDER BY src.source_index ASC, src.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64(user_id)?)
        .fetch_all(pool)
        .await?;
        rows.iter().map(import_source_from_pg_row).collect()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_standard_rows_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportStandardRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT r.*, s.session_key, src.source_index, src.parser_id
            FROM import_standard_rows r
            JOIN import_sessions s ON s.id = r.session_id
            JOIN import_sources src ON src.id = r.source_id
            WHERE r.session_id = $1 AND r.user_id = $2
            ORDER BY src.source_index ASC, r.source_row_index ASC, r.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64(user_id)?)
        .fetch_all(pool)
        .await?;
        rows.iter().map(import_standard_row_from_pg_row).collect()
    })
}

async fn upsert_import_sources(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    drafts: &[ImportSourceDraft],
) -> DbResult<std::collections::BTreeMap<i64, i64>> {
    let mut ids = std::collections::BTreeMap::new();
    for draft in drafts {
        let id: i64 = sqlx::query(
            r#"
            INSERT INTO import_sources (
                session_id, user_id, source_index, original_file_name,
                parser_id, parser_name, parser_signal, parser_confidence,
                feature_signature, metadata, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::jsonb,now(),now())
            ON CONFLICT (session_id, source_index) DO UPDATE SET
                original_file_name = excluded.original_file_name,
                parser_id = excluded.parser_id,
                parser_name = excluded.parser_name,
                parser_signal = excluded.parser_signal,
                parser_confidence = excluded.parser_confidence,
                feature_signature = excluded.feature_signature,
                metadata = excluded.metadata,
                updated_at = now(),
                version = import_sources.version + 1
            RETURNING id
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(draft.source_index)
        .bind(&draft.original_file_name)
        .bind(&draft.parser_id)
        .bind(&draft.parser_name)
        .bind(&draft.parser_signal)
        .bind(draft.parser_confidence)
        .bind(&draft.feature_signature)
        .bind(draft.metadata.to_string())
        .fetch_one(pool)
        .await?
        .try_get("id")?;
        ids.insert(draft.source_index, id);
    }
    Ok(ids)
}

async fn ensure_default_source(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    parser_id: &str,
) -> DbResult<i64> {
    let source_ids = upsert_import_sources(
        pool,
        session_db_id,
        user_id,
        &[ImportSourceDraft {
            source_index: 0,
            original_file_name: "inline-standard-bills".to_string(),
            parser_id: parser_id.to_string(),
            parser_name: parser_source_label(parser_id).to_string(),
            parser_signal: "provided".to_string(),
            parser_confidence: 1.0,
            feature_signature: format!("{session_db_id}:default"),
            metadata: json!({}),
        }],
    )
    .await?;
    source_ids.get(&0).copied().ok_or_else(|| {
        DbError::InvalidOperation("default import source was not created".to_string())
    })
}

async fn insert_standard_rows_and_parser_payloads(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    parser_drafts: &[ImportParserTemplateDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    source_ids: &std::collections::BTreeMap<i64, i64>,
) -> DbResult<usize> {
    if !standard_row_drafts.is_empty() {
        let rows = standard_row_batch_values_from_standard_row_drafts(
            parser_drafts,
            standard_row_drafts,
            source_ids,
        )?;
        insert_standard_rows_batch_async(pool, session_db_id, user_id, &rows).await?;
        return Ok(standard_row_drafts.len());
    }
    let source_id = source_ids
        .values()
        .next()
        .copied()
        .ok_or_else(|| DbError::InvalidOperation("import source missing".to_string()))?;
    let rows = standard_row_batch_values_from_parser_templates(source_id, parser_drafts);
    insert_standard_rows_batch_async(pool, session_db_id, user_id, &rows).await?;
    Ok(parser_drafts.len())
}

fn standard_row_batch_values_from_standard_row_drafts(
    parser_drafts: &[ImportParserTemplateDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    source_ids: &std::collections::BTreeMap<i64, i64>,
) -> DbResult<Vec<StandardRowBatchValue>> {
    standard_row_drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            let source_id = source_ids
                .get(&draft.source_index)
                .or_else(|| source_ids.values().next())
                .copied()
                .ok_or_else(|| DbError::InvalidOperation("import source missing".to_string()))?;
            let parser_draft = parser_drafts.get(index);
            Ok(standard_row_batch_value_from_standard_row(
                source_id,
                draft,
                parser_draft,
            ))
        })
        .collect()
}

struct StandardRowBatchValue {
    source_id: i64,
    source_row_index: i64,
    occurred_at: String,
    amount_cents: i64,
    direction: String,
    transaction_type: String,
    merchant: String,
    payment_method: String,
    description: String,
    parser_payload: String,
    standard_payload: String,
}

fn standard_row_batch_value_from_standard_row(
    source_id: i64,
    draft: &ImportStandardRowDraft,
    parser_draft: Option<&ImportParserTemplateDraft>,
) -> StandardRowBatchValue {
    StandardRowBatchValue {
        source_id,
        source_row_index: draft.source_row_index,
        occurred_at: normalize_bill_date_text(&draft.occurred_at),
        amount_cents: draft.amount_cents,
        direction: draft.direction.clone(),
        transaction_type: draft.transaction_type.clone(),
        merchant: draft.merchant.clone(),
        payment_method: draft.payment_method.clone(),
        description: draft.description.clone(),
        parser_payload: parser_payload_from_standard_row(draft, parser_draft).to_string(),
        standard_payload: draft.standard_payload.to_string(),
    }
}

fn standard_row_batch_value_from_parser_template(
    source_id: i64,
    source_row_index: i64,
    draft: &ImportParserTemplateDraft,
) -> StandardRowBatchValue {
    let amount = Money::from_yuan_str(&finite_float_text(draft.parser_amount))
        .unwrap_or(Money::ZERO)
        .to_cents()
        .abs();
    let row = ImportStandardRowDraft {
        source_index: 0,
        source_row_index,
        occurred_at: draft.parser_date.clone(),
        amount_cents: amount,
        direction: if draft.parser_type == "收入" || draft.parser_type == "income" {
            "income".to_string()
        } else {
            "expense".to_string()
        },
        transaction_type: draft.parser_type.clone(),
        merchant: draft.parser_counterparty.clone(),
        payment_method: draft.parser_payment_method.clone(),
        description: draft.parser_description.clone(),
        parser_payload: parser_payload_from_parser_template(draft),
        standard_payload: json!({
            "parser_original_type": draft.parser_original_type,
            "parser_original_category": draft.parser_original_category,
            "parser_account_id": draft.parser_account_id,
        }),
    };
    standard_row_batch_value_from_standard_row(source_id, &row, Some(draft))
}

async fn insert_standard_rows_batch_async(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    rows: &[StandardRowBatchValue],
) -> DbResult<usize> {
    for chunk in rows.chunks(IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE) {
        let mut query = build_standard_rows_insert_query(session_db_id, user_id, chunk);
        query.build().execute(pool).await?;
    }
    Ok(rows.len())
}

fn build_standard_rows_insert_query<'a>(
    session_db_id: i64,
    user_id: i64,
    rows: &'a [StandardRowBatchValue],
) -> QueryBuilder<'a, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_standard_rows (
            session_id, source_id, user_id, source_row_index, occurred_at,
            amount_cents, direction, transaction_type, merchant, payment_method,
            description, parser_payload, standard_payload, created_at, updated_at
        )
        "#,
    );
    query.push_values(rows, |mut row, value| {
        row.push_bind(session_db_id)
            .push_bind(value.source_id)
            .push_bind(user_id)
            .push_bind(value.source_row_index)
            .push_bind(&value.occurred_at)
            .push_unseparated("::timestamptz")
            .push_bind(value.amount_cents)
            .push_bind(&value.direction)
            .push_bind(&value.transaction_type)
            .push_bind(&value.merchant)
            .push_bind(&value.payment_method)
            .push_bind(&value.description)
            .push_bind(&value.parser_payload)
            .push_unseparated("::jsonb")
            .push_bind(&value.standard_payload)
            .push_unseparated("::jsonb")
            .push("now()")
            .push("now()");
    });
    query.push(
        r#"
        ON CONFLICT (source_id, source_row_index) DO UPDATE SET
            occurred_at = excluded.occurred_at,
            amount_cents = excluded.amount_cents,
            direction = excluded.direction,
            transaction_type = excluded.transaction_type,
            merchant = excluded.merchant,
            payment_method = excluded.payment_method,
            description = excluded.description,
            parser_payload = excluded.parser_payload,
            standard_payload = excluded.standard_payload,
            updated_at = now(),
            version = import_standard_rows.version + 1
        "#,
    );
    query
}

async fn insert_standard_row(
    pool: &PostgresPool,
    session_db_id: i64,
    source_id: i64,
    user_id: i64,
    draft: &ImportStandardRowDraft,
    parser_draft: Option<&ImportParserTemplateDraft>,
) -> DbResult<i64> {
    let parser_payload = parser_payload_from_standard_row(draft, parser_draft);
    let id = sqlx::query(
        r#"
        INSERT INTO import_standard_rows (
            session_id, source_id, user_id, source_row_index, occurred_at,
            amount_cents, direction, transaction_type, merchant, payment_method,
            description, parser_payload, standard_payload, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5::timestamptz,$6,$7,$8,$9,$10,$11,$12::jsonb,$13::jsonb,now(),now())
        ON CONFLICT (source_id, source_row_index) DO UPDATE SET
            occurred_at = excluded.occurred_at,
            amount_cents = excluded.amount_cents,
            direction = excluded.direction,
            transaction_type = excluded.transaction_type,
            merchant = excluded.merchant,
            payment_method = excluded.payment_method,
            description = excluded.description,
            parser_payload = excluded.parser_payload,
            standard_payload = excluded.standard_payload,
            updated_at = now(),
            version = import_standard_rows.version + 1
        RETURNING id
        "#,
    )
    .bind(session_db_id)
    .bind(source_id)
    .bind(user_id)
    .bind(draft.source_row_index)
    .bind(normalize_bill_date_text(&draft.occurred_at))
    .bind(draft.amount_cents)
    .bind(&draft.direction)
    .bind(&draft.transaction_type)
    .bind(&draft.merchant)
    .bind(&draft.payment_method)
    .bind(&draft.description)
    .bind(parser_payload.to_string())
    .bind(draft.standard_payload.to_string())
    .fetch_one(pool)
    .await?
    .try_get("id")?;
    Ok(id)
}

async fn insert_standard_row_from_parser_template(
    pool: &PostgresPool,
    session_db_id: i64,
    source_id: i64,
    user_id: i64,
    source_row_index: i64,
    draft: &ImportParserTemplateDraft,
) -> DbResult<i64> {
    let amount = Money::from_yuan_str(&finite_float_text(draft.parser_amount))
        .unwrap_or(Money::ZERO)
        .to_cents()
        .abs();
    let row = ImportStandardRowDraft {
        source_index: 0,
        source_row_index,
        occurred_at: draft.parser_date.clone(),
        amount_cents: amount,
        direction: if draft.parser_type == "收入" || draft.parser_type == "income" {
            "income".to_string()
        } else {
            "expense".to_string()
        },
        transaction_type: draft.parser_type.clone(),
        merchant: draft.parser_counterparty.clone(),
        payment_method: draft.parser_payment_method.clone(),
        description: draft.parser_description.clone(),
        parser_payload: parser_payload_from_parser_template(draft),
        standard_payload: json!({
            "parser_original_type": draft.parser_original_type,
            "parser_original_category": draft.parser_original_category,
            "parser_account_id": draft.parser_account_id,
        }),
    };
    insert_standard_row(pool, session_db_id, source_id, user_id, &row, Some(draft)).await
}

fn parser_payload_from_standard_row(
    draft: &ImportStandardRowDraft,
    parser_draft: Option<&ImportParserTemplateDraft>,
) -> Value {
    if let Some(parser_draft) = parser_draft {
        return parser_payload_from_parser_template(parser_draft);
    }
    json!({
        "parser_date": draft.occurred_at,
        "parser_amount": draft.amount_cents as f64 / 100.0,
        "parser_type": draft.transaction_type,
        "parser_description": draft.description,
        "parser_id": draft.parser_payload.get("parser_id").and_then(Value::as_str).unwrap_or("auto"),
        "parser_tags": draft.parser_payload.get("parser_tags").cloned().unwrap_or_else(|| json!([])),
        "parser_counterparty": draft.merchant,
        "parser_payment_method": draft.payment_method,
        "parser_original_type": draft.parser_payload.get("original_type").and_then(Value::as_str).unwrap_or(""),
        "parser_original_category": draft.parser_payload.get("original_category").and_then(Value::as_str).unwrap_or(""),
        "parser_account_id": draft.parser_payload.get("source_account_id").and_then(Value::as_str).unwrap_or(""),
        "parser_is_processed": false
    })
}

fn parser_payload_from_parser_template(draft: &ImportParserTemplateDraft) -> Value {
    let parser_tags = serialize_parser_tags(
        draft.parser_tags.as_ref(),
        &draft.parser_id,
        &draft.parser_payment_method,
        "",
    );
    json!({
        "parser_date": draft.parser_date,
        "parser_amount": draft.parser_amount,
        "parser_type": draft.parser_type,
        "parser_description": draft.parser_description,
        "parser_id": draft.parser_id,
        "parser_tags": parse_json_array_strings(&parser_tags),
        "parser_counterparty": draft.parser_counterparty,
        "parser_payment_method": draft.parser_payment_method,
        "parser_original_type": draft.parser_original_type,
        "parser_original_category": draft.parser_original_category,
        "parser_account_id": draft.parser_account_id,
        "parser_is_processed": false
    })
}

fn finite_float_text(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn standard_bill_parser_tags_value(bill: &StandardBill) -> Option<Value> {
    if bill.parser_tags.is_empty() {
        None
    } else {
        serde_json::to_value(&bill.parser_tags).ok()
    }
}

fn dedup_bill_parser_tags_value(bill: &DedupBill) -> Option<Value> {
    let mut tags = bill.parser_tags.clone();
    if !bill.destination_parser_id.trim().is_empty() {
        tags.push(format!("parser:{}", bill.destination_parser_id.trim()));
    }
    if tags.is_empty() {
        None
    } else {
        serde_json::to_value(tags).ok()
    }
}

fn parser_template_type(transaction_type: &str, amount: Money) -> String {
    let transaction_type = transaction_type.trim();
    if !transaction_type.is_empty() {
        return transaction_type.to_string();
    }

    if amount.is_positive() {
        "收入".to_string()
    } else if amount.is_negative() {
        "支出".to_string()
    } else {
        "其他".to_string()
    }
}

fn preview_destination_amount_cents_for_bill(bill: &DedupBill, amount_cents: i64) -> i64 {
    if is_investment_type(&bill.transaction_type) || is_transfer_type(&bill.transaction_type) {
        amount_cents.abs()
    } else {
        0
    }
}
