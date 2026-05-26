use std::{collections::BTreeSet, error::Error};

use bill_analyser_core::{
    build_import_history_rewrite_ack_token, build_import_history_rewrite_operation_id, DedupBill,
    Money, SmartDeduplicationEngine, UserId, HISTORY_REWRITE_NOTICE,
};
use bill_analyser_db::{
    apply_preview_learning_decision, apply_preview_llm_recommendation,
    apply_preview_patches_preserving_selection, apply_preview_transfer_decision,
    batch_update_preview_classification, calculate_import_bill_hash,
    clear_import_preview_materialization_state, clear_session_data, confirm_preview_to_bills,
    confirm_preview_to_bills_with_ack, count_preview_by_session, create_import_session,
    dedup_bills_from_parser_templates, get_import_annotation_samples,
    get_import_decision_groups_by_session, get_import_history_candidate_bills_for_session,
    get_import_history_materializations_by_session, get_import_learning_lifecycle_view,
    get_import_session, get_import_sources_by_session, get_import_standard_rows_by_session,
    get_llm_memory_events, get_parser_templates_by_session, get_preview_bill_by_id,
    get_preview_by_ids, get_preview_by_session, get_preview_filter_index_by_session,
    get_preview_page_by_session, get_unprocessed_templates_for_dedup, init_import_staging_schema,
    insert_import_decision_groups_batch, insert_import_history_materializations_batch,
    insert_parser_templates_batch, insert_preview_bill, insert_preview_bills_batch,
    mark_unprocessed_parser_templates_processed_for_session,
    parser_template_drafts_from_standard_bills, preview_drafts_from_dedup_bills,
    query_preview_page_by_session, record_import_learning_lifecycle_feedback,
    reset_session_preview_selection, review_preview_llm_recommendation,
    save_import_annotation_samples, stage_import_parser_templates,
    stage_import_parser_templates_with_sources, update_import_session_status,
    update_parser_template_status, update_preview_bill, update_preview_bills_batch,
    update_preview_recurring_match_decision, update_preview_selection,
    update_session_preview_selection_by_query, ImportAnnotationSampleDraft,
    ImportDecisionGroupDraft, ImportDecisionGroupMemberDraft, ImportHistoryMaterializationDraft,
    ImportHistoryRewriteAcknowledgement, ImportHistoryRewriteAcknowledgementOperation,
    ImportLearningLifecycleRecordInput, ImportParserTemplateDraft,
    ImportPreviewClassificationUpdate, ImportPreviewDecision, ImportPreviewDraft,
    ImportPreviewExpectedState, ImportPreviewLearningApply, ImportPreviewLlmApplyRequest,
    ImportPreviewLlmReviewRequest, ImportPreviewLlmSuggestion, ImportPreviewPageRequest,
    ImportPreviewPatch, ImportPreviewPatchField, ImportPreviewPatchValue,
    ImportPreviewQueryFilters, ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate,
    ImportSessionDraft, ImportSessionStatusUpdate, ImportSourceDraft, ImportStandardRowDraft,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use bill_analyser_parsers::{post_process_raw_bills, RawBill};
use serde_json::json;

fn runtime_for(path: &std::path::Path) -> Result<SqliteRuntime, Box<dyn Error>> {
    let db_path = SqliteDbPath::temporary_file(path)?;
    Ok(SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: std::time::Duration::from_secs(1),
    })?)
}

fn seed_users(runtime: &SqliteRuntime, user_ids: &[i64]) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "CREATE TABLE IF NOT EXISTS users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL
        );",
    )?;
    for user_id in user_ids {
        runtime.connection().execute(
            "INSERT INTO users(id, username) VALUES (?1, ?2)",
            (user_id, format!("user-{user_id}")),
        )?;
    }
    Ok(())
}

fn seed_category(
    runtime: &SqliteRuntime,
    id: i64,
    user_id: i64,
    category_type: i64,
    main_category: &str,
    sub_category: &str,
) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            type INTEGER DEFAULT 1,
            main_category TEXT,
            sub_category TEXT,
            priority INTEGER DEFAULT 0,
            created_at TEXT
        );
        ",
    )?;
    runtime.connection().execute(
        "
        INSERT OR REPLACE INTO categories(
            id, user_id, type, main_category, sub_category, priority, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '2026-05-01T00:00:00')
        ",
        (id, user_id, category_type, main_category, sub_category, id),
    )?;
    Ok(())
}

fn set_cash_transfer_category(
    runtime: &SqliteRuntime,
    user_id: i64,
    category_id: i64,
) -> Result<(), Box<dyn Error>> {
    if !table_columns(runtime, "users")?.contains("cash_transfer_category_id") {
        runtime.connection().execute(
            "ALTER TABLE users ADD COLUMN cash_transfer_category_id INTEGER",
            [],
        )?;
    }
    runtime.connection().execute(
        "UPDATE users SET cash_transfer_category_id = ?1 WHERE id = ?2",
        (category_id, user_id),
    )?;
    Ok(())
}

fn seed_account(
    runtime: &SqliteRuntime,
    id: i64,
    user_id: i64,
    name: &str,
    aliases: &str,
) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS accounts (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            aliases TEXT,
            hidden INTEGER DEFAULT 0
        );
        ",
    )?;
    runtime.connection().execute(
        "
        INSERT OR REPLACE INTO accounts(id, user_id, name, aliases, hidden)
        VALUES (?1, ?2, ?3, ?4, 0)
        ",
        (id, user_id, name, aliases),
    )?;
    Ok(())
}

fn seed_import_learning_rule(
    runtime: &SqliteRuntime,
    id: i64,
    user_id: i64,
    category_id: i64,
) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS import_learning_rules (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            learned_category_id INTEGER,
            enabled INTEGER DEFAULT 1
        );
        ",
    )?;
    runtime.connection().execute(
        "
        INSERT OR REPLACE INTO import_learning_rules(id, user_id, learned_category_id, enabled)
        VALUES (?1, ?2, ?3, 1)
        ",
        (id, user_id, category_id),
    )?;
    Ok(())
}

fn init_bills_schema(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            payment_method TEXT DEFAULT '',
            main_category TEXT,
            sub_category TEXT,
            batch_id TEXT,
            hash TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash);
        ",
    )?;
    Ok(())
}

fn table_columns(runtime: &SqliteRuntime, table: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut statement = runtime
        .connection()
        .prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.collect::<Result<BTreeSet<_>, _>>()?)
}

fn table_indexes(
    runtime: &SqliteRuntime,
    table: &str,
) -> Result<Vec<(String, bool)>, Box<dyn Error>> {
    let mut statement = runtime
        .connection()
        .prepare(&format!("PRAGMA index_list({table})"))?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn query_plan_details(runtime: &SqliteRuntime, sql: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let mut statement = runtime
        .connection()
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(3))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn foreign_key_targets(
    runtime: &SqliteRuntime,
    table: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let mut statement = runtime
        .connection()
        .prepare(&format!("PRAGMA foreign_key_list({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(2))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn init_learning_lifecycle_schema(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS import_learning_lifecycle (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            recommendation_key TEXT NOT NULL,
            recommendation_type TEXT NOT NULL DEFAULT 'import_preview',
            status TEXT NOT NULL DEFAULT 'yellow',
            accepted_count INTEGER NOT NULL DEFAULT 0,
            rejected_count INTEGER NOT NULL DEFAULT 0,
            auto_applied_count INTEGER NOT NULL DEFAULT 0,
            auto_apply_enabled INTEGER NOT NULL DEFAULT 0,
            suppressed_until TEXT,
            last_feedback_at TEXT,
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, recommendation_key)
        );
        CREATE TABLE IF NOT EXISTS import_learning_feedback_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            event_type TEXT NOT NULL,
            rule_id INTEGER,
            suggestion_id INTEGER,
            lifecycle_id INTEGER,
            recommendation_key TEXT,
            session_id TEXT,
            preview_id INTEGER,
            bill_id INTEGER,
            candidate_id TEXT,
            previous_signal_state TEXT,
            next_signal_state TEXT,
            payload_json TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS import_learning_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            recommendation_key TEXT NOT NULL,
            suppression_reason TEXT NOT NULL,
            suppressed_until TEXT,
            metadata_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, recommendation_key)
        );
        ",
    )?;
    Ok(())
}

fn user_id(value: u64) -> UserId {
    UserId::new(value).expect("positive test user id")
}

fn preview_draft(date: &str, amount: f64, description: &str) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: date.to_string(),
        preview_type: "支出".to_string(),
        preview_amount: amount,
        preview_destination_amount: 0.0,
        preview_main_category: "餐饮".to_string(),
        preview_sub_category: "午餐".to_string(),
        preview_counterparty: "canteen".to_string(),
        preview_payment_method: "card".to_string(),
        preview_description: description.to_string(),
        preview_parser_id: "wechat".to_string(),
        preview_parser_tags: Some(json!(["wechat", "card"])),
        dedup_type: Some("remaining".to_string()),
        dedup_source_ids: vec![11, 12],
        ..ImportPreviewDraft::default()
    }
}

fn history_rewrite_preview_draft(
    planned_operation: &str,
    history_bill_id: i64,
    history_bill_version: i64,
    group_key: &str,
    history_role: Option<&str>,
    description: &str,
) -> ImportPreviewDraft {
    let mut draft = preview_draft("2026-05-01 08:30:00", 9.25, description);
    draft.dedup_type = Some(
        if planned_operation == "merge_transfer_history" {
            "transfer_cross_batch"
        } else {
            "database_duplicate"
        }
        .to_string(),
    );
    let mut reconciliation = json!({
        "planned_operation": planned_operation,
        "history_bill_id": history_bill_id,
        "history_bill_version": history_bill_version,
        "group_key": group_key,
        "notice": HISTORY_REWRITE_NOTICE,
    });
    if let Some(role) = history_role {
        reconciliation["history_role"] = json!(role);
    }
    draft.preview_matching_feedback = json!({
        "reconciliation": reconciliation,
        "annotation": {
            "type": "history_rewrite_pending",
            "suppressed": true,
        },
    });
    draft
}

fn history_rewrite_ack(
    session_id: &str,
    selected_preview_ids: Vec<i64>,
    preview_id: i64,
    planned_operation: &str,
    history_bill_id: i64,
    history_bill_version: i64,
    group_key: &str,
) -> ImportHistoryRewriteAcknowledgement {
    let operation_id = build_import_history_rewrite_operation_id(
        planned_operation,
        history_bill_id,
        history_bill_version,
        group_key,
    );
    let acknowledgement_token = build_import_history_rewrite_ack_token(
        session_id,
        &operation_id,
        planned_operation,
        history_bill_id,
        history_bill_version,
    );
    ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids,
        operations: vec![ImportHistoryRewriteAcknowledgementOperation {
            preview_id,
            operation_id,
            planned_operation: planned_operation.to_string(),
            history_bill_id,
            history_bill_version,
            acknowledgement_token,
        }],
        selection_scope: json!({"mode": "selected_ids"}),
    }
}

fn parser_template_draft(date: &str, amount: f64, description: &str) -> ImportParserTemplateDraft {
    ImportParserTemplateDraft {
        parser_date: date.to_string(),
        parser_amount: amount,
        parser_type: "支出".to_string(),
        parser_description: description.to_string(),
        parser_id: "wechat".to_string(),
        parser_tags: Some(json!(["wechat", "wallet"])),
        parser_counterparty: "canteen".to_string(),
        parser_payment_method: "零钱".to_string(),
        parser_original_type: "商户消费".to_string(),
        parser_original_category: "餐饮".to_string(),
        parser_account_id: String::new(),
    }
}

fn import_source_draft(source_index: i64, signature: &str) -> ImportSourceDraft {
    ImportSourceDraft {
        source_index,
        original_file_name: format!("source-{source_index}.csv"),
        parser_id: "wechat".to_string(),
        parser_name: "微信".to_string(),
        parser_signal: "matched".to_string(),
        parser_confidence: 1.0,
        feature_signature: signature.to_string(),
        metadata: json!({
            "parser_decision": {
                "status": "matched",
                "selected_parser_id": "wechat"
            }
        }),
    }
}

fn import_standard_row_draft(source_index: i64, source_row_index: i64) -> ImportStandardRowDraft {
    ImportStandardRowDraft {
        source_index,
        source_row_index,
        occurred_at: "2026-05-01 08:30:00".to_string(),
        amount_cents: -925,
        direction: "expense".to_string(),
        transaction_type: "expense".to_string(),
        merchant: "canteen".to_string(),
        payment_method: "wallet".to_string(),
        description: "ledger row".to_string(),
        parser_payload: json!({
            "parser_id": "wechat",
            "parser_decision": {
                "status": "matched"
            }
        }),
        standard_payload: json!({
            "date": "2026-05-01 08:30:00",
            "amount": -9.25,
            "type": "支出"
        }),
    }
}

#[test]
fn import_session_lifecycle_is_user_scoped() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("import_session.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;

    let inserted_id = create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-a".to_string(),
            user_id: user_id(42),
            file_count: 2,
        },
    )?;
    assert!(inserted_id > 0);

    let wrong_user_session = get_import_session(runtime.connection(), "session-a", user_id(77))?;
    assert!(wrong_user_session.is_none());

    let changed = update_import_session_status(
        runtime.connection(),
        &ImportSessionStatusUpdate {
            session_id: "session-a".to_string(),
            user_id: user_id(42),
            status: "preview".to_string(),
            total_parsed: Some(10),
            total_preview: Some(8),
            total_confirmed: None,
        },
    )?;
    assert!(changed);

    let session = get_import_session(runtime.connection(), "session-a", user_id(42))?
        .expect("session should exist for owner");
    assert_eq!(session.status, "preview");
    assert_eq!(session.file_count, 2);
    assert_eq!(session.total_parsed, 10);
    assert_eq!(session.total_preview, 8);
    assert_eq!(session.total_confirmed, 0);
    assert!(!session.created_at.ends_with('Z'));
    assert!(!session.updated_at.ends_with('Z'));

    let wrong_user_update = update_import_session_status(
        runtime.connection(),
        &ImportSessionStatusUpdate {
            session_id: "session-a".to_string(),
            user_id: user_id(77),
            status: "completed".to_string(),
            total_parsed: None,
            total_preview: None,
            total_confirmed: Some(8),
        },
    )?;
    assert!(!wrong_user_update);
    Ok(())
}

