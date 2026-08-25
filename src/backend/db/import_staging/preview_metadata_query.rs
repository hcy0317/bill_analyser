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

fn push_preview_materialized_signal_predicate(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
    value: &str,
    column_prefix: &str,
) {
    query.push(" AND ");
    let filter = normalize_visible_signal_filter(value);
    let (family, status) = signal_filter_family_status(&filter)
        .map(|(family, status)| (family, Some(status)))
        .unwrap_or((filter.as_str(), None));
    let column = match ImportPreviewSignalFamily::parse(family) {
        Some(ImportPreviewSignalFamily::Parser) => "parser",
        Some(ImportPreviewSignalFamily::PlatformDuplicate) => "platform_duplicate",
        Some(ImportPreviewSignalFamily::Transfer) => "transfer",
        Some(ImportPreviewSignalFamily::History) => "history",
        Some(ImportPreviewSignalFamily::Learning) => "learning",
        Some(ImportPreviewSignalFamily::Llm) => "llm",
        None => {
            query.push("FALSE");
            return;
        }
    };
    query.push(alias);
    query.push(".");
    query.push(column_prefix);
    query.push(column);
    query.push(" = true");
    if let Some(status) = status {
        query.push(" AND ");
        push_preview_feedback_family_text_search(query, alias, family, status);
    }
}

#[cfg(test)]
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
    connection: &mut sqlx::PgConnection,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
    preview_ids: &[i64],
    signal_read_source: PreviewSignalReadSource,
) -> DbResult<ImportPreviewMetadata> {
    query_preview_metadata_aggregate(
        connection,
        session_db_id,
        user_id,
        filters,
        preview_ids,
        signal_read_source,
    )
    .await
}

async fn query_preview_metadata_aggregate(
    connection: &mut sqlx::PgConnection,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
    preview_ids: &[i64],
    signal_read_source: PreviewSignalReadSource,
) -> DbResult<ImportPreviewMetadata> {
    let mut query = build_preview_metadata_aggregate_query_with_signal_read_source(
        session_db_id,
        user_id,
        filters,
        preview_ids,
        signal_read_source,
    );
    let row = query.build().fetch_one(&mut *connection).await?;
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

#[cfg(test)]
fn build_preview_metadata_aggregate_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> QueryBuilder<'static, Postgres> {
    build_preview_metadata_aggregate_query_with_signal_read_source(
        session_db_id,
        user_id,
        filters,
        &[],
        PreviewSignalReadSource::LegacyPayload,
    )
}

fn build_preview_metadata_aggregate_query_with_signal_read_source(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
    preview_ids: &[i64],
    signal_read_source: PreviewSignalReadSource,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new("WITH preview_scope AS MATERIALIZED (SELECT p.*, ");
    match signal_read_source {
        PreviewSignalReadSource::LegacyPayload => query.push("signal_flags.parser AS read_signal_parser, signal_flags.platform_duplicate AS read_signal_platform_duplicate, signal_flags.transfer AS read_signal_transfer, signal_flags.history AS read_signal_history, signal_flags.learning AS read_signal_learning, signal_flags.llm AS read_signal_llm, "),
        PreviewSignalReadSource::TypedV1 => query.push("p.signal_parser AS read_signal_parser, p.signal_platform_duplicate AS read_signal_platform_duplicate, p.signal_transfer AS read_signal_transfer, p.signal_history AS read_signal_history, p.signal_learning AS read_signal_learning, p.signal_llm AS read_signal_llm, "),
    };
    query.push("c.id AS facet_category_id, c.path AS facet_category_path, c.name AS facet_category_name, source_account.id AS facet_source_account_id, source_account.name AS facet_source_account_name, target_account.id AS facet_target_account_id, target_account.name AS facet_target_account_name, ");
    push_preview_current_review_condition_with_joins(
        &mut query,
        "p",
        "matching_feedback.read_matching_feedback",
    );
    query.push(" AS needs_review FROM import_preview_rows p CROSS JOIN LATERAL (SELECT p.preview_payload->'preview_matching_feedback' AS read_matching_feedback OFFSET 0) matching_feedback");
    if signal_read_source == PreviewSignalReadSource::LegacyPayload {
        query.push(" CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(p.preview_payload)) AS signal_flags(parser BOOLEAN, platform_duplicate BOOLEAN, transfer BOOLEAN, history BOOLEAN, learning BOOLEAN, llm BOOLEAN)");
    }
    query.push(" LEFT JOIN categories c ON c.user_id = p.user_id AND c.id = p.category_id AND c.is_active = true LEFT JOIN accounts source_account ON source_account.user_id = p.user_id AND source_account.id = p.account_id AND source_account.is_active = true LEFT JOIN accounts target_account ON target_account.user_id = p.user_id AND target_account.id = p.transfer_target_account_id AND target_account.is_active = true WHERE p.session_id = ");
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_id_scope(&mut query, preview_ids, "p");
    if signal_read_source == PreviewSignalReadSource::TypedV1 {
        query.push(" AND p.signal_projection_version = ");
        query.push_bind(1_i16);
    }
    query.push("), core_aggregates AS (SELECT ");
    query.push("COUNT(*) FILTER (WHERE TRUE");
    push_preview_metadata_query_predicates(&mut query, filters, "p");
    query.push(")::BIGINT AS total_count, ");

    let mut selected_filters = filters.clone();
    selected_filters.selected_only = true;
    query.push("COUNT(*) FILTER (WHERE TRUE");
    push_preview_metadata_query_predicates(&mut query, &selected_filters, "p");
    query.push(")::BIGINT AS selected_count, ");

    let mut selected_invalid_filters = selected_filters;
    selected_invalid_filters.annotation = Some("needs-review".to_string());
    query.push("COUNT(*) FILTER (WHERE TRUE");
    push_preview_metadata_query_predicates(&mut query, &selected_invalid_filters, "p");
    query.push(")::BIGINT AS selected_invalid, ");

    query.push("COALESCE(array_agg(p.id ORDER BY p.id) FILTER (WHERE p.selected = true), ARRAY[]::BIGINT[]) AS selected_ids, ");
    push_preview_signal_counts_aggregate(&mut query, filters);
    query.push(" AS signal_counts FROM preview_scope p) SELECT core_aggregates.total_count, core_aggregates.selected_count, core_aggregates.selected_invalid, core_aggregates.selected_ids, core_aggregates.signal_counts, ");
    push_preview_category_facets_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS category_facets, ");
    push_preview_account_facets_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS account_facets, ");
    push_preview_tag_facets_aggregate(&mut query, session_db_id, user_id, filters);
    query.push(" AS tag_facets FROM core_aggregates");
    query
}

