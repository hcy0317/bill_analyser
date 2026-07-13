/// 读取 session 下的 preview rows；selected_only 必须只作为服务端筛选，不能依赖前端当前页状态。
#[tracing::instrument(level = "debug", skip_all)]
pub fn get_preview_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        load_preview_rows(pool, session_db_id, user_id_i64(user_id)?, selected_only).await
    })
}

/// 统计 session preview 行数，服务端分页和 confirm 前校验都依赖该计数语义。
#[tracing::instrument(level = "debug", skip_all)]
pub fn count_preview_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<i64> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE session_id = ",
        );
        query.push_bind(session_db_id);
        query.push(" AND user_id = ");
        query.push_bind(user_id_i64(user_id)?);
        if selected_only {
            query.push(" AND selected = true");
        }
        query
            .build()
            .fetch_one(pool)
            .await?
            .try_get(0)
            .map_err(DbError::from)
    })
}

/// 兼容旧分页入口，将 page/page_size 转换为当前 server-side preview query。
#[tracing::instrument(level = "debug", skip_all)]
pub fn get_preview_page_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    page: i64,
    page_size: i64,
    selected_only: bool,
) -> DbResult<(Vec<ImportPreviewRow>, i64)> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let selected_clause = if selected_only {
            " AND selected = true"
        } else {
            ""
        };
        let total = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE session_id = $1 AND user_id = $2{selected_clause}"
        ))
        .bind(session_db_id)
        .bind(user_id_i64)
        .fetch_one(pool)
        .await?;
        let rows = sqlx::query(&format!(
            "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = $1 AND p.user_id = $2{selected_clause} ORDER BY p.occurred_at ASC, p.id ASC LIMIT $3 OFFSET $4"
        ))
        .bind(session_db_id)
        .bind(user_id_i64)
        .bind(page_size.max(1))
        .bind((page.max(1) - 1) * page_size.max(1))
        .fetch_all(pool)
        .await?;
        let rows = rows
            .iter()
            .map(preview_from_pg_row)
            .collect::<DbResult<Vec<_>>>()?;
        Ok((rows, total))
    })
}

/// 执行 preview 服务端分页、筛选、排序、facet 与选择计数查询，是导入预览页的主查询入口。
#[tracing::instrument(level = "debug", skip_all)]
pub fn query_preview_page_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    request: &ImportPreviewPageRequest,
) -> DbResult<ImportPreviewPageResult> {
    if !request.preview_ids.is_empty() {
        let rows = get_preview_by_ids(pool, session_id, &request.preview_ids, user_id)?;
        return Ok(build_preview_page_result_from_rows(rows, request));
    }

    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let page = request.page.max(1);
        let page_size = request.page_size.max(1);
        let offset = page.saturating_sub(1).saturating_mul(page_size);
        let mut query = build_preview_page_query(
            session_db_id,
            user_id_i64,
            &request.filters,
            &request.sort_by,
            &request.sort_direction,
            page_size,
            offset,
        );
        let rows = query.build().fetch_all(pool).await?;
        let page_rows = rows
            .iter()
            .map(preview_from_pg_row)
            .collect::<DbResult<Vec<_>>>()?;
        let metadata = build_preview_metadata_for_query(
            pool,
            session_db_id,
            user_id_i64,
            &request.filters,
        )
        .await?;
        let total = metadata.counts.total;
        Ok(ImportPreviewPageResult {
            rows: page_rows,
            total,
            page,
            page_size,
            metadata,
        })
    })
}

pub fn preview_id_snapshot_hash(ids: &[i64]) -> String {
    let text = ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let hash = text.bytes().fold(2_166_136_261_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16_777_619)
    });
    format!("fnv1a32:{hash:08x}")
}

fn build_preview_page_result_from_rows(
    rows: Vec<ImportPreviewRow>,
    request: &ImportPreviewPageRequest,
) -> ImportPreviewPageResult {
    let mut selected_ids = rows
        .iter()
        .filter(|row| row.preview_selected)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    selected_ids.sort_unstable();
    let mut selected_count_filters = request.filters.clone();
    selected_count_filters.selected_only = true;
    let selected_count = apply_preview_filters(rows.clone(), &selected_count_filters).len();
    let mut selected_invalid_filters = selected_count_filters;
    selected_invalid_filters.annotation = Some("needs-review".to_string());
    let selected_invalid_count =
        apply_preview_filters(rows.clone(), &selected_invalid_filters).len();
    let mut signal_count_filters = request.filters.clone();
    signal_count_filters.signal = None;
    let signal_count_rows = apply_preview_filters(rows.clone(), &signal_count_filters);
    let mut rows = apply_preview_filters(rows, &request.filters);
    sort_preview_rows(&mut rows, &request.sort_by, &request.sort_direction);
    let total = rows.len();
    let page = request.page.max(1);
    let page_size = request.page_size.max(1);
    let start = page.saturating_sub(1).saturating_mul(page_size);
    let page_rows = rows
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect::<Vec<_>>();
    ImportPreviewPageResult {
        rows: page_rows,
        total,
        page,
        page_size,
        metadata: build_preview_metadata_from_rows(
            total,
            &signal_count_rows,
            &selected_ids,
            selected_count,
            selected_invalid_count,
        ),
    }
}

