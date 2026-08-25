fn performance_selection_preview_row(id: i64, selected: bool) -> ImportPreviewRow {
    ImportPreviewRow {
        id,
        version: 1,
        session_id: "selection-hash-session".to_string(),
        user_id: 1,
        preview_date: "2026-07-13 09:00:00".to_string(),
        preview_type: "支出".to_string(),
        preview_amount_cents: 1_000,
        preview_destination_amount_cents: 0,
        category_id: Some(42),
        preview_main_category: "餐饮".to_string(),
        preview_sub_category: "午餐".to_string(),
        preview_source_account_id: Some(11),
        preview_destination_account_id: None,
        preview_counterparty: "基准商户".to_string(),
        preview_payment_method: "测试账户".to_string(),
        preview_description: format!("selection hash fixture {id}"),
        preview_parser_id: "fixture".to_string(),
        preview_parser_tags: Vec::new(),
        preview_recurring_id: None,
        preview_recurring_name: String::new(),
        preview_recurring_candidate_count: 0,
        preview_recurring_match_score: 0.0,
        preview_recurring_match_reasons: String::new(),
        preview_recurring_matched_date: String::new(),
        preview_selected: selected,
        dedup_type: String::new(),
        dedup_source_ids: Vec::new(),
        preview_matching_feedback: json!({}),
        created_at: "2026-07-13 09:00:00".to_string(),
    }
}

#[test]
fn preview_metadata_selection_hash_is_nonempty_and_tracks_selected_ids() {
    let first_selection = build_preview_page_result_from_rows(
        vec![
            performance_selection_preview_row(101, true),
            performance_selection_preview_row(102, false),
        ],
        &ImportPreviewPageRequest::default(),
    );
    let second_selection = build_preview_page_result_from_rows(
        vec![
            performance_selection_preview_row(101, true),
            performance_selection_preview_row(102, true),
        ],
        &ImportPreviewPageRequest::default(),
    );

    assert_eq!(
        first_selection.metadata.selection_hash,
        preview_id_snapshot_hash(&[101]),
        "metadata must expose the canonical selected-ID snapshot used by action_scope"
    );
    assert_eq!(first_selection.metadata.counts.selected_total, 1);
    assert_eq!(
        second_selection.metadata.selection_hash,
        preview_id_snapshot_hash(&[101, 102]),
        "selection_hash must change when the selected set changes"
    );
    assert_eq!(second_selection.metadata.counts.selected_total, 2);
}

#[test]
fn preview_metadata_empty_rows_keep_canonical_zero_state() {
    let result = build_preview_page_result_from_rows(
        Vec::new(),
        &ImportPreviewPageRequest {
            page: 0,
            page_size: 0,
            ..ImportPreviewPageRequest::default()
        },
    );

    assert_eq!(result.page, 1);
    assert_eq!(result.page_size, 1);
    assert_eq!(result.total, 0);
    assert!(result.rows.is_empty());
    assert_eq!(result.metadata.counts.total, 0);
    assert_eq!(result.metadata.counts.selected, 0);
    assert_eq!(result.metadata.counts.selected_total, 0);
    assert_eq!(result.metadata.counts.selected_invalid, 0);
    assert_eq!(result.metadata.selection_hash, preview_id_snapshot_hash(&[]));
    assert!(result.metadata.counts.signals.values().all(|count| *count == 0));
    assert!(result.metadata.facets.categories.is_empty());
    assert!(result.metadata.facets.accounts.is_empty());
    assert!(result.metadata.facets.tags.is_empty());
}

