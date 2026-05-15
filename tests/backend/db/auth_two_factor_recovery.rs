use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    clear_two_factor_recovery_codes, consume_two_factor_recovery_code,
    count_active_two_factor_recovery_codes, disable_two_factor_and_clear_recovery_codes,
    enable_two_factor_with_recovery_codes_and_session, hash_two_factor_recovery_code,
    init_auth_security_schema, replace_two_factor_recovery_codes, CreateTokenSessionDraft,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};

fn runtime_for(path: &std::path::Path) -> Result<SqliteRuntime, Box<dyn Error>> {
    let db_path = SqliteDbPath::temporary_file(path)?;
    Ok(SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: std::time::Duration::from_secs(1),
    })?)
}

fn user_id(value: u64) -> UserId {
    UserId::new(value).expect("positive user id")
}

fn seed_user(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute(
        "INSERT INTO users(
             id, username, email, password_hash, created_at, updated_at
         ) VALUES (42, 'two-factor-user', 'two-factor@example.test', 'hash',
                   '2026-05-09T18:00:00', '2026-05-09T18:00:00')",
        [],
    )?;
    Ok(())
}

#[test]
fn two_factor_recovery_codes_are_hashed_normalized_and_single_use() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("two_factor_recovery.db"))?;
    init_auth_security_schema(runtime.connection())?;
    seed_user(&runtime)?;

    assert_eq!(
        hash_two_factor_recovery_code(" abcd - 1234 "),
        hash_two_factor_recovery_code("A B C D-1 2 3 4")
    );
    assert!(hash_two_factor_recovery_code(" \t\n ").is_none());

    let replaced = replace_two_factor_recovery_codes(
        runtime.connection(),
        user_id(42),
        &["ABCD-1234", "a b c d-1 2 3 4", "EFGH-5678", ""],
        "2026-05-09T18:00:00",
    )?;
    assert_eq!(replaced, 2);
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        2
    );

    assert!(consume_two_factor_recovery_code(
        runtime.connection(),
        user_id(42),
        "abcd-1234",
        "2026-05-09T18:01:00",
    )?);
    assert!(!consume_two_factor_recovery_code(
        runtime.connection(),
        user_id(42),
        "ABCD-1234",
        "2026-05-09T18:02:00",
    )?);
    assert!(!consume_two_factor_recovery_code(
        runtime.connection(),
        user_id(42),
        "NOT-EXIST",
        "2026-05-09T18:02:00",
    )?);
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        1
    );
    Ok(())
}

#[test]
fn two_factor_recovery_code_replacement_clears_previous_batch_and_rolls_back(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("two_factor_replace.db"))?;
    init_auth_security_schema(runtime.connection())?;
    seed_user(&runtime)?;

    assert_eq!(
        replace_two_factor_recovery_codes(
            runtime.connection(),
            user_id(42),
            &["OLD1-1111", "OLD2-2222"],
            "2026-05-09T18:00:00",
        )?,
        2
    );
    assert_eq!(
        replace_two_factor_recovery_codes(
            runtime.connection(),
            user_id(42),
            &["NEW1-3333", "NEW2-4444"],
            "2026-05-09T18:05:00",
        )?,
        2
    );

    assert!(!consume_two_factor_recovery_code(
        runtime.connection(),
        user_id(42),
        "OLD1-1111",
        "2026-05-09T18:10:00",
    )?);
    assert!(consume_two_factor_recovery_code(
        runtime.connection(),
        user_id(42),
        "NEW1-3333",
        "2026-05-09T18:11:00",
    )?);
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        1
    );

    let result = replace_two_factor_recovery_codes(
        runtime.connection(),
        user_id(404),
        &["BAD1-0000"],
        "2026-05-09T18:12:00",
    );
    assert!(result.is_err());
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        1
    );

    assert_eq!(
        clear_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        2
    );
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        0
    );
    Ok(())
}