#[cfg(test)]
fn build_preview_count_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates(&mut query, filters, "p");
    query
}

fn build_preview_page_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
    sort_by: &str,
    sort_direction: &str,
    page_size: usize,
    offset: usize,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = ",
    );
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates(&mut query, filters, "p");
    push_preview_order_by(&mut query, sort_by, sort_direction);
    query.push(" LIMIT ");
    query.push_bind(i64::try_from(page_size).unwrap_or(i64::MAX).max(1));
    query.push(" OFFSET ");
    query.push_bind(i64::try_from(offset).unwrap_or(i64::MAX));
    query
}

fn push_preview_query_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
) {
    push_preview_query_predicates_with_projection(query, filters, alias, false);
}

fn push_preview_metadata_query_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
) {
    push_preview_query_predicates_with_projection(query, filters, alias, true);
}

fn push_preview_query_predicates_with_projection(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
    projected_signals: bool,
) {
    let column = |name: &str| format!("{alias}.{name}");
    if filters.selected_only {
        query.push(" AND ");
        query.push(column("selected"));
        query.push(" = true");
    }
    if let Some(value) = normalized_filter(filters.min_datetime.as_deref()) {
        query.push(" AND ");
        query.push(column("occurred_at"));
        query.push(" >= ");
        query.push_bind(normalize_bill_date_text(&value));
        query.push("::timestamptz");
    }
    if let Some(value) = normalized_filter(filters.max_datetime.as_deref()) {
        query.push(" AND ");
        query.push(column("occurred_at"));
        query.push(" <= ");
        query.push_bind(normalize_bill_date_text(&value));
        query.push("::timestamptz");
    }
    if let Some(value) = normalized_filter(filters.transaction_type.as_deref()) {
        push_ilike_predicate(query, &column("transaction_type"), &value);
    }
    if let Some(value) = normalized_filter(filters.category.as_deref()) {
        push_category_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.account.as_deref()) {
        push_account_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.tag.as_deref()) {
        push_tag_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.signal.as_deref()) {
        if projected_signals {
            push_preview_projected_signal_predicate(query, alias, &value);
        } else {
            push_preview_signal_predicate(query, alias, &value);
        }
    }
    if let Some(value) = normalized_filter(filters.annotation.as_deref()) {
        if projected_signals && matches!(value.as_str(), "needs-review" | "no-issues") {
            query.push(" AND ");
            if value == "no-issues" {
                query.push("NOT ");
            }
            query.push(alias);
            query.push(".needs_review");
        } else {
            push_annotation_predicate(query, alias, &value);
        }
    }
    if let Some(value) = normalized_filter(filters.description.as_deref()) {
        query.push(" AND (");
        query.push(column("description"));
        query.push(" ILIKE ");
        query.push_bind(like_pattern(&value));
        query.push(" OR ");
        query.push(alias);
        query.push(".preview_payload->>'preview_description' ILIKE ");
        query.push_bind(like_pattern(&value));
        query.push(")");
    }
}