#[test]
fn preview_batch_insert_read_page_selection_and_clear_match_staging_semantics(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_staging.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-preview".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let inserted = insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-preview",
        user_id(42),
        &[
            preview_draft("2026/05/02 12:00", 18.5, "second"),
            preview_draft("2026-05-01", 9.25, "first"),
        ],
    )?;
    assert_eq!(inserted, 2);
    assert_eq!(
        count_preview_by_session(runtime.connection(), "session-preview", user_id(42), false)?,
        2
    );
    assert_eq!(
        count_preview_by_session(runtime.connection(), "session-preview", user_id(77), false)?,
        0
    );

    let previews =
        get_preview_by_session(runtime.connection(), "session-preview", user_id(42), false)?;
    assert_eq!(
        previews
            .iter()
            .map(|preview| preview.preview_description.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(previews[0].preview_parser_tags, vec!["wechat", "card"]);
    assert_eq!(previews[0].dedup_source_ids, vec![11, 12]);
    assert!(previews.iter().all(|preview| !preview.preview_selected));

    let filter_index =
        get_preview_filter_index_by_session(runtime.connection(), "session-preview", user_id(42))?;
    assert_eq!(
        filter_index
            .iter()
            .map(|preview| preview.preview_description.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(filter_index[0].preview_parser_tags, vec!["wechat", "card"]);
    assert_eq!(filter_index[0].dedup_source_ids, vec![11, 12]);

    let id_order = previews
        .iter()
        .map(|preview| preview.id)
        .collect::<Vec<_>>();
    let by_id = get_preview_bill_by_id(runtime.connection(), id_order[0], user_id(42))?
        .expect("preview should be visible to owner");
    assert_eq!(by_id.preview_description, "first");
    assert!(get_preview_bill_by_id(runtime.connection(), id_order[0], user_id(77))?.is_none());

    let by_ids = get_preview_by_ids(
        runtime.connection(),
        "session-preview",
        &[id_order[1], -1, id_order[0]],
        user_id(42),
    )?;
    assert_eq!(
        by_ids
            .iter()
            .map(|preview| preview.preview_description.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "first"]
    );

    let (page_rows, total) = get_preview_page_by_session(
        runtime.connection(),
        "session-preview",
        user_id(42),
        1,
        1,
        false,
    )?;
    assert_eq!(total, 2);
    assert_eq!(page_rows.len(), 1);
    assert_eq!(page_rows[0].preview_description, "first");

    let updated =
        update_preview_selection(runtime.connection_mut(), &[id_order[1]], true, user_id(42))?;
    assert_eq!(updated, 1);
    let selected =
        get_preview_by_session(runtime.connection(), "session-preview", user_id(42), true)?;
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].preview_description, "second");

    assert_eq!(
        reset_session_preview_selection(runtime.connection(), "session-preview", user_id(42))?,
        2
    );
    assert_eq!(
        count_preview_by_session(runtime.connection(), "session-preview", user_id(42), true)?,
        0
    );

    runtime.connection().execute(
        "INSERT INTO bills_parser_template(
            session_id, user_id, parser_date, parser_amount, parser_type,
            parser_description, parser_id, created_at
        ) VALUES (?1, ?2, '2026-05-01', 1.0, '支出', 'parser', 'wechat', '2026-05-01T00:00:00')",
        ("session-preview", 42),
    )?;
    runtime.connection().execute(
        "INSERT INTO import_annotation_samples(session_id, user_id, preview_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        ("session-preview", 42, id_order[0], "2026-05-01T00:00:00"),
    )?;
    stage_import_parser_templates_with_sources(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: "session-preview".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
        &[],
        &[import_source_draft(0, "clear-source")],
        &[import_standard_row_draft(0, 0)],
        true,
    )?;
    assert_eq!(
        get_import_sources_by_session(runtime.connection(), "session-preview", user_id(42))?.len(),
        1
    );
    assert_eq!(
        get_import_standard_rows_by_session(runtime.connection(), "session-preview", user_id(42))?
            .len(),
        1
    );

    let cleared = clear_session_data(runtime.connection_mut(), "session-preview", user_id(42))?;
    assert_eq!(cleared.parser_count, 1);
    assert_eq!(cleared.preview_count, 2);
    assert_eq!(cleared.annotation_count, 1);
    assert_eq!(cleared.session_count, 1);
    assert_eq!(
        count_preview_by_session(runtime.connection(), "session-preview", user_id(42), false)?,
        0
    );
    assert!(
        get_import_sources_by_session(runtime.connection(), "session-preview", user_id(42))?
            .is_empty()
    );
    assert!(get_import_standard_rows_by_session(
        runtime.connection(),
        "session-preview",
        user_id(42)
    )?
    .is_empty());
    assert!(get_import_session(runtime.connection(), "session-preview", user_id(42))?.is_none());
    Ok(())
}

#[test]
fn standard_row_ledger_requires_matching_import_source() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("standard_row_source.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;

    let result = stage_import_parser_templates_with_sources(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: "session-ledger".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
        &[],
        &[import_source_draft(0, "valid-source")],
        &[import_standard_row_draft(99, 0)],
        false,
    );

    assert!(result.is_err());
    assert!(
        get_import_sources_by_session(runtime.connection(), "session-ledger", user_id(42))?
            .is_empty()
    );
    assert!(get_import_standard_rows_by_session(
        runtime.connection(),
        "session-ledger",
        user_id(42)
    )?
    .is_empty());
    assert!(get_import_session(runtime.connection(), "session-ledger", user_id(42))?.is_none());
    Ok(())
}

