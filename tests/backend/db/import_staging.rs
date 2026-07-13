// 中文导读：导入 Postgres staging 的结构性合同测试，锁定 session/source/preview/group 生命周期。
// 维护重点：这些测试读取迁移清单和权威 schema，避免 import stage2 拆分时破坏 staging 表关系。
// 不变式：staging 子表必须由 import_sessions 级联清理，preview 与决策/历史 materialization 保持可重建。

use std::{env, error::Error, fs, time::Duration};

use bill_analyser_core::{
    build_import_history_rewrite_ack_token, build_import_history_rewrite_operation_id,
    ImportHistoryRewriteOperation, UserId,
};
use bill_analyser_db::{
    apply_import_decision_group_command, apply_preview_learning_decision,
    apply_preview_patches_preserving_selection, apply_preview_transfer_decision,
    batch_update_preview_classification, clear_import_preview_materialization_state,
    clear_session_data, clear_user_import_staging_data, confirm_import_command,
    confirm_preview_to_bills, confirm_preview_to_bills_with_ack, create_import_session,
    get_import_decision_groups_by_session, get_import_history_candidate_bills_for_session,
    get_import_history_materializations_by_session, get_import_learning_lifecycle_view,
    get_import_session, get_parser_templates_by_session, get_preview_bill_by_id,
    get_unprocessed_templates_for_dedup, insert_import_decision_groups_batch,
    insert_import_history_materializations_batch, insert_parser_template,
    insert_parser_templates_batch, insert_preview_bill, insert_preview_bills_batch,
    mark_unprocessed_parser_templates_processed_for_session, postgres_initial_schema_path,
    postgres_migration_manifest, query_preview_page_by_session,
    record_import_learning_lifecycle_feedback, replace_preview_selection_with_patches,
    reset_session_preview_selection, review_preview_llm_recommendation,
    stage_import_parser_templates, stage_import_parser_templates_with_sources,
    update_import_session_status, update_parser_template_status, update_preview_bill,
    update_preview_bills_batch, update_preview_recurring_match_decision, update_preview_selection,
    update_session_preview_selection_by_query, ConfirmCommand, ImportDecisionGroupCommand,
    ImportDecisionGroupCommandResult, ImportDecisionGroupDraft, ImportDecisionGroupMemberDraft,
    ImportDecisionPreviewVersion, ImportHistoryMaterializationDraft,
    ImportHistoryRewriteAcknowledgement, ImportHistoryRewriteAcknowledgementOperation,
    ImportLearningLifecycleRecordInput, ImportParserTemplateDraft,
    ImportPreviewClassificationUpdate, ImportPreviewDecision, ImportPreviewDraft,
    ImportPreviewExpectedState, ImportPreviewLearningApply, ImportPreviewLlmReviewRequest,
    ImportPreviewLlmSuggestion, ImportPreviewPageRequest, ImportPreviewPatch,
    ImportPreviewPatchField, ImportPreviewPatchValue, ImportPreviewQueryFilters,
    ImportPreviewRecurringCandidate, ImportPreviewRecurringMatchUpdate, ImportPreviewSelectionMode,
    ImportPreviewSelectionTarget, ImportSessionDraft, ImportSessionStatusUpdate, ImportSourceDraft,
    ImportStandardRowDraft, PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;
include!("../core/transfer_signal_parity_corpus.rs");

fn initial_schema() -> String {
    fs::read_to_string(postgres_initial_schema_path()).expect("initial PostgreSQL schema")
}

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing section start {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing section end {end}"));
    &rest[..end_index]
}

fn ordered_position(source: &str, needles: &[&str]) -> Vec<usize> {
    needles
        .iter()
        .map(|needle| {
            source
                .find(needle)
                .unwrap_or_else(|| panic!("missing marker {needle}"))
        })
        .collect()
}

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for real_postgres_import_six_signal_families_e2e",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for real_postgres_import_six_signal_families_e2e",
            )
            .into()
        })
}

#[test]
fn import_staging_tables_are_registered_in_lifecycle_order() {
    let initial = postgres_migration_manifest()
        .iter()
        .find(|descriptor| descriptor.version == 1)
        .expect("initial schema descriptor");
    let import_tables = [
        "import_sessions",
        "import_sources",
        "import_standard_rows",
        "import_preview_rows",
        "import_decision_groups",
        "import_decision_group_members",
        "import_history_materializations",
        "import_confirm_operations",
    ];

    let positions = import_tables
        .iter()
        .map(|table| {
            initial
                .required_tables
                .iter()
                .position(|registered| registered == table)
                .unwrap_or_else(|| panic!("migration manifest missing {table}"))
        })
        .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    for index in [
        "idx_import_preview_rows_session_page_sort_key",
        "idx_import_decision_groups_session_group_type",
        "idx_import_decision_group_members_group",
        "idx_import_history_materializations_session_history_bill",
    ] {
        assert!(
            initial.required_indexes.contains(&index),
            "migration manifest missing {index}"
        );
    }
}

#[test]
fn import_staging_schema_keeps_rebuildable_preview_materialization_edges() {
    let schema = initial_schema();
    let positions = ordered_position(
        &schema,
        &[
            "CREATE TABLE IF NOT EXISTS import_sessions",
            "CREATE TABLE IF NOT EXISTS import_sources",
            "CREATE TABLE IF NOT EXISTS import_standard_rows",
            "CREATE TABLE IF NOT EXISTS import_preview_rows",
            "CREATE TABLE IF NOT EXISTS import_decision_groups",
            "CREATE TABLE IF NOT EXISTS import_decision_group_members",
            "CREATE TABLE IF NOT EXISTS import_history_materializations",
            "CREATE TABLE IF NOT EXISTS import_confirm_operations",
        ],
    );
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    let sources = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_sources",
        "CREATE TABLE IF NOT EXISTS import_standard_rows",
    );
    assert!(sources
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(sources.contains("UNIQUE (session_id, feature_signature)"));

    let standard_rows = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_standard_rows",
        "COMMENT ON COLUMN import_standard_rows.amount_cents",
    );
    assert!(standard_rows
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(standard_rows
        .contains("source_id BIGINT NOT NULL REFERENCES import_sources(id) ON DELETE CASCADE"));
    assert!(standard_rows.contains("UNIQUE (source_id, source_row_index)"));

    let preview = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_preview_rows",
        "COMMENT ON COLUMN import_preview_rows.amount_cents",
    );
    assert!(preview
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(preview.contains(
        "base_standard_row_id BIGINT REFERENCES import_standard_rows(id) ON DELETE SET NULL"
    ));
    assert!(preview.contains("account_id BIGINT REFERENCES accounts(id) ON DELETE SET NULL"));
    assert!(preview
        .contains("transfer_target_account_id BIGINT REFERENCES accounts(id) ON DELETE SET NULL"));

    let decision_members = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_decision_group_members",
        "CREATE TABLE IF NOT EXISTS import_history_materializations",
    );
    assert!(decision_members.contains(
        "group_id BIGINT NOT NULL REFERENCES import_decision_groups(id) ON DELETE CASCADE"
    ));
    assert!(decision_members
        .contains("preview_row_id BIGINT REFERENCES import_preview_rows(id) ON DELETE CASCADE"));
    assert!(decision_members
        .contains("standard_row_id BIGINT REFERENCES import_standard_rows(id) ON DELETE SET NULL"));

    let history = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_history_materializations",
        "CREATE TABLE IF NOT EXISTS import_confirm_operations",
    );
    assert!(history
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(history.contains("UNIQUE (session_id, history_bill_id)"));
}

