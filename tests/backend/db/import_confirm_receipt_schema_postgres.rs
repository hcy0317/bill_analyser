use std::{env, error::Error, io};

use bill_analyser_core::UserId;
use bill_analyser_db::{clear_postgres_user_data, clear_session_data, PostgresPool};
use serde_json::json;
use sqlx::Row;

mod postgres_test_support;

const RECEIPT_CONSTRAINTS: &[&str] = &[
    "pk_import_confirm_receipts",
    "fk_import_confirm_receipts_session_user",
    "chk_import_confirm_receipts_receipt_schema_version",
    "chk_import_confirm_receipts_command_fingerprint",
    "chk_import_confirm_receipts_request_session_version",
    "chk_import_confirm_receipts_response_schema_version",
    "chk_import_confirm_receipts_http_status",
    "chk_import_confirm_receipts_success_envelope",
];

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for confirm receipt schema tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for confirm receipt schema tests",
            )
            .into()
        })
}

fn assert_sqlstate(error: &sqlx::Error, expected: &str) {
    assert_eq!(
        error
            .as_database_error()
            .and_then(|database| database.code()),
        Some(expected.into()),
        "unexpected PostgreSQL error: {error}"
    );
}

async fn insert_user(pool: &PostgresPool, username: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
        .bind(username)
        .fetch_one(pool)
        .await
}

async fn insert_session(
    pool: &PostgresPool,
    user_id: i64,
    session_key: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,$2,'preview','preview') RETURNING id",
    )
    .bind(user_id)
    .bind(session_key)
    .fetch_one(pool)
    .await
}

struct ReceiptInsert {
    session_id: i64,
    user_id: i64,
    receipt_schema_version: i16,
    command_fingerprint: String,
    request_session_version: i64,
    response_schema_version: i16,
    http_status: i16,
    success_envelope: serde_json::Value,
}

