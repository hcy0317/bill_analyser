use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    clear_two_factor_recovery_codes, consume_two_factor_recovery_code,
    count_active_two_factor_recovery_codes, hash_two_factor_recovery_code,
    init_auth_security_schema, replace_two_factor_recovery_codes, SqliteConnectionConfig,
    SqliteDbPath, SqliteRuntime,
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