fn push_preview_order_by(
    query: &mut QueryBuilder<'_, Postgres>,
    sort_by: &str,
    sort_direction: &str,
) {
    let direction = if sort_direction.eq_ignore_ascii_case("desc") {
        "DESC"
    } else {
        "ASC"
    };
    query.push(" ORDER BY ");
    match sort_by {
        "amount_cents" | "preview_amount_cents" | "previewAmountCents" | "sourceAmountCents" => {
            query.push("p.amount_cents")
        }
        "counterparty" => query.push("COALESCE(p.merchant, '')"),
        "type" => query.push(
            "CASE lower(p.transaction_type) WHEN '收入' THEN 0 WHEN 'income' THEN 0 WHEN '2' THEN 0 WHEN '支出' THEN 1 WHEN 'expense' THEN 1 WHEN '3' THEN 1 WHEN '转账' THEN 2 WHEN 'transfer' THEN 2 WHEN '4' THEN 2 WHEN '投资' THEN 3 WHEN 'investment' THEN 3 WHEN '5' THEN 3 ELSE 4 END",
        ),
        "paymentMethod" => query.push("COALESCE(p.payment_method, '')"),
        "comment" => query.push("COALESCE(p.description, '')"),
        "time" => query.push("p.occurred_at"),
        _ => query.push("p.occurred_at"),
    };
    query.push(" ");
    query.push(direction);
    query.push(", p.id ");
    query.push(direction);
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn like_pattern(value: &str) -> String {
    format!("%{value}%")
}

fn build_preview_metadata(total: usize) -> ImportPreviewMetadata {
    let counts = ImportPreviewCounts {
        signals: empty_visible_signal_counts(),
        total,
        ..ImportPreviewCounts::default()
    };
    ImportPreviewMetadata {
        counts,
        facets: ImportPreviewFacets::default(),
        selection_hash: preview_id_snapshot_hash(&[]),
    }
}

fn push_preview_projected_signal_predicate(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    value: &str,
) {
    query.push(" AND ");
    let filter = normalize_visible_signal_filter(value);
    let (family, status) = signal_filter_family_status(&filter)
        .map(|(family, status)| (family, Some(status)))
        .unwrap_or((filter.as_str(), None));
    let column = match ImportPreviewSignalFamily::parse(family) {
        Some(ImportPreviewSignalFamily::Parser) => "signal_parser",
        Some(ImportPreviewSignalFamily::PlatformDuplicate) => "signal_platform_duplicate",
        Some(ImportPreviewSignalFamily::Transfer) => "signal_transfer",
        Some(ImportPreviewSignalFamily::History) => "signal_history",
        Some(ImportPreviewSignalFamily::Learning) => "signal_learning",
        Some(ImportPreviewSignalFamily::Llm) => "signal_llm",
        None => {
            query.push("FALSE");
            return;
        }
    };
    query.push(alias);
    query.push(".");
    query.push(column);
    if let Some(status) = status {
        query.push(" AND ");
        push_preview_feedback_family_text_search(query, alias, family, status);
    }
}

fn build_preview_metadata_from_rows(
    total: usize,
    signal_count_rows: &[ImportPreviewRow],
    selected_ids: &[i64],
    selected_count: usize,
    selected_invalid_count: usize,
) -> ImportPreviewMetadata {
    let mut metadata = build_preview_metadata(total);
    metadata.counts.selected = selected_count;
    metadata.counts.selected_total = selected_ids.len();
    metadata.counts.selected_invalid = selected_invalid_count;
    metadata.selection_hash = preview_id_snapshot_hash(selected_ids);
    for family in IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES {
        let count = signal_count_rows
            .iter()
            .filter(|row| preview_signal_family_matches(family, row))
            .count();
        metadata
            .counts
            .signals
            .insert((*family).to_string(), count);
    }
    metadata
}

fn empty_visible_signal_counts() -> BTreeMap<String, usize> {
    IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES
        .iter()
        .map(|family| ((*family).to_string(), 0usize))
        .collect()
}

async fn build_preview_metadata_for_query(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<ImportPreviewMetadata> {
    query_preview_metadata_aggregate(pool, session_db_id, user_id, filters).await
}

async fn query_preview_metadata_aggregate(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<ImportPreviewMetadata> {
    let mut query = build_preview_metadata_aggregate_query(session_db_id, user_id, filters);
    let row = query.build().fetch_one(pool).await?;
    let selected_ids = row.try_get::<Vec<i64>, _>("selected_ids")?;
    let total = preview_metadata_database_count(row.try_get("total_count")?, "total_count")?;
    let mut metadata = build_preview_metadata(total);
    metadata.counts.selected =
        preview_metadata_database_count(row.try_get("selected_count")?, "selected_count")?;
    metadata.counts.selected_total = selected_ids.len();
    metadata.counts.selected_invalid = preview_metadata_database_count(
        row.try_get("selected_invalid")?,
        "selected_invalid",
    )?;
    metadata.counts.signals = query_preview_signal_counts(&row)?;
    metadata.facets.categories = query_preview_facets(&row, "category_facets")?;
    metadata.facets.accounts = query_preview_facets(&row, "account_facets")?;
    metadata.facets.tags = query_preview_facets(&row, "tag_facets")?;
    metadata.selection_hash = preview_id_snapshot_hash(&selected_ids);
    Ok(metadata)
}

fn preview_metadata_database_count(count: i64, column: &str) -> DbResult<usize> {
    usize::try_from(count).map_err(|_| {
        DbError::InvalidOperation(format!(
            "invalid preview metadata {column} returned by PostgreSQL: {count}"
        ))
    })
}

fn build_preview_metadata_aggregate_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> QueryBuilder<'static, Postgres> {
    build_preview_metadata_aggregate_query_with_prefix("", session_db_id, user_id, filters)
}

fn build_preview_metadata_aggregate_query_with_prefix(
    prefix: &'static str,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(prefix);
    query.push("WITH preview_scope AS MATERIALIZED (SELECT p.*, signal_flags.parser AS signal_parser, signal_flags.platform_duplicate AS signal_platform_duplicate, signal_flags.transfer AS signal_transfer, signal_flags.history AS signal_history, signal_flags.learning AS signal_learning, signal_flags.llm AS signal_llm, ");
    push_preview_current_review_condition_with_joins(&mut query, "p");
    query.push(" AS needs_review FROM import_preview_rows p CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(p.preview_payload)) AS signal_flags(parser BOOLEAN, platform_duplicate BOOLEAN, transfer BOOLEAN, history BOOLEAN, learning BOOLEAN, llm BOOLEAN) LEFT JOIN categories c ON c.user_id = p.user_id AND c.id = p.category_id AND c.is_active = true LEFT JOIN accounts source_account ON source_account.user_id = p.user_id AND source_account.id = p.account_id AND source_account.is_active = true LEFT JOIN accounts target_account ON target_account.user_id = p.user_id AND target_account.id = p.transfer_target_account_id AND target_account.is_active = true WHERE p.session_id = ");
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    query.push(") SELECT ");

    query.push("(SELECT COUNT(*)::BIGINT");
    push_preview_metadata_scope(&mut query, session_db_id, user_id, filters);
    query.push(") AS total_count, ");

    let mut selected_filters = filters.clone();
    selected_filters.selected_only = true;
    query.push("(SELECT COUNT(*)::BIGINT");
    push_preview_metadata_scope(&mut query, session_db_id, user_id, &selected_filters);
    query.push(") AS selected_count, ");

    let mut selected_invalid_filters = selected_filters;
    selected_invalid_filters.annotation = Some("needs-review".to_string());
    query.push("(SELECT COUNT(*)::BIGINT");
    push_preview_metadata_scope(
        &mut query,
        session_db_id,
        user_id,
        &selected_invalid_filters,
    );
    query.push(") AS selected_invalid, ");

    query.push("COALESCE((SELECT array_agg(p.id ORDER BY p.id)");
    push_preview_metadata_scope(
        &mut query,
        session_db_id,
        user_id,
        &ImportPreviewQueryFilters {
            selected_only: true,
            ..ImportPreviewQueryFilters::default()
        },
    );
    query.push("), ARRAY[]::BIGINT[]) AS selected_ids, ");

    push_preview_signal_counts_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS signal_counts, ");
    push_preview_category_facets_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS category_facets, ");
    push_preview_account_facets_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS account_facets, ");
    push_preview_tag_facets_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS tag_facets");
    query
}

fn push_preview_metadata_scope(
    query: &mut QueryBuilder<'_, Postgres>,
    _session_db_id: i64,
    _user_id: i64,
    filters: &ImportPreviewQueryFilters,
) {
    query.push(" FROM preview_scope p WHERE TRUE");
    push_preview_metadata_query_predicates(query, filters, "p");
}

fn push_preview_signal_counts_aggregate(
    query: &mut QueryBuilder<'_, Postgres>,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) {
    query.push("COALESCE((SELECT jsonb_object_agg(signal_rows.family, signal_rows.count) FROM (");
    for (index, family) in IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES.iter().enumerate() {
        if index > 0 {
            query.push(" UNION ALL ");
        }
        let mut signal_filters = filters.clone();
        signal_filters.signal = Some((*family).to_string());
        query.push("SELECT ");
        query.push_bind((*family).to_string());
        query.push(" AS family, COUNT(*)::BIGINT AS count");
        push_preview_metadata_scope(query, session_db_id, user_id, &signal_filters);
    }
    query.push(") signal_rows), '{}'::jsonb)");
}

fn push_preview_category_facets_aggregate(
    query: &mut QueryBuilder<'_, Postgres>,
    _session_db_id: i64,
    _user_id: i64,
    filters: &ImportPreviewQueryFilters,
) {
    let mut facet_filters = filters.clone();
    facet_filters.category = None;
    query.push("COALESCE((SELECT jsonb_agg(jsonb_build_object('value', facet_rows.value, 'label', facet_rows.label, 'count', facet_rows.count) ORDER BY facet_rows.count DESC, facet_rows.label ASC) FROM (SELECT p.category_id::text AS value, COALESCE(NULLIF(c.path, ''), c.name, p.category_id::text) AS label, COUNT(*)::BIGINT AS count FROM preview_scope p JOIN categories c ON c.user_id = p.user_id AND c.id = p.category_id AND c.is_active = true WHERE p.category_id IS NOT NULL");
    push_preview_metadata_query_predicates(query, &facet_filters, "p");
    query.push(" GROUP BY p.category_id, c.path, c.name ORDER BY count DESC, label ASC LIMIT ");
    query.push_bind(IMPORT_PREVIEW_FACET_LIMIT);
    query.push(") facet_rows), '[]'::jsonb)");
}

fn push_preview_account_facets_aggregate(
    query: &mut QueryBuilder<'_, Postgres>,
    _session_db_id: i64,
    _user_id: i64,
    filters: &ImportPreviewQueryFilters,
) {
    let mut facet_filters = filters.clone();
    facet_filters.account = None;
    query.push("COALESCE((SELECT jsonb_agg(jsonb_build_object('value', facet_rows.value, 'label', facet_rows.label, 'count', facet_rows.count) ORDER BY facet_rows.count DESC, facet_rows.label ASC) FROM (SELECT account_values.account_id::text AS value, COALESCE(a.name, account_values.account_id::text) AS label, COUNT(*)::BIGINT AS count FROM preview_scope p CROSS JOIN LATERAL (VALUES (p.account_id), (p.transfer_target_account_id)) AS account_values(account_id) JOIN accounts a ON a.user_id = p.user_id AND a.id = account_values.account_id AND a.is_active = true WHERE account_values.account_id IS NOT NULL");
    push_preview_metadata_query_predicates(query, &facet_filters, "p");
    query.push(" GROUP BY account_values.account_id, a.name ORDER BY count DESC, label ASC LIMIT ");
    query.push_bind(IMPORT_PREVIEW_FACET_LIMIT);
    query.push(") facet_rows), '[]'::jsonb)");
}

fn push_preview_tag_facets_aggregate(
    query: &mut QueryBuilder<'_, Postgres>,
    _session_db_id: i64,
    _user_id: i64,
    filters: &ImportPreviewQueryFilters,
) {
    let mut facet_filters = filters.clone();
    facet_filters.tag = None;
    query.push("COALESCE((SELECT jsonb_agg(jsonb_build_object('value', facet_rows.value, 'label', facet_rows.label, 'count', facet_rows.count) ORDER BY facet_rows.count DESC, facet_rows.label ASC) FROM (SELECT tag_values.tag AS value, tag_values.tag AS label, COUNT(*)::BIGINT AS count FROM preview_scope p CROSS JOIN LATERAL jsonb_array_elements_text(CASE WHEN jsonb_typeof(p.preview_payload->'preview_parser_tags') = 'array' THEN p.preview_payload->'preview_parser_tags' ELSE '[]'::jsonb END) AS tag_values(tag) WHERE btrim(tag_values.tag, ");
    query.push_bind(IMPORT_PREVIEW_SIGNAL_TRIM_CHARS);
    query.push(") <> ''");
    push_preview_metadata_query_predicates(query, &facet_filters, "p");
    query.push(" GROUP BY tag_values.tag ORDER BY count DESC, label ASC LIMIT ");
    query.push_bind(IMPORT_PREVIEW_FACET_LIMIT);
    query.push(") facet_rows), '[]'::jsonb)");
}

fn query_preview_signal_counts(row: &PgRow) -> DbResult<BTreeMap<String, usize>> {
    let value = row.try_get::<Value, _>("signal_counts")?;
    serde_json::from_value(value).map_err(|error| {
        DbError::InvalidOperation(format!("invalid preview signal count aggregate: {error}"))
    })
}

fn query_preview_facets(row: &PgRow, column: &str) -> DbResult<Vec<ImportPreviewFacetEntry>> {
    let value = row.try_get::<Value, _>(column)?;
    serde_json::from_value(value).map_err(|error| {
        DbError::InvalidOperation(format!("invalid preview facet aggregate {column}: {error}"))
    })
}