#[test]
fn preview_query_filters_sort_and_metadata_are_session_global() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_query.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-query".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let mut first = preview_draft("2026-05-01 09:00:00", 30.0, "alpha coffee");
    first.preview_selected = true;
    first.preview_sub_category = "咖啡".to_string();
    first.preview_source_account_id = Some(1001);
    first.preview_parser_tags = Some(json!(["parser:alipay", "channel:wallet"]));
    first.preview_matching_feedback = json!({
        "dedup": {"type": "remaining"},
        "annotation": {"status": "invalid"},
        "learning": {"review_status": "auto_applied"}
    });
    let mut second = preview_draft("2026-05-02 09:00:00", 10.0, "beta lunch target");
    second.preview_selected = true;
    second.preview_sub_category = "餐饮".to_string();
    second.preview_source_account_id = Some(1002);
    second.preview_parser_tags = Some(json!(["parser:wechat", "channel:wallet"]));
    second.preview_matching_feedback = json!({
        "dedup": {"type": "remaining"},
        "annotation": {"status": "valid"},
        "parser": {"parser_id": "wechat"}
    });
    let mut third = preview_draft("2026-05-03 09:00:00", 20.0, "target groceries");
    third.preview_selected = true;
    third.preview_sub_category = "购物".to_string();
    third.preview_source_account_id = Some(1003);
    third.preview_parser_tags = Some(json!(["parser:cmbc", "channel:bank"]));
    third.preview_matching_feedback = json!({
        "dedup": {"type": "transfer"},
        "transfer": {"review_status": "pending"}
    });
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-query",
        user_id(42),
        &[first, second, third],
    )?;

    let page = query_preview_page_by_session(
        runtime.connection(),
        "session-query",
        user_id(42),
        &ImportPreviewPageRequest {
            page: 2,
            page_size: 1,
            sort_by: "sourceAmount".to_string(),
            sort_direction: "asc".to_string(),
            filters: ImportPreviewQueryFilters {
                description: Some("target".to_string()),
                tag: Some("channel:wallet".to_string()),
                selected_only: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    assert_eq!(page.total, 1);
    assert!(
        page.rows.is_empty(),
        "page 2 is empty but total is session-global"
    );
    assert_eq!(page.metadata.counts.total, 1);
    assert_eq!(page.metadata.facets.tags[0].value, "channel:wallet");

    let sorted = query_preview_page_by_session(
        runtime.connection(),
        "session-query",
        user_id(42),
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 2,
            sort_by: "sourceAmount".to_string(),
            sort_direction: "asc".to_string(),
            filters: ImportPreviewQueryFilters {
                description: Some("target".to_string()),
                selected_only: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    assert_eq!(sorted.total, 2);
    assert_eq!(sorted.rows[0].preview_description, "beta lunch target");
    assert_eq!(sorted.rows[1].preview_description, "target groceries");
    assert_eq!(sorted.metadata.counts.selected, 3);
    assert_eq!(sorted.metadata.counts.selected_invalid, 2);
    assert_eq!(
        sorted.metadata.counts.annotations.get("needs-review"),
        Some(&1)
    );
    assert_eq!(
        sorted.metadata.counts.annotations.get("no-issues"),
        Some(&1)
    );
    assert!(sorted.metadata.counts.signals.contains_key("dedup"));

    let annotation = query_preview_page_by_session(
        runtime.connection(),
        "session-query",
        user_id(42),
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            filters: ImportPreviewQueryFilters {
                annotation: Some("invalid".to_string()),
                selected_only: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    assert_eq!(annotation.total, 1);
    assert_eq!(annotation.rows[0].preview_description, "alpha coffee");

    let signal = query_preview_page_by_session(
        runtime.connection(),
        "session-query",
        user_id(42),
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            filters: ImportPreviewQueryFilters {
                signal: Some("transfer".to_string()),
                selected_only: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    assert_eq!(signal.total, 1);
    assert_eq!(signal.rows[0].preview_description, "target groceries");

    let needs_review = query_preview_page_by_session(
        runtime.connection(),
        "session-query",
        user_id(42),
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            filters: ImportPreviewQueryFilters {
                annotation: Some("needs-review".to_string()),
                account: Some("__invalid__".to_string()),
                selected_only: false,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    assert_eq!(needs_review.total, 0);
    Ok(())
}

#[test]
fn resolved_known_missing_annotations_are_not_current_missing_signals() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("known_annotations_resolved.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-known-annotations-resolved".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let annotation = |annotation_type: &str| {
        json!({
            "annotation": {
                "status": "needs_review",
                "type": annotation_type,
                "review_status": "requires_manual_review"
            }
        })
    };

    let mut resolved_category = preview_draft("2026-05-01 09:00:00", 10.0, "resolved category");
    resolved_category.preview_source_account_id = Some(1001);
    resolved_category.preview_matching_feedback = annotation("missing_category");

    let mut missing_category = resolved_category.clone();
    missing_category.preview_description = "missing category".to_string();
    missing_category.preview_main_category = String::new();
    missing_category.preview_sub_category = String::new();

    let mut resolved_source = preview_draft("2026-05-02 09:00:00", 20.0, "resolved source");
    resolved_source.preview_source_account_id = Some(1002);
    resolved_source.preview_matching_feedback = annotation("missing_source_account");

    let mut missing_source = resolved_source.clone();
    missing_source.preview_description = "missing source".to_string();
    missing_source.preview_source_account_id = None;

    let mut resolved_destination =
        preview_draft("2026-05-03 09:00:00", 30.0, "resolved destination");
    resolved_destination.preview_type = "转账".to_string();
    resolved_destination.preview_main_category = "转账".to_string();
    resolved_destination.preview_sub_category = "账户互转".to_string();
    resolved_destination.preview_source_account_id = Some(1003);
    resolved_destination.preview_destination_account_id = Some(1004);
    resolved_destination.preview_matching_feedback = annotation("missing_destination_account");

    let mut missing_destination = resolved_destination.clone();
    missing_destination.preview_description = "missing destination".to_string();
    missing_destination.preview_destination_account_id = None;

    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-known-annotations-resolved",
        user_id(42),
        &[
            resolved_category,
            missing_category,
            resolved_source,
            missing_source,
            resolved_destination,
            missing_destination,
        ],
    )?;
    let preview_ids = get_preview_by_session(
        runtime.connection(),
        "session-known-annotations-resolved",
        user_id(42),
        false,
    )?
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();

    for preview_ids in [Vec::new(), preview_ids] {
        let all = query_preview_page_by_session(
            runtime.connection(),
            "session-known-annotations-resolved",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids: preview_ids.clone(),
                ..Default::default()
            },
        )?;
        assert_eq!(
            all.metadata.counts.annotations.get("needs-review"),
            Some(&3)
        );
        assert_eq!(all.metadata.counts.annotations.get("no-issues"), Some(&3));

        let needs_review = query_preview_page_by_session(
            runtime.connection(),
            "session-known-annotations-resolved",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids: preview_ids.clone(),
                filters: ImportPreviewQueryFilters {
                    annotation: Some("needs-review".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        assert_eq!(needs_review.total, 3);
        let needs_review_descriptions = needs_review
            .rows
            .iter()
            .map(|row| row.preview_description.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            needs_review_descriptions,
            BTreeSet::from(["missing category", "missing source", "missing destination"])
        );

        let no_issues = query_preview_page_by_session(
            runtime.connection(),
            "session-known-annotations-resolved",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids,
                filters: ImportPreviewQueryFilters {
                    annotation: Some("no-issues".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        assert_eq!(no_issues.total, 3);
        let no_issue_descriptions = no_issues
            .rows
            .iter()
            .map(|row| row.preview_description.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            no_issue_descriptions,
            BTreeSet::from([
                "resolved category",
                "resolved source",
                "resolved destination"
            ])
        );
    }

    Ok(())
}

#[test]
fn resolved_transfer_account_annotation_is_not_a_current_missing_signal(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("transfer_annotation_resolved.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-transfer-annotation-resolved".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let mut resolved = preview_draft("2026-05-01 09:00:00", 30.0, "resolved transfer");
    resolved.preview_type = "转账".to_string();
    resolved.preview_main_category = "转账".to_string();
    resolved.preview_sub_category = "账户互转".to_string();
    resolved.preview_source_account_id = Some(1001);
    resolved.preview_destination_account_id = Some(1002);
    resolved.preview_matching_feedback = json!({
        "annotation": {
            "status": "needs_review",
            "type": "transfer_account_direction",
            "review_status": "requires_account_review"
        }
    });

    let mut unresolved = resolved.clone();
    unresolved.preview_description = "unresolved transfer".to_string();
    unresolved.preview_source_account_id = Some(1003);
    unresolved.preview_destination_account_id = Some(1003);

    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-transfer-annotation-resolved",
        user_id(42),
        &[resolved, unresolved],
    )?;
    let preview_ids = get_preview_by_session(
        runtime.connection(),
        "session-transfer-annotation-resolved",
        user_id(42),
        false,
    )?
    .into_iter()
    .map(|row| row.id)
    .collect::<Vec<_>>();

    for preview_ids in [Vec::new(), preview_ids] {
        let all = query_preview_page_by_session(
            runtime.connection(),
            "session-transfer-annotation-resolved",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids: preview_ids.clone(),
                ..Default::default()
            },
        )?;
        assert_eq!(
            all.metadata.counts.annotations.get("needs-review"),
            Some(&1)
        );
        assert_eq!(all.metadata.counts.annotations.get("no-issues"), Some(&1));

        let needs_review = query_preview_page_by_session(
            runtime.connection(),
            "session-transfer-annotation-resolved",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids: preview_ids.clone(),
                filters: ImportPreviewQueryFilters {
                    annotation: Some("needs-review".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        assert_eq!(needs_review.total, 1);
        assert_eq!(
            needs_review.rows[0].preview_description,
            "unresolved transfer"
        );

        let no_issues = query_preview_page_by_session(
            runtime.connection(),
            "session-transfer-annotation-resolved",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids,
                filters: ImportPreviewQueryFilters {
                    annotation: Some("no-issues".to_string()),
                    ..Default::default()
                },
                ..Default::default()
            },
        )?;
        assert_eq!(no_issues.total, 1);
        assert_eq!(no_issues.rows[0].preview_description, "resolved transfer");
    }

    Ok(())
}

#[test]
fn preview_selection_query_actions_are_filter_scoped_and_issue_aware() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_selection_query.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-selection-query".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let mut target_valid = preview_draft("2026-05-01 09:00:00", 30.0, "target valid");
    target_valid.preview_source_account_id = Some(1001);
    let mut target_invalid = preview_draft("2026-05-02 09:00:00", 10.0, "target invalid");
    target_invalid.preview_source_account_id = None;
    let mut other_valid = preview_draft("2026-05-03 09:00:00", 20.0, "other valid");
    other_valid.preview_source_account_id = Some(1003);
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-selection-query",
        user_id(42),
        &[target_valid, target_invalid, other_valid],
    )?;
    assert_eq!(
        count_preview_by_session(
            runtime.connection(),
            "session-selection-query",
            user_id(42),
            true
        )?,
        0
    );

    let target_filter = ImportPreviewQueryFilters {
        description: Some("target".to_string()),
        ..Default::default()
    };
    update_session_preview_selection_by_query(
        runtime.connection(),
        "session-selection-query",
        user_id(42),
        &target_filter,
        "select_valid",
    )?;
    assert_eq!(
        selected_preview_descriptions(runtime.connection(), "session-selection-query")?,
        BTreeSet::from(["target valid".to_string()])
    );

    update_session_preview_selection_by_query(
        runtime.connection(),
        "session-selection-query",
        user_id(42),
        &target_filter,
        "select_invalid",
    )?;
    assert_eq!(
        selected_preview_descriptions(runtime.connection(), "session-selection-query")?,
        BTreeSet::from(["target invalid".to_string(), "target valid".to_string()])
    );

    update_session_preview_selection_by_query(
        runtime.connection(),
        "session-selection-query",
        user_id(42),
        &target_filter,
        "select_none",
    )?;
    assert!(
        selected_preview_descriptions(runtime.connection(), "session-selection-query")?.is_empty()
    );

    update_session_preview_selection_by_query(
        runtime.connection(),
        "session-selection-query",
        user_id(42),
        &ImportPreviewQueryFilters::default(),
        "select_all",
    )?;
    update_session_preview_selection_by_query(
        runtime.connection(),
        "session-selection-query",
        user_id(42),
        &target_filter,
        "select_none",
    )?;
    assert_eq!(
        selected_preview_descriptions(runtime.connection(), "session-selection-query")?,
        BTreeSet::from(["other valid".to_string()])
    );

    update_session_preview_selection_by_query(
        runtime.connection(),
        "session-selection-query",
        user_id(42),
        &ImportPreviewQueryFilters::default(),
        "invert",
    )?;
    assert_eq!(
        selected_preview_descriptions(runtime.connection(), "session-selection-query")?,
        BTreeSet::from(["target invalid".to_string(), "target valid".to_string()])
    );
    Ok(())
}

fn selected_preview_descriptions(
    connection: &rusqlite::Connection,
    session_id: &str,
) -> Result<BTreeSet<String>, Box<dyn Error>> {
    Ok(
        get_preview_by_session(connection, session_id, user_id(42), true)?
            .into_iter()
            .map(|preview| preview.preview_description)
            .collect(),
    )
}

#[test]
fn preview_query_filter_branches_cover_sql_and_preview_id_paths() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_query_branches.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-query-branches".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let mut empty = preview_draft("2026-05-01 08:00:00", 1.0, "");
    empty.preview_type = String::new();
    empty.preview_main_category = String::new();
    empty.preview_sub_category = String::new();
    empty.preview_source_account_id = None;
    empty.preview_destination_account_id = None;
    empty.preview_counterparty = String::new();
    empty.preview_payment_method = String::new();
    empty.preview_parser_id = String::new();
    empty.preview_parser_tags = Some(json!([]));
    empty.dedup_type = None;

    let mut income = preview_draft("2026-05-02 09:00:00", 2.0, "salary target");
    income.preview_type = "收入".to_string();
    income.preview_main_category = "收入".to_string();
    income.preview_sub_category = "工资".to_string();
    income.preview_source_account_id = Some(1001);
    income.preview_counterparty = "company".to_string();
    income.preview_payment_method = "bank".to_string();
    income.preview_parser_id = "cmbc".to_string();
    income.preview_parser_tags = Some(json!(["parser:cmbc", "salary"]));
    income.preview_matching_feedback = json!({
        "annotation": {"status": "needs_attention"},
        "learning": {"review_status": "auto_applied"}
    });

    let mut transfer = preview_draft("2026-05-03 10:00:00", 3.0, "transfer nested");
    transfer.preview_type = "转账".to_string();
    transfer.preview_main_category = String::new();
    transfer.preview_sub_category = String::new();
    transfer.preview_source_account_id = Some(1002);
    transfer.preview_destination_account_id = Some(1002);
    transfer.preview_counterparty = "wallet".to_string();
    transfer.preview_payment_method = "balance".to_string();
    transfer.preview_parser_tags = Some(json!(["wallet"]));
    transfer.dedup_type = Some("platform_bank".to_string());
    transfer.preview_matching_feedback = json!({
        "transfer": {"review_status": "pending"},
        "custom": {"nested": "needle"}
    });

    let mut investment = preview_draft("2026-05-04 11:00:00", 4.0, "invest memo");
    investment.preview_type = "投资".to_string();
    investment.preview_source_account_id = Some(1003);
    investment.preview_destination_account_id = Some(1004);
    investment.preview_parser_tags = Some(json!(["fund"]));
    investment.preview_matching_feedback = json!({"annotation": "manual-review"});

    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-query-branches",
        user_id(42),
        &[empty, income, transfer, investment],
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-query-branches",
        user_id(42),
        false,
    )?;
    let preview_ids = previews.iter().map(|row| row.id).collect::<Vec<_>>();

    for sort_by in [
        "time",
        "type",
        "sourceAmount",
        "counterparty",
        "paymentMethod",
        "comment",
        "unknown",
    ] {
        let sorted = query_preview_page_by_session(
            runtime.connection(),
            "session-query-branches",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                sort_by: sort_by.to_string(),
                sort_direction: "desc".to_string(),
                preview_ids: preview_ids.clone(),
                ..Default::default()
            },
        )?;
        assert_eq!(sorted.total, 4);
    }

    let preview_id_filters = [
        ImportPreviewQueryFilters {
            transaction_type: Some("__none__".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            transaction_type: Some("收入".to_string()),
            category: Some("工资".to_string()),
            account: Some("1001".to_string()),
            tag: Some("salary".to_string()),
            description: Some("target".to_string()),
            signal: Some("learning".to_string()),
            annotation: Some("needs_attention".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            category: Some("__invalid__".to_string()),
            account: Some("__invalid__".to_string()),
            signal: Some("transfer".to_string()),
            annotation: Some("needs-review".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            category: Some("__none__".to_string()),
            account: Some("__none__".to_string()),
            tag: Some("__none__".to_string()),
            description: Some("__none__".to_string()),
            signal: Some("parser".to_string()),
            annotation: Some("no-issues".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            signal: Some("needle".to_string()),
            annotation: Some("missing".to_string()),
            ..Default::default()
        },
    ];
    for filters in preview_id_filters {
        let _ = query_preview_page_by_session(
            runtime.connection(),
            "session-query-branches",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                preview_ids: preview_ids.clone(),
                filters,
                ..Default::default()
            },
        )?;
    }

    let sql_filters = [
        ImportPreviewQueryFilters {
            min_datetime: Some("2026-05-02 00:00:00".to_string()),
            max_datetime: Some("2026-05-04 23:59:59".to_string()),
            transaction_type: Some("支出".to_string()),
            category: Some("餐饮".to_string()),
            account: Some("1003".to_string()),
            tag: Some("fund".to_string()),
            description: Some("memo".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            transaction_type: Some("__invalid__".to_string()),
            category: Some("__none__".to_string()),
            account: Some("__none__".to_string()),
            tag: Some("__none__".to_string()),
            description: Some("__none__".to_string()),
            signal: Some("parser".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            transaction_type: Some("custom-type".to_string()),
            category: Some("__invalid__".to_string()),
            account: Some("__invalid__".to_string()),
            signal: Some("platform_duplicate".to_string()),
            annotation: Some("no-issues".to_string()),
            ..Default::default()
        },
        ImportPreviewQueryFilters {
            signal: Some("needle".to_string()),
            annotation: Some("manual-review".to_string()),
            ..Default::default()
        },
    ];
    for filters in sql_filters {
        let _ = query_preview_page_by_session(
            runtime.connection(),
            "session-query-branches",
            user_id(42),
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 2,
                sort_by: "sourceAmount".to_string(),
                sort_direction: "desc".to_string(),
                filters,
                ..Default::default()
            },
        )?;
    }

    Ok(())
}

#[test]
fn preview_partial_patches_preserve_unvisited_selection_state() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_partial_selection.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-partial-selection".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-partial-selection",
        user_id(42),
        &[
            preview_draft("2026-05-01", 1.0, "page one"),
            preview_draft("2026-05-02", 2.0, "page two"),
            preview_draft("2026-05-03", 3.0, "page three"),
        ],
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-partial-selection",
        user_id(42),
        false,
    )?;
    update_preview_selection(
        runtime.connection_mut(),
        &previews
            .iter()
            .map(|preview| preview.id)
            .collect::<Vec<_>>(),
        true,
        user_id(42),
    )?;

    let changed = apply_preview_patches_preserving_selection(
        runtime.connection_mut(),
        "session-partial-selection",
        user_id(42),
        &[ImportPreviewPatch::new(previews[0].id).with_changes([(
            ImportPreviewPatchField::Selected,
            ImportPreviewPatchValue::Bool(false),
        )])],
    )?;
    assert_eq!(changed, 1);

    let selected = get_preview_by_session(
        runtime.connection(),
        "session-partial-selection",
        user_id(42),
        true,
    )?;
    assert_eq!(
        selected
            .iter()
            .map(|preview| preview.preview_description.as_str())
            .collect::<Vec<_>>(),
        vec!["page two", "page three"]
    );
    Ok(())
}

#[test]
fn preview_update_batch_patch_and_transfer_clear_are_session_user_scoped(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_updates.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-update".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-update",
        user_id(42),
        &[
            preview_draft("2026-05-01", 9.25, "first"),
            preview_draft("2026-05-02", 18.5, "second"),
        ],
    )?;
    let previews =
        get_preview_by_session(runtime.connection(), "session-update", user_id(42), false)?;
    runtime.connection().execute(
        "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = ?2",
        (
            json!({"transfer": {"decision": "accept"}, "learning": {"id": 1}}).to_string(),
            previews[0].id,
        ),
    )?;

    let wrong_session_changed = update_preview_bill(
        runtime.connection(),
        "missing-session",
        user_id(42),
        &ImportPreviewPatch::new(previews[0].id).with_change(
            ImportPreviewPatchField::Description,
            ImportPreviewPatchValue::Text("wrong".to_string()),
        ),
    )?;
    assert!(!wrong_session_changed);

    let changed = update_preview_bill(
        runtime.connection(),
        "session-update",
        user_id(42),
        &ImportPreviewPatch::new(previews[0].id)
            .with_change(
                ImportPreviewPatchField::Type,
                ImportPreviewPatchValue::Text("收入".to_string()),
            )
            .with_change(
                ImportPreviewPatchField::Amount,
                ImportPreviewPatchValue::Real(88.0),
            )
            .with_change(
                ImportPreviewPatchField::SourceAccountId,
                ImportPreviewPatchValue::Integer(300),
            )
            .with_change(
                ImportPreviewPatchField::Selected,
                ImportPreviewPatchValue::Bool(false),
            )
            .with_transfer_decision_cleared(),
    )?;
    assert!(changed);

    let first = get_preview_bill_by_id(runtime.connection(), previews[0].id, user_id(42))?
        .expect("updated preview remains visible");
    assert_eq!(first.preview_description, "first");
    assert_eq!(first.preview_type, "收入");
    assert_eq!(first.preview_amount, 88.0);
    assert_eq!(first.preview_source_account_id, Some(300));
    assert!(!first.preview_selected);
    assert_eq!(
        first.preview_matching_feedback,
        json!({"learning": {"id": 1}})
    );

    let batch_changed = update_preview_bills_batch(
        runtime.connection_mut(),
        "session-update",
        user_id(42),
        &[
            ImportPreviewPatch::new(previews[1].id)
                .with_change(
                    ImportPreviewPatchField::DestinationAmount,
                    ImportPreviewPatchValue::Real(11.0),
                )
                .with_change(
                    ImportPreviewPatchField::DestinationAccountId,
                    ImportPreviewPatchValue::Null,
                )
                .with_change(
                    ImportPreviewPatchField::Description,
                    ImportPreviewPatchValue::Text("batch changed".to_string()),
                ),
            ImportPreviewPatch::new(-1).with_change(
                ImportPreviewPatchField::Description,
                ImportPreviewPatchValue::Text("ignored".to_string()),
            ),
        ],
    )?;
    assert_eq!(batch_changed, 1);
    let second = get_preview_bill_by_id(runtime.connection(), previews[1].id, user_id(42))?
        .expect("second preview remains visible");
    assert_eq!(second.preview_destination_amount, 11.0);
    assert_eq!(second.preview_destination_account_id, None);
    assert_eq!(second.preview_description, "batch changed");
    assert!(get_preview_bill_by_id(runtime.connection(), previews[0].id, user_id(77))?.is_none());
    Ok(())
}

#[test]
fn reclassify_and_annotation_samples_are_session_user_scoped() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("reclassify_annotations.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-reclassify".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-reclassify",
        user_id(42),
        &[preview_draft("2026-05-01", 9.25, "needs class")],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-reclassify",
        user_id(42),
        false,
    )?
    .remove(0);

    let wrong_user_changed = batch_update_preview_classification(
        runtime.connection_mut(),
        "session-reclassify",
        user_id(77),
        &[ImportPreviewClassificationUpdate {
            preview_id: preview.id,
            preview_type: "收入".to_string(),
            preview_main_category: "工资".to_string(),
            preview_sub_category: "月薪".to_string(),
            preview_source_account_id: Some(1),
            preview_destination_account_id: None,
        }],
    )?;
    assert_eq!(wrong_user_changed, 0);

    let changed = batch_update_preview_classification(
        runtime.connection_mut(),
        "session-reclassify",
        user_id(42),
        &[ImportPreviewClassificationUpdate {
            preview_id: preview.id,
            preview_type: "收入".to_string(),
            preview_main_category: "工资".to_string(),
            preview_sub_category: "月薪".to_string(),
            preview_source_account_id: Some(100),
            preview_destination_account_id: None,
        }],
    )?;
    assert_eq!(changed, 1);
    let updated = get_preview_bill_by_id(runtime.connection(), preview.id, user_id(42))?
        .expect("classification update should keep row");
    assert_eq!(updated.preview_type, "收入");
    assert_eq!(updated.preview_main_category, "工资");
    assert_eq!(updated.preview_sub_category, "月薪");
    assert_eq!(updated.preview_source_account_id, Some(100));

    let saved = save_import_annotation_samples(
        runtime.connection_mut(),
        "session-reclassify",
        user_id(42),
        &[
            ImportAnnotationSampleDraft {
                preview_id: preview.id,
                annotated_type: Some("收入".to_string()),
                annotated_category_id: Some(8),
                annotated_source_account_id: Some(100),
                annotated_destination_account_id: None,
            },
            ImportAnnotationSampleDraft {
                preview_id: -1,
                annotated_type: Some("ignored".to_string()),
                annotated_category_id: None,
                annotated_source_account_id: None,
                annotated_destination_account_id: None,
            },
        ],
    )?;
    assert_eq!(saved, 1);
    let samples =
        get_import_annotation_samples(runtime.connection(), "session-reclassify", user_id(42))?;
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0].preview_id, preview.id);
    assert_eq!(samples[0].annotated_category_id, Some(8));
    assert!(get_import_annotation_samples(
        runtime.connection(),
        "session-reclassify",
        user_id(77)
    )?
    .is_empty());

    let blocked = save_import_annotation_samples(
        runtime.connection_mut(),
        "session-reclassify",
        user_id(77),
        &[ImportAnnotationSampleDraft {
            preview_id: preview.id,
            annotated_type: Some("支出".to_string()),
            annotated_category_id: Some(99),
            annotated_source_account_id: None,
            annotated_destination_account_id: None,
        }],
    )?;
    assert_eq!(blocked, 0);
    let samples =
        get_import_annotation_samples(runtime.connection(), "session-reclassify", user_id(42))?;
    assert_eq!(samples[0].annotated_type.as_deref(), Some("收入"));
    assert_eq!(samples[0].annotated_category_id, Some(8));
    Ok(())
}

#[test]
fn preview_update_batch_rolls_back_on_staging_error() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_update_rollback.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-update-rollback",
        user_id(42),
        &[
            preview_draft("2026-05-01", 9.25, "first"),
            preview_draft("2026-05-02", 18.5, "second"),
        ],
    )?;
    runtime.connection().execute_batch(
        "
        CREATE TRIGGER fail_bad_preview_update
        BEFORE UPDATE ON bills_preview
        WHEN NEW.preview_type = 'bad'
        BEGIN
            SELECT RAISE(ABORT, 'bad preview update');
        END;
        ",
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-update-rollback",
        user_id(42),
        false,
    )?;

    let result = update_preview_bills_batch(
        runtime.connection_mut(),
        "session-update-rollback",
        user_id(42),
        &[
            ImportPreviewPatch::new(previews[0].id).with_change(
                ImportPreviewPatchField::Description,
                ImportPreviewPatchValue::Text("changed before failure".to_string()),
            ),
            ImportPreviewPatch::new(previews[1].id).with_change(
                ImportPreviewPatchField::Type,
                ImportPreviewPatchValue::Text("bad".to_string()),
            ),
        ],
    );

    assert!(result.is_err());
    let after = get_preview_by_session(
        runtime.connection(),
        "session-update-rollback",
        user_id(42),
        false,
    )?;
    assert_eq!(after[0].preview_description, "first");
    assert_eq!(after[1].preview_type, "支出");
    Ok(())
}

#[test]
fn preview_transfer_decision_restores_snapshot_and_detects_state_conflict(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_transfer_decision.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_category(&runtime, 40, 42, 4, "账户互转", "银行卡互转")?;
    set_cash_transfer_category(&runtime, 42, 40)?;
    seed_account(&runtime, 100, 42, "工资卡", r#"["农业银行", "abc"]"#)?;
    seed_account(&runtime, 200, 42, "零钱", r#"["微信钱包", "wallet"]"#)?;

    let mut draft = preview_draft("2026-05-01", 100.0, "maybe transfer");
    draft.preview_recurring_id = Some(5);
    draft.preview_recurring_name = "monthly rent".to_string();
    draft.preview_recurring_candidate_count = 2;
    draft.preview_recurring_match_score = 0.92;
    draft.preview_recurring_match_reasons = "amount|date".to_string();
    draft.preview_recurring_matched_date = "2026-05-01".to_string();
    draft.preview_matching_feedback = json!({
        "transfer": {
            "candidate_type": "transfer",
            "score": 1.0,
            "level": "high",
            "reason": "smart_dedup transfer pair",
            "review_status": "pending",
            "pair_order": "outgoing_first",
            "source_chain": [
                {
                    "role": "outgoing",
                    "parser_id": "abc",
                    "payment_method": "农业银行",
                    "account_name": "工资卡",
                    "source_account_id": "abc",
                    "tags": ["parser:abc", "channel:bank"]
                },
                {
                    "role": "incoming",
                    "parser_id": "wechat",
                    "payment_method": "微信钱包",
                    "account_name": "零钱",
                    "source_account_id": "wallet",
                    "tags": ["parser:wechat", "channel:wallet"]
                }
            ]
        },
        "learning": {
            "rule_id": 7,
            "review_status": "skipped",
            "reason": "transfer preview is protected from learning type/category overrides"
        }
    });
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-transfer-decision",
        user_id(42),
        &[draft],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-transfer-decision",
        user_id(42),
        false,
    )?
    .remove(0);

    let accepted = apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Accept,
        "转账",
        Some(&ImportPreviewExpectedState {
            session_id: Some("session-transfer-decision".to_string()),
            preview_type: Some("支出".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    let accepted_preview = accepted.preview.expect("accepted decision returns preview");
    assert_eq!(accepted_preview.preview_type, "转账");
    assert_eq!(accepted_preview.preview_main_category, "账户互转");
    assert_eq!(accepted_preview.preview_sub_category, "银行卡互转");
    assert_eq!(accepted_preview.preview_recurring_id, None);
    assert_eq!(accepted_preview.preview_source_account_id, Some(100));
    assert_eq!(accepted_preview.preview_destination_account_id, Some(200));
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("accepted")
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/transfer/previous_preview/preview_recurring_id")
            .and_then(serde_json::Value::as_i64),
        Some(5)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/transfer/resolved_source_account_id")
            .and_then(serde_json::Value::as_i64),
        Some(100)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/transfer/resolved_destination_account_id")
            .and_then(serde_json::Value::as_i64),
        Some(200)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/rule_id")
            .and_then(serde_json::Value::as_i64),
        Some(7)
    );

    let conflict = apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Reject,
        "转账",
        Some(&ImportPreviewExpectedState {
            preview_type: Some("支出".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    assert!(conflict.state_conflict);

    let rejected = apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Reject,
        "转账",
        Some(&ImportPreviewExpectedState {
            preview_type: Some("转账".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    let rejected_preview = rejected.preview.expect("rejected decision returns preview");
    assert_eq!(rejected_preview.preview_type, "支出");
    assert_eq!(rejected_preview.preview_main_category, "餐饮");
    assert_eq!(rejected_preview.preview_sub_category, "午餐");
    assert_eq!(rejected_preview.preview_recurring_id, Some(5));
    assert_eq!(rejected_preview.preview_source_account_id, None);
    assert_eq!(rejected_preview.preview_destination_account_id, None);
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("rejected")
    );
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/learning/rule_id")
            .and_then(serde_json::Value::as_i64),
        Some(7)
    );

    let cleared = apply_preview_transfer_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Clear,
        "转账",
        None,
    )?;
    assert_eq!(
        cleared
            .preview
            .expect("clear returns preview")
            .preview_matching_feedback,
        json!({"learning": {
            "rule_id": 7,
            "review_status": "skipped",
            "reason": "transfer preview is protected from learning type/category overrides"
        }})
    );
    Ok(())
}

#[test]
fn preview_recurring_match_decision_sets_candidate_fields_and_clears_transfer_feedback(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_recurring_decision.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-recurring-decision",
        user_id(42),
        &[preview_draft("2026-05-01", 88.0, "maybe recurring")],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-recurring-decision",
        user_id(42),
        false,
    )?
    .remove(0);
    runtime.connection().execute(
        "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = ?2",
        (
            json!({"transfer": {"review_status": "accepted"}, "learning": {"id": 7}}).to_string(),
            preview.id,
        ),
    )?;

    let invalid = update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: Some(9),
            candidate_count: 1,
            target_candidate: None,
        },
        None,
    )?;
    assert!(invalid.invalid_recurring_id);

    let updated = update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: Some(9),
            candidate_count: 3,
            target_candidate: Some(ImportPreviewRecurringCandidate {
                id: 9,
                name: "salary".to_string(),
                match_score: 0.87,
                match_reasons: vec!["amount".to_string(), "date".to_string()],
                matched_occurrence_date: "2026-05-01".to_string(),
            }),
        },
        Some(&ImportPreviewExpectedState {
            session_id: Some("session-recurring-decision".to_string()),
            preview_recurring_id: Some(None),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    let updated_preview = updated.preview.expect("recurring update returns preview");
    assert_eq!(updated_preview.preview_recurring_id, Some(9));
    assert_eq!(updated_preview.preview_recurring_name, "salary");
    assert_eq!(updated_preview.preview_recurring_candidate_count, 3);
    assert_eq!(updated_preview.preview_recurring_match_score, 0.87);
    assert_eq!(
        updated_preview.preview_recurring_match_reasons,
        "amount|date"
    );
    assert_eq!(updated_preview.preview_recurring_matched_date, "2026-05-01");
    assert!(updated_preview
        .preview_matching_feedback
        .get("transfer")
        .is_none());
    assert_eq!(
        updated_preview
            .preview_matching_feedback
            .pointer("/learning/id")
            .and_then(serde_json::Value::as_i64),
        Some(7)
    );

    let cleared = update_preview_recurring_match_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: None,
            candidate_count: 3,
            target_candidate: None,
        },
        Some(&ImportPreviewExpectedState {
            preview_recurring_id: Some(Some(9)),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    let cleared_preview = cleared.preview.expect("clear returns preview");
    assert_eq!(cleared_preview.preview_recurring_id, None);
    assert_eq!(cleared_preview.preview_recurring_name, "");
    assert_eq!(cleared_preview.preview_recurring_match_score, 0.0);
    assert_eq!(
        cleared_preview
            .preview_matching_feedback
            .pointer("/learning/id")
            .and_then(serde_json::Value::as_i64),
        Some(7)
    );
    Ok(())
}

#[test]
fn preview_learning_decision_applies_rejects_and_clears_with_snapshot_restore(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_learning_decision.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_category(&runtime, 21, 42, 2, "工资", "奖金")?;
    seed_import_learning_rule(&runtime, 12, 42, 21)?;
    let mut draft = preview_draft("2026-05-01", 88.0, "learning candidate");
    draft.preview_source_account_id = Some(100);
    draft.preview_matching_feedback = json!({
        "transfer": {
            "candidate_type": "transfer",
            "score": 0.91,
            "level": "high",
            "reason": "existing transfer signal",
            "review_status": "pending"
        }
    });
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-learning-decision",
        user_id(42),
        &[draft],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-learning-decision",
        user_id(42),
        false,
    )?
    .remove(0);

    let applied = ImportPreviewLearningApply {
        preview_type: Some("收入".to_string()),
        preview_main_category: Some("AI生成工资".to_string()),
        preview_sub_category: Some("同名奖金".to_string()),
        preview_source_account_id: Some(None),
        preview_destination_account_id: Some(Some(200)),
        rule_id: Some(12),
    };
    let accepted = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Accept,
        Some(&applied),
        Some(&ImportPreviewExpectedState {
            session_id: Some("session-learning-decision".to_string()),
            preview_type: Some("支出".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    let accepted_preview = accepted.preview.expect("accept returns preview");
    assert_eq!(accepted_preview.preview_type, "收入");
    assert_eq!(accepted_preview.preview_main_category, "工资");
    assert_eq!(accepted_preview.preview_sub_category, "奖金");
    assert_eq!(accepted_preview.preview_source_account_id, None);
    assert_eq!(accepted_preview.preview_destination_account_id, Some(200));
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/review_status")
            .and_then(serde_json::Value::as_str),
        Some("accepted")
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/rule_id")
            .and_then(serde_json::Value::as_i64),
        Some(12)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/previous_preview/preview_source_account_id")
            .and_then(serde_json::Value::as_i64),
        Some(100)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/applied_preview/preview_destination_account_id")
            .and_then(serde_json::Value::as_i64),
        Some(200)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("pending")
    );

    let stale = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Reject,
        None,
        Some(&ImportPreviewExpectedState {
            preview_type: Some("支出".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    assert!(stale.state_conflict);

    let rejected = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Reject,
        None,
        Some(&ImportPreviewExpectedState {
            preview_type: Some("收入".to_string()),
            preview_source_account_id: Some(None),
            preview_destination_account_id: Some(Some(200)),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    let rejected_preview = rejected.preview.expect("reject returns preview");
    assert_eq!(rejected_preview.preview_type, "支出");
    assert_eq!(rejected_preview.preview_main_category, "餐饮");
    assert_eq!(rejected_preview.preview_sub_category, "午餐");
    assert_eq!(rejected_preview.preview_source_account_id, Some(100));
    assert_eq!(rejected_preview.preview_destination_account_id, None);
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/learning/review_status")
            .and_then(serde_json::Value::as_str),
        Some("rejected")
    );
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/learning/rule_id")
            .and_then(serde_json::Value::as_i64),
        Some(12)
    );
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("pending")
    );

    apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Accept,
        Some(&applied),
        None,
    )?;
    let cleared = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Clear,
        None,
        None,
    )?;
    let cleared_preview = cleared.preview.expect("clear returns preview");
    assert_eq!(cleared_preview.preview_type, "支出");
    assert_eq!(cleared_preview.preview_main_category, "餐饮");
    assert_eq!(cleared_preview.preview_sub_category, "午餐");
    assert_eq!(cleared_preview.preview_source_account_id, Some(100));
    assert_eq!(cleared_preview.preview_destination_account_id, None);
    assert_eq!(
        cleared_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("pending")
    );
    assert!(cleared_preview
        .preview_matching_feedback
        .get("learning")
        .is_none());
    Ok(())
}

#[test]
fn learning_lifecycle_feedback_persists_events_thresholds_and_user_scope(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("learning_lifecycle.db"))?;
    init_learning_lifecycle_schema(&runtime)?;
    let key = "import-learning-recommendation-key-v1:test";

    for _ in 0..2 {
        let view = record_import_learning_lifecycle_feedback(
            runtime.connection_mut(),
            42,
            &ImportLearningLifecycleRecordInput {
                recommendation_key: key.to_string(),
                recommendation_type: "import_preview".to_string(),
                feedback: "accept".to_string(),
                rule_id: Some(7),
                suggestion_id: None,
                session_id: Some("session-learning".to_string()),
                preview_id: Some(11),
                bill_id: None,
                candidate_id: Some("preview:11:learning".to_string()),
                payload_json: Some(json!({"source": "test"}).to_string()),
            },
        )?;
        assert_eq!(view.signal_state, "yellow");
        assert!(!view.auto_apply_enabled);
    }

    let green = record_import_learning_lifecycle_feedback(
        runtime.connection_mut(),
        42,
        &ImportLearningLifecycleRecordInput {
            recommendation_key: key.to_string(),
            recommendation_type: "import_preview".to_string(),
            feedback: "accept".to_string(),
            rule_id: Some(7),
            suggestion_id: None,
            session_id: Some("session-learning".to_string()),
            preview_id: Some(11),
            bill_id: None,
            candidate_id: Some("preview:11:learning".to_string()),
            payload_json: Some(json!({"source": "test"}).to_string()),
        },
    )?;
    assert_eq!(green.status, "green");
    assert_eq!(green.accepted_count, 3);
    assert!(green.auto_apply_enabled);

    let other_user =
        get_import_learning_lifecycle_view(runtime.connection(), 77, key, "import_preview")?;
    assert_eq!(other_user.signal_state, "yellow");
    assert_eq!(other_user.accepted_count, 0);

    let auto_applied = record_import_learning_lifecycle_feedback(
        runtime.connection_mut(),
        42,
        &ImportLearningLifecycleRecordInput {
            recommendation_key: key.to_string(),
            recommendation_type: "import_preview".to_string(),
            feedback: "auto_apply".to_string(),
            rule_id: Some(7),
            suggestion_id: None,
            session_id: Some("session-learning".to_string()),
            preview_id: Some(12),
            bill_id: None,
            candidate_id: Some("preview:12:learning".to_string()),
            payload_json: Some(json!({"source": "test"}).to_string()),
        },
    )?;
    assert_eq!(auto_applied.status, "auto_applied");
    assert_eq!(auto_applied.auto_applied_count, 1);

    for _ in 0..2 {
        record_import_learning_lifecycle_feedback(
            runtime.connection_mut(),
            42,
            &ImportLearningLifecycleRecordInput {
                recommendation_key: key.to_string(),
                recommendation_type: "import_preview".to_string(),
                feedback: "reject".to_string(),
                rule_id: Some(7),
                suggestion_id: None,
                session_id: Some("session-learning".to_string()),
                preview_id: Some(12),
                bill_id: None,
                candidate_id: Some("preview:12:learning".to_string()),
                payload_json: Some(json!({"source": "test"}).to_string()),
            },
        )?;
    }
    let downgraded =
        get_import_learning_lifecycle_view(runtime.connection(), 42, key, "import_preview")?;
    assert_eq!(downgraded.status, "downgraded");
    assert_eq!(downgraded.signal_state, "yellow");

    let suppressed_key = "import-learning-recommendation-key-v1:suppressed";
    for _ in 0..3 {
        record_import_learning_lifecycle_feedback(
            runtime.connection_mut(),
            42,
            &ImportLearningLifecycleRecordInput {
                recommendation_key: suppressed_key.to_string(),
                recommendation_type: "import_preview".to_string(),
                feedback: "reject".to_string(),
                rule_id: Some(9),
                suggestion_id: None,
                session_id: None,
                preview_id: None,
                bill_id: None,
                candidate_id: None,
                payload_json: Some(json!({"source": "test"}).to_string()),
            },
        )?;
    }
    let suppressed = get_import_learning_lifecycle_view(
        runtime.connection(),
        42,
        suppressed_key,
        "import_preview",
    )?;
    assert!(suppressed.suppressed);
    let suppression_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM import_learning_suppressions WHERE user_id = 42 AND recommendation_key = ?1",
        [suppressed_key],
        |row| row.get(0),
    )?;
    assert_eq!(suppression_count, 1);
    let event_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM import_learning_feedback_events WHERE user_id = 42 AND recommendation_key = ?1",
        [key],
        |row| row.get(0),
    )?;
    assert_eq!(event_count, 6);
    Ok(())
}

#[test]
fn preview_learning_decision_records_recommendation_key_lifecycle_feedback(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_learning_lifecycle.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    init_learning_lifecycle_schema(&runtime)?;
    seed_category(&runtime, 21, 42, 2, "工资", "奖金")?;
    seed_import_learning_rule(&runtime, 12, 42, 21)?;
    let recommendation_key = "import-learning-recommendation-key-v1:preview-test";
    let mut draft = preview_draft("2026-05-01", 88.0, "learning candidate");
    draft.preview_matching_feedback = json!({
        "learning": {
            "rule_id": 12,
            "review_status": "pending",
            "recommendation_key": recommendation_key,
            "applied_preview": {
                "preview_type": "收入",
                "preview_main_category": "工资",
                "preview_sub_category": "奖金",
                "preview_source_account_id": null,
                "preview_destination_account_id": 200
            }
        }
    });
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-learning-lifecycle",
        user_id(42),
        &[draft],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-learning-lifecycle",
        user_id(42),
        false,
    )?
    .remove(0);
    let accepted = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Accept,
        Some(&ImportPreviewLearningApply {
            rule_id: Some(12),
            ..ImportPreviewLearningApply::default()
        }),
        None,
    )?;
    let accepted_preview = accepted.preview.expect("accepted preview");
    assert_eq!(accepted_preview.preview_type, "收入");
    assert_eq!(accepted_preview.preview_destination_account_id, Some(200));
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/recommendation_key")
            .and_then(serde_json::Value::as_str),
        Some(recommendation_key)
    );
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/accepted_count")
            .and_then(serde_json::Value::as_i64),
        Some(1)
    );
    let rejected = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Reject,
        Some(&ImportPreviewLearningApply {
            rule_id: Some(12),
            ..ImportPreviewLearningApply::default()
        }),
        None,
    )?;
    let rejected_preview = rejected.preview.expect("rejected preview");
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/learning/rejected_count")
            .and_then(serde_json::Value::as_i64),
        Some(1)
    );
    let event_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM import_learning_feedback_events WHERE recommendation_key = ?1",
        [recommendation_key],
        |row| row.get(0),
    )?;
    assert_eq!(event_count, 2);
    Ok(())
}

#[test]
fn preview_learning_decision_clears_generated_category_names_without_taxonomy_match(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_learning_invalid_category.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    seed_category(&runtime, 31, 42, 3, "餐饮", "午餐")?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-learning-invalid-category",
        user_id(42),
        &[preview_draft(
            "2026-05-01",
            18.0,
            "learning generated category",
        )],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-learning-invalid-category",
        user_id(42),
        false,
    )?
    .remove(0);

    let applied = ImportPreviewLearningApply {
        preview_type: Some("支出".to_string()),
        preview_main_category: Some("餐饮".to_string()),
        preview_sub_category: Some("同名但未建分类".to_string()),
        ..ImportPreviewLearningApply::default()
    };
    let accepted = apply_preview_learning_decision(
        runtime.connection_mut(),
        preview.id,
        user_id(42),
        ImportPreviewDecision::Accept,
        Some(&applied),
        None,
    )?;
    let accepted_preview = accepted.preview.expect("accept returns preview");
    assert_eq!(accepted_preview.preview_type, "支出");
    assert_eq!(accepted_preview.preview_main_category, "");
    assert_eq!(accepted_preview.preview_sub_category, "");
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/applied_preview/preview_main_category")
            .and_then(serde_json::Value::as_str),
        Some("")
    );
    Ok(())
}

