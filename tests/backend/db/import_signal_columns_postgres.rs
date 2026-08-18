use std::{env, error::Error, io};

use bill_analyser_db::PostgresPool;
use sqlx::Row;

mod postgres_test_support;

const SIGNAL_COLUMNS: &[&str] = &[
    "signal_parser",
    "signal_platform_duplicate",
    "signal_transfer",
    "signal_history",
    "signal_learning",
    "signal_llm",
];

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal column tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for import signal column tests",
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

async fn insert_preview_row(pool: &PostgresPool) -> Result<i64, sqlx::Error> {
    let user_id: i64 =
        sqlx::query_scalar("INSERT INTO users (username) VALUES ('signal-expand') RETURNING id")
            .fetch_one(pool)
            .await?;
    let session_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_sessions (user_id,session_key,status,import_mode) VALUES ($1,'signal-expand','preview','preview') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    sqlx::query_scalar(
        "INSERT INTO import_preview_rows (session_id,user_id,page_sort_key,operation_kind,occurred_at,amount_cents,direction) VALUES ($1,$2,'signal-expand','insert',now(),-100,'expense') RETURNING id",
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn signal_projection_expand_is_nullable_unindexed_and_versioned() -> Result<(), Box<dyn Error>>
{
    let isolated = strict_isolated_postgres_database("import_signal_columns").await?;
    let result = async {
        let columns = sqlx::query(
            "SELECT column_name,data_type,is_nullable,column_default FROM information_schema.columns WHERE table_schema='public' AND table_name='import_preview_rows' AND column_name = ANY($1::text[]) ORDER BY column_name",
        )
        .bind(
            SIGNAL_COLUMNS
                .iter()
                .copied()
                .chain(std::iter::once("signal_projection_version"))
                .collect::<Vec<_>>(),
        )
        .fetch_all(&isolated.pool)
        .await?;
        assert_eq!(columns.len(), SIGNAL_COLUMNS.len() + 1);

        for row in &columns {
            let column_name: &str = row.get("column_name");
            let data_type: &str = row.get("data_type");
            let is_nullable: &str = row.get("is_nullable");
            let column_default: Option<&str> = row.get("column_default");
            if column_name == "signal_projection_version" {
                assert_eq!(data_type, "smallint");
                assert_eq!(is_nullable, "NO");
                assert_eq!(column_default, Some("0"));
            } else {
                assert_eq!(data_type, "boolean");
                assert_eq!(is_nullable, "YES");
                assert_eq!(column_default, None);
            }
        }

        let signal_indexes: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_indexes WHERE schemaname='public' AND tablename='import_preview_rows' AND indexdef ~ 'signal_(parser|platform_duplicate|transfer|history|learning|llm)'",
        )
        .fetch_one(&isolated.pool)
        .await?;
        assert_eq!(signal_indexes, 0, "C5c must not add signal-specific indexes");

        let preview_id = insert_preview_row(&isolated.pool).await?;
        let row = sqlx::query(
            "SELECT signal_parser,signal_platform_duplicate,signal_transfer,signal_history,signal_learning,signal_llm,signal_projection_version FROM import_preview_rows WHERE id=$1",
        )
        .bind(preview_id)
        .fetch_one(&isolated.pool)
        .await?;
        for column in SIGNAL_COLUMNS {
            assert_eq!(row.get::<Option<bool>, _>(*column), None);
        }
        assert_eq!(row.get::<i16, _>("signal_projection_version"), 0);

        sqlx::query(
            "UPDATE import_preview_rows SET signal_parser=true,signal_platform_duplicate=false,signal_transfer=true,signal_history=false,signal_learning=true,signal_llm=false,signal_projection_version=1 WHERE id=$1",
        )
        .bind(preview_id)
        .execute(&isolated.pool)
        .await?;

        let invalid_version = sqlx::query(
            "UPDATE import_preview_rows SET signal_projection_version=2 WHERE id=$1",
        )
        .bind(preview_id)
        .execute(&isolated.pool)
        .await
        .expect_err("unknown projection versions must fail closed");
        assert_sqlstate(&invalid_version, "23514");

        Ok::<(), Box<dyn Error>>(())
    }
    .await;
    isolated.cleanup().await?;
    result
}