async fn insert_receipt(pool: &PostgresPool, receipt: ReceiptInsert) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO import_confirm_receipts (
               session_id,user_id,receipt_schema_version,command_fingerprint,
               request_session_version,response_schema_version,http_status,success_envelope
           ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)"#,
    )
    .bind(receipt.session_id)
    .bind(receipt.user_id)
    .bind(receipt.receipt_schema_version)
    .bind(receipt.command_fingerprint)
    .bind(receipt.request_session_version)
    .bind(receipt.response_schema_version)
    .bind(receipt.http_status)
    .bind(receipt.success_envelope)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn confirm_receipt_schema_is_typed_owned_unique_and_cleanup_safe(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_schema").await?;
    let result = async {
        let columns = sqlx::query(
            "SELECT column_name,data_type,is_nullable,column_default FROM information_schema.columns WHERE table_schema='public' AND table_name='import_confirm_receipts' ORDER BY ordinal_position",
        )
        .fetch_all(&isolated.pool)
        .await?;
        let column_names = columns
            .iter()
            .map(|row| row.get::<String, _>("column_name"))
            .collect::<Vec<_>>();
        assert_eq!(
            column_names,
            [
                "session_id",
                "user_id",
                "receipt_schema_version",
                "command_fingerprint",
                "request_session_version",
                "response_schema_version",
                "http_status",
                "success_envelope",
                "created_at",
            ]
        );
        assert!(!column_names.iter().any(|column| column == "updated_at"));
        assert!(columns
            .iter()
            .all(|row| row.get::<String, _>("is_nullable") == "NO"));
        for (column_name, data_type) in [
            ("session_id", "bigint"),
            ("user_id", "bigint"),
            ("receipt_schema_version", "smallint"),
            ("command_fingerprint", "text"),
            ("request_session_version", "bigint"),
            ("response_schema_version", "smallint"),
            ("http_status", "smallint"),
            ("success_envelope", "jsonb"),
            ("created_at", "timestamp with time zone"),
        ] {
            let column = columns
                .iter()
                .find(|row| row.get::<String, _>("column_name") == column_name)
                .expect("receipt column");
            assert_eq!(column.get::<String, _>("data_type"), data_type);
        }
        let created_at = columns
            .iter()
            .find(|row| row.get::<String, _>("column_name") == "created_at")
            .expect("created_at column");
        assert_eq!(
            created_at.get::<Option<String>, _>("column_default"),
            Some("now()".to_string())
        );

        let constraints = sqlx::query(
            "SELECT conname,convalidated FROM pg_constraint WHERE conname = ANY($1::text[]) ORDER BY conname",
        )
        .bind(RECEIPT_CONSTRAINTS)
        .fetch_all(&isolated.pool)
        .await?;
        assert_eq!(constraints.len(), RECEIPT_CONSTRAINTS.len());
        assert!(constraints
            .iter()
            .all(|row| row.get::<bool, _>("convalidated")));

        let initial_rows: i64 =
            sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM import_confirm_receipts")
                .fetch_one(&isolated.pool)
                .await?;
        assert_eq!(initial_rows, 0, "C6a expand must not backfill receipts");

        let owner_id = insert_user(&isolated.pool, "receipt-owner").await?;
        let other_id = insert_user(&isolated.pool, "receipt-other").await?;
        let session_id = insert_session(&isolated.pool, owner_id, "receipt-session").await?;
        let fingerprint = "a".repeat(64);
        let envelope = json!({"success": true, "data": {"imported": 2}});
        insert_receipt(
            &isolated.pool,
            ReceiptInsert {
                session_id,
                user_id: owner_id,
                receipt_schema_version: 1,
                command_fingerprint: fingerprint.clone(),
                request_session_version: 1,
                response_schema_version: 1,
                http_status: 200,
                success_envelope: envelope.clone(),
            },
        )
        .await?;

        let row = sqlx::query(
            "SELECT receipt_schema_version,command_fingerprint,request_session_version,response_schema_version,http_status,success_envelope,created_at FROM import_confirm_receipts WHERE session_id=$1 AND user_id=$2",
        )
        .bind(session_id)
        .bind(owner_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(row.get::<i16, _>("receipt_schema_version"), 1);
        assert_eq!(row.get::<String, _>("command_fingerprint"), fingerprint);
        assert_eq!(row.get::<i64, _>("request_session_version"), 1);
        assert_eq!(row.get::<i16, _>("response_schema_version"), 1);
        assert_eq!(row.get::<i16, _>("http_status"), 200);
        assert_eq!(row.get::<serde_json::Value, _>("success_envelope"), envelope);
        assert!(row
            .try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .is_ok());

        let immutable = sqlx::query(
            "UPDATE import_confirm_receipts SET success_envelope='{}'::jsonb WHERE session_id=$1",
        )
        .bind(session_id)
        .execute(&isolated.pool)
        .await
        .expect_err("terminal receipts must be immutable");
        assert_sqlstate(&immutable, "55000");

        let duplicate = insert_receipt(
            &isolated.pool,
            ReceiptInsert {
                session_id,
                user_id: owner_id,
                receipt_schema_version: 1,
                command_fingerprint: "b".repeat(64),
                request_session_version: 1,
                response_schema_version: 1,
                http_status: 200,
                success_envelope: json!({"success": true}),
            },
        )
        .await
        .expect_err("a session must have exactly one terminal receipt");
        assert_sqlstate(&duplicate, "23505");

        let other_session_id =
            insert_session(&isolated.pool, other_id, "receipt-other-session").await?;
        let cross_user = insert_receipt(
            &isolated.pool,
            ReceiptInsert {
                session_id: other_session_id,
                user_id: owner_id,
                receipt_schema_version: 1,
                command_fingerprint: "c".repeat(64),
                request_session_version: 1,
                response_schema_version: 1,
                http_status: 200,
                success_envelope: json!({"success": true}),
            },
        )
        .await
        .expect_err("receipt ownership must match the session owner");
        assert_sqlstate(&cross_user, "23503");

        sqlx::query("UPDATE import_sessions SET status='confirmed' WHERE id=$1 AND user_id=$2")
            .bind(session_id)
            .bind(owner_id)
            .execute(&isolated.pool)
            .await?;
        let clear_result = clear_session_data(
            &isolated.pool,
            "receipt-session",
            UserId::new(u64::try_from(owner_id)?).expect("positive user id"),
        )?;
        assert_eq!(clear_result.session_count, 0);
        let receipt_rows: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM import_confirm_receipts WHERE session_id=$1",
        )
        .bind(session_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(receipt_rows, 1, "ordinary cleanup must preserve terminal receipt");

        clear_postgres_user_data(
            &isolated.pool,
            UserId::new(u64::try_from(owner_id)?).expect("positive user id"),
        )
        .await?;
        let receipts_after_user_clear: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::BIGINT FROM import_confirm_receipts WHERE user_id=$1",
        )
        .bind(owner_id)
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(
            receipts_after_user_clear, 0,
            "account-level data clear must remove receipts through the session cascade"
        );

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}

#[tokio::test]
async fn confirm_receipt_schema_rejects_unknown_or_malformed_contracts(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_contracts").await?;
    let result = async {
        let user_id = insert_user(&isolated.pool, "receipt-invalid").await?;
        let session_id = insert_session(&isolated.pool, user_id, "receipt-invalid").await?;
        let valid_fingerprint = "d".repeat(64);

        for (
            receipt_schema_version,
            fingerprint,
            request_session_version,
            response_schema_version,
            http_status,
            envelope,
        ) in [
            (2, valid_fingerprint.clone(), 1, 1, 200, json!({})),
            (1, "D".repeat(64), 1, 1, 200, json!({})),
            (1, "e".repeat(63), 1, 1, 200, json!({})),
            (1, valid_fingerprint.clone(), 0, 1, 200, json!({})),
            (1, valid_fingerprint.clone(), 1, 2, 200, json!({})),
            (1, valid_fingerprint.clone(), 1, 1, 201, json!({})),
            (1, valid_fingerprint.clone(), 1, 1, 200, json!([])),
        ] {
            let error = insert_receipt(
                &isolated.pool,
                ReceiptInsert {
                    session_id,
                    user_id,
                    receipt_schema_version,
                    command_fingerprint: fingerprint,
                    request_session_version,
                    response_schema_version,
                    http_status,
                    success_envelope: envelope,
                },
            )
            .await
            .expect_err("invalid receipt contract must fail closed");
            assert_sqlstate(&error, "23514");
        }

        let remaining: i64 =
            sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM import_confirm_receipts")
                .fetch_one(&isolated.pool)
                .await?;
        assert_eq!(remaining, 0);
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
