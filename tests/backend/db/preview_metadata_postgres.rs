use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_import_session, insert_preview_bill, preview_id_snapshot_hash,
    query_preview_page_by_session, ImportPreviewDraft, ImportPreviewPageRequest,
    ImportPreviewQueryFilters, ImportSessionDraft, PostgresPool,
};
use serde_json::json;
use sqlx::Row;

mod postgres_test_support;

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
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO categories (user_id, name, category_type, path) VALUES ($1, $2, 'expense', $2) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

fn preview_draft(
    description: &str,
    category_id: i64,
    account_id: i64,
    selected: bool,
    llm_signal: bool,
) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: "2026-07-13 09:00:00".to_string(),
        preview_type: "支出".to_string(),
        preview_amount_cents: 1_288,
        preview_destination_amount_cents: 0,
        category_id: Some(category_id),
        preview_source_account_id: Some(account_id),
        preview_counterparty: "metadata fixture".to_string(),
        preview_payment_method: "fixture account".to_string(),
        preview_description: description.to_string(),
        preview_parser_id: "metadata-fixture".to_string(),
        preview_parser_tags: Some(json!(["metadata-tag"])),
        preview_selected: selected,
        preview_matching_feedback: if llm_signal {
            json!({"llm": {"reason": "fixture recommendation", "score": 0.9}})
        } else {
            json!({})
        },
        ..ImportPreviewDraft::default()
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_metadata_aggregate_preserves_scope_facets_and_selection_snapshot(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("preview_metadata_aggregate").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-metadata-owner").await?;
    let other_user_id = insert_user(pool, "preview-metadata-other").await?;
    let scoped_user = UserId::new(user_id as u64).expect("positive user id");
    let scoped_other = UserId::new(other_user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "Metadata Wallet").await?;
    let category_id = insert_category(pool, user_id, "Metadata Food").await?;
    let other_account_id = insert_account(pool, other_user_id, "Other Wallet").await?;
    let other_category_id = insert_category(pool, other_user_id, "Other Food").await?;

    for (session_id, user_id) in [
        ("preview-metadata-session", scoped_user),
        ("preview-metadata-other-session", scoped_other),
    ] {
        create_import_session(
            pool,
            &ImportSessionDraft {
                session_id: session_id.to_string(),
                user_id,
                file_count: 1,
            },
        )?;
    }

    let first_id = insert_preview_bill(
        pool,
        "preview-metadata-session",
        scoped_user,
        &preview_draft("coffee first", category_id, account_id, true, true),
    )?;
    insert_preview_bill(
        pool,
        "preview-metadata-session",
        scoped_user,
        &preview_draft("tea second", category_id, account_id, false, false),
    )?;
    let third_id = insert_preview_bill(
        pool,
        "preview-metadata-session",
        scoped_user,
        &preview_draft("coffee third", category_id, account_id, true, false),
    )?;
    let other_id = insert_preview_bill(
        pool,
        "preview-metadata-other-session",
        scoped_other,
        &preview_draft(
            "coffee other user",
            other_category_id,
            other_account_id,
            true,
            true,
        ),
    )?;

    let page = query_preview_page_by_session(
        pool,
        "preview-metadata-session",
        scoped_user,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 1,
            filters: ImportPreviewQueryFilters {
                description: Some("coffee first".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;

    assert_eq!(page.total, 1);
    assert_eq!(page.rows.len(), 1);
    assert_eq!(page.metadata.counts.total, 1);
    assert_eq!(page.metadata.counts.selected, 1);
    assert_eq!(
        page.metadata.counts.selected_total, 2,
        "full-session selection count is independent of the active filter"
    );
    assert_eq!(page.metadata.counts.selected_invalid, 0);
    assert_eq!(page.metadata.counts.signals.get("llm"), Some(&1));
    for family in [
        "parser",
        "platform_duplicate",
        "transfer",
        "history",
        "learning",
    ] {
        assert_eq!(
            page.metadata.counts.signals.get(family),
            Some(&0),
            "{family}"
        );
    }
    assert!(page.metadata.facets.categories.iter().any(|facet| {
        facet.value == category_id.to_string()
            && facet.label.as_deref() == Some("Metadata Food")
            && facet.count == 1
    }));
    assert!(page.metadata.facets.accounts.iter().any(|facet| {
        facet.value == account_id.to_string()
            && facet.label.as_deref() == Some("Metadata Wallet")
            && facet.count == 1
    }));
    assert!(page
        .metadata
        .facets
        .tags
        .iter()
        .any(|facet| { facet.value == "metadata-tag" && facet.count == 1 }));
    assert_eq!(
        page.metadata.selection_hash,
        preview_id_snapshot_hash(&[first_id, third_id]),
        "selection hash covers every selected preview in this user/session, independent of page and filters"
    );
    assert_ne!(
        page.metadata.selection_hash,
        preview_id_snapshot_hash(&[first_id, third_id, other_id]),
        "selection hash must not include another user's session"
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_metadata_aggregate_keeps_empty_facets_and_zero_signals_canonical(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("preview_metadata_empty_facets").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-metadata-empty-owner").await?;
    let scoped_user = UserId::new(user_id as u64).expect("positive user id");
    let session_id = "preview-metadata-empty-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user,
            file_count: 1,
        },
    )?;
    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewDraft {
            preview_date: "2026-07-13 10:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 500,
            preview_counterparty: "identity-free fixture".to_string(),
            preview_payment_method: "".to_string(),
            preview_description: "identity-free preview".to_string(),
            preview_parser_id: "".to_string(),
            preview_parser_tags: None,
            preview_selected: false,
            preview_matching_feedback: json!({}),
            ..ImportPreviewDraft::default()
        },
    )?;

    let page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewPageRequest {
            page: 0,
            page_size: 0,
            sort_by: "time".to_string(),
            sort_direction: "desc".to_string(),
            filters: ImportPreviewQueryFilters {
                description: Some("identity-free".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;

    assert_eq!(page.page, 1);
    assert_eq!(page.page_size, 1);
    assert_eq!(page.total, 1);
    assert_eq!(
        page.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![preview_id]
    );
    assert_eq!(page.metadata.counts.selected, 0);
    assert_eq!(page.metadata.counts.selected_total, 0);
    assert_eq!(page.metadata.selection_hash, preview_id_snapshot_hash(&[]));
    assert!(page
        .metadata
        .counts
        .signals
        .values()
        .all(|count| *count == 0));
    assert!(page.metadata.facets.categories.is_empty());
    assert!(page.metadata.facets.accounts.is_empty());
    assert!(page.metadata.facets.tags.is_empty());

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn postgres_transfer_signal_projection_keeps_accepted_evidence_visible(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("transfer_signal_projection").await?
    else {
        return Ok(());
    };

    for (status, expected_visible) in [
        ("pending", true),
        ("accepted", true),
        ("auto_applied", true),
        ("auto-applied", true),
        ("rejected", false),
        ("skipped", false),
    ] {
        let payload = json!({
            "preview_type": "支出",
            "preview_matching_feedback": {"transfer": {"review_status": status}}
        });
        let visible: bool = sqlx::query_scalar(
            "SELECT COALESCE((import_preview_signal_flags($1::jsonb)->>'transfer')::boolean, false)",
        )
        .bind(payload)
        .fetch_one(&test_db.pool)
        .await?;
        assert_eq!(visible, expected_visible, "transfer status {status}");
    }

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn likely_transfer_counts_and_filters_as_transfer_and_learning() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("transfer_learning_overlap").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-transfer-learning-overlap").await?;
    let scoped_user = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "Transfer Learning Wallet").await?;
    let category_id = insert_category(pool, user_id, "Transfer Learning Category").await?;
    let session_id = "preview-transfer-learning-overlap-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user,
            file_count: 1,
        },
    )?;

    let likely_transfer_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewDraft {
            preview_date: "2026-07-13 10:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 2_188,
            category_id: Some(category_id),
            preview_source_account_id: Some(account_id),
            preview_description: "likely transfer overlap".to_string(),
            preview_parser_id: "wechat".to_string(),
            preview_selected: true,
            preview_matching_feedback: json!({
                "transfer": {
                    "review_status": "pending",
                    "candidate_type": "transfer",
                    "learning_level": "green"
                },
                "learning": {
                    "review_status": "skipped",
                    "reason": "transfer preview is protected from learning type/category overrides"
                }
            }),
            ..ImportPreviewDraft::default()
        },
    )?;
    insert_preview_bill(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewDraft {
            preview_date: "2026-07-13 11:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 688,
            category_id: Some(category_id),
            preview_source_account_id: Some(account_id),
            preview_description: "parser only".to_string(),
            preview_parser_id: "wechat".to_string(),
            preview_selected: true,
            ..ImportPreviewDraft::default()
        },
    )?;

    let all_rows = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    assert_eq!(all_rows.metadata.counts.signals.get("transfer"), Some(&1));
    assert_eq!(all_rows.metadata.counts.signals.get("learning"), Some(&1));
    assert_eq!(all_rows.metadata.counts.signals.get("parser"), Some(&1));

    for family in ["transfer", "learning"] {
        let page = query_preview_page_by_session(
            pool,
            session_id,
            scoped_user,
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 10,
                filters: ImportPreviewQueryFilters {
                    signal: Some(family.to_string()),
                    ..ImportPreviewQueryFilters::default()
                },
                ..ImportPreviewPageRequest::default()
            },
        )?;
        assert_eq!(page.total, 1, "{family} filter total");
        assert_eq!(page.rows[0].id, likely_transfer_id, "{family} filter row");
    }

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_metadata_combines_transfer_signal_with_missing_category_filter(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("preview_transfer_missing_category")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-transfer-missing-category").await?;
    let scoped_user = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "Transfer Signal Wallet").await?;
    let session_id = "preview-transfer-missing-category-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user,
            file_count: 1,
        },
    )?;

    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewDraft {
            preview_date: "2026-07-13 11:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 2_588,
            preview_destination_amount_cents: 0,
            category_id: None,
            preview_source_account_id: Some(account_id),
            preview_counterparty: "transfer candidate".to_string(),
            preview_payment_method: "fixture account".to_string(),
            preview_description: "transfer signal missing category".to_string(),
            preview_parser_id: "metadata-fixture".to_string(),
            preview_selected: true,
            preview_matching_feedback: json!({
                "transfer": {
                    "review_status": "pending",
                    "candidate_type": "transfer"
                }
            }),
            ..ImportPreviewDraft::default()
        },
    )?;

    let page = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 10,
            filters: ImportPreviewQueryFilters {
                signal: Some("transfer".to_string()),
                category: Some("__invalid__".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;

    assert_eq!(page.total, 1);
    assert_eq!(
        page.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![preview_id]
    );
    assert_eq!(page.metadata.counts.total, 1);
    assert_eq!(page.metadata.counts.selected, 0);
    assert_eq!(page.metadata.counts.selected_invalid, 0);
    assert_eq!(page.metadata.counts.signals.get("transfer"), Some(&1));
    assert_eq!(page.metadata.selection_hash, preview_id_snapshot_hash(&[]));

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn preview_needs_review_respects_manually_owned_active_category_type_mismatch(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("preview_manual_category_ownership")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "preview-manual-category-owner").await?;
    let scoped_user = UserId::new(user_id as u64).expect("positive user id");
    let account_id = insert_account(pool, user_id, "Manual Category Wallet").await?;
    let expense_category_id = insert_category(pool, user_id, "Manual Expense Category").await?;
    let session_id = "preview-manual-category-session";
    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user,
            file_count: 1,
        },
    )?;

    let preview_id = insert_preview_bill(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewDraft {
            preview_date: "2026-07-13 12:00:00".to_string(),
            preview_type: "收入".to_string(),
            preview_amount_cents: 1_688,
            category_id: Some(expense_category_id),
            preview_source_account_id: Some(account_id),
            preview_counterparty: "manual category fixture".to_string(),
            preview_payment_method: "fixture account".to_string(),
            preview_description: "manual category type mismatch".to_string(),
            preview_parser_id: "metadata-fixture".to_string(),
            preview_selected: true,
            preview_matching_feedback: json!({
                "annotation": {
                    "is_manually_annotated": true,
                    "manual_fields": {"category_id": true}
                }
            }),
            ..ImportPreviewDraft::default()
        },
    )?;

    let no_issues = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewPageRequest {
            filters: ImportPreviewQueryFilters {
                annotation: Some("no-issues".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;
    let needs_review = query_preview_page_by_session(
        pool,
        session_id,
        scoped_user,
        &ImportPreviewPageRequest {
            filters: ImportPreviewQueryFilters {
                annotation: Some("needs-review".to_string()),
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        },
    )?;

    assert_eq!(
        no_issues.rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![preview_id]
    );
    assert_eq!(no_issues.metadata.counts.selected_invalid, 0);
    assert!(needs_review.rows.is_empty());

    test_db.cleanup().await?;
    Ok(())
}
