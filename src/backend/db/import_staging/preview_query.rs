#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewSignalReadSource {
    LegacyPayload,
    TypedV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewSignalPredicateSource {
    LegacyPayload,
    TypedV1,
    ScopeProjection,
}

struct PreviewPageQuerySpec<'a> {
    filters: &'a ImportPreviewQueryFilters,
    sort_by: &'a str,
    sort_direction: &'a str,
    page_size: usize,
    offset: usize,
    signal_read_source: PreviewSignalReadSource,
}

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
    query_preview_page_by_session_with_signal_read_source(
        pool,
        session_id,
        user_id,
        request,
        PreviewSignalReadSource::LegacyPayload,
    )
}

fn query_preview_page_by_session_with_signal_read_source(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    request: &ImportPreviewPageRequest,
    signal_read_source: PreviewSignalReadSource,
) -> DbResult<ImportPreviewPageResult> {
    if !request.preview_ids.is_empty() {
        if signal_read_source == PreviewSignalReadSource::TypedV1 {
            return Err(DbError::InvalidOperation(
                "typed import signal read shadow does not accept preview_ids bypass queries"
                    .to_string(),
            ));
        }
        let rows = get_preview_by_ids(pool, session_id, &request.preview_ids, user_id)?;
        return Ok(build_preview_page_result_from_rows(rows, request));
    }

    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let mut connection = pool.acquire().await?;
        if signal_read_source == PreviewSignalReadSource::TypedV1 {
            ensure_preview_signal_projection_v1(&mut connection, session_db_id, user_id_i64)
                .await?;
        }
        query_preview_page_from_connection(
            &mut connection,
            session_db_id,
            user_id_i64,
            request,
            signal_read_source,
        )
        .await
    })
}

async fn query_preview_page_from_connection(
    connection: &mut sqlx::PgConnection,
    session_db_id: i64,
    user_id: i64,
    request: &ImportPreviewPageRequest,
    signal_read_source: PreviewSignalReadSource,
) -> DbResult<ImportPreviewPageResult> {
    let page = request.page.max(1);
    let page_size = request.page_size.max(1);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    let mut query = build_preview_page_query_with_signal_read_source(
        session_db_id,
        user_id,
        PreviewPageQuerySpec {
            filters: &request.filters,
            sort_by: &request.sort_by,
            sort_direction: &request.sort_direction,
            page_size,
            offset,
            signal_read_source,
        },
    );
    let rows = query.build().fetch_all(&mut *connection).await?;
    let page_rows = rows
        .iter()
        .map(preview_from_pg_row)
        .collect::<DbResult<Vec<_>>>()?;
    let metadata = build_preview_metadata_for_query(
        connection,
        session_db_id,
        user_id,
        &request.filters,
        signal_read_source,
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
}

/// 迁移期只读 shadow：同一请求分别走 legacy payload 与 typed v1 columns，生产入口不调用它。
#[tracing::instrument(level = "debug", skip_all)]
pub fn audit_import_preview_signal_read_parity(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    request: &ImportPreviewPageRequest,
) -> DbResult<ImportPreviewSignalReadParityReport> {
    if !request.preview_ids.is_empty() {
        return Err(DbError::InvalidOperation(
            "import signal read parity audit requires a server-side query without preview_ids"
                .to_string(),
        ));
    }
    block_on_db(async move {
        let user_id_i64 = user_id_i64(user_id)?;
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let session_db_id = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2",
        )
        .bind(session_id)
        .bind(user_id_i64)
        .fetch_one(&mut *tx)
        .await?
        .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))?;
        ensure_preview_signal_projection_v1(&mut tx, session_db_id, user_id_i64).await?;
        let legacy = query_preview_page_from_connection(
            &mut tx,
            session_db_id,
            user_id_i64,
            request,
            PreviewSignalReadSource::LegacyPayload,
        )
        .await?;
        let typed = query_preview_page_from_connection(
            &mut tx,
            session_db_id,
            user_id_i64,
            request,
            PreviewSignalReadSource::TypedV1,
        )
        .await?;
        tx.commit().await?;
        Ok(build_import_preview_signal_read_parity_report(
            legacy, typed,
        ))
    })
}

