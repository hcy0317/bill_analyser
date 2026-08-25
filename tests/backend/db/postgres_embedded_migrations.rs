use std::{
    error::Error,
    time::{Duration, Instant},
};

use bill_analyser_db::{
    embedded_postgres_migration_versions, postgres_migrations_dir, run_postgres_migrations,
};
use sqlx::Executor;

mod postgres_test_support;

#[test]
fn embedded_migrations_cover_every_declared_postgres_migration() {
    let embedded_versions = embedded_postgres_migration_versions();
    let mut declared_versions = std::fs::read_dir(postgres_migrations_dir())
        .expect("migration directory")
        .map(|entry| {
            let file_name = entry
                .expect("migration entry")
                .file_name()
                .into_string()
                .expect("UTF-8 migration filename");
            file_name
                .split_once('_')
                .expect("versioned migration filename")
                .0
                .parse::<i64>()
                .expect("numeric migration version")
        })
        .collect::<Vec<_>>();
    declared_versions.sort_unstable();

    assert_eq!(embedded_versions, declared_versions);
    assert_eq!(embedded_versions.first(), Some(&1));
    assert_eq!(embedded_versions.last(), Some(&40));
}

#[tokio::test]
async fn migrated_bills_foreign_keys_all_have_leading_indexes() -> Result<(), Box<dyn Error>> {
    let Some(database) =
        postgres_test_support::isolated_postgres_database("bills_foreign_key_indexes").await?
    else {
        return Ok(());
    };

    let indexed_foreign_keys: Vec<(String, String, bool)> = sqlx::query_as(
        r#"
        SELECT c.conrelid::regclass::text,
               a.attname,
               EXISTS (
                   SELECT 1
                   FROM pg_index i
                   WHERE i.indrelid = c.conrelid
                     AND i.indisvalid
                     AND i.indisready
                     AND i.indkey[0] = c.conkey[1]
               )
        FROM pg_constraint c
        JOIN pg_attribute a
          ON a.attrelid = c.conrelid
         AND a.attnum = c.conkey[1]
        WHERE c.contype = 'f'
          AND c.confrelid = 'bills'::regclass
          AND cardinality(c.conkey) = 1
        ORDER BY c.conrelid::regclass::text, a.attname
        "#,
    )
    .fetch_all(&database.pool)
    .await?;

    assert_eq!(indexed_foreign_keys.len(), 12);
    let missing = indexed_foreign_keys
        .into_iter()
        .filter_map(|(table, column, indexed)| (!indexed).then_some(format!("{table}.{column}")))
        .collect::<Vec<_>>();
    assert!(
        missing.is_empty(),
        "unindexed bills foreign keys: {missing:?}"
    );

    database.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn invalid_concurrent_index_retry_fails_closed_and_releases_migration_lock(
) -> Result<(), Box<dyn Error>> {
    let Some(database) =
        postgres_test_support::isolated_postgres_database("invalid_fk_index_retry").await?
    else {
        return Ok(());
    };

    sqlx::query(
        r#"
        UPDATE pg_index
        SET indisvalid = FALSE,
            indisready = FALSE
        WHERE indexrelid = 'idx_import_preview_rows_history_bill_id'::regclass
        "#,
    )
    .execute(&database.pool)
    .await?;
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 30")
        .execute(&database.pool)
        .await?;

    let first_attempt = run_postgres_migrations(&database.pool).await;
    assert!(
        first_attempt.is_err(),
        "same-name invalid index must not be recorded as a successful migration"
    );
    let recorded: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = 30)")
            .fetch_one(&database.pool)
            .await?;
    assert!(!recorded, "failed migration must remain pending");

    let advisory_locks: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_locks l
        JOIN pg_stat_activity a ON a.pid = l.pid
        WHERE l.locktype = 'advisory'
          AND a.datname = current_database()
        "#,
    )
    .fetch_one(&database.pool)
    .await?;
    assert_eq!(
        advisory_locks, 0,
        "migration failure must close the session holding SQLx's advisory lock"
    );

    database
        .pool
        .execute("DROP INDEX CONCURRENTLY idx_import_preview_rows_history_bill_id")
        .await?;
    let (first_retry, second_retry) = tokio::join!(
        run_postgres_migrations(&database.pool),
        run_postgres_migrations(&database.pool)
    );
    first_retry?;
    second_retry?;

    let recovered: bool = sqlx::query_scalar(
        r#"
        SELECT i.indisvalid AND i.indisready AND i.indpred IS NULL
        FROM pg_index i
        WHERE i.indexrelid = 'idx_import_preview_rows_history_bill_id'::regclass
          AND i.indrelid = 'import_preview_rows'::regclass
          AND i.indkey[0] = (
              SELECT attnum
              FROM pg_attribute
              WHERE attrelid = 'import_preview_rows'::regclass
                AND attname = 'history_bill_id'
          )
        "#,
    )
    .fetch_one(&database.pool)
    .await?;
    assert!(recovered, "retry must build a ready, valid leading index");

    database.cleanup().await?;
    Ok(())
}

async fn migration_advisory_lock_count(
    pool: &bill_analyser_db::PostgresPool,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_locks l
        JOIN pg_stat_activity a ON a.pid = l.pid
        WHERE l.locktype = 'advisory'
          AND a.datname = current_database()
        "#,
    )
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn cancelled_migration_future_closes_the_advisory_lock_session() -> Result<(), Box<dyn Error>>
{
    let Some(database) =
        postgres_test_support::isolated_postgres_database("cancelled_migration_lock").await?
    else {
        return Ok(());
    };

    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 30")
        .execute(&database.pool)
        .await?;
    database
        .pool
        .execute("DROP INDEX CONCURRENTLY idx_import_preview_rows_history_bill_id")
        .await?;

    let mut blocker = database.pool.begin().await?;
    sqlx::query("LOCK TABLE import_preview_rows IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await?;

    let migration_pool = database.pool.clone();
    let migration = tokio::spawn(async move { run_postgres_migrations(&migration_pool).await });
    let wait_started = Instant::now();
    while migration_advisory_lock_count(&database.pool).await? == 0 {
        assert!(
            wait_started.elapsed() < Duration::from_secs(5),
            "migration must acquire SQLx's advisory lock before cancellation"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    migration.abort();
    assert!(migration
        .await
        .expect_err("migration task must be cancelled")
        .is_cancelled());
    blocker.rollback().await?;

    let release_started = Instant::now();
    let remaining_locks = loop {
        let count = migration_advisory_lock_count(&database.pool).await?;
        if count == 0 || release_started.elapsed() >= Duration::from_secs(5) {
            break count;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    assert_eq!(
        remaining_locks, 0,
        "cancelling migration must close the pooled session that owns the advisory lock"
    );

    database
        .pool
        .execute("DROP INDEX CONCURRENTLY IF EXISTS idx_import_preview_rows_history_bill_id")
        .await?;
    run_postgres_migrations(&database.pool).await?;
    database.cleanup().await?;
    Ok(())
}