#[test]
fn preview_llm_recommendation_applies_reviews_and_records_memory_events(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_llm_review.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    let mut draft = preview_draft("2026-05-01", 32.0, "llm candidate");
    draft.preview_main_category = String::new();
    draft.preview_sub_category = String::new();
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-llm-review",
        user_id(42),
        &[draft],
    )?;
    let preview = get_preview_by_session(
        runtime.connection(),
        "session-llm-review",
        user_id(42),
        false,
    )?
    .remove(0);
    runtime.connection().execute(
        "UPDATE bills_preview SET preview_matching_feedback_json = ?1 WHERE id = ?2",
        (
            json!({"transfer": {"review_status": "accepted"}}).to_string(),
            preview.id,
        ),
    )?;

    let suggestion = ImportPreviewLlmSuggestion {
        suggested_main_category: "餐饮".to_string(),
        suggested_sub_category: "早餐".to_string(),
        suggested_source_account: "招商银行".to_string(),
        suggested_destination_account: String::new(),
        resolved_source_account_id: Some(300),
        resolved_destination_account_id: None,
        confidence: 0.91,
        reason: "merchant pattern".to_string(),
    };
    let long_prompt = "prompt ".repeat(4_000);
    let applied = apply_preview_llm_recommendation(
        runtime.connection_mut(),
        ImportPreviewLlmApplyRequest {
            session_id: "session-llm-review",
            preview_id: preview.id,
            user_id: user_id(42),
            suggestion: &suggestion,
            prompt_text: Some(&long_prompt),
            llm_provider: Some("openai"),
            llm_model: Some("gpt-test"),
        },
    )?;
    let applied_preview = applied.preview.expect("llm apply returns preview");
    assert_eq!(
        applied.applied_fields,
        vec![
            "preview_main_category",
            "preview_sub_category",
            "preview_source_account_id"
        ]
    );
    assert_eq!(applied_preview.preview_main_category, "餐饮");
    assert_eq!(applied_preview.preview_sub_category, "早餐");
    assert_eq!(applied_preview.preview_source_account_id, Some(300));
    assert_eq!(
        applied_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("accepted")
    );
    assert_eq!(
        applied_preview
            .preview_matching_feedback
            .pointer("/llm/review_status")
            .and_then(serde_json::Value::as_str),
        Some("pending")
    );
    assert_eq!(
        applied_preview
            .preview_matching_feedback
            .pointer("/llm/applied_preview/preview_source_account_id")
            .and_then(serde_json::Value::as_i64),
        Some(300)
    );

    let accepted = review_preview_llm_recommendation(
        runtime.connection_mut(),
        ImportPreviewLlmReviewRequest {
            session_id: "session-llm-review",
            preview_id: preview.id,
            user_id: user_id(42),
            decision: ImportPreviewDecision::Accept,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
        },
    )?;
    let accepted_preview = accepted.preview.expect("llm accept returns preview");
    assert!(!accepted.restored);
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/llm/review_status")
            .and_then(serde_json::Value::as_str),
        Some("accepted")
    );

    let rejected = review_preview_llm_recommendation(
        runtime.connection_mut(),
        ImportPreviewLlmReviewRequest {
            session_id: "session-llm-review",
            preview_id: preview.id,
            user_id: user_id(42),
            decision: ImportPreviewDecision::Reject,
            suggestion: None,
            user_correction_category: Some("餐饮纠正"),
            user_correction_account: Some("现金账户"),
        },
    )?;
    let rejected_preview = rejected.preview.expect("llm reject returns preview");
    assert!(rejected.restored);
    assert_eq!(rejected_preview.preview_main_category, "");
    assert_eq!(rejected_preview.preview_sub_category, "");
    assert_eq!(rejected_preview.preview_source_account_id, None);
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/llm/review_status")
            .and_then(serde_json::Value::as_str),
        Some("rejected")
    );
    assert_eq!(
        rejected_preview
            .preview_matching_feedback
            .pointer("/transfer/review_status")
            .and_then(serde_json::Value::as_str),
        Some("accepted")
    );

    let events = get_llm_memory_events(
        runtime.connection(),
        user_id(42),
        Some("session-llm-review"),
        None,
        10,
        0,
    )?;
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, "feedback");
    assert_eq!(events[0].decision.as_deref(), Some("reject"));
    assert_eq!(
        events[0].user_correction_category.as_deref(),
        Some("餐饮纠正")
    );
    assert_eq!(
        events[0]
            .metadata
            .as_ref()
            .and_then(|value| value.get("rollback"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(events[2].event_type, "recommendation");
    assert_eq!(events[2].llm_provider.as_deref(), Some("openai"));
    assert!(events[2].prompt_text.as_deref().expect("prompt text").len() <= 16_384);
    assert_eq!(
        events[2].snapshot_after.as_ref().unwrap()["preview_main_category"],
        "餐饮"
    );
    Ok(())
}

#[test]
fn parser_templates_round_trip_processed_filter_and_user_scope() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("parser_templates.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;

    let inserted = insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-parser",
        user_id(42),
        &[
            parser_template_draft("2026/05/02 12:00", -18.5, "second"),
            parser_template_draft("2026-05-01", -9.25, "first"),
        ],
    )?;
    assert_eq!(inserted, 2);

    let templates =
        get_parser_templates_by_session(runtime.connection(), "session-parser", user_id(42), None)?;
    assert_eq!(
        templates
            .iter()
            .map(|template| template.parser_description.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second"]
    );
    assert_eq!(templates[0].parser_tags, vec!["wechat", "wallet"]);
    assert!(!templates[0].parser_is_processed);
    assert!(!templates[0].created_at.ends_with('Z'));
    assert!(get_parser_templates_by_session(
        runtime.connection(),
        "session-parser",
        user_id(77),
        None
    )?
    .is_empty());

    let changed = update_parser_template_status(
        runtime.connection_mut(),
        &[templates[0].id, -1],
        true,
        Some("100"),
        user_id(42),
    )?;
    assert_eq!(changed, 1);
    let processed = get_parser_templates_by_session(
        runtime.connection(),
        "session-parser",
        user_id(42),
        Some(true),
    )?;
    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0].parser_description, "first");
    assert_eq!(processed[0].parser_account_id, "100");

    let unprocessed =
        get_unprocessed_templates_for_dedup(runtime.connection(), "session-parser", user_id(42))?;
    assert_eq!(unprocessed.len(), 1);
    assert_eq!(unprocessed[0].parser_description, "second");

    let wrong_user_changed = update_parser_template_status(
        runtime.connection_mut(),
        &[templates[1].id],
        true,
        None,
        user_id(77),
    )?;
    assert_eq!(wrong_user_changed, 0);
    Ok(())
}