fn build_import_preview_signal_read_parity_report(
    legacy: ImportPreviewPageResult,
    typed: ImportPreviewPageResult,
) -> ImportPreviewSignalReadParityReport {
    ImportPreviewSignalReadParityReport {
        observed_rows: legacy.total,
        legacy_total: legacy.total,
        typed_total: typed.total,
        rows_mismatch: legacy.rows != typed.rows,
        total_mismatch: legacy.total != typed.total,
        signal_counts_mismatch: legacy.metadata.counts.signals
            != typed.metadata.counts.signals,
        selection_counts_mismatch: legacy.metadata.counts.annotations
            != typed.metadata.counts.annotations
            || legacy.metadata.counts.selected != typed.metadata.counts.selected
            || legacy.metadata.counts.selected_total != typed.metadata.counts.selected_total
            || legacy.metadata.counts.selected_invalid
                != typed.metadata.counts.selected_invalid,
        facets_mismatch: legacy.metadata.facets != typed.metadata.facets,
        selection_hash_mismatch: legacy.metadata.selection_hash
            != typed.metadata.selection_hash,
    }
}

async fn ensure_preview_signal_projection_v1(
    connection: &mut sqlx::PgConnection,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<()> {
    let unmaterialized_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE session_id=$1 AND user_id=$2 AND signal_projection_version<>1",
    )
    .bind(session_db_id)
    .bind(user_id)
    .fetch_one(&mut *connection)
    .await?;
    if unmaterialized_rows > 0 {
        return Err(DbError::InvalidOperation(format!(
            "typed import signal read requires signal_projection_version=1 for every session row; found {unmaterialized_rows} unmaterialized rows"
        )));
    }
    Ok(())
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

#[cfg(test)]
fn build_preview_page_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
    sort_by: &str,
    sort_direction: &str,
    page_size: usize,
    offset: usize,
) -> QueryBuilder<'static, Postgres> {
    build_preview_page_query_with_signal_read_source(
        session_db_id,
        user_id,
        PreviewPageQuerySpec {
            filters,
            sort_by,
            sort_direction,
            page_size,
            offset,
            signal_read_source: PreviewSignalReadSource::LegacyPayload,
        },
    )
}

fn build_preview_page_query_with_signal_read_source(
    session_db_id: i64,
    user_id: i64,
    spec: PreviewPageQuerySpec<'_>,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = ",
    );
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates_with_signal_source(
        &mut query,
        spec.filters,
        "p",
        match spec.signal_read_source {
            PreviewSignalReadSource::LegacyPayload => {
                PreviewSignalPredicateSource::LegacyPayload
            }
            PreviewSignalReadSource::TypedV1 => PreviewSignalPredicateSource::TypedV1,
        },
    );
    push_preview_order_by(&mut query, spec.sort_by, spec.sort_direction);
    query.push(" LIMIT ");
    query.push_bind(i64::try_from(spec.page_size).unwrap_or(i64::MAX).max(1));
    query.push(" OFFSET ");
    query.push_bind(i64::try_from(spec.offset).unwrap_or(i64::MAX));
    query
}

fn push_preview_query_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
) {
    push_preview_query_predicates_with_signal_source(
        query,
        filters,
        alias,
        PreviewSignalPredicateSource::LegacyPayload,
    );
}

fn push_preview_metadata_query_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
) {
    push_preview_query_predicates_with_signal_source(
        query,
        filters,
        alias,
        PreviewSignalPredicateSource::ScopeProjection,
    );
}

fn push_preview_query_predicates_with_signal_source(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
    signal_source: PreviewSignalPredicateSource,
) {
    let column = |name: &str| format!("{alias}.{name}");
    if signal_source == PreviewSignalPredicateSource::TypedV1 {
        query.push(" AND ");
        query.push(column("signal_projection_version"));
        query.push(" = ");
        query.push_bind(1_i16);
    }
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
        match signal_source {
            PreviewSignalPredicateSource::LegacyPayload => {
                push_preview_signal_predicate(query, alias, &value)
            }
            PreviewSignalPredicateSource::TypedV1 => {
                push_preview_materialized_signal_predicate(
                    query,
                    alias,
                    &value,
                    "signal_",
                )
            }
            PreviewSignalPredicateSource::ScopeProjection => {
                push_preview_materialized_signal_predicate(
                    query,
                    alias,
                    &value,
                    "read_signal_",
                )
            }
        }
    }
    if let Some(value) = normalized_filter(filters.annotation.as_deref()) {
        if signal_source == PreviewSignalPredicateSource::ScopeProjection
            && matches!(value.as_str(), "needs-review" | "no-issues")
        {
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
