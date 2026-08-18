#[test]
fn preview_sql_query_builder_covers_server_filter_and_sort_contract() {
    let filters = ImportPreviewQueryFilters {
        min_datetime: Some("2026-01-01".to_string()),
        max_datetime: Some("2026-01-31".to_string()),
        transaction_type: Some("支出".to_string()),
        category: Some("42".to_string()),
        account: Some("__none__".to_string()),
        tag: Some("工资".to_string()),
        signal: Some("learning:needs_review".to_string()),
        annotation: Some("missing_category".to_string()),
        description: Some("手续费".to_string()),
        selected_only: true,
    };
    let mut query = build_preview_page_query(1, 2, &filters, "sourceAmountCents", "desc", 50, 100);

    let built = query.build();
    let sql = built.sql();

    assert!(sql.contains("JOIN import_sessions"));
    assert!(sql.contains("p.selected = true"));
    assert!(sql.contains("p.category_id = "));
    assert!(!sql.contains("preview_main_category"));
    assert!(!sql.contains("preview_sub_category"));
    assert!(sql.contains("p.account_id IS NULL"));
    assert!(sql.contains("COALESCE(NULLIF(LOWER("));
    assert!(sql.contains("preview_matching_feedback,learning,review_status"));
    assert!(sql.contains("preview_matching_feedback' ? 'learning'"));
    assert!(sql.contains("preview_matching_feedback' ? 'transfer'"));
    assert!(sql.contains("learning_level"));
    assert!(!sql.contains("preview_matching_feedback')::text ILIKE"));
    assert!(sql.contains("ORDER BY p.amount_cents DESC"));
    assert!(sql.contains("LIMIT"));
    assert!(sql.contains("OFFSET"));

    let mut count_query = build_preview_count_query(1, 2, &filters);
    let count_sql = count_query.build().sql().to_string();
    assert!(count_sql.contains("SELECT COUNT(*)::BIGINT"));
    assert!(count_sql.contains("p.selected = true"));
}