#[test]
fn parser_templates_can_be_marked_processed_by_session_scope() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("parser_templates_session_update.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_import_staging_schema(runtime.connection())?;

    insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-main",
        user_id(42),
        &[
            parser_template_draft("2026-05-01", -9.25, "first"),
            parser_template_draft("2026-05-02", -18.5, "second"),
            parser_template_draft("2026-05-03", -27.75, "third"),
        ],
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-other",
        user_id(42),
        &[parser_template_draft("2026-05-01", -9.25, "other-session")],
    )?;
    insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-main",
        user_id(77),
        &[parser_template_draft("2026-05-01", -9.25, "other-user")],
    )?;

    let changed = mark_unprocessed_parser_templates_processed_for_session(
        runtime.connection_mut(),
        "session-main",
        user_id(42),
    )?;
    assert_eq!(changed, 3);
    assert_eq!(
        get_unprocessed_templates_for_dedup(runtime.connection(), "session-main", user_id(42))?
            .len(),
        0
    );
    assert_eq!(
        get_unprocessed_templates_for_dedup(runtime.connection(), "session-other", user_id(42))?
            .len(),
        1
    );
    assert_eq!(
        get_unprocessed_templates_for_dedup(runtime.connection(), "session-main", user_id(77))?
            .len(),
        1
    );
    assert_eq!(
        mark_unprocessed_parser_templates_processed_for_session(
            runtime.connection_mut(),
            "session-main",
            user_id(42),
        )?,
        0
    );
    Ok(())
}

