// 中文导读：导入 Postgres staging 的结构性合同测试，锁定 session/source/preview/group 生命周期。
// 维护重点：这些测试读取迁移清单和权威 schema，避免 import stage2 拆分时破坏 staging 表关系。
// 不变式：staging 子表必须由 import_sessions 级联清理，preview 与决策/历史 materialization 保持可重建。

use std::{error::Error, fs};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    confirm_preview_to_bills, create_import_session, get_import_session, get_preview_bill_by_id,
    insert_preview_bill, insert_preview_bills_batch, postgres_initial_schema_path,
    postgres_migration_manifest, query_preview_page_by_session,
    replace_preview_selection_with_patches, update_session_preview_selection_by_query,
    ImportPreviewDraft, ImportPreviewPageRequest, ImportPreviewPatch, ImportPreviewPatchField,
    ImportPreviewPatchValue, ImportPreviewQueryFilters, ImportPreviewSelectionMode,
    ImportPreviewSelectionTarget, ImportSessionDraft, PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

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