#[test]
fn import_session_schema_supports_a_durable_metadata_receipt_without_a_new_table() {
    let schema = initial_schema();
    let sessions = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_sessions",
        "CREATE TABLE IF NOT EXISTS import_sources",
    );

    assert!(sessions.contains("metadata JSONB NOT NULL DEFAULT '{}'::jsonb"));
    assert!(sessions.contains("status TEXT NOT NULL DEFAULT 'created'"));
    assert!(sessions.contains("version BIGINT NOT NULL DEFAULT 1"));
    assert!(sessions.contains("UNIQUE (user_id, session_key)"));
    assert!(!schema.contains("CREATE TABLE IF NOT EXISTS import_confirm_receipts"));
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_import_six_signal_families_e2e() -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("import_six_signal_families").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-six-signal-families").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "six-signal-families-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let mut drafts = Vec::new();
    let mut parser = preview_draft("2026-07-01 09:00:00", "支出", -100, None, None, None, true);
    parser.preview_description = "parser actionable".to_string();
    drafts.push(parser);

    let mut platform = preview_draft("2026-07-01 09:01:00", "支出", -101, None, None, None, true);
    platform.preview_description = "platform duplicate actionable".to_string();
    platform.dedup_type = Some("platform_bank".to_string());
    drafts.push(platform);

    let mut transfer = preview_draft("2026-07-01 09:02:00", "转账", 102, None, None, None, true);
    transfer.preview_description = "transfer actionable".to_string();
    transfer.preview_matching_feedback = json!({"transfer": {"review_status": "pending"}});
    drafts.push(transfer);

    let mut history = preview_draft("2026-07-01 09:03:00", "支出", -103, None, None, None, true);
    history.preview_description = "history actionable".to_string();
    history.preview_matching_feedback =
        json!({"reconciliation": {"planned_operation": "update_history"}});
    drafts.push(history);

    let mut learning = preview_draft("2026-07-01 09:04:00", "支出", -104, None, None, None, true);
    learning.preview_description = "learning actionable".to_string();
    learning.preview_matching_feedback = json!({"learning": {"score": 0.81}});
    drafts.push(learning);

    let mut llm = preview_draft("2026-07-01 09:05:00", "支出", -105, None, None, None, true);
    llm.preview_description = "llm actionable".to_string();
    llm.preview_matching_feedback = json!({"llm": {"confidence": 0.76}});
    drafts.push(llm);

    for (index, payload) in [
        json!({}),
        json!({"review_status": "none"}),
        json!({"score": 0.92, "summary": "actionable", "suppressed": true}),
        json!({"summary": " \t\n\u{3000}"}),
        json!({"score": -0.5}),
        json!({"score": " -0.5 "}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut draft = preview_draft(
            "2026-07-01 09:10:00",
            "支出",
            -200 - index as i64,
            None,
            None,
            None,
            true,
        );
        draft.preview_description = format!("non-actionable learning {index}");
        draft.preview_matching_feedback = json!({"learning": payload});
        drafts.push(draft);
    }

    for (index, payload) in [
        json!({}),
        json!({"review_status": "none"}),
        json!({"confidence": 0.87, "suggested_main_category": "餐饮", "suppressed": true}),
        json!({"suggested_main_category": " \t\n\u{3000}"}),
        json!({"confidence": -0.75}),
        json!({"confidence": " -0.75 "}),
    ]
    .into_iter()
    .enumerate()
    {
        let mut draft = preview_draft(
            "2026-07-01 09:20:00",
            "支出",
            -300 - index as i64,
            None,
            None,
            None,
            true,
        );
        draft.preview_description = format!("non-actionable llm {index}");
        draft.preview_matching_feedback = json!({"llm": payload});
        drafts.push(draft);
    }

    let mut auxiliary = preview_draft("2026-07-01 09:30:00", "支出", -400, None, None, None, true);
    auxiliary.preview_description = "auxiliary-only parser-visible".to_string();
    auxiliary.preview_parser_tags = Some(json!([]));
    auxiliary.preview_matching_feedback = json!({
        "recurring": {"review_status": "pending"},
        "identity_validation": {"issues": [{"field": "category_id"}]}
    });
    drafts.push(auxiliary);

    insert_preview_bills_batch(pool, session_id, scoped_user_id, &drafts)?;
    let all_page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            page_size: 100,
            sort_by: "time".to_string(),
            sort_direction: "asc".to_string(),
            ..ImportPreviewPageRequest::default()
        },
    )?;
    let id_for = |description: &str| {
        all_page
            .rows
            .iter()
            .find(|row| row.preview_description == description)
            .unwrap_or_else(|| panic!("missing preview row {description}"))
            .id
    };
    let parser_id = id_for("parser actionable");
    let platform_id = id_for("platform duplicate actionable");
    let transfer_id = id_for("transfer actionable");
    let history_id = id_for("history actionable");
    let learning_id = id_for("learning actionable");
    let llm_id = id_for("llm actionable");
    let auxiliary_id = id_for("auxiliary-only parser-visible");
    let non_actionable_learning_ids = (0..6)
        .map(|index| id_for(&format!("non-actionable learning {index}")))
        .collect::<Vec<_>>();
    let non_actionable_llm_ids = (0..6)
        .map(|index| id_for(&format!("non-actionable llm {index}")))
        .collect::<Vec<_>>();
    let all_ids = all_page.rows.iter().map(|row| row.id).collect::<Vec<_>>();
    let mut expected_parser_ids = Vec::from([parser_id]);
    expected_parser_ids.extend(non_actionable_learning_ids);
    expected_parser_ids.extend(non_actionable_llm_ids);
    expected_parser_ids.push(auxiliary_id);
    let mut executed_case_count = 0usize;

    for (family, expected_ids) in [
        ("parser", expected_parser_ids),
        ("platform_duplicate", vec![platform_id]),
        ("transfer", vec![transfer_id]),
        ("history", vec![history_id]),
        ("learning", vec![learning_id]),
        ("llm", vec![llm_id]),
    ] {
        for preview_ids in [Vec::new(), all_ids.clone()] {
            let page = query_preview_page_by_session(
                pool,
                session_id,
                scoped_user_id,
                &ImportPreviewPageRequest {
                    page_size: 100,
                    sort_by: "time".to_string(),
                    sort_direction: "asc".to_string(),
                    preview_ids,
                    filters: ImportPreviewQueryFilters {
                        signal: Some(family.to_string()),
                        ..ImportPreviewQueryFilters::default()
                    },
                    ..ImportPreviewPageRequest::default()
                },
            )?;
            let row_ids = page.rows.iter().map(|row| row.id).collect::<Vec<_>>();
            assert_eq!(
                row_ids, expected_ids,
                "SQL-backed family {family} must return exact row IDs in time order"
            );
            assert_eq!(page.total, expected_ids.len());
            executed_case_count += 1;
        }
    }

    let count_keys = all_page
        .metadata
        .counts
        .signals
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        count_keys,
        vec![
            "history".to_string(),
            "learning".to_string(),
            "llm".to_string(),
            "parser".to_string(),
            "platform_duplicate".to_string(),
            "transfer".to_string(),
        ],
        "metadata.counts.signals must contain exactly the six visible families"
    );
    assert!(!all_page.metadata.counts.signals.contains_key("recurring"));
    assert!(!all_page
        .metadata
        .counts
        .signals
        .contains_key("identity_validation"));
    assert_eq!(all_page.metadata.counts.signals.get("parser"), Some(&14));
    assert_eq!(
        all_page.metadata.counts.signals.get("platform_duplicate"),
        Some(&1)
    );
    assert_eq!(all_page.metadata.counts.signals.get("transfer"), Some(&1));
    assert_eq!(all_page.metadata.counts.signals.get("history"), Some(&1));
    assert_eq!(all_page.metadata.counts.signals.get("learning"), Some(&1));
    assert_eq!(all_page.metadata.counts.signals.get("llm"), Some(&1));

    let empty_session_id = "six-signal-families-empty-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: empty_session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 0,
        },
    )?;
    let empty_page = query_preview_page_by_session(
        pool,
        empty_session_id,
        scoped_user_id,
        &ImportPreviewPageRequest::default(),
    )?;
    assert_eq!(empty_page.rows, Vec::new());
    assert_eq!(empty_page.total, 0);
    assert_eq!(
        empty_page.metadata.counts.signals,
        [
            ("history".to_string(), 0usize),
            ("learning".to_string(), 0),
            ("llm".to_string(), 0),
            ("parser".to_string(), 0),
            ("platform_duplicate".to_string(), 0),
            ("transfer".to_string(), 0),
        ]
        .into_iter()
        .collect(),
        "empty SQL-backed preview sessions expose all six zero-count keys"
    );
    assert!(
        executed_case_count >= 6,
        "strict real PostgreSQL target executed only {executed_case_count} family cases"
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_transfer_signal_projection_uses_shared_parity_corpus(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("transfer_signal_projection_parity").await?;
    let pool = &test_db.pool;

    for case in TRANSFER_SIGNAL_PARITY_CORPUS {
        let projected = sqlx::query_scalar::<_, bool>(
            "SELECT COALESCE((import_preview_signal_flags(jsonb_build_object(\
                'preview_type', $1::text, \
                'preview_matching_feedback', jsonb_build_object('transfer', $2::jsonb)\
            ))->>'transfer')::boolean, false)",
        )
        .bind(case.preview_type)
        .bind(case.transfer_json)
        .fetch_one(pool)
        .await?;

        assert_eq!(
            projected, case.expected_filter_visible,
            "PostgreSQL projection diverged from shared transfer parity corpus: {}",
            case.name
        );
    }

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_signal_filters_fail_closed_for_unknown_review_statuses(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("import_invalid_signal_status").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-invalid-signal-status").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "invalid-signal-status-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let cases = [
        (
            "learning invalid",
            "learning",
            json!({"review_status": "pending", "score": 0.91}),
        ),
        (
            "learning blank",
            "learning",
            json!({"review_status": "  ", "score": 0.91}),
        ),
        (
            "learning pending",
            "learning",
            json!({"review_status": "pending", "score": 0.91}),
        ),
        (
            "learning accepted",
            "learning",
            json!({"review_status": "accepted", "score": 0.91}),
        ),
        (
            "learning rejected",
            "learning",
            json!({"review_status": "rejected", "score": 0.91}),
        ),
        (
            "learning needs review",
            "learning",
            json!({"review_status": "needs_review", "score": 0.91}),
        ),
        (
            "llm invalid",
            "llm",
            json!({"review_status": "pending", "confidence": 0.91}),
        ),
        (
            "llm blank",
            "llm",
            json!({"review_status": "", "confidence": 0.91}),
        ),
        (
            "llm pending",
            "llm",
            json!({"review_status": "pending", "confidence": 0.91}),
        ),
        (
            "llm accepted",
            "llm",
            json!({"review_status": "accepted", "confidence": 0.91}),
        ),
        (
            "llm rejected",
            "llm",
            json!({"review_status": "rejected", "confidence": 0.91}),
        ),
        (
            "llm needs review",
            "llm",
            json!({"review_status": "needs_review", "confidence": 0.91}),
        ),
        (
            "transfer invalid",
            "transfer",
            json!({"review_status": "pending", "candidate_type": "transfer", "score": 0.91}),
        ),
        (
            "transfer blank",
            "transfer",
            json!({"review_status": "", "candidate_type": "transfer", "score": 0.91}),
        ),
        (
            "transfer pending",
            "transfer",
            json!({"review_status": "pending", "candidate_type": "transfer", "score": 0.91}),
        ),
        (
            "transfer accepted",
            "transfer",
            json!({"review_status": "accepted", "candidate_type": "transfer", "score": 0.91}),
        ),
        (
            "transfer rejected",
            "transfer",
            json!({"review_status": "rejected", "candidate_type": "transfer", "score": 0.91}),
        ),
        (
            "transfer needs review",
            "transfer",
            json!({"review_status": "needs_review", "candidate_type": "transfer", "score": 0.91}),
        ),
    ];
    let mut drafts = Vec::new();
    for (index, (description, family, section)) in cases.iter().enumerate() {
        let mut draft = preview_draft(
            "2026-07-10 10:00:00",
            "支出",
            -100 - index as i64,
            None,
            None,
            None,
            true,
        );
        draft.preview_description = (*description).to_string();
        draft.preview_matching_feedback = json!({(*family): section});
        drafts.push(draft);
    }
    insert_preview_bills_batch(pool, session_id, scoped_user_id, &drafts)?;
    let seeded = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            page_size: 100,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    let id_for = |description: &str| {
        seeded
            .rows
            .iter()
            .find(|row| row.preview_description == description)
            .unwrap_or_else(|| panic!("missing {description}"))
            .id
    };
    for (description, family) in [
        ("learning invalid", "learning"),
        ("llm invalid", "llm"),
        ("transfer invalid", "transfer"),
    ] {
        sqlx::query(
            r#"UPDATE import_preview_rows
               SET preview_payload = jsonb_set(
                   preview_payload,
                   ARRAY['preview_matching_feedback', $3, 'review_status'],
                   to_jsonb('__invalid_status__'::text),
                   true
               )
               WHERE id = $1 AND user_id = $2"#,
        )
        .bind(id_for(description))
        .bind(user_id)
        .bind(family)
        .execute(pool)
        .await?;
    }

    let all_ids = seeded.rows.iter().map(|row| row.id).collect::<Vec<_>>();
    for (family, expected_descriptions) in [
        (
            "learning",
            vec![
                "learning blank",
                "learning pending",
                "learning accepted",
                "learning rejected",
                "learning needs review",
            ],
        ),
        (
            "llm",
            vec!["llm blank", "llm pending", "llm accepted", "llm rejected"],
        ),
        (
            "transfer",
            vec!["transfer blank", "transfer pending", "transfer accepted"],
        ),
    ] {
        let expected_ids = expected_descriptions
            .iter()
            .map(|description| id_for(description))
            .collect::<Vec<_>>();
        for preview_ids in [Vec::new(), all_ids.clone()] {
            let page = query_preview_page_by_session(
                pool,
                session_id,
                scoped_user_id,
                &ImportPreviewPageRequest {
                    page_size: 100,
                    preview_ids,
                    filters: ImportPreviewQueryFilters {
                        signal: Some(family.to_string()),
                        ..ImportPreviewQueryFilters::default()
                    },
                    ..ImportPreviewPageRequest::default()
                },
            )?;
            let actual_ids = page.rows.iter().map(|row| row.id).collect::<Vec<_>>();
            assert_eq!(actual_ids, expected_ids, "{family} full/index parity");
            assert_eq!(page.total, expected_ids.len(), "{family} total parity");
        }
    }
    let refreshed = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            page_size: 100,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(refreshed.metadata.counts.signals.get("learning"), Some(&5));
    assert_eq!(refreshed.metadata.counts.signals.get("llm"), Some(&4));
    assert_eq!(refreshed.metadata.counts.signals.get("transfer"), Some(&3));

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn creating_import_session_keeps_existing_user_sessions() -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("import_session_create_keeps_old")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-session-create-keeps-old").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: "first-import-session".to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: "second-import-session".to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    assert!(get_import_session(pool, "first-import-session", scoped_user_id)?.is_some());
    assert!(get_import_session(pool, "second-import-session", scoped_user_id)?.is_some());
    let session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM import_sessions WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(session_count, 2);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn recreating_import_session_resets_same_session_children() -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("import_session_recreate_resets").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-session-recreate-resets").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "reused-import-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        pool,
        session_id,
        scoped_user_id,
        &[preview_draft(
            "2026-01-01 09:00:00",
            "支出",
            1234,
            None,
            None,
            None,
            true,
        )],
    )?;
    let before = get_import_session(pool, session_id, scoped_user_id)?.expect("session");
    assert_eq!(before.total_preview, 1);

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 2,
        },
    )?;

    let after = get_import_session(pool, session_id, scoped_user_id)?.expect("session");
    assert_eq!(after.file_count, 2);
    assert_eq!(after.total_parsed, 0);
    assert_eq!(after.total_preview, 0);
    assert_eq!(after.total_confirmed, 0);
    let stale_preview_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM import_preview_rows p
        JOIN import_sessions s ON s.id = p.session_id
        WHERE s.user_id = $1 AND s.session_key = $2
        "#,
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(stale_preview_count, 0);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn import_preview_runtime_facets_identity_validation_and_confirm_are_db_backed(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("import_preview_runtime_contract")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-preview-runtime-contract").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let wallet_id = insert_account(pool, user_id, "现金钱包").await?;
    let bank_id = insert_account(pool, user_id, "银行卡").await?;
    let food_id = insert_category(pool, user_id, "咖啡", "expense", "餐饮/咖啡").await?;
    let transfer_id = insert_category(pool, user_id, "转账", "transfer", "转账").await?;
    let inactive_category_id =
        insert_category(pool, user_id, "已停用", "expense", "停用/已停用").await?;
    sqlx::query("UPDATE categories SET is_active = false WHERE id = $1")
        .bind(inactive_category_id)
        .execute(pool)
        .await?;

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: "runtime-contract-session".to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let inserted = insert_preview_bills_batch(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &[
            preview_draft(
                "2026-05-01 09:00:00",
                "支出",
                1288,
                Some(food_id),
                Some(wallet_id),
                None,
                true,
            ),
            preview_draft(
                "2026-05-02 09:00:00",
                "支出",
                2588,
                Some(inactive_category_id),
                Some(wallet_id),
                None,
                true,
            ),
            preview_draft(
                "2026-05-03 09:00:00",
                "转账",
                5000,
                Some(transfer_id),
                Some(bank_id),
                Some(bank_id),
                true,
            ),
        ],
    )?;
    assert_eq!(inserted, 3);

    let invalid_page = query_preview_page_by_session(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            filters: ImportPreviewQueryFilters {
                category: Some("__invalid__".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(invalid_page.total, 1);
    assert_eq!(
        invalid_page.rows[0]
            .preview_matching_feedback
            .pointer("/identity_validation/issues/0/field"),
        Some(&json!("category_id"))
    );
    assert_eq!(invalid_page.metadata.counts.selected_invalid, 0);
    assert!(invalid_page
        .metadata
        .facets
        .categories
        .iter()
        .all(|facet| facet.value != inactive_category_id.to_string()));

    let all_page = query_preview_page_by_session(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            sort_by: "time".to_string(),
            sort_direction: "asc".to_string(),
            filters: ImportPreviewQueryFilters {
                transaction_type: Some("支出".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(all_page.total, 2);
    assert!(all_page.metadata.facets.categories.iter().any(|facet| {
        facet.value == food_id.to_string() && facet.label.as_deref() == Some("餐饮/咖啡")
    }));
    assert!(all_page.metadata.facets.accounts.iter().any(|facet| {
        facet.value == wallet_id.to_string() && facet.label.as_deref() == Some("现金钱包")
    }));
    assert!(all_page
        .metadata
        .facets
        .tags
        .iter()
        .any(|facet| facet.value == "parser:runtime"));

    let invalid_single_id = insert_preview_bill(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &preview_draft(
            "2026-05-04 09:00:00",
            "支出",
            3888,
            Some(food_id),
            Some(9_999_999),
            None,
            true,
        ),
    )?;
    let invalid_single =
        get_preview_bill_by_id(pool, invalid_single_id, scoped_user_id)?.expect("inserted preview");
    assert_eq!(invalid_single.preview_source_account_id, None);
    assert!(!invalid_single.preview_selected);
    assert_eq!(
        invalid_single
            .preview_matching_feedback
            .pointer("/identity_validation/issues/0/field"),
        Some(&json!("source_account_id"))
    );

    let patched = replace_preview_selection_with_patches(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &[ImportPreviewPatch::new(invalid_single_id).with_changes([
            (
                ImportPreviewPatchField::SourceAccountId,
                ImportPreviewPatchValue::Integer(wallet_id),
            ),
            (
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(9_999_999),
            ),
        ])],
    )?;
    assert_eq!(patched, 1);
    let patched_row =
        get_preview_bill_by_id(pool, invalid_single_id, scoped_user_id)?.expect("patched preview");
    assert_eq!(patched_row.preview_source_account_id, Some(wallet_id));
    assert_eq!(patched_row.category_id, None);
    assert!(patched_row
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .is_some_and(|issues| !issues.is_empty()));

    let resolved_single_id = insert_preview_bill(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &preview_draft(
            "2026-05-04 10:00:00",
            "支出",
            2888,
            Some(9_999_999),
            Some(9_999_999),
            None,
            true,
        ),
    )?;
    let resolved_before_patch = get_preview_bill_by_id(pool, resolved_single_id, scoped_user_id)?
        .expect("inserted preview");
    assert!(resolved_before_patch
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .and_then(Value::as_array)
        .is_some_and(|issues| !issues.is_empty()));

    let resolved_patch_count = replace_preview_selection_with_patches(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &[ImportPreviewPatch::new(resolved_single_id).with_changes([
            (
                ImportPreviewPatchField::SourceAccountId,
                ImportPreviewPatchValue::Integer(wallet_id),
            ),
            (
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(food_id),
            ),
        ])],
    )?;
    assert_eq!(resolved_patch_count, 1);
    let resolved_row =
        get_preview_bill_by_id(pool, resolved_single_id, scoped_user_id)?.expect("patched preview");
    assert_eq!(resolved_row.preview_source_account_id, Some(wallet_id));
    assert_eq!(resolved_row.category_id, Some(food_id));
    assert!(resolved_row
        .preview_matching_feedback
        .pointer("/identity_validation/issues")
        .is_none());

    let resolved_needs_review = query_preview_page_by_session(
        pool,
        "runtime-contract-session",
        scoped_user_id,
        &ImportPreviewPageRequest {
            preview_ids: vec![resolved_single_id],
            filters: ImportPreviewQueryFilters {
                annotation: Some("needs-review".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(resolved_needs_review.total, 0);

    sqlx::query(
        r#"
        UPDATE import_preview_rows
        SET selected = true,
            preview_payload = jsonb_set(preview_payload, '{preview_selected}', 'true'::jsonb, true)
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(invalid_single_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    let confirm_error = confirm_preview_to_bills(pool, "runtime-contract-session", scoped_user_id)
        .expect_err("identity review blocks confirm");
    assert!(
        confirm_error.to_string().contains("identity validation")
            || confirm_error.to_string().contains("requires review"),
        "unexpected confirm error: {confirm_error}"
    );
    let bill_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(bill_count, 0);

    let valid_session = "runtime-contract-valid-confirm";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: valid_session.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bills_batch(
        pool,
        valid_session,
        scoped_user_id,
        &[preview_draft(
            "2026-05-05 09:00:00",
            "支出",
            6888,
            Some(food_id),
            Some(wallet_id),
            None,
            true,
        )],
    )?;
    let confirm = confirm_preview_to_bills(pool, valid_session, scoped_user_id)?;
    assert_eq!(confirm.confirmed_count, 1);
    let session = get_import_session(pool, valid_session, scoped_user_id)?.expect("session");
    assert_eq!(session.status, "confirmed");
    assert_eq!(session.total_confirmed, 1);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn import_preview_selection_by_query_locks_cross_page_targets() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("import_preview_selection_targets")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-preview-selection-targets").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let wallet_id = insert_account(pool, user_id, "现金钱包").await?;
    let bank_id = insert_account(pool, user_id, "银行卡").await?;
    let food_id = insert_category(pool, user_id, "咖啡", "expense", "餐饮/咖啡").await?;
    let transfer_id = insert_category(pool, user_id, "转账", "transfer", "转账").await?;
    let session_id = "selection-target-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let valid_expense_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-06-01 09:00:00",
            "支出",
            1001,
            Some(food_id),
            Some(wallet_id),
            None,
            false,
        ),
    )?;
    let missing_category_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-06-02 09:00:00",
            "支出",
            1002,
            None,
            Some(wallet_id),
            None,
            false,
        ),
    )?;
    let valid_transfer_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-06-03 09:00:00",
            "转账",
            1003,
            Some(transfer_id),
            Some(wallet_id),
            Some(bank_id),
            false,
        ),
    )?;
    let same_account_transfer_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-06-04 09:00:00",
            "转账",
            1004,
            Some(transfer_id),
            Some(bank_id),
            Some(bank_id),
            false,
        ),
    )?;

    let selected_valid = update_session_preview_selection_by_query(
        pool,
        session_id,
        scoped_user_id,
        ImportPreviewSelectionMode::Select,
        ImportPreviewSelectionTarget::Valid,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 1,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(selected_valid, 2);
    let selected_page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            filters: ImportPreviewQueryFilters {
                selected_only: true,
                ..ImportPreviewQueryFilters::default()
            },
            sort_by: "time".to_string(),
            sort_direction: "asc".to_string(),
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(
        selected_page
            .rows
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![valid_expense_id, valid_transfer_id]
    );
    assert_eq!(selected_page.metadata.counts.selected, 2);
    assert_eq!(selected_page.metadata.counts.selected_invalid, 0);

    update_session_preview_selection_by_query(
        pool,
        session_id,
        scoped_user_id,
        ImportPreviewSelectionMode::Deselect,
        ImportPreviewSelectionTarget::All,
        &ImportPreviewPageRequest::default(),
    )?;
    let selected_transfer_reviews = update_session_preview_selection_by_query(
        pool,
        session_id,
        scoped_user_id,
        ImportPreviewSelectionMode::Select,
        ImportPreviewSelectionTarget::NeedsReview,
        &ImportPreviewPageRequest {
            filters: ImportPreviewQueryFilters {
                transaction_type: Some("转账".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(selected_transfer_reviews, 1);
    let transfer_review_page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            filters: ImportPreviewQueryFilters {
                selected_only: true,
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(transfer_review_page.rows[0].id, same_account_transfer_id);
    assert_eq!(transfer_review_page.metadata.counts.selected_invalid, 1);

    update_session_preview_selection_by_query(
        pool,
        session_id,
        scoped_user_id,
        ImportPreviewSelectionMode::Deselect,
        ImportPreviewSelectionTarget::All,
        &ImportPreviewPageRequest::default(),
    )?;
    let inverted = update_session_preview_selection_by_query(
        pool,
        session_id,
        scoped_user_id,
        ImportPreviewSelectionMode::Invert,
        ImportPreviewSelectionTarget::All,
        &ImportPreviewPageRequest {
            preview_ids: vec![valid_expense_id, missing_category_id],
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(inverted, 2);
    let inverted_page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            filters: ImportPreviewQueryFilters {
                selected_only: true,
                ..ImportPreviewQueryFilters::default()
            },
            sort_by: "time".to_string(),
            sort_direction: "asc".to_string(),
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(
        inverted_page
            .rows
            .iter()
            .map(|row| row.id)
            .collect::<Vec<_>>(),
        vec![valid_expense_id, missing_category_id]
    );
    assert_eq!(inverted_page.metadata.counts.selected, 2);
    assert_eq!(inverted_page.metadata.counts.selected_invalid, 1);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_history_materialization_repository_round_trips_and_clears(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("history_materialization_round_trip").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "history-materialization-round-trip").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "history-materialization-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let history_bill_id =
        insert_history_bill(pool, user_id, "materialization-history-bill").await?;

    let candidates =
        get_import_history_candidate_bills_for_session(pool, session_id, scoped_user_id)?;
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.history_bill_id == history_bill_id)
        .expect("history candidate must come from the concrete repository");
    assert_eq!(candidate.history_bill_version, 1);
    assert_eq!(candidate.bill.amount.to_cents(), -1888);

    let inserted = insert_import_history_materializations_batch(
        pool,
        session_id,
        scoped_user_id,
        &[ImportHistoryMaterializationDraft {
            history_bill_id,
            history_bill_version: 1,
            materialized_payload: json!({"planned_operation": "update_history"}),
            rewrite_reason: "RED repository round trip".to_string(),
        }],
    )?;
    assert_eq!(
        inserted, 1,
        "history materialization insert must not be a no-op"
    );

    let fetched = get_import_history_materializations_by_session(pool, session_id, scoped_user_id)?;
    assert_eq!(
        fetched.len(),
        1,
        "inserted history materialization must round-trip"
    );
    assert_eq!(fetched[0].history_bill_id, history_bill_id);
    assert_eq!(fetched[0].history_bill_version, 1);

    let cleared = bill_analyser_db::clear_import_preview_materialization_state(
        pool,
        session_id,
        scoped_user_id,
    )?;
    assert!(
        cleared >= 1,
        "clearing preview materialization must remove history materializations"
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirm_rejects_invalid_history_ack() -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_invalid_history_ack").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-invalid-history-ack").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "现金钱包").await?;
    let category_id = insert_category(pool, user_id, "咖啡", "expense", "餐饮/咖啡").await?;
    let history_bill_id = insert_history_bill(pool, user_id, "invalid-ack-history-bill").await?;
    let session_id = "confirm-invalid-history-ack-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let mut draft = preview_draft(
        "2026-07-02 09:00:00",
        "支出",
        -1888,
        Some(category_id),
        Some(account_id),
        None,
        true,
    );
    draft.preview_matching_feedback = json!({
        "reconciliation": {
            "planned_operation": "update_history",
            "history_bill_id": history_bill_id,
            "history_bill_version": 1,
            "operation_id": "op-invalid",
            "acknowledgement_token": "invalid-token"
        }
    });
    insert_preview_bill(pool, session_id, scoped_user_id, &draft)?;

    let error = confirm_preview_to_bills(pool, session_id, scoped_user_id)
        .expect_err("invalid history acknowledgement must reject confirm");
    assert!(
        error
            .to_string()
            .contains("invalid history acknowledgement")
            || error.to_string().contains("acknowledgement token"),
        "unexpected invalid ack error: {error}"
    );
    let new_bill_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1 AND id <> $2")
            .bind(user_id)
            .bind(history_bill_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(new_bill_count, 0);
    let history_bill_version: i64 =
        sqlx::query_scalar("SELECT version FROM bills WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(history_bill_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(history_bill_version, 1);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirm_receipt_replays_and_generic_paths_preserve_terminal_state(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_receipt_replay_guards").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-receipt-replay-guards").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "回执现金钱包").await?;
    let category_id = insert_category(pool, user_id, "回执餐饮", "expense", "餐饮/回执").await?;
    let session_id = "confirm-receipt-replay-session";
    let unrelated_session_id = "confirm-receipt-unrelated-session";

    for key in [session_id, unrelated_session_id] {
        create_import_session(
            pool,
            &ImportSessionDraft {
                session_id: key.to_string(),
                user_id: scoped_user_id,
                file_count: 1,
            },
        )?;
    }
    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-03 09:00:00",
            "支出",
            -2088,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let acknowledgement = ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids: vec![preview_id],
        operations: Vec::new(),
        selection_scope: json!({
            "mode": "selected",
            "credential": "must-not-be-persisted",
            "raw_bill": "sensitive raw bill"
        }),
    };

    let first = confirm_preview_to_bills_with_ack(
        pool,
        session_id,
        scoped_user_id,
        Some(&acknowledgement),
    )?;
    assert_eq!(first.confirmed_count, 1);

    let (status, metadata, request_version) =
        session_receipt_state(pool, user_id, session_id).await?;
    assert_eq!(status, "confirmed");
    let receipt = metadata
        .get("confirm_receipt")
        .and_then(Value::as_object)
        .expect("terminal confirm receipt");
    let receipt_keys = receipt.keys().map(String::as_str).collect::<Vec<_>>();
    assert_eq!(
        receipt_keys,
        vec![
            "command_fingerprint",
            "http_status",
            "receipt_schema_version",
            "request_session_version",
            "response_schema_version",
            "success_envelope",
        ]
    );
    assert_eq!(receipt["receipt_schema_version"], 1);
    assert_eq!(receipt["response_schema_version"], 1);
    assert_eq!(receipt["http_status"], 200);
    assert_eq!(receipt["request_session_version"], request_version - 1);
    assert_eq!(
        receipt["success_envelope"],
        json!({
            "success": true,
            "data": {
                "imported_count": first.confirmed_count,
                "skipped_count": first.skipped_count + first.duplicate_count,
                "errors": first.errors.clone(),
            }
        })
    );
    assert_eq!(
        receipt["command_fingerprint"]
            .as_str()
            .expect("fingerprint")
            .len(),
        64
    );
    let persisted_metadata = serde_json::to_string(&metadata)?;
    for forbidden in [
        "must-not-be-persisted",
        "sensitive raw bill",
        "credential",
        "raw_bill",
        "selected_preview_ids",
        "operations",
        "acknowledgement_token",
        "fingerprint_source",
    ] {
        assert!(
            !persisted_metadata.contains(forbidden),
            "receipt leaked forbidden source field {forbidden}"
        );
    }

    let replay = confirm_preview_to_bills_with_ack(
        pool,
        session_id,
        scoped_user_id,
        Some(&acknowledgement),
    )?;
    assert_eq!(replay, first);
    assert_eq!(count_user_bills(pool, user_id).await?, 1);
    assert_eq!(count_session_children(pool, user_id, session_id).await?, 0);

    let conflicting_acknowledgement = ImportHistoryRewriteAcknowledgement {
        selection_scope: json!({"mode": "all"}),
        ..acknowledgement.clone()
    };
    let conflict = confirm_preview_to_bills_with_ack(
        pool,
        session_id,
        scoped_user_id,
        Some(&conflicting_acknowledgement),
    )
    .expect_err("a different command fingerprint must conflict");
    assert!(conflict.to_string().contains("fingerprint conflict"));
    assert_eq!(count_user_bills(pool, user_id).await?, 1);

    let restage = create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 2,
        },
    )
    .expect_err("confirmed session must not be restaged");
    assert!(restage.to_string().contains("confirmed"));
    assert!(!update_import_session_status(
        pool,
        &ImportSessionStatusUpdate {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            status: "preview_ready".to_string(),
            total_parsed: Some(99),
            total_preview: Some(99),
            total_confirmed: None,
        },
    )?);
    let cleared = clear_session_data(pool, session_id, scoped_user_id)?;
    assert_eq!(cleared.session_count, 0);
    let insert_error = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-03 10:00:00",
            "支出",
            -1,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )
    .expect_err("confirmed session must reject new preview children");
    assert!(insert_error.to_string().contains("confirmed"));

    let (status_after_guards, metadata_after_guards, _) =
        session_receipt_state(pool, user_id, session_id).await?;
    assert_eq!(status_after_guards, "confirmed");
    assert_eq!(metadata_after_guards, metadata);
    assert!(get_import_session(pool, unrelated_session_id, scoped_user_id)?.is_some());

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_history_ack_cas_updates_once_and_replays_without_children(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("history_ack_cas_replay").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "history-ack-cas-replay").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "历史回执钱包").await?;
    let category_id =
        insert_category(pool, user_id, "历史回执分类", "expense", "餐饮/历史").await?;
    let history_bill_id = insert_history_bill(pool, user_id, "history-cas-source").await?;
    let session_id = "history-ack-cas-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let group_key = "history-cas-group";
    let operation = ImportHistoryRewriteOperation::UpdateHistory;
    let operation_id =
        build_import_history_rewrite_operation_id(operation, history_bill_id, 1, group_key);
    let acknowledgement_token = build_import_history_rewrite_ack_token(
        session_id,
        &operation_id,
        operation,
        history_bill_id,
        1,
    );
    let mut draft = preview_draft(
        "2026-07-04 09:00:00",
        "支出",
        -1888,
        Some(category_id),
        Some(account_id),
        None,
        true,
    );
    draft.preview_description = "history-cas-updated".to_string();
    draft.preview_matching_feedback = json!({
        "reconciliation": {
            "planned_operation": "update_history",
            "history_bill_id": history_bill_id,
            "history_bill_version": 1,
            "group_key": group_key,
            "operation_id": operation_id,
            "acknowledgement_token": acknowledgement_token,
            "review_status": "pending"
        }
    });
    let preview_id = insert_preview_bill(pool, session_id, scoped_user_id, &draft)?;
    insert_import_history_materializations_batch(
        pool,
        session_id,
        scoped_user_id,
        &[ImportHistoryMaterializationDraft {
            history_bill_id,
            history_bill_version: 1,
            materialized_payload: json!({
                "planned_operation": "update_history",
                "operation_id": operation_id
            }),
            rewrite_reason: "history CAS fixture".to_string(),
        }],
    )?;
    let acknowledgement = ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids: vec![preview_id],
        operations: vec![ImportHistoryRewriteAcknowledgementOperation {
            preview_id,
            operation_id,
            planned_operation: "update_history".to_string(),
            history_bill_id,
            history_bill_version: 1,
            acknowledgement_token,
        }],
        selection_scope: json!({"mode": "selected"}),
    };

    let first = confirm_preview_to_bills_with_ack(
        pool,
        session_id,
        scoped_user_id,
        Some(&acknowledgement),
    )?;
    assert_eq!(first.confirmed_count, 1);
    let history =
        sqlx::query("SELECT description, version FROM bills WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(history_bill_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(
        history
            .try_get::<Option<String>, _>("description")?
            .as_deref(),
        Some("history-cas-updated")
    );
    assert_eq!(history.try_get::<i64, _>("version")?, 2);
    assert_eq!(count_user_bills(pool, user_id).await?, 1);

    let replay = confirm_preview_to_bills_with_ack(
        pool,
        session_id,
        scoped_user_id,
        Some(&acknowledgement),
    )?;
    assert_eq!(replay, first);
    let replay_version: i64 =
        sqlx::query_scalar("SELECT version FROM bills WHERE user_id = $1 AND id = $2")
            .bind(user_id)
            .bind(history_bill_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(replay_version, 2);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirm_command_applies_patch_selection_version_and_replay_in_one_lock(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_command_locked_mutations").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-command-locked-mutations").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "命令钱包").await?;
    let category_id =
        insert_category(pool, user_id, "命令分类", "expense", "餐饮/命令分类").await?;
    let session_id = "confirm-command-locked-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let first_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-08 09:00:00",
            "支出",
            -6000,
            None,
            Some(account_id),
            None,
            true,
        ),
    )?;
    let second_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-08 09:01:00",
            "支出",
            -7000,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let request_version: i64 = sqlx::query_scalar(
        "SELECT version FROM import_sessions WHERE user_id = $1 AND session_key = $2",
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    let command = ConfirmCommand {
        session_id: session_id.to_string(),
        expected_session_version: Some(request_version),
        preview_patches: vec![ImportPreviewPatch::new(first_id).with_changes([
            (
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(category_id),
            ),
            (
                ImportPreviewPatchField::Description,
                ImportPreviewPatchValue::Text("命令内补丁".to_string()),
            ),
        ])],
        selected_preview_ids: Some(vec![first_id]),
        preserve_unpatched_selection: false,
        history_acknowledgement: None,
        declared_confirm_time_effects: Vec::new(),
    };

    let first = confirm_import_command(pool, scoped_user_id, &command)?;
    assert!(!first.replayed);
    assert_eq!(first.http_status, 200);
    assert_eq!(first.success_envelope["data"]["imported_count"], 1);
    let bill = sqlx::query(
        "SELECT transaction_type, category_id, description FROM bills WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(bill.try_get::<String, _>("transaction_type")?, "expense");
    assert_eq!(
        bill.try_get::<Option<i64>, _>("category_id")?,
        Some(category_id)
    );
    assert_eq!(
        bill.try_get::<Option<String>, _>("description")?.as_deref(),
        Some("命令内补丁")
    );
    assert_eq!(count_user_bills(pool, user_id).await?, 1);
    assert!(get_preview_bill_by_id(pool, second_id, scoped_user_id)?.is_none());

    let replay = confirm_import_command(pool, scoped_user_id, &command)?;
    assert!(replay.replayed);
    assert_eq!(replay.http_status, first.http_status);
    assert_eq!(replay.success_envelope, first.success_envelope);

    let empty_conflict = ConfirmCommand {
        selected_preview_ids: Some(Vec::new()),
        preview_patches: Vec::new(),
        ..command.clone()
    };
    assert!(
        confirm_import_command(pool, scoped_user_id, &empty_conflict)
            .expect_err("explicit empty selection is a different semantic command")
            .to_string()
            .contains("fingerprint conflict")
    );

    let empty_session = "confirm-command-explicit-empty-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: empty_session.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        empty_session,
        scoped_user_id,
        &preview_draft(
            "2026-07-08 10:00:00",
            "支出",
            -8000,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let empty_version: i64 = sqlx::query_scalar(
        "SELECT version FROM import_sessions WHERE user_id = $1 AND session_key = $2",
    )
    .bind(user_id)
    .bind(empty_session)
    .fetch_one(pool)
    .await?;
    let mut empty_command = ConfirmCommand {
        session_id: empty_session.to_string(),
        expected_session_version: Some(empty_version - 1),
        preview_patches: Vec::new(),
        selected_preview_ids: Some(Vec::new()),
        preserve_unpatched_selection: false,
        history_acknowledgement: None,
        declared_confirm_time_effects: Vec::new(),
    };
    assert!(confirm_import_command(pool, scoped_user_id, &empty_command)
        .expect_err("stale session version must reject before mutation")
        .to_string()
        .contains("version conflict"));
    assert_failed_confirm_retryable(pool, user_id, empty_session, 1, 1).await?;
    empty_command.expected_session_version = Some(empty_version);
    let empty = confirm_import_command(pool, scoped_user_id, &empty_command)?;
    assert_eq!(empty.success_envelope["data"]["imported_count"], 0);
    assert_eq!(count_user_bills(pool, user_id).await?, 1);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_postgres_concurrent_identical_confirm_commands_share_one_effect_set(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_concurrent_identical").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-concurrent-identical").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "并发相同钱包").await?;
    let category_id =
        insert_category(pool, user_id, "并发相同分类", "expense", "餐饮/并发").await?;
    let session_id = "confirm-concurrent-identical-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-05 09:00:00",
            "支出",
            -3000,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let acknowledgement = ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids: vec![preview_id],
        operations: Vec::new(),
        selection_scope: json!({"mode": "selected"}),
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let first_pool = pool.clone();
    let first_barrier = barrier.clone();
    let first_ack = acknowledgement.clone();
    let first = tokio::task::spawn_blocking(move || {
        first_barrier.wait();
        confirm_preview_to_bills_with_ack(&first_pool, session_id, scoped_user_id, Some(&first_ack))
    });
    let second_pool = pool.clone();
    let second_barrier = barrier.clone();
    let second_ack = acknowledgement.clone();
    let second = tokio::task::spawn_blocking(move || {
        second_barrier.wait();
        confirm_preview_to_bills_with_ack(
            &second_pool,
            session_id,
            scoped_user_id,
            Some(&second_ack),
        )
    });
    barrier.wait();
    let first = first.await??;
    let second = second.await??;
    assert_eq!(first, second);
    assert_eq!(first.confirmed_count, 1);
    assert_eq!(count_user_bills(pool, user_id).await?, 1);
    assert_eq!(count_session_children(pool, user_id, session_id).await?, 0);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_postgres_concurrent_different_confirm_command_conflicts_without_second_effect(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_concurrent_conflict").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-concurrent-conflict").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "并发冲突钱包").await?;
    let category_id =
        insert_category(pool, user_id, "并发冲突分类", "expense", "餐饮/冲突").await?;
    let session_id = "confirm-concurrent-conflict-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-05 10:00:00",
            "支出",
            -3100,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let command = |mode: &str| ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids: vec![preview_id],
        operations: Vec::new(),
        selection_scope: json!({"mode": mode}),
    };
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let first_pool = pool.clone();
    let first_barrier = barrier.clone();
    let first_ack = command("selected");
    let first = tokio::task::spawn_blocking(move || {
        first_barrier.wait();
        confirm_preview_to_bills_with_ack(&first_pool, session_id, scoped_user_id, Some(&first_ack))
    });
    let second_pool = pool.clone();
    let second_barrier = barrier.clone();
    let second_ack = command("all");
    let second = tokio::task::spawn_blocking(move || {
        second_barrier.wait();
        confirm_preview_to_bills_with_ack(
            &second_pool,
            session_id,
            scoped_user_id,
            Some(&second_ack),
        )
    });
    barrier.wait();
    let outcomes = [first.await?, second.await?];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    let conflict = outcomes
        .iter()
        .find_map(|result| result.as_ref().err())
        .expect("one conflicting command");
    assert!(conflict.to_string().contains("fingerprint conflict"));
    assert_eq!(count_user_bills(pool, user_id).await?, 1);
    assert_eq!(count_session_children(pool, user_id, session_id).await?, 0);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_writer_started_before_confirm_cannot_recreate_terminal_children(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_writer_lock_order").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-writer-lock-order").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "并发写守卫钱包").await?;
    let category_id =
        insert_category(pool, user_id, "并发写守卫分类", "expense", "餐饮/并发").await?;
    let session_id = "confirm-writer-lock-order-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-06 08:00:00",
            "支出",
            -1200,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;

    let advisory_key = 7_004_002_i64;
    sqlx::query(
        r#"
        CREATE FUNCTION p2_block_preview_writer() RETURNS trigger AS $$
        BEGIN
            PERFORM pg_advisory_xact_lock(7004002);
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TRIGGER p2_block_preview_writer_trigger
        BEFORE INSERT ON import_preview_rows
        FOR EACH ROW EXECUTE FUNCTION p2_block_preview_writer()
        "#,
    )
    .execute(pool)
    .await?;
    let mut advisory_holder = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(advisory_key)
        .execute(&mut *advisory_holder)
        .await?;

    let writer_pool = pool.clone();
    let writer_preview = preview_draft(
        "2026-07-06 09:00:00",
        "支出",
        -2300,
        Some(category_id),
        Some(account_id),
        None,
        true,
    );
    let writer = tokio::task::spawn_blocking(move || {
        insert_preview_bill(&writer_pool, session_id, scoped_user_id, &writer_preview)
    });
    wait_for_postgres_lock(pool, "advisory").await?;

    let confirm_pool = pool.clone();
    let confirm = tokio::task::spawn_blocking(move || {
        confirm_preview_to_bills(&confirm_pool, session_id, scoped_user_id)
    });
    wait_for_postgres_lock(pool, "transactionid").await?;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(advisory_key)
        .execute(&mut *advisory_holder)
        .await?;
    assert!(writer.await?? > 0);
    assert_eq!(confirm.await??.confirmed_count, 2);

    sqlx::query("DROP TRIGGER p2_block_preview_writer_trigger ON import_preview_rows")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_block_preview_writer()")
        .execute(pool)
        .await?;
    let (status, metadata, _) = session_receipt_state(pool, user_id, session_id).await?;
    assert_eq!(status, "confirmed");
    assert!(metadata.get("confirm_receipt").is_some());
    assert_eq!(count_session_children(pool, user_id, session_id).await?, 0);
    assert_eq!(count_user_bills(pool, user_id).await?, 2);
    assert_eq!(
        confirm_preview_to_bills(pool, session_id, scoped_user_id)?.confirmed_count,
        2
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirm_commits_before_waiting_writer_rechecks_terminal_state(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_first_writer_recheck").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-first-writer-recheck").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "确认先行钱包").await?;
    let category_id =
        insert_category(pool, user_id, "确认先行分类", "expense", "餐饮/确认").await?;
    let session_id = "confirm-first-writer-recheck-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-06 10:00:00",
            "支出",
            -3400,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;

    let confirm_key = 7_004_003_i64;
    let writer_key = 7_004_004_i64;
    sqlx::query(
        r#"
        CREATE FUNCTION p2_block_confirm_bill_insert() RETURNS trigger AS $$
        BEGIN
            PERFORM pg_advisory_xact_lock(7004003);
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TRIGGER p2_block_confirm_bill_insert_trigger
        BEFORE INSERT ON bills
        FOR EACH ROW EXECUTE FUNCTION p2_block_confirm_bill_insert()
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE FUNCTION p2_block_late_preview_writer() RETURNS trigger AS $$
        BEGIN
            PERFORM pg_advisory_xact_lock(7004004);
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TRIGGER p2_block_late_preview_writer_trigger
        BEFORE INSERT ON import_preview_rows
        FOR EACH ROW EXECUTE FUNCTION p2_block_late_preview_writer()
        "#,
    )
    .execute(pool)
    .await?;
    let mut advisory_holder = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1), pg_advisory_lock($2)")
        .bind(confirm_key)
        .bind(writer_key)
        .execute(&mut *advisory_holder)
        .await?;

    let confirm_pool = pool.clone();
    let confirm = tokio::task::spawn_blocking(move || {
        confirm_preview_to_bills(&confirm_pool, session_id, scoped_user_id)
    });
    wait_for_postgres_lock(pool, "advisory").await?;

    let writer_pool = pool.clone();
    let writer_preview = preview_draft(
        "2026-07-06 11:00:00",
        "支出",
        -4500,
        Some(category_id),
        Some(account_id),
        None,
        true,
    );
    let writer = tokio::task::spawn_blocking(move || {
        insert_preview_bill(&writer_pool, session_id, scoped_user_id, &writer_preview)
    });
    wait_for_postgres_lock(pool, "transactionid").await?;

    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(confirm_key)
        .execute(&mut *advisory_holder)
        .await?;
    assert_eq!(confirm.await??.confirmed_count, 1);
    let writer_error = writer
        .await?
        .expect_err("waiting writer must recheck terminal state");
    assert!(writer_error.to_string().contains("terminal"));
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(writer_key)
        .execute(&mut *advisory_holder)
        .await?;

    sqlx::query("DROP TRIGGER p2_block_confirm_bill_insert_trigger ON bills")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_block_confirm_bill_insert()")
        .execute(pool)
        .await?;
    sqlx::query("DROP TRIGGER p2_block_late_preview_writer_trigger ON import_preview_rows")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_block_late_preview_writer()")
        .execute(pool)
        .await?;
    let (status, metadata, _) = session_receipt_state(pool, user_id, session_id).await?;
    assert_eq!(status, "confirmed");
    assert!(metadata.get("confirm_receipt").is_some());
    assert_eq!(count_session_children(pool, user_id, session_id).await?, 0);
    assert_eq!(count_user_bills(pool, user_id).await?, 1);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_preview_multi_patch_rolls_back_when_second_patch_fails(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("preview_patch_atomic_rollback").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-patch-atomic-rollback").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "preview-patch-atomic-rollback-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let first_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft("2026-07-07 08:00:00", "支出", -100, None, None, None, true),
    )?;
    let second_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft("2026-07-07 09:00:00", "支出", -200, None, None, None, true),
    )?;
    let original_first = get_preview_bill_by_id(pool, first_id, scoped_user_id)?
        .expect("first preview before failed patch");
    let patches = vec![
        ImportPreviewPatch::new(first_id).with_change(
            ImportPreviewPatchField::Description,
            ImportPreviewPatchValue::Text("must roll back".to_string()),
        ),
        ImportPreviewPatch::new(second_id).with_change(
            ImportPreviewPatchField::Date,
            ImportPreviewPatchValue::Text("not-a-postgres-timestamp".to_string()),
        ),
    ];

    replace_preview_selection_with_patches(pool, session_id, scoped_user_id, &patches)
        .expect_err("the invalid second patch must fail the whole batch");

    let after_first = get_preview_bill_by_id(pool, first_id, scoped_user_id)?
        .expect("first preview after failed patch");
    assert_eq!(
        after_first.preview_description, original_first.preview_description,
        "the first patch must roll back with the failing second patch"
    );
    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirmed_session_rejects_every_public_preview_mutation_without_writes(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("preview_terminal_mutation_guards").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-terminal-mutation-guards").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "preview-terminal-mutation-guards-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft("2026-07-07 10:00:00", "支出", -300, None, None, None, true),
    )?;
    sqlx::query(
        "UPDATE import_sessions SET status = 'confirmed' WHERE user_id = $1 AND session_key = $2",
    )
    .bind(user_id)
    .bind(session_id)
    .execute(pool)
    .await?;
    let before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(preview) - 'updated_at' FROM import_preview_rows preview WHERE id = $1",
    )
    .bind(preview_id)
    .fetch_one(pool)
    .await?;
    let patch = ImportPreviewPatch::new(preview_id).with_change(
        ImportPreviewPatchField::Description,
        ImportPreviewPatchValue::Text("must stay terminal".to_string()),
    );

    for (name, result) in [
        (
            "single patch",
            update_preview_bill(pool, session_id, scoped_user_id, &patch).map(|_| ()),
        ),
        (
            "batch patch",
            update_preview_bills_batch(
                pool,
                session_id,
                scoped_user_id,
                std::slice::from_ref(&patch),
            )
            .map(|_| ()),
        ),
        (
            "replace patch",
            replace_preview_selection_with_patches(
                pool,
                session_id,
                scoped_user_id,
                std::slice::from_ref(&patch),
            )
            .map(|_| ()),
        ),
        (
            "preserving patch",
            apply_preview_patches_preserving_selection(
                pool,
                session_id,
                scoped_user_id,
                std::slice::from_ref(&patch),
            )
            .map(|_| ()),
        ),
        (
            "selection by ids",
            update_preview_selection(pool, &[preview_id], false, scoped_user_id).map(|_| ()),
        ),
        (
            "selection reset",
            reset_session_preview_selection(pool, session_id, scoped_user_id).map(|_| ()),
        ),
        (
            "selection query",
            update_session_preview_selection_by_query(
                pool,
                session_id,
                scoped_user_id,
                ImportPreviewSelectionMode::Deselect,
                ImportPreviewSelectionTarget::All,
                &ImportPreviewPageRequest::default(),
            )
            .map(|_| ()),
        ),
        (
            "classification",
            batch_update_preview_classification(
                pool,
                session_id,
                scoped_user_id,
                &[ImportPreviewClassificationUpdate {
                    preview_id,
                    preview_type: "收入".to_string(),
                    preview_main_category: "终态".to_string(),
                    preview_sub_category: "不可写".to_string(),
                    preview_source_account_id: None,
                    preview_destination_account_id: None,
                }],
            )
            .map(|_| ()),
        ),
        (
            "transfer decision",
            apply_preview_transfer_decision(
                pool,
                session_id,
                preview_id,
                scoped_user_id,
                ImportPreviewDecision::Accept,
                None,
            )
            .map(|_| ()),
        ),
        (
            "recurring decision",
            update_preview_recurring_match_decision(
                pool,
                session_id,
                preview_id,
                scoped_user_id,
                &ImportPreviewRecurringMatchUpdate {
                    recurring_id: None,
                    candidate_count: 0,
                    target_candidate: None,
                },
                None,
            )
            .map(|_| ()),
        ),
    ] {
        let error = result.expect_err(name);
        assert!(error.to_string().contains("terminal"), "{name}: {error}");
    }

    let after: Value = sqlx::query_scalar(
        "SELECT to_jsonb(preview) - 'updated_at' FROM import_preview_rows preview WHERE id = $1",
    )
    .bind(preview_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(
        after, before,
        "terminal mutation attempts changed preview data"
    );
    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_selection_writer_before_confirm_uses_parent_lock_order(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("selection_before_confirm_lock_order").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "selection-before-confirm-lock-order").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "选择先行账户").await?;
    let category_id =
        insert_category(pool, user_id, "选择先行分类", "expense", "餐饮/选择").await?;
    let session_id = "selection-before-confirm-lock-order-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-07 11:00:00",
            "支出",
            -400,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let advisory_key = 7_004_021_i64;
    sqlx::query(
        "CREATE FUNCTION p2_block_selection_writer() RETURNS trigger AS $$ BEGIN PERFORM pg_advisory_xact_lock(7004021); RETURN NEW; END; $$ LANGUAGE plpgsql",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TRIGGER p2_block_selection_writer_trigger BEFORE UPDATE OF selected ON import_preview_rows FOR EACH ROW EXECUTE FUNCTION p2_block_selection_writer()",
    )
    .execute(pool)
    .await?;
    let mut advisory_holder = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(advisory_key)
        .execute(&mut *advisory_holder)
        .await?;

    let writer_pool = pool.clone();
    let writer = tokio::task::spawn_blocking(move || {
        update_session_preview_selection_by_query(
            &writer_pool,
            session_id,
            scoped_user_id,
            ImportPreviewSelectionMode::Deselect,
            ImportPreviewSelectionTarget::All,
            &ImportPreviewPageRequest::default(),
        )
    });
    wait_for_postgres_lock(pool, "advisory").await?;
    let confirm_pool = pool.clone();
    let confirm = tokio::task::spawn_blocking(move || {
        confirm_preview_to_bills(&confirm_pool, session_id, scoped_user_id)
    });
    wait_for_postgres_lock(pool, "transactionid").await?;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(advisory_key)
        .execute(&mut *advisory_holder)
        .await?;

    assert_eq!(writer.await??, 1);
    assert_eq!(confirm.await??.confirmed_count, 0);
    sqlx::query("DROP TRIGGER p2_block_selection_writer_trigger ON import_preview_rows")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_block_selection_writer()")
        .execute(pool)
        .await?;
    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirm_before_selection_writer_rechecks_terminal_state(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_before_selection_lock_order").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-before-selection-lock-order").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "确认先行选择账户").await?;
    let category_id =
        insert_category(pool, user_id, "确认先行选择分类", "expense", "餐饮/确认").await?;
    let session_id = "confirm-before-selection-lock-order-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-07 12:00:00",
            "支出",
            -500,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let advisory_key = 7_004_022_i64;
    sqlx::query(
        "CREATE FUNCTION p2_block_selection_confirm() RETURNS trigger AS $$ BEGIN PERFORM pg_advisory_xact_lock(7004022); RETURN NEW; END; $$ LANGUAGE plpgsql",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE TRIGGER p2_block_selection_confirm_trigger BEFORE INSERT ON bills FOR EACH ROW EXECUTE FUNCTION p2_block_selection_confirm()",
    )
    .execute(pool)
    .await?;
    let mut advisory_holder = pool.acquire().await?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(advisory_key)
        .execute(&mut *advisory_holder)
        .await?;

    let confirm_pool = pool.clone();
    let confirm = tokio::task::spawn_blocking(move || {
        confirm_preview_to_bills(&confirm_pool, session_id, scoped_user_id)
    });
    wait_for_postgres_lock(pool, "advisory").await?;
    let writer_pool = pool.clone();
    let writer = tokio::task::spawn_blocking(move || {
        reset_session_preview_selection(&writer_pool, session_id, scoped_user_id)
    });
    wait_for_postgres_lock(pool, "transactionid").await?;
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(advisory_key)
        .execute(&mut *advisory_holder)
        .await?;

    assert_eq!(confirm.await??.confirmed_count, 1);
    let writer_error = writer
        .await?
        .expect_err("waiting selection writer must reject terminal");
    assert!(writer_error.to_string().contains("terminal"));
    sqlx::query("DROP TRIGGER p2_block_selection_confirm_trigger ON bills")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_block_selection_confirm()")
        .execute(pool)
        .await?;
    test_db.cleanup().await?;
    Ok(())
}