fn push_preview_signal_counts_aggregate(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
) {
    query.push("jsonb_build_object(");
    for (index, family) in IMPORT_PREVIEW_VISIBLE_SIGNAL_FAMILIES.iter().enumerate() {
        if index > 0 {
            query.push(", ");
        }
        let mut signal_filters = filters.clone();
        signal_filters.signal = Some((*family).to_string());
        query.push_bind((*family).to_string());
        query.push(", COUNT(*) FILTER (WHERE TRUE");
        push_preview_metadata_query_predicates(query, &signal_filters, "p");
        query.push(")::BIGINT");
    }
    query.push(")");
}

fn push_preview_category_facets_aggregate(
    query: &mut QueryBuilder<'_, Postgres>,
    _session_db_id: i64,
    _user_id: i64,
    filters: &ImportPreviewQueryFilters,
) {
    let mut facet_filters = filters.clone();
    facet_filters.category = None;
    query.push("COALESCE((SELECT jsonb_agg(jsonb_build_object('value', facet_rows.value, 'label', facet_rows.label, 'count', facet_rows.count) ORDER BY facet_rows.count DESC, facet_rows.label ASC) FROM (SELECT p.facet_category_id::text AS value, COALESCE(NULLIF(p.facet_category_path, ''), p.facet_category_name, p.facet_category_id::text) AS label, COUNT(*)::BIGINT AS count FROM preview_scope p WHERE p.facet_category_id IS NOT NULL");
    push_preview_metadata_query_predicates(query, &facet_filters, "p");
    query.push(" GROUP BY p.facet_category_id, p.facet_category_path, p.facet_category_name ORDER BY count DESC, label ASC LIMIT ");
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
    query.push("COALESCE((SELECT jsonb_agg(jsonb_build_object('value', facet_rows.value, 'label', facet_rows.label, 'count', facet_rows.count) ORDER BY facet_rows.count DESC, facet_rows.label ASC) FROM (SELECT account_values.account_id::text AS value, COALESCE(account_values.account_name, account_values.account_id::text) AS label, COUNT(*)::BIGINT AS count FROM preview_scope p CROSS JOIN LATERAL (VALUES (p.facet_source_account_id, p.facet_source_account_name), (p.facet_target_account_id, p.facet_target_account_name)) AS account_values(account_id, account_name) WHERE account_values.account_id IS NOT NULL");
    push_preview_metadata_query_predicates(query, &facet_filters, "p");
    query.push(" GROUP BY account_values.account_id, account_values.account_name ORDER BY count DESC, label ASC LIMIT ");
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