#[test]
fn standard_bills_convert_to_parser_templates_for_stage1_parse() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("standard_bill_templates.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;

    let raw_bills = vec![RawBill {
        date: "2026/05/03 09:10".to_string(),
        amount: "12.30".to_string(),
        transaction_type: "收入".to_string(),
        description: "salary".to_string(),
        counterparty: "company".to_string(),
        payment_method: "招商银行".to_string(),
        original_category: "工资".to_string(),
        parser_tags: vec!["parser:cmbc".to_string(), "channel:bank".to_string()],
        ..RawBill::default()
    }];
    let standard_bills = post_process_raw_bills("cmbc", &raw_bills);
    let drafts = parser_template_drafts_from_standard_bills(&standard_bills, "cmbc");

    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].parser_date, "2026-05-03 09:10:00");
    assert_eq!(drafts[0].parser_amount, 12.30);
    assert_eq!(drafts[0].parser_type, "收入");
    assert_eq!(drafts[0].parser_id, "cmbc");
    assert_eq!(drafts[0].parser_account_id, "cmbc");

    let inserted = insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-stage1-parse",
        user_id(42),
        &drafts,
    )?;
    assert_eq!(inserted, 1);

    let templates = get_parser_templates_by_session(
        runtime.connection(),
        "session-stage1-parse",
        user_id(42),
        None,
    )?;
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0].parser_date, "2026-05-03 09:10:00");
    assert_eq!(templates[0].parser_amount, 12.30);
    assert_eq!(templates[0].parser_type, "收入");
    assert_eq!(
        templates[0].parser_description,
        "salary | company | 招商银行 | 工资 | 收入"
    );
    assert_eq!(
        templates[0].parser_tags,
        vec!["parser:cmbc", "channel:bank"]
    );
    assert_eq!(templates[0].parser_counterparty, "company");
    assert_eq!(templates[0].parser_payment_method, "招商银行");
    assert_eq!(templates[0].parser_original_category, "工资");
    assert!(!templates[0].parser_is_processed);
    Ok(())
}

#[test]
fn parser_templates_flow_through_smart_dedup_into_preview_drafts() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("template_to_preview.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;

    let mut outgoing = parser_template_draft("2026-05-04 10:00:00", -100.0, "transfer out");
    outgoing.parser_id = "abc".to_string();
    outgoing.parser_payment_method = "农业银行".to_string();
    outgoing.parser_tags = Some(json!(["parser:abc", "channel:bank"]));
    outgoing.parser_account_id = "101".to_string();
    outgoing.parser_counterparty = "savings".to_string();

    let mut incoming = parser_template_draft("2026-05-04 10:00:30", 100.0, "transfer in");
    incoming.parser_type = "收入".to_string();
    incoming.parser_id = "cmbc".to_string();
    incoming.parser_payment_method = String::new();
    incoming.parser_tags = Some(json!(["parser:cmbc", "channel:bank"]));
    incoming.parser_account_id = "202".to_string();
    incoming.parser_counterparty = "wallet".to_string();

    insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-dedup-preview",
        user_id(42),
        &[outgoing, incoming],
    )?;

    let templates = get_parser_templates_by_session(
        runtime.connection(),
        "session-dedup-preview",
        user_id(42),
        None,
    )?;
    let dedup_input = dedup_bills_from_parser_templates(&templates);
    assert_eq!(dedup_input[1].payment_method, "cmbc");

    let dedup_result = SmartDeduplicationEngine.process(dedup_input);
    assert_eq!(dedup_result.transfer_pairs.len(), 1);
    assert_eq!(dedup_result.kept_bills.len(), 1);

    let preview_drafts = preview_drafts_from_dedup_bills(&dedup_result.kept_bills);
    assert_eq!(preview_drafts.len(), 1);
    assert_eq!(preview_drafts[0].preview_type, "转账");
    assert_eq!(preview_drafts[0].preview_amount, 100.0);
    assert_eq!(preview_drafts[0].preview_destination_amount, 100.0);
    assert_eq!(preview_drafts[0].preview_source_account_id, Some(101));
    assert_eq!(preview_drafts[0].preview_destination_account_id, Some(202));
    assert_eq!(preview_drafts[0].dedup_type.as_deref(), Some("transfer"));
    assert_eq!(
        preview_drafts[0].dedup_source_ids,
        vec![templates[0].id, templates[1].id]
    );

    let inserted = insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-dedup-preview",
        user_id(42),
        &preview_drafts,
    )?;
    assert_eq!(inserted, 1);

    let previews = get_preview_by_session(
        runtime.connection(),
        "session-dedup-preview",
        user_id(42),
        false,
    )?;
    assert_eq!(previews.len(), 1);
    assert_eq!(previews[0].preview_type, "转账");
    assert_eq!(previews[0].preview_destination_account_id, Some(202));
    assert_eq!(previews[0].dedup_type, "transfer");
    assert_eq!(
        previews[0].dedup_source_ids,
        vec![templates[0].id, templates[1].id]
    );
    assert_eq!(
        previews[0].preview_parser_tags,
        vec!["parser:abc", "channel:bank", "parser:cmbc"]
    );
    Ok(())
}

#[test]
fn decision_groups_and_history_materializations_are_persisted_and_cleaned(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("decision_groups.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-groups".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    let preview_id = insert_preview_bill(
        runtime.connection(),
        "session-groups",
        user_id(42),
        &preview_draft("2026-05-01 08:30:00", 9.25, "ledger row"),
    )?;

    let inserted_groups = insert_import_decision_groups_batch(
        runtime.connection_mut(),
        "session-groups",
        user_id(42),
        &[ImportDecisionGroupDraft {
            group_type: "duplicate".to_string(),
            group_key: "same_batch:1-2".to_string(),
            decision_status: "merged".to_string(),
            base_preview_row_id: Some(preview_id),
            signal_payload: json!({
                "signal": "duplicate",
                "source_label": "来源1: 微信 | 来源2: 工商银行"
            }),
            members: vec![
                ImportDecisionGroupMemberDraft {
                    preview_row_id: Some(preview_id),
                    standard_row_id: None,
                    history_bill_id: None,
                    member_role: "base".to_string(),
                    parser_name: "微信".to_string(),
                    metadata: json!({"template_id": 1}),
                },
                ImportDecisionGroupMemberDraft {
                    preview_row_id: None,
                    standard_row_id: None,
                    history_bill_id: Some(9001),
                    member_role: "history_base".to_string(),
                    parser_name: "history_db".to_string(),
                    metadata: json!({"planned_operation": "update_history"}),
                },
            ],
        }],
    )?;
    assert_eq!(inserted_groups, 1);
    insert_import_history_materializations_batch(
        runtime.connection_mut(),
        "session-groups",
        user_id(42),
        &[ImportHistoryMaterializationDraft {
            history_bill_id: 9001,
            history_bill_version: 1,
            materialized_payload: json!({"operation": "update_history"}),
            rewrite_reason: "same_amount|same_direction".to_string(),
        }],
    )?;

    let groups =
        get_import_decision_groups_by_session(runtime.connection(), "session-groups", user_id(42))?;
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].members.len(), 2);
    assert_eq!(groups[0].signal_payload["signal"], "duplicate");
    let materializations = get_import_history_materializations_by_session(
        runtime.connection(),
        "session-groups",
        user_id(42),
    )?;
    assert_eq!(materializations.len(), 1);
    assert_eq!(materializations[0].history_bill_id, 9001);

    clear_import_preview_materialization_state(
        runtime.connection_mut(),
        "session-groups",
        user_id(42),
    )?;
    assert!(
        get_preview_by_session(runtime.connection(), "session-groups", user_id(42), false)?
            .is_empty()
    );
    assert!(get_import_decision_groups_by_session(
        runtime.connection(),
        "session-groups",
        user_id(42),
    )?
    .is_empty());
    assert!(get_import_history_materializations_by_session(
        runtime.connection(),
        "session-groups",
        user_id(42),
    )?
    .is_empty());

    insert_preview_bill(
        runtime.connection(),
        "session-groups",
        user_id(42),
        &preview_draft("2026-05-01 08:30:00", 9.25, "ledger row after reset"),
    )?;

    clear_session_data(runtime.connection_mut(), "session-groups", user_id(42))?;
    assert!(get_import_decision_groups_by_session(
        runtime.connection(),
        "session-groups",
        user_id(42),
    )?
    .is_empty());
    assert!(get_import_history_materializations_by_session(
        runtime.connection(),
        "session-groups",
        user_id(42),
    )?
    .is_empty());
    Ok(())
}

#[test]
fn history_candidate_query_uses_standard_row_day_and_user_scope() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("history_candidates.db"))?;
    seed_users(&runtime, &[42, 77])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    stage_import_parser_templates_with_sources(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: "session-history-query".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
        &[parser_template_draft(
            "2026-05-01 08:30:00",
            -9.25,
            "ledger row",
        )],
        &[import_source_draft(0, "history-query-source")],
        &[import_standard_row_draft(0, 0)],
        false,
    )?;
    runtime.connection().execute(
        "
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, hash, created_at, updated_at
        ) VALUES
            (42, '2026-05-01 08:30:10', '支出', -9.25, 'canteen', 'ledger row', 'card', '餐饮', '午餐', 'h-1', 'now', 'now'),
            (77, '2026-05-01 08:30:10', '支出', -9.25, 'other user', 'hidden', 'card', '餐饮', '午餐', 'h-2', 'now', 'now'),
            (42, '2026-05-02 08:30:10', '支出', -9.25, 'other day', 'hidden', 'card', '餐饮', '午餐', 'h-3', 'now', 'now')
        ",
        [],
    )?;

    let candidates = get_import_history_candidate_bills_for_session(
        runtime.connection(),
        "session-history-query",
        user_id(42),
    )?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].history_bill_id, 1);
    assert_eq!(candidates[0].bill.counterparty, "canteen");
    assert_eq!(candidates[0].bill.amount, Money::from_yuan_str("-9.25")?);
    Ok(())
}

#[test]
fn no_income_expenditure_transfer_pair_is_suppressed_and_skipped_on_confirm(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("no_income_suppression.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-no-income".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let mut outgoing = parser_template_draft("2019-02-09 11:52:47", -813.22, "花呗自动还款");
    outgoing.parser_id = "cmbc".to_string();
    outgoing.parser_payment_method = "跨行支付".to_string();
    outgoing.parser_account_id = "1001".to_string();
    outgoing.parser_original_type = "支出".to_string();

    let mut incoming = parser_template_draft("2019-02-09 11:52:40", 813.22, "余额宝还款");
    incoming.parser_type = "转账".to_string();
    incoming.parser_id = "alipay".to_string();
    incoming.parser_payment_method = "支付宝".to_string();
    incoming.parser_account_id = "1002".to_string();
    incoming.parser_original_type = "不计收支".to_string();
    incoming.parser_original_category = "信用借还".to_string();

    insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-no-income",
        user_id(42),
        &[outgoing, incoming],
    )?;
    let templates = get_parser_templates_by_session(
        runtime.connection(),
        "session-no-income",
        user_id(42),
        None,
    )?;
    let dedup_result =
        SmartDeduplicationEngine.process(dedup_bills_from_parser_templates(&templates));
    assert_eq!(dedup_result.transfer_pairs.len(), 1);
    let preview_drafts = preview_drafts_from_dedup_bills(&dedup_result.kept_bills);
    assert_eq!(preview_drafts.len(), 1);
    assert_eq!(preview_drafts[0].preview_type, "转账");
    assert!(!preview_drafts[0].preview_selected);
    assert_eq!(
        preview_drafts[0]
            .preview_matching_feedback
            .pointer("/annotation/type")
            .and_then(|value| value.as_str()),
        Some("no_income_expenditure")
    );

    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-no-income",
        user_id(42),
        &preview_drafts,
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-no-income",
        user_id(42),
        false,
    )?;
    assert_eq!(previews.len(), 1);
    assert!(!previews[0].preview_selected);

    update_preview_selection(
        runtime.connection_mut(),
        &[previews[0].id],
        true,
        user_id(42),
    )?;
    let result =
        confirm_preview_to_bills(runtime.connection_mut(), "session-no-income", user_id(42))?;
    assert_eq!(result.confirmed_count, 0);
    assert_eq!(result.skipped_count, 1);
    let bill_count: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(bill_count, 0);
    Ok(())
}

#[test]
fn transfer_pair_missing_destination_account_is_review_blocked() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("transfer_account_review.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-transfer-review".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let transfer_bill = DedupBill {
        date: "2026-05-04 10:00:00".to_string(),
        amount: Money::from_yuan_str("-100.00").expect("amount"),
        transaction_type: "转账".to_string(),
        source_account_id: "101".to_string(),
        destination_account_id: Some("wallet".to_string()),
        dedup_type: Some("transfer".to_string()),
        template_id: Some("1".to_string()),
        merged_template_ids: vec!["2".to_string()],
        ..DedupBill::default()
    };
    let preview_drafts = preview_drafts_from_dedup_bills(&[transfer_bill]);
    assert_eq!(preview_drafts[0].preview_type, "转账");
    assert!(!preview_drafts[0].preview_selected);
    assert_eq!(
        preview_drafts[0]
            .preview_matching_feedback
            .pointer("/annotation/type")
            .and_then(|value| value.as_str()),
        Some("transfer_account_direction")
    );

    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-transfer-review",
        user_id(42),
        &preview_drafts,
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-transfer-review",
        user_id(42),
        false,
    )?;
    update_preview_selection(
        runtime.connection_mut(),
        &[previews[0].id],
        true,
        user_id(42),
    )?;
    let result = confirm_preview_to_bills(
        runtime.connection_mut(),
        "session-transfer-review",
        user_id(42),
    )?;
    assert_eq!(result.confirmed_count, 0);
    assert_eq!(result.skipped_count, 1);
    Ok(())
}

#[test]
fn parser_template_batch_insert_rolls_back_on_staging_error() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("parser_rollback.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    runtime.connection().execute_batch(
        "
        CREATE TRIGGER fail_bad_parser_template
        BEFORE INSERT ON bills_parser_template
        WHEN NEW.parser_type = 'bad'
        BEGIN
            SELECT RAISE(ABORT, 'bad parser template');
        END;
        ",
    )?;

    let mut bad = parser_template_draft("2026-05-02", -18.5, "bad");
    bad.parser_type = "bad".to_string();
    let result = insert_parser_templates_batch(
        runtime.connection_mut(),
        "session-parser-rollback",
        user_id(42),
        &[parser_template_draft("2026-05-01", -9.25, "good"), bad],
    );

    assert!(result.is_err());
    assert!(get_parser_templates_by_session(
        runtime.connection(),
        "session-parser-rollback",
        user_id(42),
        None,
    )?
    .is_empty());
    Ok(())
}

