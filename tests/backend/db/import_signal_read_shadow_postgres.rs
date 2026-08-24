use std::{env, error::Error, io};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    audit_import_preview_signal_read_parity, backfill_import_preview_signal_projection_batch,
    query_preview_page_by_session, DbError, ImportPreviewPageRequest, ImportPreviewQueryFilters,
    PostgresPool,
};
use serde_json::{json, Value};

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal read shadow tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal read shadow tests",
            )
            .into()
        })
}

async fn create_session(
    pool: &PostgresPool,
    suffix: &str,
) -> Result<(i64, i64, String), sqlx::Error> {
    let user_id: i64 = sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(format!("signal-read-shadow-{suffix}"))
        .fetch_one(pool)
        .await?;
    let session_key = format!("signal-read-shadow-{suffix}");
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
    )
    .bind(user_id)
    .bind(&session_key)
    .fetch_one(pool)
    .await?;
    Ok((user_id, session_id, session_key))
}

async fn insert_legacy_preview(
    pool: &PostgresPool,
    user_id: i64,
    session_id: i64,
    sort_key: &str,
    payload: Value,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO import_preview_rows (session_id,user_id,page_sort_key,operation_kind,occurred_at,amount_cents,direction,preview_payload) VALUES ($1,$2,$3,'insert',now(),-100,'expense',$4) RETURNING id",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(sort_key)
    .bind(payload)
    .fetch_one(pool)
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn signal_read_shadow_matches_legacy_for_all_visible_families() -> Result<(), Box<dyn Error>>
{
    let isolated = strict_isolated_postgres_database("import_signal_read_shadow_parity").await?;
    let result = async {
        let (user_id, session_db_id, session_key) =
            create_session(&isolated.pool, "parity").await?;
        let category_id: i64 = sqlx::query_scalar(
            "INSERT INTO categories (user_id,name,category_type,path) VALUES ($1,'Shadow Category','expense','Shadow Category') RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&isolated.pool)
        .await?;
        let account_id: i64 = sqlx::query_scalar(
            "INSERT INTO accounts (user_id,name,account_type,currency,balance_cents) VALUES ($1,'Shadow Account','cash','CNY',0) RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&isolated.pool)
        .await?;
        let payloads = [
            json!({"preview_parser_id":"wechat","preview_parser_tags":["shadow-tag"],"preview_matching_feedback":{}}),
            json!({"preview_parser_id":"wechat","preview_parser_tags":["shadow-tag"],"dedup_type":"platform_bank","preview_matching_feedback":{}}),
            json!({"preview_type":"支出","preview_parser_tags":["shadow-tag"],"preview_matching_feedback":{"transfer":{"review_status":"pending","candidate_type":"transfer","learning_level":"green"}}}),
            json!({"preview_parser_tags":["shadow-tag"],"preview_matching_feedback":{"reconciliation":{"review_status":"pending","planned_operation":"update"}}}),
            json!({"preview_parser_tags":["shadow-tag"],"preview_matching_feedback":{"learning":{"review_status":"needs_review","reason":"fixture","score":0.8}}}),
            json!({"preview_parser_tags":["shadow-tag"],"preview_matching_feedback":{"llm":{"review_status":"pending","reason":"fixture","score":0.9}}}),
        ];
        for (index, payload) in payloads.into_iter().enumerate() {
            insert_legacy_preview(
                &isolated.pool,
                user_id,
                session_db_id,
                &format!("row-{index}"),
                payload,
            )
            .await?;
        }
        sqlx::query(
            "UPDATE import_preview_rows SET category_id=$1,account_id=$2 WHERE session_id=$3",
        )
        .bind(category_id)
        .bind(account_id)
        .bind(session_db_id)
        .execute(&isolated.pool)
        .await?;
        let backfill =
            backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 100).await?;
        assert_eq!(backfill.updated_rows, 6);
        assert_eq!(backfill.mismatch_rows, 0);

        let scoped_user = UserId::new(user_id as u64).expect("positive user id");
        let baseline = query_preview_page_by_session(
            &isolated.pool,
            &session_key,
            scoped_user,
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 100,
                ..ImportPreviewPageRequest::default()
            },
        )?;
        assert_eq!(baseline.metadata.facets.categories.len(), 1);
        assert_eq!(baseline.metadata.facets.accounts.len(), 1);
        assert_eq!(baseline.metadata.facets.tags.len(), 1);
        for signal in [
            None,
            Some("parser"),
            Some("platform_duplicate"),
            Some("transfer"),
            Some("history"),
            Some("learning"),
            Some("llm"),
            Some("transfer:pending"),
            Some("learning:needs_review"),
        ] {
            let request = ImportPreviewPageRequest {
                page: 1,
                page_size: 100,
                filters: ImportPreviewQueryFilters {
                    signal: signal.map(str::to_string),
                    ..ImportPreviewQueryFilters::default()
                },
                ..ImportPreviewPageRequest::default()
            };
            let report = audit_import_preview_signal_read_parity(
                &isolated.pool,
                &session_key,
                scoped_user,
                &request,
            )?;
            assert!(report.is_match(), "signal {signal:?}: {report:?}");
            assert_eq!(report.mismatch_count(), 0);
        }
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test(flavor = "multi_thread")]
async fn signal_read_shadow_fails_closed_for_unmaterialized_rows_and_reports_drift(
) -> Result<(), Box<dyn Error>> {
    let isolated =
        strict_isolated_postgres_database("import_signal_read_shadow_fail_closed").await?;
    let result = async {
        let (user_id, session_db_id, session_key) =
            create_session(&isolated.pool, "fail-closed").await?;
        let preview_id = insert_legacy_preview(
            &isolated.pool,
            user_id,
            session_db_id,
            "parser",
            json!({"preview_parser_id":"wechat","preview_matching_feedback":{}}),
        )
        .await?;
        let scoped_user = UserId::new(user_id as u64).expect("positive user id");
        let request = ImportPreviewPageRequest {
            page: 1,
            page_size: 100,
            ..ImportPreviewPageRequest::default()
        };

        let public_error =
            query_preview_page_by_session(&isolated.pool, &session_key, scoped_user, &request)
                .expect_err("the public reader must fail closed before target materialization");
        assert!(matches!(public_error, DbError::InvalidOperation(_)));
        assert!(public_error
            .to_string()
            .contains("signal_projection_version=1"));

        let error = audit_import_preview_signal_read_parity(
            &isolated.pool,
            &session_key,
            scoped_user,
            &request,
        )
        .expect_err("version zero rows must block typed read shadow");
        assert!(matches!(error, DbError::InvalidOperation(_)));
        assert!(error.to_string().contains("signal_projection_version=1"));

        let backfill =
            backfill_import_preview_signal_projection_batch(&isolated.pool, 0, 100).await?;
        assert_eq!(backfill.updated_rows, 1);
        sqlx::query("UPDATE import_preview_rows SET signal_parser=false WHERE id=$1")
            .bind(preview_id)
            .execute(&isolated.pool)
            .await?;

        let report = audit_import_preview_signal_read_parity(
            &isolated.pool,
            &session_key,
            scoped_user,
            &request,
        )?;
        assert!(!report.is_match());
        assert!(report.signal_counts_mismatch);
        assert!(report.mismatch_count() > 0);

        let explicit_typed = query_preview_page_by_session(
            &isolated.pool,
            &session_key,
            scoped_user,
            &ImportPreviewPageRequest {
                page: 1,
                page_size: 100,
                preview_ids: vec![preview_id],
                filters: ImportPreviewQueryFilters {
                    signal: Some("parser".to_string()),
                    ..ImportPreviewQueryFilters::default()
                },
                ..ImportPreviewPageRequest::default()
            },
        )?;
        assert_eq!(explicit_typed.total, 0);
        assert!(explicit_typed.rows.is_empty());
        assert_eq!(
            explicit_typed.metadata.counts.signals.get("parser"),
            Some(&0)
        );

        let bypass_error = audit_import_preview_signal_read_parity(
            &isolated.pool,
            &session_key,
            scoped_user,
            &ImportPreviewPageRequest {
                preview_ids: vec![preview_id],
                ..ImportPreviewPageRequest::default()
            },
        )
        .expect_err("preview_ids bypass cannot prove SQL read parity");
        assert!(matches!(bypass_error, DbError::InvalidOperation(_)));
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