#[test]
fn preview_sql_query_builder_covers_none_category_and_account_id_filters() {
    let filters = ImportPreviewQueryFilters {
        category: Some("__none__".to_string()),
        account: Some("11".to_string()),
        signal: Some("learning".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(1_i64);
    push_preview_query_predicates(&mut query, &filters, "p");
    push_preview_order_by(&mut query, "type", "asc");

    let built = query.build();
    let sql = built.sql();

    assert!(sql.contains("p.category_id IS NULL"));
    assert!(sql.contains("p.account_id = "));
    assert!(sql.contains("p.transfer_target_account_id = "));
    assert!(sql.contains("preview_matching_feedback' ? 'learning'"));
    assert!(sql.contains("learning_level"));
    assert!(sql.contains("candidate_type"));
    assert!(sql.contains("suppressed"));
    assert!(!sql.contains("preview_matching_feedback')::text ILIKE"));
    assert!(sql.contains("CASE lower(p.transaction_type)"));
}

#[test]
fn preview_sql_learning_filter_includes_transfer_learning_evidence() {
    let filters = ImportPreviewQueryFilters {
        signal: Some("learning".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(1_i64);
    push_preview_query_predicates(&mut query, &filters, "p");

    let sql = query.build().sql().to_string();
    assert!(sql.contains("preview_matching_feedback' ? 'learning'"));
    assert!(sql.contains("learning_level"));
    assert!(sql.contains("candidate_type"));
    assert!(sql.contains("suppressed"));
    assert!(sql.contains("yellow"));
    assert!(sql.contains("green"));
    assert!(sql.contains("blue"));
}

#[test]
fn preview_sql_query_builder_filters_parser_by_visible_signal_family() {
    let filters = ImportPreviewQueryFilters {
        signal: Some("parser".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(1_i64);
    push_preview_query_predicates(&mut query, &filters, "p");

    let sql = query.build().sql().to_string();

    assert!(sql.contains("preview_parser_id"));
    assert!(sql.contains("NOT"));
    assert!(sql.contains("platform_bank"));
    assert!(!sql.contains("transfer_cross_batch"));
    assert!(sql.contains("planned_operation"));
    assert!(sql.contains("preview_matching_feedback' ? 'learning'"));
    assert!(sql.contains("preview_matching_feedback' ? 'llm'"));
    assert!(!sql.contains("preview_matching_feedback')::text ILIKE"));
}

#[test]
fn preview_sql_query_builder_filters_transfer_by_feedback_signal_only() {
    let filters = ImportPreviewQueryFilters {
        signal: Some("transfer".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(1_i64);
    push_preview_query_predicates(&mut query, &filters, "p");

    let sql = query.build().sql().to_string();

    assert!(sql.contains("preview_matching_feedback' ? 'transfer'"));
    assert!(sql.contains("review_status"));
    assert!(sql.contains("candidate_type"));
    assert!(sql.contains("IN ('pending', 'accepted', 'auto_applied', 'auto-applied') OR ("));
    assert!(!sql.contains("IN ('pending', 'accepted', 'rejected'"));
    assert!(sql.contains("= '' AND LOWER"));
    assert!(sql.contains("jsonb_typeof"));
    assert!(sql.contains("candidate_type}') = 'string'"));
    assert!(sql.contains("reason}') = 'string'"));
    assert!(!sql.contains("transfer_cross_batch"));
    assert!(!sql.contains("IN ('transfer'"));
    assert!(!sql.contains("transaction_type"));
    assert_sql_parentheses_balanced(&sql);
}

fn assert_sql_parentheses_balanced(sql: &str) {
    let mut depth = 0_i32;
    let mut in_single_quoted_string = false;
    let mut chars = sql.chars().peekable();

    while let Some(character) = chars.next() {
        if character == '\'' {
            if in_single_quoted_string && chars.peek() == Some(&'\'') {
                chars.next();
            } else {
                in_single_quoted_string = !in_single_quoted_string;
            }
            continue;
        }
        if in_single_quoted_string {
            continue;
        }

        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                assert!(depth >= 0, "SQL closes a parenthesis before one is opened: {sql}");
            }
            _ => {}
        }
    }

    assert!(!in_single_quoted_string, "SQL contains an unterminated string: {sql}");
    assert_eq!(depth, 0, "SQL contains unbalanced parentheses: {sql}");
}

#[test]
fn preview_sql_query_builder_rejects_raw_identity_label_filters() {
    let filters = ImportPreviewQueryFilters {
        category: Some("餐饮".to_string()),
        account: Some("现金钱包".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(1_i64);
    push_preview_query_predicates(&mut query, &filters, "p");

    let sql = query.build().sql().to_string();

    assert!(sql.contains("AND FALSE"));
    assert!(!sql.contains("category_id::text"));
    assert!(!sql.contains("account_id::text"));
}

#[test]
fn preview_sql_query_builder_preserves_invalid_sentinel_semantics() {
    let filters = ImportPreviewQueryFilters {
        category: Some("__invalid__".to_string()),
        account: Some("__invalid__".to_string()),
        tag: Some("__invalid__".to_string()),
        annotation: Some("needs-review".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(1_i64);
    push_preview_query_predicates(&mut query, &filters, "p");

    let sql = query.build().sql().to_string();

    assert!(sql.contains("lower(p.transaction_type) IN ('收入'"));
    assert!(sql.contains("p.category_id IS NULL"));
    assert!(sql.contains("NOT EXISTS (SELECT 1 FROM categories c"));
    assert!(sql.contains("preview_matching_feedback,annotation,manual_fields,category_id"));
    assert!(sql.contains("= 'true'::jsonb"));
    assert!(sql.contains("NOT EXISTS (SELECT 1 FROM accounts a"));
    assert!(sql.contains("jsonb_array_length"));
    assert!(sql.contains("p.transfer_target_account_id IS NULL"));
    assert!(sql.contains("identity_validation"));
}

#[test]
fn preview_identity_feedback_predicate_coalesces_missing_and_preserves_nonempty_issues() {
    let mut query = QueryBuilder::<Postgres>::new("SELECT ");
    push_preview_identity_feedback_condition(&mut query, "p");

    let sql = query.build().sql().to_string();

    assert!(sql.contains("COALESCE((jsonb_typeof("));
    assert!(sql.contains("identity_validation,issues}') = 'array'"));
    assert!(sql.contains("identity_validation,issues}') > 0), false)"));
}

#[test]
fn preview_selection_update_query_covers_modes_filters_and_ids() {
    let request = ImportPreviewPageRequest {
        preview_ids: vec![11, 12],
        filters: ImportPreviewQueryFilters {
            category: Some("42".to_string()),
            selected_only: true,
            ..ImportPreviewQueryFilters::default()
        },
        ..ImportPreviewPageRequest::default()
    };

    let mut select_query = build_preview_selection_update_query(
        1,
        2,
        ImportPreviewSelectionMode::Select,
        ImportPreviewSelectionTarget::All,
        &request,
    );
    let select_sql = select_query.build().sql().to_string();
    assert!(select_sql.contains("UPDATE import_preview_rows p SET selected ="));
    assert!(select_sql.contains("jsonb_set"));
    assert!(!select_sql.contains("signal_projection_version"));
    assert!(select_sql.contains("p.selected = true"));
    assert!(select_sql.contains("p.category_id = "));
    assert!(select_sql.contains("p.id IN"));

    let mut deselect_query = build_preview_selection_update_query(
        1,
        2,
        ImportPreviewSelectionMode::Deselect,
        ImportPreviewSelectionTarget::All,
        &ImportPreviewPageRequest::default(),
    );
    let deselect_sql = deselect_query.build().sql().to_string();
    assert!(deselect_sql.contains("to_jsonb($"));

    let mut invert_query = build_preview_selection_update_query(
        1,
        2,
        ImportPreviewSelectionMode::Invert,
        ImportPreviewSelectionTarget::All,
        &ImportPreviewPageRequest::default(),
    );
    let invert_sql = invert_query.build().sql().to_string();
    assert!(invert_sql.contains("NOT p.selected"));

    let mut needs_review_query = build_preview_selection_update_query(
        1,
        2,
        ImportPreviewSelectionMode::Select,
        ImportPreviewSelectionTarget::NeedsReview,
        &ImportPreviewPageRequest::default(),
    );
    let needs_review_sql = needs_review_query.build().sql().to_string();
    assert!(needs_review_sql.contains("p.category_id IS NULL"));
    assert!(needs_review_sql.contains("p.account_id IS NULL"));
    assert!(needs_review_sql.contains("p.transfer_target_account_id IS NULL"));

    let mut valid_query = build_preview_selection_update_query(
        1,
        2,
        ImportPreviewSelectionMode::Select,
        ImportPreviewSelectionTarget::Valid,
        &ImportPreviewPageRequest::default(),
    );
    let valid_sql = valid_query.build().sql().to_string();
    assert!(valid_sql.contains("AND NOT ("));
}

#[test]
fn bulk_insert_query_builders_preserve_insert_shapes() {
    let preview_draft = ImportPreviewDraft {
        preview_date: "2026-01-01 09:00:00".to_string(),
        preview_type: "支出".to_string(),
        preview_amount_cents: 1000,
        category_id: Some(42),
        preview_counterparty: "商户".to_string(),
        preview_payment_method: "招商卡".to_string(),
        preview_description: "备注".to_string(),
        dedup_source_ids: vec![1, 2],
        ..ImportPreviewDraft::default()
    };
    let preview_values = vec![
        preview_row_batch_value_from_draft(&preview_draft).expect("preview batch value")
    ];
    let mut preview_builder = build_preview_rows_insert_query(1, 2, &preview_values);
    let preview_query = preview_builder.build();
    let preview_sql = preview_query.sql();
    assert!(preview_sql.contains("INSERT INTO import_preview_rows"));
    assert!(preview_sql.contains("category_id"));
    assert!(preview_sql.contains("preview_payload"));
    assert!(preview_sql.contains("signal_projection_version"));
    assert!(preview_sql.contains("signal_parser"));

    let parser_draft = ImportParserTemplateDraft {
        parser_date: "2026-01-01 09:00:00".to_string(),
        parser_amount: 10.0,
        parser_type: "收入".to_string(),
        parser_id: "fixture".to_string(),
        ..ImportParserTemplateDraft::default()
    };
    let standard_values = vec![standard_row_batch_value_from_parser_template(
        3,
        4,
        &parser_draft,
    )];
    let mut standard_builder = build_standard_rows_insert_query(1, 2, &standard_values);
    let standard_query = standard_builder.build();
    let standard_sql = standard_query.sql();
    assert!(standard_sql.contains("INSERT INTO import_standard_rows"));
    assert!(standard_sql.contains("ON CONFLICT (source_id, source_row_index)"));
    assert!(standard_sql.contains("standard_payload"));
    let mut single_insert_builder = build_preview_row_insert_returning_query(
        1,
        2,
        &preview_draft,
        json!({"category_id": 42}).to_string(),
        import_preview_signal_projection_from_payload(&json!({"category_id": 42}))
            .expect("signal projection"),
        1000,
        "expense",
    );
    let single_insert_sql = single_insert_builder.build().sql().to_string();
    assert!(single_insert_sql.contains("INSERT INTO import_preview_rows"));
    assert!(single_insert_sql.contains("category_id"));
    assert!(single_insert_sql.contains("signal_projection_version"));
    assert!(single_insert_sql.contains("signal_llm"));
    assert!(single_insert_sql.contains("RETURNING id"));

    let mut preview = preview_row(7);
    preview.preview_type = "转账".to_string();
    preview.category_id = Some(42);
    let mut update_builder = build_preview_row_update_query(
        &preview,
        json!({"category_id": 42}).to_string(),
        import_preview_signal_projection_from_payload(&json!({"category_id": 42}))
            .expect("signal projection"),
        1000,
        "expense",
        PreviewRowUpdateTarget {
            preview_id: 7,
            session_db_id: 1,
            user_id: 2,
            expected_row_version: None,
        },
    );
    let update_sql = update_builder.build().sql().to_string();
    assert!(update_sql.contains("UPDATE import_preview_rows"));
    assert!(update_sql.contains("category_id ="));
    assert!(update_sql.contains("signal_projection_version ="));
    assert!(update_sql.contains("signal_transfer ="));
    assert!(!update_sql.contains("hidden_transfer_payload = '{}'::jsonb"));
    assert!(update_sql.contains("WHERE id ="));

    preview.preview_type = "支出".to_string();
    let mut demoted_update_builder = build_preview_row_update_query(
        &preview,
        json!({"preview_type": "支出"}).to_string(),
        import_preview_signal_projection_from_payload(&json!({"preview_type": "支出"}))
            .expect("signal projection"),
        1000,
        "expense",
        PreviewRowUpdateTarget {
            preview_id: 7,
            session_db_id: 1,
            user_id: 2,
            expected_row_version: Some(11),
        },
    );
    let demoted_update_sql = demoted_update_builder.build().sql().to_string();
    assert!(demoted_update_sql.contains("hidden_transfer_payload = '{}'::jsonb"));
    assert!(demoted_update_sql.contains("AND version ="));
}

#[test]
fn signal_projection_shadow_sql_is_read_only_and_covers_every_family() {
    let normalized = IMPORT_PREVIEW_SIGNAL_PROJECTION_PARITY_SQL.to_ascii_lowercase();

    assert_eq!(IMPORT_PREVIEW_SIGNAL_SHADOW_SAMPLE_SIZE, 64);
    assert!(normalized.trim_start().starts_with("select "));
    assert!(!normalized.contains(" update "));
    assert!(!normalized.contains(" insert "));
    assert!(!normalized.contains(" delete "));
    assert!(normalized.contains("import_preview_signal_flags(p.preview_payload)"));
    for column in [
        "signal_parser",
        "signal_platform_duplicate",
        "signal_transfer",
        "signal_history",
        "signal_learning",
        "signal_llm",
    ] {
        assert!(normalized.contains(column), "missing parity check for {column}");
    }
}

#[test]
fn preview_sort_accepts_frontend_server_paged_keys() {
    let mut earlier = preview_row(1);
    earlier.preview_date = "2026-01-01 09:00:00".to_string();
    earlier.preview_type = "支出".to_string();
    earlier.preview_amount_cents = 3000;
    earlier.preview_payment_method = "B卡".to_string();
    earlier.preview_description = "bbb".to_string();

    let mut later = preview_row(2);
    later.preview_date = "2026-01-02 09:00:00".to_string();
    later.preview_type = "收入".to_string();
    later.preview_amount_cents = 1000;
    later.preview_payment_method = "A卡".to_string();
    later.preview_description = "aaa".to_string();

    let mut rows = vec![earlier.clone(), later.clone()];
    sort_preview_rows(&mut rows, "time", "desc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 1]
    );

    let mut rows = vec![earlier.clone(), later.clone()];
    sort_preview_rows(&mut rows, "sourceAmountCents", "asc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 1]
    );

    let mut rows = vec![earlier.clone(), later.clone()];
    sort_preview_rows(&mut rows, "type", "asc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 1]
    );

    let mut rows = vec![earlier.clone(), later.clone()];
    sort_preview_rows(&mut rows, "paymentMethod", "asc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 1]
    );

    let mut rows = vec![earlier, later];
    sort_preview_rows(&mut rows, "comment", "asc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![2, 1]
    );

    let mut fallback_earlier = preview_row(1);
    fallback_earlier.preview_date = "2026-01-01 09:00:00".to_string();
    let mut fallback_later = preview_row(2);
    fallback_later.preview_date = "2026-01-02 09:00:00".to_string();
    let mut rows = vec![fallback_later, fallback_earlier];
    sort_preview_rows(&mut rows, "unknown", "asc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2]
    );

    let mut transfer = preview_row(1);
    transfer.preview_type = "转账".to_string();
    let mut investment = preview_row(2);
    investment.preview_type = "投资".to_string();
    let mut rows = vec![investment, transfer];
    sort_preview_rows(&mut rows, "type", "asc");
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
}
#[test]
fn database_counts_reject_negative_values_instead_of_returning_sentinels() {
    assert_eq!(preview_metadata_database_count(3, "total_count").unwrap(), 3);
    assert!(preview_metadata_database_count(-1, "total_count").is_err());
    assert_eq!(
        decision_group_database_count(2, "preview member").unwrap(),
        2
    );
    assert!(decision_group_database_count(-1, "preview member").is_err());
}

#[test]
fn typed_signal_read_queries_use_versioned_columns_without_legacy_projection() {
    let filters = ImportPreviewQueryFilters {
        signal: Some("learning:pending".to_string()),
        ..ImportPreviewQueryFilters::default()
    };
    let mut page_query = build_preview_page_query_with_signal_read_source(
        1,
        2,
        PreviewPageQuerySpec {
            filters: &filters,
            sort_by: "time",
            sort_direction: "asc",
            page_size: 50,
            offset: 0,
            signal_read_source: PreviewSignalReadSource::TypedV1,
        },
    );
    let page_sql = page_query.build().sql().to_string();
    assert!(page_sql.contains("p.signal_projection_version ="));
    assert!(page_sql.contains("p.signal_learning = true"));
    assert!(page_sql.contains("preview_matching_feedback,learning,review_status"));
    assert!(!page_sql.contains("import_preview_signal_flags"));

    let mut metadata_query = build_preview_metadata_aggregate_query_with_signal_read_source(
        1,
        2,
        &filters,
        PreviewSignalReadSource::TypedV1,
    );
    let metadata_sql = metadata_query.build().sql().to_string();
    for family in [
        "parser",
        "platform_duplicate",
        "transfer",
        "history",
        "learning",
        "llm",
    ] {
        assert!(metadata_sql.contains(&format!(
            "p.signal_{family} AS read_signal_{family}"
        )));
    }
    assert!(metadata_sql.contains("p.signal_projection_version ="));
    assert!(metadata_sql.contains("p.read_signal_learning"));
    assert!(!metadata_sql.contains("import_preview_signal_flags"));
    assert!(!metadata_sql.contains("legacy_signal_"));
}