#[test]
fn parse_staging_rolls_back_session_templates_and_status_on_error() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("parse_stage_rollback.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    runtime.connection().execute_batch(
        "
        CREATE TRIGGER fail_bad_parser_stage
        BEFORE INSERT ON bills_parser_template
        WHEN NEW.parser_type = 'bad'
        BEGIN
            SELECT RAISE(ABORT, 'bad parser stage');
        END;
        ",
    )?;

    let mut bad = parser_template_draft("2026-05-02", -18.5, "bad");
    bad.parser_type = "bad".to_string();
    let result = stage_import_parser_templates(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: "session-parse-stage-rollback".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
        &[parser_template_draft("2026-05-01", -9.25, "good"), bad],
        false,
    );

    assert!(result.is_err());
    assert!(get_import_session(
        runtime.connection(),
        "session-parse-stage-rollback",
        user_id(42),
    )?
    .is_none());
    assert!(get_parser_templates_by_session(
        runtime.connection(),
        "session-parse-stage-rollback",
        user_id(42),
        None,
    )?
    .is_empty());
    Ok(())
}

#[test]
fn preview_single_insert_uses_shared_preview_insert_projection() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("preview_single_insert.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-single-preview".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;

    let inserted_id = insert_preview_bill(
        runtime.connection(),
        "session-single-preview",
        user_id(42),
        &preview_draft("2026-05-03", 27.75, "single"),
    )?;
    assert!(inserted_id > 0);

    let preview = get_preview_bill_by_id(runtime.connection(), inserted_id, user_id(42))?
        .expect("single preview is visible");
    assert_eq!(preview.preview_description, "single");
    assert_eq!(preview.preview_parser_tags, vec!["wechat", "card"]);
    assert_eq!(preview.dedup_source_ids, vec![11, 12]);
    Ok(())
}

#[test]
fn confirm_preview_to_bills_inserts_selected_rows_and_marks_session_completed(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("confirm_preview.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-confirm".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    stage_import_parser_templates_with_sources(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: "session-confirm".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
        &[],
        &[import_source_draft(0, "confirm-source")],
        &[import_standard_row_draft(0, 0)],
        true,
    )?;

    let mut numeric_expense = preview_draft("2026/05/01 08:30", 9.25, "selected expense");
    numeric_expense.preview_type = "3".to_string();
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-confirm",
        user_id(42),
        &[
            numeric_expense,
            preview_draft("2026-05-02", 18.5, "not selected"),
        ],
    )?;
    let previews =
        get_preview_by_session(runtime.connection(), "session-confirm", user_id(42), false)?;
    update_preview_selection(
        runtime.connection_mut(),
        &[previews[0].id],
        true,
        user_id(42),
    )?;

    let result =
        confirm_preview_to_bills(runtime.connection_mut(), "session-confirm", user_id(42))?;

    assert_eq!(result.confirmed_count, 1);
    assert_eq!(result.duplicate_count, 0);
    assert!(result.errors.is_empty());

    let bill = runtime.connection().query_row(
        "SELECT date, type, amount, counterparty, description, payment_method, main_category, \
         sub_category, source_account_id, destination_account_id, destination_amount, batch_id, \
         hash FROM bills WHERE user_id = ?1",
        [42],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<i64>>(8)?,
                row.get::<_, Option<i64>>(9)?,
                row.get::<_, f64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
            ))
        },
    )?;
    assert_eq!(bill.0, "2026-05-01 08:30:00");
    assert_eq!(bill.1, "支出");
    assert_eq!(bill.2, -9.25);
    assert_eq!(bill.3, "canteen");
    assert_eq!(bill.4, "selected expense");
    assert_eq!(bill.5, "card");
    assert_eq!(bill.6, "餐饮");
    assert_eq!(bill.7, "午餐");
    assert_eq!(bill.8, None);
    assert_eq!(bill.9, None);
    assert_eq!(bill.10, 0.0);
    assert_eq!(bill.11.len(), 14);
    assert_eq!(
        bill.12,
        calculate_import_bill_hash(
            "2026-05-01 08:30:00",
            "支出",
            -9.25,
            "canteen",
            "selected expense"
        )
    );

    assert!(get_import_session(runtime.connection(), "session-confirm", user_id(42))?.is_none());
    assert!(
        get_preview_by_session(runtime.connection(), "session-confirm", user_id(42), false)?
            .is_empty()
    );
    assert!(
        get_import_sources_by_session(runtime.connection(), "session-confirm", user_id(42))?
            .is_empty()
    );
    assert!(get_import_standard_rows_by_session(
        runtime.connection(),
        "session-confirm",
        user_id(42)
    )?
    .is_empty());

    let retry = confirm_preview_to_bills(runtime.connection_mut(), "session-confirm", user_id(42));
    assert!(retry.is_err());
    let total_after_retry: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(total_after_retry, 1);
    Ok(())
}

#[test]
fn confirm_history_rewrite_rejects_missing_acknowledgement() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("history_ack_missing.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-history-ack-missing".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    runtime.connection().execute(
        "
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, hash, created_at, updated_at,
            import_history_id
        ) VALUES (9001, 42, '2026-05-01 08:30:00', '支出', -9.25, 'old',
                  'old note', 'card', '餐饮', '午餐', 'old-hash', 'old', 'old', 1)
        ",
        [],
    )?;
    let preview_id = insert_preview_bill(
        runtime.connection(),
        "session-history-ack-missing",
        user_id(42),
        &history_rewrite_preview_draft(
            "update_history",
            9001,
            1,
            "history-duplicate:9001",
            None,
            "merged note",
        ),
    )?;
    insert_import_history_materializations_batch(
        runtime.connection_mut(),
        "session-history-ack-missing",
        user_id(42),
        &[ImportHistoryMaterializationDraft {
            history_bill_id: 9001,
            history_bill_version: 1,
            materialized_payload: json!({"operation": "update_history"}),
            rewrite_reason: "same_amount|same_direction".to_string(),
        }],
    )?;
    update_preview_selection(runtime.connection_mut(), &[preview_id], true, user_id(42))?;

    let result = confirm_preview_to_bills(
        runtime.connection_mut(),
        "session-history-ack-missing",
        user_id(42),
    );

    assert!(result
        .expect_err("missing ack is rejected")
        .to_string()
        .contains("history rewrite acknowledgement is required"));
    let description: String = runtime.connection().query_row(
        "SELECT description FROM bills WHERE id = 9001 AND user_id = 42",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(description, "old note");
    assert!(get_import_session(
        runtime.connection(),
        "session-history-ack-missing",
        user_id(42)
    )?
    .is_some());
    Ok(())
}

#[test]
fn confirm_history_rewrite_rejects_missing_visible_marker() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("history_ack_marker.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-history-marker".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    let mut draft = history_rewrite_preview_draft(
        "update_history",
        9001,
        1,
        "history-duplicate:9001",
        None,
        "merged note",
    );
    draft.preview_matching_feedback["annotation"]["type"] = json!("manual_review");
    let preview_id = insert_preview_bill(
        runtime.connection(),
        "session-history-marker",
        user_id(42),
        &draft,
    )?;
    update_preview_selection(runtime.connection_mut(), &[preview_id], true, user_id(42))?;

    let result = confirm_preview_to_bills(
        runtime.connection_mut(),
        "session-history-marker",
        user_id(42),
    );

    assert!(result
        .expect_err("missing visible marker is rejected")
        .to_string()
        .contains("visible preview marker"));
    assert!(
        get_import_session(runtime.connection(), "session-history-marker", user_id(42))?.is_some()
    );
    Ok(())
}

#[test]
fn confirm_history_duplicate_ack_updates_history_and_audits() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("history_ack_update.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-history-ack-update".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    runtime.connection().execute(
        "
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, hash, created_at, updated_at,
            source_account_id, import_history_id
        ) VALUES (9001, 42, '2026-05-01 08:30:00', '支出', -9.25, 'old',
                  'old note', 'card', '餐饮', '午餐', 'old-hash', 'old', 'old', 101, 1)
        ",
        [],
    )?;
    let mut draft = history_rewrite_preview_draft(
        "update_history",
        9001,
        1,
        "history-duplicate:9001",
        None,
        "merged note",
    );
    draft.preview_counterparty = "old | imported".to_string();
    draft.preview_source_account_id = Some(101);
    let preview_id = insert_preview_bill(
        runtime.connection(),
        "session-history-ack-update",
        user_id(42),
        &draft,
    )?;
    insert_import_history_materializations_batch(
        runtime.connection_mut(),
        "session-history-ack-update",
        user_id(42),
        &[ImportHistoryMaterializationDraft {
            history_bill_id: 9001,
            history_bill_version: 1,
            materialized_payload: json!({"operation": "update_history"}),
            rewrite_reason: "same_amount|same_direction".to_string(),
        }],
    )?;
    update_preview_selection(runtime.connection_mut(), &[preview_id], true, user_id(42))?;
    let ack = history_rewrite_ack(
        "session-history-ack-update",
        vec![preview_id],
        preview_id,
        "update_history",
        9001,
        1,
        "history-duplicate:9001",
    );

    let result = confirm_preview_to_bills_with_ack(
        runtime.connection_mut(),
        "session-history-ack-update",
        user_id(42),
        Some(&ack),
    )?;

    assert_eq!(result.confirmed_count, 1);
    let bill = runtime.connection().query_row(
        "
        SELECT counterparty, description, import_history_id
        FROM bills WHERE id = 9001 AND user_id = 42
        ",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    assert_eq!(bill.0, "old | imported");
    assert_eq!(bill.1, "merged note");
    assert_eq!(bill.2, 2);
    let operations: i64 = runtime.connection().query_row(
        "
        SELECT COUNT(*) FROM import_confirm_operations
        WHERE session_id = 'session-history-ack-update'
          AND user_id = 42
          AND operation_kind = 'update_history'
          AND history_bill_id = 9001
          AND status = 'applied'
        ",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(operations, 1);
    assert!(get_import_session(
        runtime.connection(),
        "session-history-ack-update",
        user_id(42)
    )?
    .is_none());
    Ok(())
}

#[test]
fn confirm_history_rewrite_rejects_stale_history_version() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("history_ack_stale.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-history-ack-stale".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    runtime.connection().execute(
        "
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, hash, created_at, updated_at,
            import_history_id
        ) VALUES (9001, 42, '2026-05-01 08:30:00', '支出', -9.25, 'old',
                  'old note', 'card', '餐饮', '午餐', 'old-hash', 'old', 'old', 2)
        ",
        [],
    )?;
    let preview_id = insert_preview_bill(
        runtime.connection(),
        "session-history-ack-stale",
        user_id(42),
        &history_rewrite_preview_draft(
            "update_history",
            9001,
            1,
            "history-duplicate:9001",
            None,
            "merged note",
        ),
    )?;
    insert_import_history_materializations_batch(
        runtime.connection_mut(),
        "session-history-ack-stale",
        user_id(42),
        &[ImportHistoryMaterializationDraft {
            history_bill_id: 9001,
            history_bill_version: 1,
            materialized_payload: json!({"operation": "update_history"}),
            rewrite_reason: "same_amount|same_direction".to_string(),
        }],
    )?;
    update_preview_selection(runtime.connection_mut(), &[preview_id], true, user_id(42))?;
    let ack = history_rewrite_ack(
        "session-history-ack-stale",
        vec![preview_id],
        preview_id,
        "update_history",
        9001,
        1,
        "history-duplicate:9001",
    );

    let result = confirm_preview_to_bills_with_ack(
        runtime.connection_mut(),
        "session-history-ack-stale",
        user_id(42),
        Some(&ack),
    );

    assert!(result
        .expect_err("stale history version is rejected")
        .to_string()
        .contains("history bill version is stale"));
    let version: i64 = runtime.connection().query_row(
        "SELECT import_history_id FROM bills WHERE id = 9001 AND user_id = 42",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(version, 2);
    assert!(get_import_session(
        runtime.connection(),
        "session-history-ack-stale",
        user_id(42)
    )?
    .is_some());
    Ok(())
}

#[test]
fn confirm_history_transfer_ack_inserts_base_and_deletes_history_counterpart(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("history_ack_transfer.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0,
            updated_at TEXT
        );
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL,
            name TEXT NOT NULL
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (bill_id, tag_id)
        );
        INSERT INTO accounts(id, user_id, name, balance, initial_balance, updated_at)
        VALUES (100, 42, 'wallet', 1000.0, 1000.0, 'old'),
               (200, 42, 'bank', 50.0, 50.0, 'old');
        INSERT INTO tags(id, user_id, name) VALUES (7, 42, 'history-tag');
        INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, hash, created_at, updated_at,
            source_account_id, import_history_id
        ) VALUES (9002, 42, '2026-05-01 08:30:00', '收入', 100.0, 'bank',
                  'incoming side', 'bank-card', '转账', '入账', 'old-transfer',
                  'old', 'old', 200, 1);
        INSERT INTO bill_tags(bill_id, tag_id, created_at) VALUES (9002, 7, 'old');
        ",
    )?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-history-transfer-ack".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    let mut draft = history_rewrite_preview_draft(
        "merge_transfer_history",
        9002,
        1,
        "history-transfer:9002",
        Some("incoming"),
        "merged transfer",
    );
    draft.preview_type = "转账".to_string();
    draft.preview_amount = 100.0;
    draft.preview_destination_amount = 100.0;
    draft.preview_counterparty = "wallet | bank".to_string();
    draft.preview_source_account_id = Some(100);
    draft.preview_destination_account_id = Some(200);
    let preview_id = insert_preview_bill(
        runtime.connection(),
        "session-history-transfer-ack",
        user_id(42),
        &draft,
    )?;
    insert_import_history_materializations_batch(
        runtime.connection_mut(),
        "session-history-transfer-ack",
        user_id(42),
        &[ImportHistoryMaterializationDraft {
            history_bill_id: 9002,
            history_bill_version: 1,
            materialized_payload: json!({"operation": "merge_transfer_history"}),
            rewrite_reason: "same_amount|opposite_direction".to_string(),
        }],
    )?;
    update_preview_selection(runtime.connection_mut(), &[preview_id], true, user_id(42))?;
    let ack = history_rewrite_ack(
        "session-history-transfer-ack",
        vec![preview_id],
        preview_id,
        "merge_transfer_history",
        9002,
        1,
        "history-transfer:9002",
    );

    let result = confirm_preview_to_bills_with_ack(
        runtime.connection_mut(),
        "session-history-transfer-ack",
        user_id(42),
        Some(&ack),
    )?;

    assert_eq!(result.confirmed_count, 1);
    let bills = runtime.connection().query_row(
        "
        SELECT COUNT(*), MIN(id), MAX(type), MAX(source_account_id), MAX(destination_account_id)
        FROM bills WHERE user_id = 42
        ",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
    )?;
    assert_eq!(bills.0, 1);
    assert_ne!(bills.1, 9002);
    assert_eq!(bills.2, "转账");
    assert_eq!(bills.3, 100);
    assert_eq!(bills.4, 200);
    let moved_tags: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bill_tags WHERE tag_id = 7 AND bill_id != 9002",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(moved_tags, 1);
    let balances = runtime.connection().query_row(
        "
        SELECT
            (SELECT balance FROM accounts WHERE id = 100 AND user_id = 42),
            (SELECT balance FROM accounts WHERE id = 200 AND user_id = 42)
        ",
        [],
        |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
    )?;
    assert_eq!(balances, (900.0, 150.0));
    let audit: i64 = runtime.connection().query_row(
        "
        SELECT COUNT(*) FROM import_confirm_operations
        WHERE operation_kind = 'merge_transfer_history'
          AND history_bill_id = 9002
          AND deleted_bill_id = 9002
        ",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(audit, 1);
    Ok(())
}