#[test]
fn two_factor_enable_and_disable_update_user_recovery_codes_and_session_atomically(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("two_factor_enable_disable.db"))?;
    init_auth_security_schema(runtime.connection())?;
    seed_user(&runtime)?;
    runtime.connection().execute(
        "INSERT INTO users(
             id, username, email, password_hash, created_at, updated_at
         ) VALUES (43, 'other-two-factor-user', 'other-2fa@example.test', 'hash',
                   '2026-05-09T18:00:00', '2026-05-09T18:00:00')",
        [],
    )?;

    let session_draft = CreateTokenSessionDraft {
        user_id: user_id(42),
        token_hash: "access-hash".to_string(),
        refresh_token_hash: Some("refresh-hash".to_string()),
        expires_at: "2026-05-10T18:00:00".to_string(),
        refresh_expires_at: Some("2026-05-11T18:00:00".to_string()),
        user_agent: "db-test".to_string(),
        ip_address: "127.0.0.1".to_string(),
        created_at: "2026-05-09T18:00:00".to_string(),
    };
    let mut mismatched_session_draft = session_draft.clone();
    mismatched_session_draft.user_id = user_id(43);
    let mismatch_result = enable_two_factor_with_recovery_codes_and_session(
        runtime.connection(),
        user_id(42),
        "BADSECRET",
        &["BAD1-0000"],
        &mismatched_session_draft,
        "2026-05-09T17:59:00",
    );
    let mismatch_error = mismatch_result.expect_err("mismatched session user is rejected");
    assert!(mismatch_error.to_string().contains("session user mismatch"));
    let (enabled, secret): (i64, String) = runtime.connection().query_row(
        "SELECT two_factor_enabled, COALESCE(two_factor_secret, '') FROM users WHERE id = 42",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(enabled, 0);
    assert_eq!(secret, "");
    let session_count: i64 =
        runtime
            .connection()
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
    assert_eq!(session_count, 0);
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        0
    );

    let (stored_count, session_id) = enable_two_factor_with_recovery_codes_and_session(
        runtime.connection(),
        user_id(42),
        "JBSWY3DPEHPK3PXP",
        &["ABCD-1234", "EFGH-5678"],
        &session_draft,
        "2026-05-09T18:00:00",
    )?;
    assert_eq!(stored_count, 2);
    assert!(session_id > 0);
    let (enabled, secret): (i64, String) = runtime.connection().query_row(
        "SELECT two_factor_enabled, two_factor_secret FROM users WHERE id = 42",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(enabled, 1);
    assert_eq!(secret, "JBSWY3DPEHPK3PXP");
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        2
    );

    let mut second_session_draft = session_draft.clone();
    second_session_draft.token_hash = "second-access-hash".to_string();
    second_session_draft.refresh_token_hash = Some("second-refresh-hash".to_string());
    let already_enabled_result = enable_two_factor_with_recovery_codes_and_session(
        runtime.connection(),
        user_id(42),
        "REPLACEMENTSECRET",
        &["ZZZZ-9999"],
        &second_session_draft,
        "2026-05-09T18:01:00",
    );
    let already_enabled_error =
        already_enabled_result.expect_err("already-enabled user is rejected");
    assert!(already_enabled_error
        .to_string()
        .contains("two-factor authentication is already enabled"));
    let (enabled, secret): (i64, String) = runtime.connection().query_row(
        "SELECT two_factor_enabled, COALESCE(two_factor_secret, '') FROM users WHERE id = 42",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(enabled, 1);
    assert_eq!(secret, "JBSWY3DPEHPK3PXP");
    let session_count: i64 =
        runtime
            .connection()
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;
    assert_eq!(session_count, 1);
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        2
    );
    assert!(!consume_two_factor_recovery_code(
        runtime.connection(),
        user_id(42),
        "ZZZZ-9999",
        "2026-05-09T18:02:00",
    )?);
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        2
    );

    let cleared = disable_two_factor_and_clear_recovery_codes(
        runtime.connection(),
        user_id(42),
        "2026-05-09T18:05:00",
    )?;
    assert_eq!(cleared, 2);
    let (enabled, secret): (i64, String) = runtime.connection().query_row(
        "SELECT two_factor_enabled, two_factor_secret FROM users WHERE id = 42",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(enabled, 0);
    assert_eq!(secret, "");
    assert_eq!(
        count_active_two_factor_recovery_codes(runtime.connection(), user_id(42))?,
        0
    );

    Ok(())
}
