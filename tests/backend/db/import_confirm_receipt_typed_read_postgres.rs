use std::{env, error::Error, io};

use bill_analyser_core::UserId;
use bill_analyser_db::{
    confirm_import_command, confirm_import_command_with_receipt_read_source, create_import_session,
    ConfirmCommand, ConfirmReceiptReadSource, ImportSessionDraft,
};

mod postgres_test_support;

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for typed confirm receipt read tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for typed confirm receipt read tests",
            )
            .into()
        })
}

#[tokio::test(flavor = "multi_thread")]
async fn explicit_typed_reader_replays_and_missing_projection_fails_without_metadata_fallback(
) -> Result<(), Box<dyn Error>> {
    let isolated = strict_isolated_postgres_database("confirm_receipt_typed_read").await?;
    let result = async {
        let user_id: i64 =
            sqlx::query_scalar("INSERT INTO users (username) VALUES ($1) RETURNING id")
                .bind("confirm-receipt-typed-read")
                .fetch_one(&isolated.pool)
                .await?;
        let scoped_user_id = UserId::new(user_id as u64).expect("positive user id");
        let session_id = "confirm-receipt-typed-read-session";
        create_import_session(
            &isolated.pool,
            &ImportSessionDraft {
                session_id: session_id.to_string(),
                user_id: scoped_user_id,
                file_count: 0,
            },
        )?;
        let command = ConfirmCommand {
            session_id: session_id.to_string(),
            expected_session_version: Some(1),
            preview_patches: Vec::new(),
            selected_preview_ids: Some(Vec::new()),
            preserve_unpatched_selection: false,
            history_acknowledgement: None,
            declared_confirm_time_effects: Vec::new(),
        };

        let first = confirm_import_command(&isolated.pool, scoped_user_id, &command)?;
        assert!(!first.replayed);
        let typed = confirm_import_command_with_receipt_read_source(
            &isolated.pool,
            scoped_user_id,
            &command,
            ConfirmReceiptReadSource::TypedV1,
        )?;
        assert!(typed.replayed);
        assert_eq!(typed.http_status, first.http_status);
        assert_eq!(typed.success_envelope, first.success_envelope);

        sqlx::query("DELETE FROM import_confirm_receipts WHERE user_id = $1")
            .bind(user_id)
            .execute(&isolated.pool)
            .await?;

        let typed_error = confirm_import_command_with_receipt_read_source(
            &isolated.pool,
            scoped_user_id,
            &command,
            ConfirmReceiptReadSource::TypedV1,
        )
        .expect_err("typed mode must fail closed when the projection is missing");
        assert!(typed_error
            .to_string()
            .contains("typed confirm receipt is missing"));

        let metadata_replay = confirm_import_command(&isolated.pool, scoped_user_id, &command)?;
        assert!(metadata_replay.replayed);
        assert_eq!(metadata_replay.success_envelope, first.success_envelope);
        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