#[test]
fn confirm_preview_to_bills_counts_duplicates_and_continues() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("confirm_duplicates.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-duplicate".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-duplicate",
        user_id(42),
        &[
            preview_draft("2026-05-01", 9.25, "already exists"),
            preview_draft("2026-05-02", 18.5, "new one"),
        ],
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-duplicate",
        user_id(42),
        false,
    )?;
    update_preview_selection(
        runtime.connection_mut(),
        &previews
            .iter()
            .map(|preview| preview.id)
            .collect::<Vec<_>>(),
        true,
        user_id(42),
    )?;
    let existing_hash = calculate_import_bill_hash(
        "2026-05-01 00:00:00",
        "支出",
        -9.25,
        "canteen",
        "already exists",
    );
    runtime.connection().execute(
        "
        INSERT INTO bills (
            user_id, date, type, amount, counterparty, description,
            payment_method, main_category, sub_category, batch_id, hash, created_at, updated_at
        ) VALUES (?1, '2026-05-01 00:00:00', '支出', -9.25, 'canteen', 'already exists',
                  'card', '餐饮', '午餐', 'existing', ?2, '2026-05-01T00:00:00', '2026-05-01T00:00:00')
        ",
        (42, existing_hash),
    )?;

    let result =
        confirm_preview_to_bills(runtime.connection_mut(), "session-duplicate", user_id(42))?;

    assert_eq!(result.confirmed_count, 1);
    assert_eq!(result.duplicate_count, 1);
    assert!(result.errors.is_empty());
    let total: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(total, 2);
    assert!(get_import_session(runtime.connection(), "session-duplicate", user_id(42))?.is_none());
    assert!(get_preview_by_session(
        runtime.connection(),
        "session-duplicate",
        user_id(42),
        false
    )?
    .is_empty());
    Ok(())
}

#[test]
fn confirm_preview_to_bills_rolls_back_on_non_duplicate_insert_error() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("confirm_rollback.db"))?;
    seed_users(&runtime, &[42])?;
    init_bills_schema(&runtime)?;
    init_import_staging_schema(runtime.connection())?;
    runtime.connection().execute_batch(
        "
        CREATE TRIGGER fail_bad_bill
        BEFORE INSERT ON bills
        WHEN NEW.description = 'bad insert'
        BEGIN
            SELECT RAISE(ABORT, 'bad bill');
        END;
        ",
    )?;
    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-insert-error".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-insert-error",
        user_id(42),
        &[
            preview_draft("2026-05-01", 9.25, "good insert"),
            preview_draft("2026-05-02", 18.5, "bad insert"),
        ],
    )?;
    let previews = get_preview_by_session(
        runtime.connection(),
        "session-insert-error",
        user_id(42),
        false,
    )?;
    update_preview_selection(
        runtime.connection_mut(),
        &previews
            .iter()
            .map(|preview| preview.id)
            .collect::<Vec<_>>(),
        true,
        user_id(42),
    )?;

    let result = confirm_preview_to_bills(
        runtime.connection_mut(),
        "session-insert-error",
        user_id(42),
    );

    assert!(result.is_err());
    let total: i64 = runtime.connection().query_row(
        "SELECT COUNT(*) FROM bills WHERE user_id = ?1",
        [42],
        |row| row.get(0),
    )?;
    assert_eq!(total, 0);
    let session = get_import_session(runtime.connection(), "session-insert-error", user_id(42))?
        .expect("session remains visible");
    assert_eq!(session.status, "parsing");
    assert_eq!(session.total_confirmed, 0);
    Ok(())
}

#[test]
fn preview_batch_insert_rolls_back_on_staging_error() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("preview_rollback.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;
    runtime.connection().execute_batch(
        "
        CREATE TRIGGER fail_bad_preview
        BEFORE INSERT ON bills_preview
        WHEN NEW.preview_type = 'bad'
        BEGIN
            SELECT RAISE(ABORT, 'bad preview');
        END;
        ",
    )?;

    let mut bad = preview_draft("2026-05-02", 18.5, "bad");
    bad.preview_type = "bad".to_string();
    let result = insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-rollback",
        user_id(42),
        &[preview_draft("2026-05-01", 9.25, "good"), bad],
    );

    assert!(result.is_err());
    assert_eq!(
        count_preview_by_session(runtime.connection(), "session-rollback", user_id(42), false)?,
        0
    );
    Ok(())
}

#[test]
fn import_preview_page_queries_use_ordered_composite_indexes() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("preview_page_indexes.db"))?;
    init_import_staging_schema(runtime.connection())?;

    let preview_indexes = table_indexes(&runtime, "bills_preview")?;
    assert!(preview_indexes
        .iter()
        .any(|(name, _)| name == "idx_preview_session_user_order"));
    assert!(preview_indexes
        .iter()
        .any(|(name, _)| name == "idx_preview_session_user_selected_order"));
    let parser_indexes = table_indexes(&runtime, "bills_parser_template")?;
    assert!(parser_indexes
        .iter()
        .any(|(name, _)| name == "idx_parser_template_session_user_processed_order"));

    let page_plan = query_plan_details(
        &runtime,
        "SELECT * FROM bills_preview \
         WHERE session_id = 'session-large' AND user_id = 42 \
         ORDER BY preview_date ASC, id ASC LIMIT 100 OFFSET 0",
    )?;
    assert!(
        page_plan
            .iter()
            .any(|detail| detail.contains("idx_preview_session_user_order")),
        "preview page query should use the ordered session/user index: {page_plan:?}"
    );
    assert!(
        page_plan
            .iter()
            .all(|detail| !detail.contains("TEMP B-TREE")),
        "preview page query should not need a temporary sort: {page_plan:?}"
    );
    let filtered_page_plan = query_plan_details(
        &runtime,
        "SELECT * FROM bills_preview \
         WHERE session_id = 'session-large' AND user_id = 42 \
         AND preview_type = '支出' \
         AND LOWER(COALESCE(preview_description, '')) LIKE '%target%' \
         ORDER BY preview_date ASC, id ASC LIMIT 100 OFFSET 0",
    )?;
    assert!(
        filtered_page_plan
            .iter()
            .any(|detail| detail.contains("idx_preview_session_user_order")),
        "filtered preview page query should keep page bounds on the ordered session/user index: {filtered_page_plan:?}"
    );
    assert!(
        filtered_page_plan
            .iter()
            .all(|detail| !detail.contains("TEMP B-TREE")),
        "filtered preview page query should not need a temporary sort: {filtered_page_plan:?}"
    );
    let signal_filtered_page_plan = query_plan_details(
        &runtime,
        "SELECT * FROM bills_preview \
         WHERE session_id = 'session-large' AND user_id = 42 \
         AND (LOWER(COALESCE(dedup_type, '')) LIKE '%transfer%' \
              OR json_type(CASE WHEN json_valid(COALESCE(preview_matching_feedback_json, '')) THEN preview_matching_feedback_json ELSE '{}' END, '$.transfer') IS NOT NULL) \
         ORDER BY preview_date ASC, id ASC LIMIT 100 OFFSET 0",
    )?;
    assert!(
        signal_filtered_page_plan
            .iter()
            .any(|detail| detail.contains("idx_preview_session_user_order")),
        "signal-filtered preview page query should still keep page bounds on the ordered session/user index: {signal_filtered_page_plan:?}"
    );
    assert!(
        signal_filtered_page_plan
            .iter()
            .all(|detail| !detail.contains("TEMP B-TREE")),
        "signal-filtered preview page query should not need a temporary sort: {signal_filtered_page_plan:?}"
    );

    let preview_index_plan = query_plan_details(
        &runtime,
        "SELECT id, preview_date, preview_type, preview_amount,
                preview_main_category, preview_sub_category,
                preview_source_account_id, preview_destination_account_id,
                preview_counterparty, preview_payment_method, preview_description,
                preview_parser_id, preview_parser_tags_json, preview_recurring_id,
                preview_recurring_candidate_count, preview_recurring_match_reasons,
                preview_recurring_matched_date, preview_selected, dedup_type,
                dedup_source_ids, preview_matching_feedback_json
         FROM bills_preview
         WHERE session_id = 'session-large' AND user_id = 42
         ORDER BY preview_date ASC, id ASC",
    )?;
    assert!(
        preview_index_plan
            .iter()
            .any(|detail| detail.contains("idx_preview_session_user_order")),
        "preview filter index query should use the ordered session/user index: {preview_index_plan:?}"
    );
    assert!(
        preview_index_plan
            .iter()
            .all(|detail| !detail.contains("TEMP B-TREE")),
        "preview filter index query should not need a temporary sort: {preview_index_plan:?}"
    );

    let selected_page_plan = query_plan_details(
        &runtime,
        "SELECT * FROM bills_preview \
         WHERE session_id = 'session-large' AND user_id = 42 AND preview_selected = 1 \
         ORDER BY preview_date ASC, id ASC LIMIT 100 OFFSET 0",
    )?;
    assert!(
        selected_page_plan
            .iter()
            .any(|detail| detail.contains("idx_preview_session_user_selected_order")),
        "selected preview page query should use the selected ordered index: {selected_page_plan:?}"
    );
    assert!(
        selected_page_plan
            .iter()
            .all(|detail| !detail.contains("TEMP B-TREE")),
        "selected preview page query should not need a temporary sort: {selected_page_plan:?}"
    );

    let parser_template_plan = query_plan_details(
        &runtime,
        "SELECT * FROM bills_parser_template \
         WHERE session_id = 'session-large' AND user_id = 42 AND parser_is_processed = '0' \
         ORDER BY parser_date ASC, id ASC",
    )?;
    assert!(
        parser_template_plan
            .iter()
            .any(|detail| detail.contains("idx_parser_template_session_user_processed_order")),
        "stage2 parser template query should use the session/user/processed ordered index: {parser_template_plan:?}"
    );
    assert!(
        parser_template_plan
            .iter()
            .all(|detail| !detail.contains("TEMP B-TREE")),
        "stage2 parser template query should not need a temporary sort: {parser_template_plan:?}"
    );
    let parser_template_update_plan = query_plan_details(
        &runtime,
        "UPDATE bills_parser_template
         SET parser_is_processed = '1'
         WHERE session_id = 'session-large' AND user_id = 42 AND parser_is_processed = '0'",
    )?;
    assert!(
        parser_template_update_plan
            .iter()
            .any(|detail| detail.contains("idx_parser_template_session_user_processed_order")),
        "stage2 parser template status update should use the session/user/processed index: {parser_template_update_plan:?}"
    );
    Ok(())
}

#[test]
fn import_staging_schema_preserves_foreign_key_cascade() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("import_cascade.db"))?;
    seed_users(&runtime, &[42])?;
    init_import_staging_schema(runtime.connection())?;

    let annotation_columns = table_columns(&runtime, "import_annotation_samples")?;
    for column in [
        "annotated_type",
        "annotated_category_id",
        "annotated_source_account_id",
        "annotated_destination_account_id",
        "created_at",
        "updated_at",
    ] {
        assert!(
            annotation_columns.contains(column),
            "missing annotation column {column}"
        );
    }
    assert!(table_indexes(&runtime, "import_annotation_samples")?
        .iter()
        .any(|(name, unique)| name == "idx_annotation_samples_session_preview_unique" && *unique));
    assert!(foreign_key_targets(&runtime, "import_annotation_samples")?
        .iter()
        .any(|target| target == "users"));

    create_import_session(
        runtime.connection(),
        &ImportSessionDraft {
            session_id: "session-cascade".to_string(),
            user_id: user_id(42),
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        runtime.connection_mut(),
        "session-cascade",
        user_id(42),
        &[preview_draft("2026-05-01", 9.25, "cascade")],
    )?;

    runtime
        .connection()
        .execute("DELETE FROM users WHERE id = ?1", [42])?;
    assert!(get_import_session(runtime.connection(), "session-cascade", user_id(42))?.is_none());
    assert_eq!(
        count_preview_by_session(runtime.connection(), "session-cascade", user_id(42), false)?,
        0
    );
    Ok(())
}

#[test]
fn import_staging_schema_upgrades_legacy_staging_columns() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("legacy_staging.db"))?;
    seed_users(&runtime, &[42])?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE bills_parser_template (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            parser_date TEXT NOT NULL,
            parser_amount REAL NOT NULL,
            parser_type TEXT NOT NULL,
            parser_description TEXT,
            parser_id TEXT NOT NULL,
            parser_counterparty TEXT,
            parser_payment_method TEXT,
            parser_original_type TEXT,
            parser_original_category TEXT,
            parser_account_id TEXT,
            parser_is_processed TEXT DEFAULT '0',
            created_at TEXT NOT NULL
        );
        CREATE TABLE bills_preview (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL DEFAULT 1,
            preview_date TEXT NOT NULL,
            preview_type TEXT NOT NULL,
            preview_amount REAL NOT NULL,
            preview_destination_amount REAL DEFAULT 0,
            preview_main_category TEXT,
            preview_sub_category TEXT,
            preview_source_account_id INTEGER,
            preview_destination_account_id INTEGER,
            preview_counterparty TEXT,
            preview_payment_method TEXT,
            preview_description TEXT,
            preview_selected INTEGER DEFAULT 1,
            dedup_type TEXT,
            dedup_source_ids TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE import_annotation_samples (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            user_id INTEGER NOT NULL,
            preview_id INTEGER NOT NULL
        );
        ",
    )?;

    init_import_staging_schema(runtime.connection())?;

    let parser_columns = table_columns(&runtime, "bills_parser_template")?;
    assert!(parser_columns.contains("parser_tags_json"));

    let preview_columns = table_columns(&runtime, "bills_preview")?;
    for column in [
        "preview_parser_id",
        "preview_parser_tags_json",
        "preview_recurring_id",
        "preview_recurring_name",
        "preview_recurring_candidate_count",
        "preview_recurring_match_score",
        "preview_recurring_match_reasons",
        "preview_recurring_matched_date",
        "preview_matching_feedback_json",
    ] {
        assert!(
            preview_columns.contains(column),
            "missing preview legacy column {column}"
        );
    }

    let annotation_columns = table_columns(&runtime, "import_annotation_samples")?;
    for column in [
        "annotated_type",
        "annotated_category_id",
        "annotated_source_account_id",
        "annotated_destination_account_id",
        "created_at",
        "updated_at",
    ] {
        assert!(
            annotation_columns.contains(column),
            "missing annotation legacy column {column}"
        );
    }
    assert!(table_indexes(&runtime, "import_annotation_samples")?
        .iter()
        .any(|(name, unique)| name == "idx_annotation_samples_session_preview_unique" && *unique));
    Ok(())
}