async fn wait_for_postgres_lock(
    pool: &PostgresPool,
    lock_type: &str,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..200 {
        let waiting: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM pg_locks locks
            JOIN pg_stat_activity activity ON activity.pid = locks.pid
            WHERE activity.datname = current_database()
              AND locks.locktype = $1
              AND locks.granted = false
            "#,
        )
        .bind(lock_type)
        .fetch_one(pool)
        .await?;
        if waiting > 0 {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    Err(format!("timed out waiting for PostgreSQL {lock_type} lock").into())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirmed_receipt_survives_complete_generic_path_inventory(
) -> Result<(), Box<dyn Error>> {
    #[derive(Clone, Copy)]
    enum GenericPath {
        SessionRestage,
        StatusUpdate,
        SessionCancel,
        ParserRestage,
        ParserMarkProcessed,
        ParserStatusUpdate,
        ParserInsert,
        ParserBatchInsert,
        PreviewInsert,
        PreviewBatchInsert,
        PreviewMaterializationClear,
        HistoryMaterializationUpsert,
        DecisionGroupUpsert,
    }

    let test_db = strict_isolated_postgres_database("confirmed_receipt_generic_inventory").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirmed-receipt-generic-inventory").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "通用守卫钱包").await?;
    let category_id =
        insert_category(pool, user_id, "通用守卫分类", "expense", "餐饮/守卫").await?;
    let history_bill_id = insert_history_bill(pool, user_id, "generic-guard-history").await?;
    let session_id = "confirmed-receipt-generic-inventory-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-06 09:00:00",
            "支出",
            -4000,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;
    let forged_terminal = update_import_session_status(
        pool,
        &ImportSessionStatusUpdate {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            status: "confirmed".to_string(),
            total_parsed: None,
            total_preview: None,
            total_confirmed: Some(1),
        },
    )
    .expect_err("generic status update must not forge confirmed without a receipt");
    assert!(forged_terminal.to_string().contains("canonical confirm"));
    let (pre_confirm_status, pre_confirm_metadata, _) =
        session_receipt_state(pool, user_id, session_id).await?;
    assert_ne!(pre_confirm_status, "confirmed");
    assert!(pre_confirm_metadata.get("confirm_receipt").is_none());
    confirm_preview_to_bills(pool, session_id, scoped_user_id)?;
    let (_, expected_metadata, _) = session_receipt_state(pool, user_id, session_id).await?;
    let parser_draft = ImportParserTemplateDraft {
        parser_date: "2026-07-06 10:00:00".to_string(),
        parser_amount: -1.0,
        parser_type: "支出".to_string(),
        parser_description: "confirmed parser mutation".to_string(),
        parser_id: "guard-parser".to_string(),
        ..ImportParserTemplateDraft::default()
    };
    let preview = preview_draft(
        "2026-07-06 10:00:00",
        "支出",
        -100,
        Some(category_id),
        Some(account_id),
        None,
        true,
    );
    let paths = [
        GenericPath::SessionRestage,
        GenericPath::StatusUpdate,
        GenericPath::SessionCancel,
        GenericPath::ParserRestage,
        GenericPath::ParserMarkProcessed,
        GenericPath::ParserStatusUpdate,
        GenericPath::ParserInsert,
        GenericPath::ParserBatchInsert,
        GenericPath::PreviewInsert,
        GenericPath::PreviewBatchInsert,
        GenericPath::PreviewMaterializationClear,
        GenericPath::HistoryMaterializationUpsert,
        GenericPath::DecisionGroupUpsert,
    ];
    let mut executed = Vec::new();
    for path in paths {
        let name = match path {
            GenericPath::SessionRestage => {
                assert!(create_import_session(
                    pool,
                    &ImportSessionDraft {
                        session_id: session_id.to_string(),
                        user_id: scoped_user_id,
                        file_count: 2,
                    },
                )
                .is_err());
                "session_lifecycle.create_import_session"
            }
            GenericPath::StatusUpdate => {
                assert!(!update_import_session_status(
                    pool,
                    &ImportSessionStatusUpdate {
                        session_id: session_id.to_string(),
                        user_id: scoped_user_id,
                        status: "preview_ready".to_string(),
                        total_parsed: Some(10),
                        total_preview: Some(10),
                        total_confirmed: None,
                    },
                )?);
                "session_lifecycle.update_import_session_status"
            }
            GenericPath::SessionCancel => {
                assert_eq!(
                    clear_session_data(pool, session_id, scoped_user_id)?.session_count,
                    0
                );
                "session_lifecycle.clear_session_data"
            }
            GenericPath::ParserRestage => {
                assert!(stage_import_parser_templates_with_sources(
                    pool,
                    &ImportSessionDraft {
                        session_id: session_id.to_string(),
                        user_id: scoped_user_id,
                        file_count: 1,
                    },
                    std::slice::from_ref(&parser_draft),
                    &[],
                    &[],
                    true,
                )
                .is_err());
                "parser_templates.stage_import_parser_templates_with_sources"
            }
            GenericPath::ParserMarkProcessed => {
                assert!(mark_unprocessed_parser_templates_processed_for_session(
                    pool,
                    session_id,
                    scoped_user_id,
                )
                .is_err());
                "parser_templates.mark_unprocessed_parser_templates_processed_for_session"
            }
            GenericPath::ParserStatusUpdate => {
                assert!(!update_parser_template_status(
                    pool,
                    i64::MAX,
                    scoped_user_id,
                    true,
                )?);
                "parser_templates.update_parser_template_status"
            }
            GenericPath::ParserInsert => {
                assert!(
                    insert_parser_template(pool, session_id, scoped_user_id, &parser_draft,)
                        .is_err()
                );
                "parser_templates.insert_parser_template"
            }
            GenericPath::ParserBatchInsert => {
                assert!(insert_parser_templates_batch(
                    pool,
                    session_id,
                    scoped_user_id,
                    std::slice::from_ref(&parser_draft),
                )
                .is_err());
                "parser_templates.insert_parser_templates_batch"
            }
            GenericPath::PreviewInsert => {
                assert!(insert_preview_bill(pool, session_id, scoped_user_id, &preview).is_err());
                "preview_write.insert_preview_bill"
            }
            GenericPath::PreviewBatchInsert => {
                assert!(insert_preview_bills_batch(
                    pool,
                    session_id,
                    scoped_user_id,
                    std::slice::from_ref(&preview),
                )
                .is_err());
                "preview_write.insert_preview_bills_batch"
            }
            GenericPath::PreviewMaterializationClear => {
                assert!(clear_import_preview_materialization_state(
                    pool,
                    session_id,
                    scoped_user_id,
                )
                .is_err());
                "materialization_ledger.clear_import_preview_materialization_state"
            }
            GenericPath::HistoryMaterializationUpsert => {
                assert!(insert_import_history_materializations_batch(
                    pool,
                    session_id,
                    scoped_user_id,
                    &[ImportHistoryMaterializationDraft {
                        history_bill_id,
                        history_bill_version: 1,
                        materialized_payload: json!({"planned_operation": "update_history"}),
                        rewrite_reason: "confirmed guard".to_string(),
                    }],
                )
                .is_err());
                "materialization_ledger.insert_import_history_materializations_batch"
            }
            GenericPath::DecisionGroupUpsert => {
                assert!(insert_import_decision_groups_batch(
                    pool,
                    session_id,
                    scoped_user_id,
                    &[ImportDecisionGroupDraft {
                        group_type: "confirmed_guard".to_string(),
                        group_key: "confirmed_guard".to_string(),
                        decision_status: "pending".to_string(),
                        base_preview_row_id: None,
                        signal_payload: json!({}),
                        members: Vec::new(),
                    }],
                )
                .is_err());
                "materialization_ledger.insert_import_decision_groups_batch"
            }
        };
        executed.push(name);
        let (status, metadata, _) = session_receipt_state(pool, user_id, session_id).await?;
        assert_eq!(status, "confirmed", "{name} changed terminal status");
        assert_eq!(
            metadata, expected_metadata,
            "{name} changed terminal receipt"
        );
        assert_eq!(
            confirm_preview_to_bills(pool, session_id, scoped_user_id)?.confirmed_count,
            1,
            "{name} made same-command replay unavailable"
        );
    }
    assert_eq!(executed.len(), 13);

    let erased = clear_user_import_staging_data(pool, user_id)?;
    assert!(erased >= 1);
    assert!(get_import_session(pool, session_id, scoped_user_id)?.is_none());

    test_db.cleanup().await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmFailureStage {
    Lock,
    Patch,
    Selection,
    Validation,
    HistoryCas,
    BillCreation,
    NoEffectBoundary,
    ReceiptWrite,
    ChildCleanup,
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_confirm_failure_stage_matrix_restores_full_state_and_retries(
) -> Result<(), Box<dyn Error>> {
    let stages = [
        ConfirmFailureStage::Lock,
        ConfirmFailureStage::Patch,
        ConfirmFailureStage::Selection,
        ConfirmFailureStage::Validation,
        ConfirmFailureStage::HistoryCas,
        ConfirmFailureStage::BillCreation,
        ConfirmFailureStage::NoEffectBoundary,
        ConfirmFailureStage::ReceiptWrite,
        ConfirmFailureStage::ChildCleanup,
    ];
    let test_db = strict_isolated_postgres_database("confirm_failure_stage_matrix").await?;
    let pool = &test_db.pool;

    for (index, stage) in stages.into_iter().enumerate() {
        let stage_name = confirm_failure_stage_name(stage);
        let user_id = insert_user(pool, &format!("confirm-failure-{stage_name}-{index}")).await?;
        let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
        let account_id = insert_account(pool, user_id, &format!("回滚钱包-{stage_name}")).await?;
        let category_id = insert_category(
            pool,
            user_id,
            &format!("回滚分类-{stage_name}"),
            "expense",
            &format!("餐饮/{stage_name}"),
        )
        .await?;
        let session_id = format!("confirm-failure-{stage_name}-{index}-session");
        create_import_session(
            pool,
            &ImportSessionDraft {
                session_id: session_id.clone(),
                user_id: scoped_user_id,
                file_count: 1,
            },
        )?;

        let mut history_acknowledgement = None;
        let valid_preview_id = if stage == ConfirmFailureStage::HistoryCas {
            let history_bill_id =
                insert_history_bill(pool, user_id, &format!("history-{stage_name}")).await?;
            let group_key = format!("history-group-{stage_name}");
            let operation = ImportHistoryRewriteOperation::UpdateHistory;
            let operation_id = build_import_history_rewrite_operation_id(
                operation,
                history_bill_id,
                1,
                &group_key,
            );
            let acknowledgement_token = build_import_history_rewrite_ack_token(
                &session_id,
                &operation_id,
                operation,
                history_bill_id,
                1,
            );
            let mut draft = preview_draft(
                "2026-07-08 08:00:00",
                "支出",
                -8_888,
                Some(category_id),
                Some(account_id),
                None,
                true,
            );
            draft.preview_description = "history rollback target".to_string();
            draft.preview_matching_feedback = json!({
                "reconciliation": {
                    "planned_operation": "update_history",
                    "history_bill_id": history_bill_id,
                    "history_bill_version": 1,
                    "group_key": group_key,
                    "operation_id": operation_id,
                    "acknowledgement_token": acknowledgement_token,
                    "review_status": "pending"
                }
            });
            let preview_id = insert_preview_bill(pool, &session_id, scoped_user_id, &draft)?;
            insert_import_history_materializations_batch(
                pool,
                &session_id,
                scoped_user_id,
                &[ImportHistoryMaterializationDraft {
                    history_bill_id,
                    history_bill_version: 1,
                    materialized_payload: json!({
                        "planned_operation": "update_history",
                        "operation_id": operation_id
                    }),
                    rewrite_reason: "failure matrix history CAS".to_string(),
                }],
            )?;
            history_acknowledgement = Some(ImportHistoryRewriteAcknowledgement {
                acknowledged: true,
                selected_preview_ids: vec![preview_id],
                operations: vec![ImportHistoryRewriteAcknowledgementOperation {
                    preview_id,
                    operation_id,
                    planned_operation: "update_history".to_string(),
                    history_bill_id,
                    history_bill_version: 1,
                    acknowledgement_token,
                }],
                selection_scope: json!({"mode": "selected"}),
            });
            preview_id
        } else {
            insert_preview_bill(
                pool,
                &session_id,
                scoped_user_id,
                &preview_draft(
                    "2026-07-08 08:00:00",
                    "支出",
                    -5_000,
                    Some(category_id),
                    Some(account_id),
                    None,
                    true,
                ),
            )?
        };
        let selection_target_preview_id = if stage == ConfirmFailureStage::Selection {
            Some(insert_preview_bill(
                pool,
                &session_id,
                scoped_user_id,
                &preview_draft(
                    "2026-07-08 09:00:00",
                    "支出",
                    -6_000,
                    Some(category_id),
                    Some(account_id),
                    None,
                    false,
                ),
            )?)
        } else {
            None
        };
        let (_, _, session_version) = session_receipt_state(pool, user_id, &session_id).await?;
        let base_command = ConfirmCommand {
            session_id: session_id.clone(),
            expected_session_version: Some(session_version),
            preview_patches: Vec::new(),
            selected_preview_ids: None,
            preserve_unpatched_selection: false,
            history_acknowledgement: history_acknowledgement.clone(),
            declared_confirm_time_effects: Vec::new(),
        };
        let mut failing_command = base_command.clone();
        let mut retry_command = base_command;
        let expected_error = match stage {
            ConfirmFailureStage::Lock => {
                failing_command.expected_session_version = Some(session_version - 1);
                "version conflict"
            }
            ConfirmFailureStage::Patch => {
                let patch = ImportPreviewPatch {
                    preview_id: valid_preview_id,
                    changes: vec![(
                        ImportPreviewPatchField::Description,
                        ImportPreviewPatchValue::Text("patched before rollback".to_string()),
                    )],
                    clear_transfer_decision: false,
                    clear_learning_decision: false,
                    clear_llm_decision: false,
                };
                failing_command.preview_patches = vec![patch.clone()];
                retry_command.preview_patches = vec![patch];
                "patch failure injection"
            }
            ConfirmFailureStage::Selection => {
                failing_command.selected_preview_ids = Some(vec![
                    selection_target_preview_id.expect("selection target preview")
                ]);
                retry_command.selected_preview_ids = failing_command.selected_preview_ids.clone();
                "selection failure injection"
            }
            ConfirmFailureStage::Validation => {
                failing_command.history_acknowledgement =
                    Some(ImportHistoryRewriteAcknowledgement {
                        acknowledged: true,
                        selected_preview_ids: vec![valid_preview_id],
                        operations: vec![ImportHistoryRewriteAcknowledgementOperation {
                            preview_id: valid_preview_id,
                            operation_id: "invalid-operation".to_string(),
                            planned_operation: "update_history".to_string(),
                            history_bill_id: i64::MAX,
                            history_bill_version: 1,
                            acknowledgement_token: "not-persisted".to_string(),
                        }],
                        selection_scope: json!({"mode": "selected"}),
                    });
                "invalid history acknowledgement operation set"
            }
            ConfirmFailureStage::HistoryCas => "history CAS failure injection",
            ConfirmFailureStage::BillCreation => "bill creation failure injection",
            ConfirmFailureStage::NoEffectBoundary => "no-effect boundary failure injection",
            ConfirmFailureStage::ReceiptWrite => "receipt write failure injection",
            ConfirmFailureStage::ChildCleanup => "child cleanup failure injection",
        };

        install_confirm_failure_stage(pool, stage).await?;
        let before = confirm_rollback_snapshot(pool, user_id, &session_id).await?;
        let failure = confirm_import_command(pool, scoped_user_id, &failing_command)
            .expect_err("configured confirm stage must fail");
        assert!(
            failure.to_string().contains(expected_error),
            "{stage_name} failed at unexpected boundary: {failure}"
        );
        uninstall_confirm_failure_stage(pool, stage).await?;
        let after = confirm_rollback_snapshot(pool, user_id, &session_id).await?;
        assert_eq!(after, before, "{stage_name} left partial transaction state");

        let first = confirm_import_command(pool, scoped_user_id, &retry_command)?;
        assert!(!first.replayed, "{stage_name} retry was not first success");
        let replay = confirm_import_command(pool, scoped_user_id, &retry_command)?;
        assert!(replay.replayed, "{stage_name} receipt did not replay");
        assert_eq!(replay.success_envelope, first.success_envelope);
        let (status, metadata, _) = session_receipt_state(pool, user_id, &session_id).await?;
        assert_eq!(status, "confirmed", "{stage_name} retry did not confirm");
        assert!(metadata.get("confirm_receipt").is_some());
        assert_eq!(count_session_children(pool, user_id, &session_id).await?, 0);
    }

    test_db.cleanup().await?;
    Ok(())
}

fn confirm_failure_stage_name(stage: ConfirmFailureStage) -> &'static str {
    match stage {
        ConfirmFailureStage::Lock => "lock",
        ConfirmFailureStage::Patch => "patch",
        ConfirmFailureStage::Selection => "selection",
        ConfirmFailureStage::Validation => "validation",
        ConfirmFailureStage::HistoryCas => "history_cas",
        ConfirmFailureStage::BillCreation => "bill_creation",
        ConfirmFailureStage::NoEffectBoundary => "no_effect_boundary",
        ConfirmFailureStage::ReceiptWrite => "receipt_write",
        ConfirmFailureStage::ChildCleanup => "child_cleanup",
    }
}

async fn install_confirm_failure_stage(
    pool: &PostgresPool,
    stage: ConfirmFailureStage,
) -> Result<(), Box<dyn Error>> {
    let definition = match stage {
        ConfirmFailureStage::Patch => Some((
            r#"CREATE FUNCTION p2_fail_patch_stage() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'patch failure injection'; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE TRIGGER p2_fail_patch_stage_trigger AFTER UPDATE OF description ON import_preview_rows FOR EACH ROW WHEN (OLD.description IS DISTINCT FROM NEW.description) EXECUTE FUNCTION p2_fail_patch_stage()"#,
        )),
        ConfirmFailureStage::Selection => Some((
            r#"CREATE FUNCTION p2_fail_selection_stage() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'selection failure injection'; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE TRIGGER p2_fail_selection_stage_trigger AFTER UPDATE OF selected ON import_preview_rows FOR EACH ROW WHEN (OLD.selected IS DISTINCT FROM NEW.selected) EXECUTE FUNCTION p2_fail_selection_stage()"#,
        )),
        ConfirmFailureStage::HistoryCas => Some((
            r#"CREATE FUNCTION p2_fail_history_cas() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'history CAS failure injection'; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE TRIGGER p2_fail_history_cas_trigger AFTER UPDATE ON accounts FOR EACH ROW EXECUTE FUNCTION p2_fail_history_cas()"#,
        )),
        ConfirmFailureStage::BillCreation => Some((
            r#"CREATE FUNCTION p2_fail_bill_creation() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'bill creation failure injection'; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE TRIGGER p2_fail_bill_creation_trigger AFTER INSERT ON bills FOR EACH ROW EXECUTE FUNCTION p2_fail_bill_creation()"#,
        )),
        ConfirmFailureStage::NoEffectBoundary => Some((
            r#"CREATE FUNCTION p2_fail_no_effect_boundary() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'no-effect boundary failure injection'; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE CONSTRAINT TRIGGER p2_fail_no_effect_boundary_trigger AFTER INSERT ON bills DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION p2_fail_no_effect_boundary()"#,
        )),
        ConfirmFailureStage::ReceiptWrite => Some((
            r#"CREATE FUNCTION p2_fail_receipt_write_matrix() RETURNS trigger AS $$ BEGIN IF NEW.status = 'confirmed' THEN RAISE EXCEPTION 'receipt write failure injection'; END IF; RETURN NEW; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE TRIGGER p2_fail_receipt_write_matrix_trigger BEFORE UPDATE ON import_sessions FOR EACH ROW EXECUTE FUNCTION p2_fail_receipt_write_matrix()"#,
        )),
        ConfirmFailureStage::ChildCleanup => Some((
            r#"CREATE FUNCTION p2_fail_child_cleanup_matrix() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'child cleanup failure injection'; END; $$ LANGUAGE plpgsql"#,
            r#"CREATE TRIGGER p2_fail_child_cleanup_matrix_trigger BEFORE DELETE ON import_preview_rows FOR EACH ROW EXECUTE FUNCTION p2_fail_child_cleanup_matrix()"#,
        )),
        _ => None,
    };
    if let Some((function, trigger)) = definition {
        sqlx::query(function).execute(pool).await?;
        sqlx::query(trigger).execute(pool).await?;
    }
    Ok(())
}

async fn uninstall_confirm_failure_stage(
    pool: &PostgresPool,
    stage: ConfirmFailureStage,
) -> Result<(), Box<dyn Error>> {
    let definition = match stage {
        ConfirmFailureStage::Patch => Some((
            "DROP TRIGGER p2_fail_patch_stage_trigger ON import_preview_rows",
            "DROP FUNCTION p2_fail_patch_stage()",
        )),
        ConfirmFailureStage::Selection => Some((
            "DROP TRIGGER p2_fail_selection_stage_trigger ON import_preview_rows",
            "DROP FUNCTION p2_fail_selection_stage()",
        )),
        ConfirmFailureStage::HistoryCas => Some((
            "DROP TRIGGER p2_fail_history_cas_trigger ON accounts",
            "DROP FUNCTION p2_fail_history_cas()",
        )),
        ConfirmFailureStage::BillCreation => Some((
            "DROP TRIGGER p2_fail_bill_creation_trigger ON bills",
            "DROP FUNCTION p2_fail_bill_creation()",
        )),
        ConfirmFailureStage::NoEffectBoundary => Some((
            "DROP TRIGGER p2_fail_no_effect_boundary_trigger ON bills",
            "DROP FUNCTION p2_fail_no_effect_boundary()",
        )),
        ConfirmFailureStage::ReceiptWrite => Some((
            "DROP TRIGGER p2_fail_receipt_write_matrix_trigger ON import_sessions",
            "DROP FUNCTION p2_fail_receipt_write_matrix()",
        )),
        ConfirmFailureStage::ChildCleanup => Some((
            "DROP TRIGGER p2_fail_child_cleanup_matrix_trigger ON import_preview_rows",
            "DROP FUNCTION p2_fail_child_cleanup_matrix()",
        )),
        _ => None,
    };
    if let Some((trigger, function)) = definition {
        sqlx::query(trigger).execute(pool).await?;
        sqlx::query(function).execute(pool).await?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_receipt_and_child_cleanup_failures_roll_back_and_remain_retryable(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("confirm_receipt_cleanup_rollback").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "confirm-receipt-cleanup-rollback").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "回滚钱包").await?;
    let category_id = insert_category(pool, user_id, "回滚分类", "expense", "餐饮/回滚").await?;
    let session_id = "confirm-receipt-cleanup-rollback-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user_id,
        &preview_draft(
            "2026-07-07 09:00:00",
            "支出",
            -5000,
            Some(category_id),
            Some(account_id),
            None,
            true,
        ),
    )?;

    sqlx::query(
        r#"
        CREATE FUNCTION p2_fail_confirm_receipt_write() RETURNS trigger AS $$
        BEGIN
            IF NEW.status = 'confirmed' THEN
                RAISE EXCEPTION 'p2 receipt write failure injection';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TRIGGER p2_fail_confirm_receipt_write_trigger
        BEFORE UPDATE ON import_sessions
        FOR EACH ROW EXECUTE FUNCTION p2_fail_confirm_receipt_write()
        "#,
    )
    .execute(pool)
    .await?;
    let receipt_error = confirm_preview_to_bills(pool, session_id, scoped_user_id)
        .expect_err("receipt write trigger must abort confirm");
    assert!(receipt_error
        .to_string()
        .contains("receipt write failure injection"));
    sqlx::query("DROP TRIGGER p2_fail_confirm_receipt_write_trigger ON import_sessions")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_fail_confirm_receipt_write()")
        .execute(pool)
        .await?;
    assert_failed_confirm_retryable(pool, user_id, session_id, 1, 0).await?;

    sqlx::query(
        r#"
        CREATE FUNCTION p2_fail_confirm_child_cleanup() RETURNS trigger AS $$
        BEGIN
            RAISE EXCEPTION 'p2 child cleanup failure injection';
        END;
        $$ LANGUAGE plpgsql;
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        r#"
        CREATE TRIGGER p2_fail_confirm_child_cleanup_trigger
        BEFORE DELETE ON import_preview_rows
        FOR EACH ROW EXECUTE FUNCTION p2_fail_confirm_child_cleanup()
        "#,
    )
    .execute(pool)
    .await?;
    let cleanup_error = confirm_preview_to_bills(pool, session_id, scoped_user_id)
        .expect_err("child cleanup trigger must abort confirm");
    assert!(cleanup_error
        .to_string()
        .contains("child cleanup failure injection"));
    sqlx::query("DROP TRIGGER p2_fail_confirm_child_cleanup_trigger ON import_preview_rows")
        .execute(pool)
        .await?;
    sqlx::query("DROP FUNCTION p2_fail_confirm_child_cleanup()")
        .execute(pool)
        .await?;
    assert_failed_confirm_retryable(pool, user_id, session_id, 1, 0).await?;

    let success = confirm_preview_to_bills(pool, session_id, scoped_user_id)?;
    assert_eq!(success.confirmed_count, 1);
    assert_eq!(count_user_bills(pool, user_id).await?, 1);
    assert_eq!(count_session_children(pool, user_id, session_id).await?, 0);

    test_db.cleanup().await?;
    Ok(())
}

async fn assert_failed_confirm_retryable(
    pool: &PostgresPool,
    user_id: i64,
    session_id: &str,
    expected_preview_rows: i64,
    expected_bill_rows: i64,
) -> Result<(), Box<dyn Error>> {
    let (status, metadata, _) = session_receipt_state(pool, user_id, session_id).await?;
    assert_ne!(status, "confirmed");
    assert!(metadata.get("confirm_receipt").is_none());
    let session_db_id: i64 = sqlx::query_scalar(
        "SELECT id FROM import_sessions WHERE user_id = $1 AND session_key = $2",
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    let preview_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE user_id = $1 AND session_id = $2",
    )
    .bind(user_id)
    .bind(session_db_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(preview_rows, expected_preview_rows);
    assert_eq!(count_user_bills(pool, user_id).await?, expected_bill_rows);
    Ok(())
}

async fn confirm_rollback_snapshot(
    pool: &PostgresPool,
    user_id: i64,
    session_id: &str,
) -> Result<Value, Box<dyn Error>> {
    Ok(sqlx::query_scalar(
        r#"
        WITH target_session AS (
            SELECT * FROM import_sessions WHERE user_id = $1 AND session_key = $2
        )
        SELECT jsonb_build_object(
            'session', (
                SELECT to_jsonb(session) - 'created_at' - 'updated_at'
                FROM target_session session
            ),
            'sources', (
                SELECT COALESCE(jsonb_agg(to_jsonb(source) - 'created_at' - 'updated_at' ORDER BY source.id), '[]'::jsonb)
                FROM import_sources source JOIN target_session session ON session.id = source.session_id
                WHERE source.user_id = $1
            ),
            'standard_rows', (
                SELECT COALESCE(jsonb_agg(to_jsonb(row) - 'created_at' - 'updated_at' ORDER BY row.id), '[]'::jsonb)
                FROM import_standard_rows row JOIN target_session session ON session.id = row.session_id
                WHERE row.user_id = $1
            ),
            'previews', (
                SELECT COALESCE(jsonb_agg(to_jsonb(preview) - 'created_at' - 'updated_at' ORDER BY preview.id), '[]'::jsonb)
                FROM import_preview_rows preview JOIN target_session session ON session.id = preview.session_id
                WHERE preview.user_id = $1
            ),
            'history_materializations', (
                SELECT COALESCE(jsonb_agg(to_jsonb(history) - 'created_at' ORDER BY history.id), '[]'::jsonb)
                FROM import_history_materializations history JOIN target_session session ON session.id = history.session_id
                WHERE history.user_id = $1
            ),
            'decision_groups', (
                SELECT COALESCE(jsonb_agg(to_jsonb(decision_group) - 'created_at' - 'updated_at' ORDER BY decision_group.id), '[]'::jsonb)
                FROM import_decision_groups decision_group JOIN target_session session ON session.id = decision_group.session_id
                WHERE decision_group.user_id = $1
            ),
            'decision_members', (
                SELECT COALESCE(jsonb_agg(to_jsonb(member) - 'created_at' ORDER BY member.id), '[]'::jsonb)
                FROM import_decision_group_members member
                JOIN import_decision_groups decision_group ON decision_group.id = member.group_id
                JOIN target_session session ON session.id = decision_group.session_id
                WHERE decision_group.user_id = $1
            ),
            'confirm_effect_rows', (
                SELECT COALESCE(jsonb_agg(to_jsonb(operation) - 'created_at' ORDER BY operation.id), '[]'::jsonb)
                FROM import_confirm_operations operation JOIN target_session session ON session.id = operation.session_id
                WHERE operation.user_id = $1
            ),
            'learning_suggestions', (
                SELECT COALESCE(jsonb_agg(to_jsonb(suggestion) - 'created_at' - 'updated_at' ORDER BY suggestion.id), '[]'::jsonb)
                FROM import_learning_suggestions suggestion JOIN target_session session ON session.id = suggestion.session_id
                WHERE suggestion.user_id = $1
            ),
            'matching_feedback', (
                SELECT COALESCE(jsonb_agg(to_jsonb(feedback) - 'created_at' - 'updated_at' ORDER BY feedback.id), '[]'::jsonb)
                FROM preview_matching_feedback feedback JOIN target_session session ON session.id = feedback.session_id
                WHERE feedback.user_id = $1
            ),
            'annotation_samples', (
                SELECT COALESCE(jsonb_agg(to_jsonb(annotation) - 'created_at' - 'updated_at' ORDER BY annotation.id), '[]'::jsonb)
                FROM import_annotation_samples annotation
                WHERE annotation.user_id = $1 AND annotation.session_id = $2
            ),
            'bills', (
                SELECT COALESCE(jsonb_agg(to_jsonb(bill) - 'created_at' - 'updated_at' ORDER BY bill.id), '[]'::jsonb)
                FROM bills bill WHERE bill.user_id = $1
            ),
            'accounts', (
                SELECT COALESCE(jsonb_agg(to_jsonb(account) - 'created_at' - 'updated_at' ORDER BY account.id), '[]'::jsonb)
                FROM accounts account WHERE account.user_id = $1
            )
        )
        "#,
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?)
}

async fn session_receipt_state(
    pool: &PostgresPool,
    user_id: i64,
    session_id: &str,
) -> Result<(String, Value, i64), Box<dyn Error>> {
    let row = sqlx::query(
        "SELECT status, metadata, version FROM import_sessions WHERE user_id = $1 AND session_key = $2",
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?;
    Ok((
        row.try_get("status")?,
        row.try_get("metadata")?,
        row.try_get("version")?,
    ))
}

async fn count_user_bills(pool: &PostgresPool, user_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(pool)
            .await?,
    )
}

async fn count_session_children(
    pool: &PostgresPool,
    user_id: i64,
    session_id: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query_scalar(
        r#"
        SELECT
            (SELECT COUNT(*) FROM import_sources WHERE user_id = $1 AND session_id = s.id)
          + (SELECT COUNT(*) FROM import_standard_rows WHERE user_id = $1 AND session_id = s.id)
          + (SELECT COUNT(*) FROM import_preview_rows WHERE user_id = $1 AND session_id = s.id)
          + (SELECT COUNT(*) FROM import_decision_groups WHERE user_id = $1 AND session_id = s.id)
          + (SELECT COUNT(*) FROM import_history_materializations WHERE user_id = $1 AND session_id = s.id)
          + (SELECT COUNT(*) FROM import_confirm_operations WHERE user_id = $1 AND session_id = s.id)
        FROM import_sessions s
        WHERE s.user_id = $1 AND s.session_key = $2
        "#,
    )
    .bind(user_id)
    .bind(session_id)
    .fetch_one(pool)
    .await?)
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_parser_staging_covers_missing_inline_explicit_and_mutation_paths(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("import_parser_staging_coverage").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-parser-staging-coverage").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let parser_drafts = vec![
        ImportParserTemplateDraft {
            parser_date: "2026-07-10 10:00:00".to_string(),
            parser_amount: -12.34,
            parser_type: "支出".to_string(),
            parser_description: "inline parser first".to_string(),
            parser_id: "wechat".to_string(),
            parser_tags: Some(json!(["parser:wechat", "fixture"])),
            parser_counterparty: "商户甲".to_string(),
            parser_payment_method: "零钱".to_string(),
            parser_original_type: "消费".to_string(),
            parser_original_category: "餐饮".to_string(),
            parser_account_id: "wallet".to_string(),
        },
        ImportParserTemplateDraft {
            parser_date: "2026-07-10 11:00:00".to_string(),
            parser_amount: 56.78,
            parser_type: "收入".to_string(),
            parser_description: "inline parser second".to_string(),
            parser_id: "wechat".to_string(),
            ..ImportParserTemplateDraft::default()
        },
    ];

    let missing_session = stage_import_parser_templates_with_sources(
        pool,
        &ImportSessionDraft {
            session_id: "parser-required-missing".to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
        &parser_drafts,
        &[],
        &[],
        true,
    )?;
    assert!(!missing_session.session_found);
    assert_eq!(missing_session.inserted_count, 0);
    assert_eq!(missing_session.total_parsed, 0);

    let inline_session = "parser-inline-session";
    let inline_result = stage_import_parser_templates(
        pool,
        &ImportSessionDraft {
            session_id: inline_session.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
        &parser_drafts,
        false,
    )?;
    assert!(inline_result.session_found);
    assert_eq!(inline_result.inserted_count, 2);
    assert_eq!(inline_result.total_parsed, 2);
    let unprocessed = get_unprocessed_templates_for_dedup(pool, inline_session, scoped_user_id)?;
    assert_eq!(unprocessed.len(), 2);
    assert_eq!(unprocessed[0].parser_description, "inline parser first");
    assert_eq!(unprocessed[0].parser_tags, vec!["parser:wechat", "fixture"]);
    assert_eq!(unprocessed[1].parser_description, "inline parser second");
    assert_eq!(
        mark_unprocessed_parser_templates_processed_for_session(
            pool,
            inline_session,
            scoped_user_id,
        )?,
        2
    );
    assert!(get_unprocessed_templates_for_dedup(pool, inline_session, scoped_user_id)?.is_empty());
    let all_inline = get_parser_templates_by_session(pool, inline_session, scoped_user_id)?;
    assert_eq!(all_inline.len(), 2);
    assert!(all_inline.iter().all(|row| row.parser_is_processed));
    assert!(update_parser_template_status(
        pool,
        all_inline[0].id,
        scoped_user_id,
        false,
    )?);
    assert_eq!(
        get_unprocessed_templates_for_dedup(pool, inline_session, scoped_user_id)?.len(),
        1
    );
    assert!(update_parser_template_status(
        pool,
        all_inline[0].id,
        scoped_user_id,
        true,
    )?);
    assert!(!update_parser_template_status(
        pool,
        i64::MAX,
        scoped_user_id,
        true,
    )?);

    let explicit_session = "parser-explicit-session";
    let explicit_result = stage_import_parser_templates_with_sources(
        pool,
        &ImportSessionDraft {
            session_id: explicit_session.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
        &parser_drafts,
        &[ImportSourceDraft {
            source_index: 7,
            original_file_name: "explicit.csv".to_string(),
            parser_id: "alipay".to_string(),
            parser_name: "支付宝".to_string(),
            parser_signal: "exact".to_string(),
            parser_confidence: 0.99,
            feature_signature: "explicit-source-signature".to_string(),
            metadata: json!({"fixture": true}),
        }],
        &[ImportStandardRowDraft {
            source_index: 7,
            source_row_index: 9,
            occurred_at: "2026-07-10 12:00:00".to_string(),
            amount_cents: -4321,
            direction: "expense".to_string(),
            transaction_type: "支出".to_string(),
            merchant: "显式商户".to_string(),
            payment_method: "余额宝".to_string(),
            description: "explicit standard row".to_string(),
            parser_payload: json!({"parser_is_processed": false, "source": "explicit"}),
            standard_payload: json!({"description": "explicit standard row"}),
        }],
        false,
    )?;
    assert_eq!(explicit_result.inserted_count, 1);
    assert_eq!(explicit_result.total_parsed, 1);
    let explicit_rows =
        get_unprocessed_templates_for_dedup(pool, explicit_session, scoped_user_id)?;
    assert_eq!(explicit_rows.len(), 1);
    assert_eq!(explicit_rows[0].parser_id, "wechat");
    assert_eq!(explicit_rows[0].parser_description, "inline parser first");

    let mutation_session = "parser-mutation-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: mutation_session.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;
    assert_eq!(
        insert_parser_templates_batch(pool, mutation_session, scoped_user_id, &[])?,
        0
    );
    let single_id =
        insert_parser_template(pool, mutation_session, scoped_user_id, &parser_drafts[0])?;
    assert!(single_id > 0);
    assert_eq!(
        insert_parser_templates_batch(pool, mutation_session, scoped_user_id, &parser_drafts[1..],)?,
        1
    );
    let mutation_rows = get_parser_templates_by_session(pool, mutation_session, scoped_user_id)?;
    assert_eq!(mutation_rows.len(), 1);
    assert_eq!(
        mutation_rows
            .iter()
            .map(|row| row.parser_description.as_str())
            .collect::<Vec<_>>(),
        vec!["inline parser second"]
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_same_batch_transfer_reject_accepts_nullable_standard_row_text(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("transfer_reject_nullable_text").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "transfer-reject-nullable-text").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let category_id =
        insert_category(pool, user_id, "人工转账分类", "transfer", "转账/人工分类").await?;
    let source_account_id = insert_account(pool, user_id, "人工转出账户").await?;
    let destination_account_id = insert_account(pool, user_id, "人工转入账户").await?;
    let session_id = "transfer-reject-nullable-text-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let session_db_id: i64 =
        sqlx::query_scalar("SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2")
            .bind(session_id)
            .bind(user_id)
            .fetch_one(pool)
            .await?;
    let source_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO import_sources
           (session_id,user_id,source_index,parser_id,parser_name,feature_signature)
           VALUES($1,$2,0,'nullable-fixture','nullable-fixture','nullable-fixture')
           RETURNING id"#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let mut standard_row_ids = Vec::new();
    for (source_row_index, direction, amount_cents) in
        [(0, "expense", -5_000_i64), (1, "income", 5_000_i64)]
    {
        standard_row_ids.push(
            sqlx::query_scalar(
                r#"INSERT INTO import_standard_rows
                   (session_id,source_id,user_id,source_row_index,occurred_at,amount_cents,
                    direction,transaction_type,merchant,payment_method,description,parser_payload)
                   VALUES($1,$2,$3,$4,'2026-07-13 09:00:00+08',$5,$6,$6,
                          NULL,NULL,NULL,'{"parser_id":"nullable-fixture"}'::jsonb)
                   RETURNING id"#,
            )
            .bind(session_db_id)
            .bind(source_id)
            .bind(user_id)
            .bind(source_row_index)
            .bind(amount_cents)
            .bind(direction)
            .fetch_one(pool)
            .await?,
        );
    }

    let mut preview = preview_draft(
        "2026-07-13 09:00:00",
        "转账",
        5_000,
        Some(category_id),
        Some(source_account_id),
        Some(destination_account_id),
        true,
    );
    preview.preview_matching_feedback = json!({
        "annotation": {
            "is_manually_annotated": true,
            "manual_fields": {
                "category_id": true,
                "source_account_id": true,
                "destination_account_id": true
            }
        },
        "transfer": {
            "state": "pending",
            "review_status": "pending",
            "owned_fields": {
                "category_id": false,
                "source_account_id": false,
                "destination_account_id": false
            }
        }
    });
    let preview_id = insert_preview_bill(pool, session_id, scoped_user_id, &preview)?;
    insert_import_decision_groups_batch(
        pool,
        session_id,
        scoped_user_id,
        &[ImportDecisionGroupDraft {
            group_type: "same_batch_transfer".to_string(),
            group_key: "nullable-text-pair".to_string(),
            decision_status: "pending".to_string(),
            base_preview_row_id: Some(preview_id),
            signal_payload: json!({"candidate_type": "transfer"}),
            members: vec![
                ImportDecisionGroupMemberDraft {
                    preview_row_id: Some(preview_id),
                    standard_row_id: Some(standard_row_ids[0]),
                    history_bill_id: None,
                    member_role: "outgoing".to_string(),
                    parser_name: "nullable-fixture".to_string(),
                    metadata: json!({}),
                },
                ImportDecisionGroupMemberDraft {
                    preview_row_id: Some(preview_id),
                    standard_row_id: Some(standard_row_ids[1]),
                    history_bill_id: None,
                    member_role: "incoming".to_string(),
                    parser_name: "nullable-fixture".to_string(),
                    metadata: json!({}),
                },
            ],
        }],
    )?;
    let (group_id, group_version): (i64, i64) = sqlx::query_as(
        "SELECT id, version FROM import_decision_groups WHERE session_id=$1 AND group_key=$2",
    )
    .bind(session_db_id)
    .bind("nullable-text-pair")
    .fetch_one(pool)
    .await?;

    let initial_group = get_import_decision_groups_by_session(pool, session_id, scoped_user_id)?
        .into_iter()
        .find(|group| group.id == group_id)
        .expect("user-scoped decision group");
    assert_eq!(initial_group.members.len(), 2);
    assert!(initial_group
        .members
        .iter()
        .all(|member| member.version == 1));

    let result = apply_import_decision_group_command(
        pool,
        scoped_user_id,
        &ImportDecisionGroupCommand {
            operation_id: "reject-nullable-text-pair".to_string(),
            session_id: session_id.to_string(),
            group_id,
            decision: "reject".to_string(),
            expected_group_version: group_version,
            expected_preview_versions: vec![ImportDecisionPreviewVersion {
                preview_row_id: preview_id,
                version: 1,
            }],
        },
    )
    .expect("same-batch reject must accept nullable merchant/payment method/description");
    let ImportDecisionGroupCommandResult::Applied(mutation) = result else {
        panic!("expected applied same-batch rejection, got {result:?}");
    };
    assert_eq!(mutation.removed_preview_ids, vec![preview_id]);
    assert_eq!(mutation.upserted_preview_items.len(), 2);
    for item in &mutation.upserted_preview_items {
        assert_eq!(item.get("preview_counterparty"), Some(&json!("")));
        assert_eq!(item.get("preview_payment_method"), Some(&json!("")));
        assert_eq!(item.get("preview_description"), Some(&json!("")));
    }
    let outgoing = mutation
        .upserted_preview_items
        .iter()
        .find(|item| item["preview_type"] == "支出")
        .expect("outgoing replacement");
    assert_eq!(outgoing["category_id"], category_id);
    assert_eq!(outgoing["preview_source_account_id"], source_account_id);
    assert_eq!(outgoing["preview_destination_account_id"], Value::Null);
    assert_eq!(
        outgoing.pointer("/preview_matching_feedback/annotation/manual_fields"),
        Some(&json!({
            "category_id": true,
            "source_account_id": true,
            "destination_account_id": false
        }))
    );
    let incoming = mutation
        .upserted_preview_items
        .iter()
        .find(|item| item["preview_type"] == "收入")
        .expect("incoming replacement");
    assert_eq!(incoming["category_id"], Value::Null);
    assert_eq!(
        incoming["preview_source_account_id"], destination_account_id,
        "manual transfer destination becomes the incoming row source account"
    );
    assert_eq!(incoming["preview_destination_account_id"], Value::Null);
    assert_eq!(
        incoming.pointer("/preview_matching_feedback/annotation/manual_fields"),
        Some(&json!({
            "category_id": false,
            "source_account_id": true,
            "destination_account_id": false
        }))
    );

    for replacement_id in &mutation.upserted_preview_ids {
        let row = get_preview_bill_by_id(pool, *replacement_id, scoped_user_id)?
            .expect("replacement remains user scoped");
        assert_eq!(row.session_id, session_id);
    }

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_decision_group_rejects_cross_user_preview_and_standard_members(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("decision_group_cross_scope").await?;
    let pool = &test_db.pool;
    let user_a = insert_user(pool, "decision-group-scope-a").await?;
    let user_b = insert_user(pool, "decision-group-scope-b").await?;
    let scoped_a = UserId::new(user_a as u64).expect("positive user id");
    let scoped_b = UserId::new(user_b as u64).expect("positive user id");
    let session_a = "decision-group-scope-a-session";
    let session_b = "decision-group-scope-b-session";
    for (session_id, user_id) in [(session_a, scoped_a), (session_b, scoped_b)] {
        create_import_session(
            pool,
            &ImportSessionDraft {
                session_id: session_id.to_string(),
                user_id,
                file_count: 1,
            },
        )?;
    }
    let session_a_db_id: i64 =
        sqlx::query_scalar("SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2")
            .bind(session_a)
            .bind(user_a)
            .fetch_one(pool)
            .await?;
    let session_b_db_id: i64 =
        sqlx::query_scalar("SELECT id FROM import_sessions WHERE session_key=$1 AND user_id=$2")
            .bind(session_b)
            .bind(user_b)
            .fetch_one(pool)
            .await?;
    let source_b: i64 = sqlx::query_scalar(
        r#"INSERT INTO import_sources
           (session_id,user_id,source_index,parser_id,parser_name,feature_signature)
           VALUES($1,$2,0,'cross-scope','cross-scope','cross-scope') RETURNING id"#,
    )
    .bind(session_b_db_id)
    .bind(user_b)
    .fetch_one(pool)
    .await?;
    let standard_b: i64 = sqlx::query_scalar(
        r#"INSERT INTO import_standard_rows
           (session_id,source_id,user_id,source_row_index,occurred_at,amount_cents,
            direction,transaction_type,merchant,payment_method,description,parser_payload)
           VALUES($1,$2,$3,0,'2026-07-13 09:00:00+08',5000,'expense','expense',
                  'scope-b','scope-b','scope-b','{"parser_id":"cross-scope"}'::jsonb)
           RETURNING id"#,
    )
    .bind(session_b_db_id)
    .bind(source_b)
    .bind(user_b)
    .fetch_one(pool)
    .await?;
    let mut preview_b = preview_draft("2026-07-13 09:00:00", "转账", 5_000, None, None, None, true);
    preview_b.preview_matching_feedback = json!({
        "transfer": {"state": "pending", "review_status": "pending"}
    });
    let preview_b_id = insert_preview_bill(pool, session_b, scoped_b, &preview_b)?;

    let cross_scope_draft = ImportDecisionGroupDraft {
        group_type: "same_batch_transfer".to_string(),
        group_key: "cross-scope-api-boundary".to_string(),
        decision_status: "pending".to_string(),
        base_preview_row_id: Some(preview_b_id),
        signal_payload: json!({"candidate_type": "transfer"}),
        members: vec![ImportDecisionGroupMemberDraft {
            preview_row_id: Some(preview_b_id),
            standard_row_id: Some(standard_b),
            history_bill_id: None,
            member_role: "outgoing".to_string(),
            parser_name: "cross-scope".to_string(),
            metadata: json!({}),
        }],
    };
    assert!(
        insert_import_decision_groups_batch(pool, session_a, scoped_a, &[cross_scope_draft])
            .is_err()
    );
    let boundary_group_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM import_decision_groups WHERE session_id=$1 AND group_key=$2",
    )
    .bind(session_a_db_id)
    .bind("cross-scope-api-boundary")
    .fetch_one(pool)
    .await?;
    assert_eq!(boundary_group_count, 0, "invalid group insert rolls back");

    let corrupted_group_id: i64 = sqlx::query_scalar(
        r#"INSERT INTO import_decision_groups
           (session_id,user_id,group_type,group_key,decision_status,base_preview_row_id,signal_payload)
           VALUES($1,$2,'same_batch_transfer','cross-scope-corruption','pending',$3,'{}'::jsonb)
           RETURNING id"#,
    )
    .bind(session_a_db_id)
    .bind(user_a)
    .bind(preview_b_id)
    .fetch_one(pool)
    .await?;
    sqlx::query(
        r#"INSERT INTO import_decision_group_members
           (group_id,preview_row_id,standard_row_id,member_role,parser_name,metadata)
           VALUES($1,$2,$3,'outgoing','cross-scope','{}'::jsonb)"#,
    )
    .bind(corrupted_group_id)
    .bind(preview_b_id)
    .bind(standard_b)
    .execute(pool)
    .await?;
    let preview_b_before: (i64, Value) = sqlx::query_as(
        "SELECT version, preview_payload FROM import_preview_rows WHERE id=$1 AND user_id=$2",
    )
    .bind(preview_b_id)
    .bind(user_b)
    .fetch_one(pool)
    .await?;
    let standard_b_before: Value = sqlx::query_scalar(
        "SELECT standard_payload FROM import_standard_rows WHERE id=$1 AND user_id=$2",
    )
    .bind(standard_b)
    .bind(user_b)
    .fetch_one(pool)
    .await?;

    let result = apply_import_decision_group_command(
        pool,
        scoped_a,
        &ImportDecisionGroupCommand {
            operation_id: "cross-scope-command".to_string(),
            session_id: session_a.to_string(),
            group_id: corrupted_group_id,
            decision: "accept".to_string(),
            expected_group_version: 1,
            expected_preview_versions: vec![ImportDecisionPreviewVersion {
                preview_row_id: preview_b_id,
                version: preview_b_before.0,
            }],
        },
    )?;
    assert_eq!(result, ImportDecisionGroupCommandResult::Conflict);
    let empty_expected_result = apply_import_decision_group_command(
        pool,
        scoped_a,
        &ImportDecisionGroupCommand {
            operation_id: "cross-scope-command-empty-expected".to_string(),
            session_id: session_a.to_string(),
            group_id: corrupted_group_id,
            decision: "accept".to_string(),
            expected_group_version: 1,
            expected_preview_versions: vec![],
        },
    )?;
    assert_eq!(
        empty_expected_result,
        ImportDecisionGroupCommandResult::Conflict,
        "cross-scope member references must fail closed even when expected previews are empty"
    );
    let preview_b_after: (i64, Value) = sqlx::query_as(
        "SELECT version, preview_payload FROM import_preview_rows WHERE id=$1 AND user_id=$2",
    )
    .bind(preview_b_id)
    .bind(user_b)
    .fetch_one(pool)
    .await?;
    assert_eq!(preview_b_after, preview_b_before);
    let standard_b_after: Value = sqlx::query_scalar(
        "SELECT standard_payload FROM import_standard_rows WHERE id=$1 AND user_id=$2",
    )
    .bind(standard_b)
    .bind(user_b)
    .fetch_one(pool)
    .await?;
    assert_eq!(standard_b_after, standard_b_before);
    let group_after: (String, i64) = sqlx::query_as(
        "SELECT decision_status, version FROM import_decision_groups WHERE id=$1 AND user_id=$2",
    )
    .bind(corrupted_group_id)
    .bind(user_a)
    .fetch_one(pool)
    .await?;
    assert_eq!(group_after, ("pending".to_string(), 1));
    let operation_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM import_confirm_operations WHERE user_id=$1 AND operation_id=ANY($2)",
    )
    .bind(user_a)
    .bind(vec![
        "cross-scope-command".to_string(),
        "cross-scope-command-empty-expected".to_string(),
    ])
    .fetch_one(pool)
    .await?;
    assert_eq!(operation_count, 0);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_transfer_and_recurring_decisions_cover_success_conflict_and_clear_paths(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("import_preview_decision_coverage").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-preview-decision-coverage").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "import-preview-decision-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let mut drafts = Vec::new();
    for description in [
        "transfer accept",
        "transfer reject",
        "transfer clear",
        "transfer stale",
        "recurring candidate",
        "recurring clear",
        "recurring stale",
    ] {
        let mut draft = preview_draft("2026-07-10 13:00:00", "支出", -2468, None, None, None, true);
        draft.preview_description = description.to_string();
        if description.starts_with("transfer") {
            draft.preview_matching_feedback = json!({
                "transfer": {
                    "review_status": "pending",
                    "candidate_type": "transfer",
                    "score": 0.95
                }
            });
        }
        drafts.push(draft);
    }
    assert_eq!(
        insert_preview_bills_batch(pool, session_id, scoped_user_id, &drafts)?,
        drafts.len()
    );
    let page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            page_size: 100,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    let id_for = |description: &str| {
        page.rows
            .iter()
            .find(|row| row.preview_description == description)
            .unwrap_or_else(|| panic!("missing preview row {description}"))
            .id
    };

    let accepted = apply_preview_transfer_decision(
        pool,
        session_id,
        id_for("transfer accept"),
        scoped_user_id,
        ImportPreviewDecision::Accept,
        Some(&ImportPreviewExpectedState {
            session_id: Some(session_id.to_string()),
            preview_type: Some("支出".to_string()),
            preview_main_category: Some(String::new()),
            preview_sub_category: Some(String::new()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    assert!(!accepted.state_conflict);
    assert_eq!(
        accepted
            .preview
            .expect("accepted transfer preview")
            .preview_matching_feedback
            .pointer("/transfer/review_status"),
        Some(&json!("accepted"))
    );
    for (description, decision) in [
        ("transfer reject", ImportPreviewDecision::Reject),
        ("transfer clear", ImportPreviewDecision::Clear),
    ] {
        let result = apply_preview_transfer_decision(
            pool,
            session_id,
            id_for(description),
            scoped_user_id,
            decision,
            None,
        )?;
        assert!(!result.state_conflict);
        assert!(result
            .preview
            .expect("cleared transfer preview")
            .preview_matching_feedback
            .get("transfer")
            .is_none());
    }
    let stale_transfer = apply_preview_transfer_decision(
        pool,
        session_id,
        id_for("transfer stale"),
        scoped_user_id,
        ImportPreviewDecision::Accept,
        Some(&ImportPreviewExpectedState {
            session_id: Some("stale-session".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    assert!(stale_transfer.state_conflict);
    assert!(stale_transfer.preview.is_none());
    assert!(apply_preview_transfer_decision(
        pool,
        session_id,
        i64::MAX,
        scoped_user_id,
        ImportPreviewDecision::Accept,
        None,
    )?
    .preview
    .is_none());

    let recurring_candidate = ImportPreviewRecurringCandidate {
        id: 9001,
        name: "每月会员".to_string(),
        match_score: 0.88,
        match_reasons: vec!["same_amount".to_string(), "monthly".to_string()],
        matched_occurrence_date: "2026-06-10".to_string(),
    };
    let recurring_result = update_preview_recurring_match_decision(
        pool,
        session_id,
        id_for("recurring candidate"),
        scoped_user_id,
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: Some(recurring_candidate.id),
            candidate_count: 2,
            target_candidate: Some(recurring_candidate.clone()),
        },
        None,
    )?;
    assert!(!recurring_result.state_conflict);
    let recurring_preview = recurring_result.preview.expect("recurring preview");
    assert_eq!(recurring_preview.preview_recurring_id, Some(9001));
    assert_eq!(recurring_preview.preview_recurring_name, "每月会员");
    assert_eq!(recurring_preview.preview_recurring_candidate_count, 2);
    assert_eq!(recurring_preview.preview_recurring_match_score, 0.88);
    assert_eq!(
        recurring_preview.preview_recurring_match_reasons,
        "same_amount；monthly"
    );
    assert_eq!(
        recurring_preview.preview_recurring_matched_date,
        "2026-06-10"
    );

    let recurring_clear = update_preview_recurring_match_decision(
        pool,
        session_id,
        id_for("recurring clear"),
        scoped_user_id,
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: None,
            candidate_count: 0,
            target_candidate: None,
        },
        None,
    )?;
    let cleared_preview = recurring_clear.preview.expect("cleared recurring preview");
    assert_eq!(cleared_preview.preview_recurring_id, None);
    assert!(cleared_preview.preview_recurring_name.is_empty());
    assert_eq!(cleared_preview.preview_recurring_candidate_count, 0);
    assert_eq!(cleared_preview.preview_recurring_match_score, 0.0);
    assert!(cleared_preview.preview_recurring_match_reasons.is_empty());
    assert!(cleared_preview.preview_recurring_matched_date.is_empty());

    let stale_recurring = update_preview_recurring_match_decision(
        pool,
        session_id,
        id_for("recurring stale"),
        scoped_user_id,
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: None,
            candidate_count: 0,
            target_candidate: None,
        },
        Some(&ImportPreviewExpectedState {
            preview_type: Some("收入".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    assert!(stale_recurring.state_conflict);
    assert!(stale_recurring.preview.is_none());
    assert!(update_preview_recurring_match_decision(
        pool,
        session_id,
        i64::MAX,
        scoped_user_id,
        &ImportPreviewRecurringMatchUpdate {
            recurring_id: None,
            candidate_count: 0,
            target_candidate: None,
        },
        None,
    )?
    .preview
    .is_none());

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_learning_and_llm_lifecycle_transitions_are_transactional_and_idempotent(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("import_signal_lifecycle_coverage").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "import-signal-lifecycle-coverage").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "生命周期账户").await?;
    let category_id =
        insert_category(pool, user_id, "生命周期分类", "expense", "测试/生命周期").await?;
    let session_id = "import-signal-lifecycle-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let mut drafts = Vec::new();
    for (description, feedback) in [
        (
            "learning accept",
            json!({"learning": {"review_status": "pending", "recommendation_key": "learning:accept"}}),
        ),
        (
            "learning reject",
            json!({"learning": {"status": "pending", "recommendation_key": "learning:reject"}}),
        ),
        (
            "learning clear",
            json!({"learning": {"signal_state": "pending", "recommendation_key": "learning:clear"}}),
        ),
        (
            "learning stale",
            json!({"learning": {"review_status": "pending"}}),
        ),
        ("learning absent", json!({"parser": {"source": "fixture"}})),
        (
            "llm accept",
            json!({"llm": {"review_status": "pending", "confidence": 0.91}}),
        ),
        (
            "llm reject",
            json!({"llm": {"status": "pending", "confidence": 0.82}}),
        ),
        (
            "llm clear",
            json!({"llm": {"signal_state": "pending", "confidence": 0.73}}),
        ),
        ("llm stale", json!({"llm": {"review_status": "pending"}})),
        ("llm absent", json!({"parser": {"source": "fixture"}})),
    ] {
        let mut draft = preview_draft(
            "2026-07-10 09:00:00",
            "支出",
            -1234,
            Some(category_id),
            Some(account_id),
            None,
            true,
        );
        draft.preview_description = description.to_string();
        draft.preview_matching_feedback = feedback;
        drafts.push(draft);
    }
    assert_eq!(
        insert_preview_bills_batch(pool, session_id, scoped_user_id, &drafts)?,
        drafts.len()
    );
    let page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user_id,
        &ImportPreviewPageRequest {
            page_size: 100,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    let id_for = |description: &str| {
        page.rows
            .iter()
            .find(|row| row.preview_description == description)
            .unwrap_or_else(|| panic!("missing preview row {description}"))
            .id
    };

    let learning_apply = ImportPreviewLearningApply {
        preview_type: Some("支出".to_string()),
        preview_main_category: Some("测试".to_string()),
        preview_sub_category: Some("生命周期".to_string()),
        preview_source_account_id: Some(Some(account_id)),
        preview_destination_account_id: Some(None),
        rule_id: None,
    };
    let learning_expected = ImportPreviewExpectedState {
        session_id: Some(session_id.to_string()),
        review_status: Some("pending".to_string()),
        preview_category_id: Some(Some(category_id)),
        preview_source_account_id: Some(Some(account_id)),
        preview_destination_account_id: Some(None),
        ..ImportPreviewExpectedState::default()
    };
    let learning_accept_id = id_for("learning accept");
    let accepted = apply_preview_learning_decision(
        pool,
        session_id,
        learning_accept_id,
        scoped_user_id,
        ImportPreviewDecision::Accept,
        Some(&learning_apply),
        Some(&learning_expected),
    )?;
    assert!(!accepted.state_conflict);
    let accepted_preview = accepted.preview.expect("accepted learning preview");
    assert_eq!(
        accepted_preview
            .preview_matching_feedback
            .pointer("/learning/review_status"),
        Some(&json!("accepted"))
    );
    assert_eq!(accepted_preview.preview_source_account_id, Some(account_id));
    assert_eq!(accepted_preview.preview_destination_account_id, None);

    let accepted_retry = apply_preview_learning_decision(
        pool,
        session_id,
        learning_accept_id,
        scoped_user_id,
        ImportPreviewDecision::Accept,
        Some(&learning_apply),
        None,
    )?;
    assert!(!accepted_retry.state_conflict);
    assert!(accepted_retry.preview.is_some());
    let accepted_opposite = apply_preview_learning_decision(
        pool,
        session_id,
        learning_accept_id,
        scoped_user_id,
        ImportPreviewDecision::Reject,
        None,
        None,
    )?;
    assert!(accepted_opposite.state_conflict);
    assert!(accepted_opposite.preview.is_none());

    let rejected = apply_preview_learning_decision(
        pool,
        session_id,
        id_for("learning reject"),
        scoped_user_id,
        ImportPreviewDecision::Reject,
        None,
        None,
    )?;
    assert_eq!(
        rejected
            .preview
            .expect("rejected learning preview")
            .preview_matching_feedback
            .pointer("/learning/review_status"),
        Some(&json!("rejected"))
    );
    let cleared = apply_preview_learning_decision(
        pool,
        session_id,
        id_for("learning clear"),
        scoped_user_id,
        ImportPreviewDecision::Clear,
        None,
        None,
    )?;
    assert!(cleared
        .preview
        .expect("cleared learning preview")
        .preview_matching_feedback
        .get("learning")
        .is_none());
    let absent_clear = apply_preview_learning_decision(
        pool,
        session_id,
        id_for("learning absent"),
        scoped_user_id,
        ImportPreviewDecision::Clear,
        None,
        None,
    )?;
    assert!(!absent_clear.state_conflict);
    assert!(absent_clear.preview.is_some());
    let absent_accept = apply_preview_learning_decision(
        pool,
        session_id,
        id_for("learning absent"),
        scoped_user_id,
        ImportPreviewDecision::Accept,
        None,
        None,
    )?;
    assert!(absent_accept.state_conflict);
    let stale_learning = apply_preview_learning_decision(
        pool,
        session_id,
        id_for("learning stale"),
        scoped_user_id,
        ImportPreviewDecision::Accept,
        None,
        Some(&ImportPreviewExpectedState {
            review_status: Some("rejected".to_string()),
            ..ImportPreviewExpectedState::default()
        }),
    )?;
    assert!(stale_learning.state_conflict);
    assert!(apply_preview_learning_decision(
        pool,
        "wrong-session",
        id_for("learning stale"),
        scoped_user_id,
        ImportPreviewDecision::Accept,
        None,
        None,
    )?
    .preview
    .is_none());
    assert!(apply_preview_learning_decision(
        pool,
        session_id,
        i64::MAX,
        scoped_user_id,
        ImportPreviewDecision::Accept,
        None,
        None,
    )?
    .preview
    .is_none());

    let learning_event_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM import_learning_feedback_events WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(
        learning_event_count, 2,
        "accept retry/conflicts must not duplicate events"
    );
    let accepted_lifecycle =
        get_import_learning_lifecycle_view(pool, scoped_user_id, "learning:accept")?
            .expect("accepted lifecycle view");
    assert_eq!(accepted_lifecycle.accepted_count, 1);
    assert_eq!(accepted_lifecycle.rejected_count, 0);

    let lifecycle_input = ImportLearningLifecycleRecordInput {
        recommendation_key: "learning:direct-matrix".to_string(),
        recommendation_type: "import_preview".to_string(),
        feedback: "accept".to_string(),
        rule_id: None,
        suggestion_id: None,
        session_id: Some(session_id.to_string()),
        preview_id: Some(learning_accept_id),
        bill_id: None,
        candidate_id: Some("direct-matrix".to_string()),
        payload_json: Some(json!({"source": "coverage-matrix"}).to_string()),
    };
    let first_direct =
        record_import_learning_lifecycle_feedback(pool, scoped_user_id, &lifecycle_input)?;
    assert_eq!(first_direct.accepted_count, 1);
    for expected_accepts in 2..=3 {
        let view =
            record_import_learning_lifecycle_feedback(pool, scoped_user_id, &lifecycle_input)?;
        assert_eq!(view.accepted_count, expected_accepts);
    }
    let auto_applied = record_import_learning_lifecycle_feedback(
        pool,
        scoped_user_id,
        &ImportLearningLifecycleRecordInput {
            feedback: "auto-applied".to_string(),
            ..lifecycle_input.clone()
        },
    )?;
    assert_eq!(auto_applied.auto_applied_count, 1);
    let suppressed = record_import_learning_lifecycle_feedback(
        pool,
        scoped_user_id,
        &ImportLearningLifecycleRecordInput {
            feedback: "suppress".to_string(),
            ..lifecycle_input.clone()
        },
    )?;
    assert_eq!(suppressed.status, "suppressed");
    assert_eq!(suppressed.signal_state, "suppressed");
    assert!(record_import_learning_lifecycle_feedback(
        pool,
        scoped_user_id,
        &ImportLearningLifecycleRecordInput {
            feedback: "unknown".to_string(),
            ..lifecycle_input.clone()
        },
    )
    .is_err());
    assert!(
        get_import_learning_lifecycle_view(pool, scoped_user_id, "missing-learning-key")?.is_none()
    );

    let llm_suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "expense".to_string(),
        suggested_category_id: Some(category_id),
        suggested_main_category: "测试".to_string(),
        suggested_sub_category: "生命周期".to_string(),
        suggested_source_account: "生命周期账户".to_string(),
        suggested_destination_account: String::new(),
        resolved_source_account_id: Some(account_id),
        resolved_destination_account_id: None,
        confidence: 0.91,
        reason: "fixture suggestion".to_string(),
    };
    let llm_expected = ImportPreviewExpectedState {
        session_id: Some(session_id.to_string()),
        review_status: Some("pending".to_string()),
        preview_category_id: Some(Some(category_id)),
        ..ImportPreviewExpectedState::default()
    };
    let llm_accept_id = id_for("llm accept");
    let llm_accept_request = ImportPreviewLlmReviewRequest {
        session_id,
        preview_id: llm_accept_id,
        user_id: scoped_user_id,
        decision: ImportPreviewDecision::Accept,
        suggestion: Some(&llm_suggestion),
        user_correction_category: Some("生命周期分类"),
        user_correction_account: Some("生命周期账户"),
        expected_state: Some(&llm_expected),
    };
    let llm_accepted = review_preview_llm_recommendation(pool, &llm_accept_request)?;
    assert!(!llm_accepted.state_conflict);
    assert!(llm_accepted.event_id.is_some());
    assert_eq!(
        llm_accepted
            .preview
            .expect("accepted LLM preview")
            .preview_matching_feedback
            .pointer("/llm/review_status"),
        Some(&json!("accepted"))
    );
    let llm_retry = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            expected_state: None,
            ..llm_accept_request
        },
    )?;
    assert!(llm_retry.event_id.is_none());
    assert!(!llm_retry.state_conflict);
    let llm_opposite = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            decision: ImportPreviewDecision::Reject,
            expected_state: None,
            ..llm_accept_request
        },
    )?;
    assert!(llm_opposite.state_conflict);
    assert!(llm_opposite.preview.is_some());

    let llm_reject_id = id_for("llm reject");
    let llm_rejected = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            session_id,
            preview_id: llm_reject_id,
            user_id: scoped_user_id,
            decision: ImportPreviewDecision::Reject,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
            expected_state: None,
        },
    )?;
    assert_eq!(
        llm_rejected
            .preview
            .expect("rejected LLM preview")
            .preview_matching_feedback
            .pointer("/llm/review_status"),
        Some(&json!("rejected"))
    );
    let llm_cleared = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            session_id,
            preview_id: id_for("llm clear"),
            user_id: scoped_user_id,
            decision: ImportPreviewDecision::Clear,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
            expected_state: None,
        },
    )?;
    assert!(llm_cleared
        .preview
        .expect("cleared LLM preview")
        .preview_matching_feedback
        .get("llm")
        .is_none());
    let llm_absent_id = id_for("llm absent");
    let llm_absent_clear = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            session_id,
            preview_id: llm_absent_id,
            user_id: scoped_user_id,
            decision: ImportPreviewDecision::Clear,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
            expected_state: None,
        },
    )?;
    assert!(llm_absent_clear.preview.is_some());
    assert!(llm_absent_clear.event_id.is_none());
    let llm_absent_accept = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            decision: ImportPreviewDecision::Accept,
            ..ImportPreviewLlmReviewRequest {
                session_id,
                preview_id: llm_absent_id,
                user_id: scoped_user_id,
                decision: ImportPreviewDecision::Clear,
                suggestion: None,
                user_correction_category: None,
                user_correction_account: None,
                expected_state: None,
            }
        },
    )?;
    assert!(llm_absent_accept.state_conflict);
    let llm_stale = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            session_id,
            preview_id: id_for("llm stale"),
            user_id: scoped_user_id,
            decision: ImportPreviewDecision::Accept,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
            expected_state: Some(&ImportPreviewExpectedState {
                review_status: Some("rejected".to_string()),
                ..ImportPreviewExpectedState::default()
            }),
        },
    )?;
    assert!(llm_stale.state_conflict);
    let wrong_session_llm = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            session_id: "wrong-session",
            preview_id: id_for("llm stale"),
            user_id: scoped_user_id,
            decision: ImportPreviewDecision::Accept,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
            expected_state: None,
        },
    )?;
    assert!(wrong_session_llm.preview.is_none());
    let missing_llm = review_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmReviewRequest {
            session_id,
            preview_id: i64::MAX,
            user_id: scoped_user_id,
            decision: ImportPreviewDecision::Accept,
            suggestion: None,
            user_correction_category: None,
            user_correction_account: None,
            expected_state: None,
        },
    )?;
    assert!(missing_llm.preview.is_none());

    let llm_event_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM llm_memory_events WHERE user_id = $1 AND event_type = 'preview_review'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(
        llm_event_count, 3,
        "only accept, reject, and clear persist review events"
    );

    test_db.cleanup().await?;
    Ok(())
}

