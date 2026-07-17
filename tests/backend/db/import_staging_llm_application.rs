use std::{env, error::Error};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    apply_preview_llm_recommendation, create_import_session, get_preview_bill_by_id,
    insert_preview_bills_batch, ImportPreviewDraft, ImportPreviewLlmApplyRequest,
    ImportPreviewLlmSuggestion, ImportSessionDraft, PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for LLM application coverage",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for LLM application coverage",
            )
            .into()
        })
}

#[tokio::test(flavor = "multi_thread")]
async fn real_postgres_llm_application_resolves_scoped_categories_and_transfer_authority(
) -> Result<(), Box<dyn Error>> {
    let test_db = strict_isolated_postgres_database("import_llm_application_coverage").await?;
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "llm-application-owner").await?;
    let other_user_id = insert_user(pool, "llm-application-other").await?;
    let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
    let source_account_id = insert_account(pool, user_id, "LLM 来源账户").await?;
    let destination_account_id = insert_account(pool, user_id, "LLM 目标账户").await?;
    let expense_category_id =
        insert_category(pool, user_id, "咖啡", "expense", "餐饮/咖啡").await?;
    let transfer_category_id =
        insert_category(pool, user_id, "账户互转", "transfer", "转账/账户互转").await?;
    let foreign_category_id =
        insert_category(pool, other_user_id, "越权分类", "expense", "越权/分类").await?;
    let session_id = "llm-application-session";

    create_import_session(
        pool,
        &ImportSessionDraft {
            session_id: session_id.to_string(),
            user_id: scoped_user_id,
            file_count: 1,
        },
    )?;

    let drafts = [
        preview_draft("plain type", json!(["legacy-feedback"])),
        preview_draft("named category", json!({"parser": {"source": "fixture"}})),
        preview_draft("unauthorized transfer", json!({})),
        preview_draft(
            "authorized transfer",
            json!({
                "transfer": {
                    "candidate_type": "transfer_cross_batch",
                    "review_status": "pending"
                }
            }),
        ),
        preview_draft("foreign category", json!({})),
    ];
    assert_eq!(
        insert_preview_bills_batch(pool, session_id, scoped_user_id, &drafts)?,
        drafts.len()
    );

    let rows = sqlx::query(
        r#"
        SELECT id, description
        FROM import_preview_rows
        WHERE user_id = $1
        ORDER BY id
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    let preview_id = |description: &str| {
        rows.iter()
            .find(|row| row.get::<String, _>("description") == description)
            .map(|row| row.get::<i64, _>("id"))
            .unwrap_or_else(|| panic!("missing preview {description}"))
    };

    let missing = apply_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmApplyRequest {
            session_id,
            preview_id: i64::MAX,
            user_id: scoped_user_id,
            suggestion: &ImportPreviewLlmSuggestion::default(),
            prompt_text: None,
            llm_provider: None,
            llm_model: None,
        },
    )?;
    assert!(missing.preview.is_none());
    assert!(missing.event_id.is_none());
    assert!(missing.applied_fields.is_empty());

    let plain_type_suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "expense".to_string(),
        resolved_source_account_id: Some(source_account_id),
        confidence: 0.71,
        reason: "plain type and account".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let plain_type = apply_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmApplyRequest {
            session_id,
            preview_id: preview_id("plain type"),
            user_id: scoped_user_id,
            suggestion: &plain_type_suggestion,
            prompt_text: Some("classify plain type"),
            llm_provider: Some("fixture-provider"),
            llm_model: Some("fixture-model"),
        },
    )?;
    let plain_type_preview = plain_type.preview.expect("plain type preview");
    assert_eq!(plain_type_preview.preview_type, "支出");
    assert_eq!(
        plain_type_preview.preview_source_account_id,
        Some(source_account_id)
    );
    assert_eq!(plain_type.applied_fields, vec!["type", "source_account_id"]);
    assert_eq!(
        plain_type_preview
            .preview_matching_feedback
            .pointer("/llm/review_status"),
        Some(&json!("pending"))
    );

    let named_category_suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "expense".to_string(),
        suggested_main_category: "餐饮".to_string(),
        suggested_sub_category: "咖啡".to_string(),
        resolved_destination_account_id: Some(destination_account_id),
        confidence: 0.82,
        reason: "match category by path".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let named_category = apply_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmApplyRequest {
            session_id,
            preview_id: preview_id("named category"),
            user_id: scoped_user_id,
            suggestion: &named_category_suggestion,
            prompt_text: None,
            llm_provider: None,
            llm_model: None,
        },
    )?;
    let named_category_preview = named_category.preview.expect("named category preview");
    assert_eq!(
        named_category_preview.category_id,
        Some(expense_category_id)
    );
    assert_eq!(named_category_preview.preview_main_category, "餐饮");
    assert_eq!(named_category_preview.preview_sub_category, "咖啡");
    assert_eq!(
        named_category_preview.preview_destination_account_id,
        None,
        "non-transfer previews keep destination identity demoted even when the LLM signal records it"
    );
    assert_eq!(
        named_category.applied_fields,
        vec![
            "type",
            "category_id",
            "main_category",
            "sub_category",
            "destination_account_id"
        ]
    );

    let unauthorized_transfer_suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "transfer".to_string(),
        suggested_category_id: Some(transfer_category_id),
        confidence: 0.93,
        reason: "must remain signal only".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let unauthorized_transfer = apply_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmApplyRequest {
            session_id,
            preview_id: preview_id("unauthorized transfer"),
            user_id: scoped_user_id,
            suggestion: &unauthorized_transfer_suggestion,
            prompt_text: None,
            llm_provider: None,
            llm_model: None,
        },
    )?;
    let unauthorized_preview = unauthorized_transfer
        .preview
        .expect("unauthorized transfer preview");
    assert!(unauthorized_transfer.applied_fields.is_empty());
    assert_eq!(unauthorized_preview.preview_type, "支出");
    assert_eq!(unauthorized_preview.category_id, None);
    assert_eq!(
        unauthorized_preview
            .preview_matching_feedback
            .pointer("/llm/type_ignored_reason"),
        Some(&json!("unauthorized_transfer"))
    );
    assert_eq!(
        unauthorized_preview
            .preview_matching_feedback
            .pointer("/llm/category_ignored_reason"),
        Some(&json!("unresolved_or_unauthorized_category"))
    );

    let authorized_transfer_suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "transfer".to_string(),
        suggested_category_id: Some(transfer_category_id),
        suggested_main_category: "转账".to_string(),
        suggested_sub_category: "账户互转".to_string(),
        resolved_source_account_id: Some(source_account_id),
        resolved_destination_account_id: Some(destination_account_id),
        confidence: 0.97,
        reason: "authorized structured transfer".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let authorized_transfer = apply_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmApplyRequest {
            session_id,
            preview_id: preview_id("authorized transfer"),
            user_id: scoped_user_id,
            suggestion: &authorized_transfer_suggestion,
            prompt_text: None,
            llm_provider: None,
            llm_model: None,
        },
    )?;
    let authorized_preview = authorized_transfer
        .preview
        .expect("authorized transfer preview");
    assert_eq!(authorized_preview.preview_type, "转账");
    assert_eq!(authorized_preview.category_id, Some(transfer_category_id));
    assert_eq!(
        authorized_preview.preview_source_account_id,
        Some(source_account_id)
    );
    assert_eq!(
        authorized_preview.preview_destination_account_id,
        Some(destination_account_id)
    );
    assert_eq!(authorized_transfer.applied_fields.len(), 6);

    let foreign_category_suggestion = ImportPreviewLlmSuggestion {
        suggested_type: "transfer".to_string(),
        suggested_category_id: Some(foreign_category_id),
        confidence: 0.64,
        reason: "foreign category must not resolve".to_string(),
        ..ImportPreviewLlmSuggestion::default()
    };
    let foreign_category = apply_preview_llm_recommendation(
        pool,
        &ImportPreviewLlmApplyRequest {
            session_id,
            preview_id: preview_id("foreign category"),
            user_id: scoped_user_id,
            suggestion: &foreign_category_suggestion,
            prompt_text: None,
            llm_provider: None,
            llm_model: None,
        },
    )?;
    let foreign_category_preview = foreign_category.preview.expect("foreign category preview");
    assert!(foreign_category.applied_fields.is_empty());
    assert_eq!(foreign_category_preview.category_id, None);

    let event_rows = sqlx::query(
        r#"
        SELECT prompt_text, llm_provider, llm_model, event_type, decision
        FROM llm_memory_events
        WHERE user_id = $1 AND event_type = 'preview_apply'
        ORDER BY id
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    assert_eq!(event_rows.len(), 5);
    assert_eq!(
        event_rows[0].get::<String, _>("prompt_text"),
        "classify plain type"
    );
    assert_eq!(
        event_rows[0].get::<String, _>("llm_provider"),
        "fixture-provider"
    );
    assert_eq!(event_rows[0].get::<String, _>("llm_model"), "fixture-model");
    assert_eq!(
        event_rows[0].get::<String, _>("event_type"),
        "preview_apply"
    );
    assert_eq!(event_rows[0].get::<String, _>("decision"), "accept");

    assert!(get_preview_bill_by_id(pool, preview_id("plain type"), scoped_user_id)?.is_some());
    test_db.cleanup().await?;
    Ok(())
}

fn preview_draft(description: &str, feedback: Value) -> ImportPreviewDraft {
    ImportPreviewDraft {
        preview_date: "2026-07-15 09:00:00".to_string(),
        preview_type: "支出".to_string(),
        preview_amount_cents: -1288,
        preview_destination_amount_cents: -1288,
        preview_counterparty: "LLM 测试商户".to_string(),
        preview_payment_method: "LLM 测试渠道".to_string(),
        preview_description: description.to_string(),
        preview_parser_id: "llm-application-fixture".to_string(),
        preview_parser_tags: Some(json!(["parser:llm-application"])),
        preview_selected: true,
        preview_matching_feedback: feedback,
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