#[test]
fn preview_metadata_selection_snapshot_is_independent_of_filter_and_page() {
    let mut hidden_selected = performance_selection_preview_row(301, true);
    hidden_selected.preview_description = "hidden selected".to_string();
    let mut visible_unselected = performance_selection_preview_row(302, false);
    visible_unselected.preview_description = "visible row".to_string();

    let result = build_preview_page_result_from_rows(
        vec![hidden_selected, visible_unselected],
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 1,
            sort_by: "time".to_string(),
            sort_direction: "desc".to_string(),
            filters: ImportPreviewQueryFilters {
                description: Some("visible".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    );

    assert_eq!(result.total, 1);
    assert_eq!(result.rows.iter().map(|row| row.id).collect::<Vec<_>>(), vec![302]);
    assert_eq!(result.metadata.counts.selected, 0);
    assert_eq!(result.metadata.counts.selected_total, 1);
    assert_eq!(result.metadata.selection_hash, preview_id_snapshot_hash(&[301]));
}

fn performance_function_slice<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_offset = source
        .find(start)
        .unwrap_or_else(|| panic!("missing function marker {start}"));
    let remaining = &source[start_offset..];
    let end_offset = remaining
        .find(end)
        .unwrap_or_else(|| panic!("missing function end marker {end}"));
    &remaining[..end_offset]
}

#[test]
fn preview_page_metadata_uses_one_bounded_database_round_trip() {
    let source = include_str!("preview_metadata_query.rs");
    let metadata_builder = performance_function_slice(
        source,
        "async fn build_preview_metadata_for_query",
        "async fn query_preview_metadata_aggregate",
    );
    let aggregate_query = performance_function_slice(
        source,
        "async fn query_preview_metadata_aggregate",
        "fn build_preview_metadata_aggregate_query",
    );
    let estimated_metadata_round_trips = aggregate_query
        .matches("fetch_one(&mut *connection).await")
        .count();

    assert_eq!(
        metadata_builder.matches(".await").count(),
        1,
        "metadata facade must await only the bounded aggregate query"
    );
    assert_eq!(
        estimated_metadata_round_trips,
        1,
        "preview metadata currently performs about {estimated_metadata_round_trips} SQL round-trips; counts, selection hash, and facets must come from one bounded aggregate query"
    );
}

#[test]
fn preview_page_total_is_reused_from_the_metadata_aggregate() {
    let source = include_str!("preview_query.rs");
    let page_query = performance_function_slice(
        source,
        "pub fn query_preview_page_by_session",
        "pub fn preview_id_snapshot_hash",
    );

    assert!(
        !page_query.contains("count_preview_rows_by_query"),
        "the page route must not rescan every complex filter only to recompute the total already owned by metadata"
    );
    assert!(
        page_query.contains("metadata.counts.total"),
        "the result total must come from the same metadata snapshot"
    );
}

#[test]
fn preview_metadata_materializes_expensive_signal_projection_once() {
    let mut query = build_preview_metadata_aggregate_query(
        1,
        2,
        &ImportPreviewQueryFilters {
            signal: Some("parser".to_string()),
            annotation: Some("needs-review".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    let sql = query.build().sql().to_string();

    assert!(sql.contains("WITH preview_scope AS MATERIALIZED"));
    assert!(sql.contains("import_preview_signal_flags"));
    assert_eq!(
        sql.matches("FROM import_preview_rows").count(),
        1,
        "metadata must decode the six signal families and review state from each session row only once"
    );
    for projected_column in [
        "read_signal_parser",
        "read_signal_platform_duplicate",
        "read_signal_transfer",
        "read_signal_history",
        "read_signal_learning",
        "read_signal_llm",
        "needs_review",
    ] {
        assert!(sql.contains(projected_column), "missing {projected_column}");
    }
    assert!(!sql.contains("signal_flags.parser AS signal_parser"));
}

#[test]
fn preview_metadata_scans_projected_scope_once_for_core_aggregates() {
    let mut query = build_preview_metadata_aggregate_query(
        1,
        2,
        &ImportPreviewQueryFilters {
            signal: Some("transfer".to_string()),
            category: Some("__invalid__".to_string()),
            ..ImportPreviewQueryFilters::default()
        },
    );
    let sql = query.build().sql().to_string();

    assert_eq!(
        sql.matches("FROM preview_scope p").count(),
        4,
        "counts, the full selection snapshot, and all six signal counts must share one scan; only the three bounded facet aggregates may rescan preview_scope"
    );
    assert!(
        sql.contains("COUNT(*) FILTER (WHERE TRUE"),
        "core preview metadata must use conditional aggregation instead of scalar subquery rescans"
    );
}

#[test]
fn preview_metadata_reuses_materialized_identity_labels_for_all_facets() {
    let mut query = build_preview_metadata_aggregate_query(
        1,
        2,
        &ImportPreviewQueryFilters::default(),
    );
    let sql = query.build().sql().to_string();

    assert!(sql.contains("facet_category_id"));
    assert!(sql.contains("facet_category_path"));
    assert!(sql.contains("facet_source_account_id"));
    assert!(sql.contains("facet_source_account_name"));
    assert!(sql.contains("facet_target_account_id"));
    assert!(sql.contains("facet_target_account_name"));
    assert_eq!(
        sql.matches("JOIN categories").count(),
        1,
        "category identity and facet labels must share the session-scope join"
    );
    assert_eq!(
        sql.matches("JOIN accounts").count(),
        2,
        "source and destination identity joins must also supply account facet labels"
    );
}