fn preview_draft(
    date: &str,
    preview_type: &str,
    amount_cents: i64,
    category_id: Option<i64>,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
    selected: bool,
) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: date.to_string(),
        preview_type: preview_type.to_string(),
        preview_amount_cents: amount_cents,
        preview_destination_amount_cents: amount_cents,
        category_id,
        preview_source_account_id: source_account_id,
        preview_destination_account_id: destination_account_id,
        preview_counterparty: "运行时商户".to_string(),
        preview_payment_method: "测试渠道".to_string(),
        preview_description: format!("{preview_type} {amount_cents}"),
        preview_parser_id: "runtime-parser".to_string(),
        preview_parser_tags: Some(json!(["parser:runtime"])),
        preview_selected: selected,
        ..ImportPreviewDraft::default()
    }
}

async fn insert_user(pool: &PostgresPool, name: &str) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(name)
            .bind(format!("{name}@example.test"))
            .fetch_one(pool)
            .await?
            .try_get("id")?,
    )
}

async fn insert_account(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO accounts (user_id, name, account_type, currency, balance_cents) VALUES ($1, $2, 'cash', 'CNY', 0) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_category(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
    category_type: &str,
    path: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query("INSERT INTO categories (user_id, name, category_type, path) VALUES ($1, $2, $3, $4) RETURNING id")
            .bind(user_id)
            .bind(name)
            .bind(category_type)
            .bind(path)
            .fetch_one(pool)
            .await?
            .try_get("id")?,
    )
}

async fn insert_history_bill(
    pool: &PostgresPool,
    user_id: i64,
    description: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, amount_cents, direction, transaction_type,
            merchant, payment_method, description, parser_name, source_hash
        )
        VALUES ($1, '2026-06-30 09:00:00+08', 1888, 'expense', 'expense',
            '历史商户', '测试渠道', $2, 'history-fixture', $3)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(description)
    .bind(format!("history-fixture-{user_id}-{description}"))
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}
