use std::{env, error::Error, io};

use bill_analyser_db::PostgresPool;
use sqlx::Row;

mod postgres_test_support;

const CHECK_CONSTRAINTS: &[&str] = &[
    "chk_import_sessions_status",
    "chk_import_sessions_import_mode",
    "chk_import_preview_rows_operation_kind",
    "chk_import_decision_groups_group_type",
    "chk_import_decision_groups_decision_status",
    "chk_import_confirm_operations_operation_kind",
    "chk_import_confirm_operations_status",
    "chk_import_learning_lifecycle_status",
];

const SESSION_CONSTRAINTS: &[&str] = &["uq_import_sessions_id_user"];

const OWNERSHIP_CONSTRAINTS: &[&str] = &[
    "fk_import_sources_session_user",
    "fk_import_standard_rows_session_user",
    "fk_import_preview_rows_session_user",
    "fk_import_decision_groups_session_user",
    "fk_import_history_materializations_session_user",
    "fk_import_confirm_operations_session_user",
];

const MEMBER_INDEXES: &[&str] = &[
    "uq_import_decision_group_members_preview_role",
    "uq_import_decision_group_members_standard_role",
    "uq_import_decision_group_members_history_role",
];

async fn strict_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    if env::var_os("BILL_ANALYSER_TEST_POSTGRES_URL").is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "BILL_ANALYSER_TEST_POSTGRES_URL is required for import schema constraint tests",
        )
        .into());
    }
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for import schema constraint tests",
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

async fn insert_source(
    pool: &PostgresPool,
    session_id: i64,
    user_id: i64,
    suffix: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO import_sources (session_id,user_id,source_index,parser_id,parser_name,feature_signature) VALUES ($1,$2,$3,'parser','Parser',$4) RETURNING id",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(if suffix == "one" { 1_i32 } else { 2_i32 })
    .bind(format!("feature-{suffix}"))
    .fetch_one(pool)
    .await
}

async fn insert_standard(
    pool: &PostgresPool,
    session_id: i64,
    source_id: i64,
    user_id: i64,
    source_row_index: i32,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO import_standard_rows (session_id,source_id,user_id,source_row_index,occurred_at,amount_cents,direction) VALUES ($1,$2,$3,$4,now(),-100,'expense') RETURNING id",
    )
    .bind(session_id)
    .bind(source_id)
    .bind(user_id)
    .bind(source_row_index)
    .fetch_one(pool)
    .await
}

async fn insert_preview(
    pool: &PostgresPool,
    session_id: i64,
    user_id: i64,
    page_sort_key: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO import_preview_rows (session_id,user_id,page_sort_key,operation_kind,occurred_at,amount_cents,direction) VALUES ($1,$2,$3,'insert',now(),-100,'expense') RETURNING id",
    )
    .bind(session_id)
    .bind(user_id)
    .bind(page_sort_key)
    .fetch_one(pool)
    .await
}

async fn insert_bill(
    pool: &PostgresPool,
    user_id: i64,
    amount_cents: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO bills (user_id,occurred_at,amount_cents,direction) VALUES ($1,now(),$2,'expense') RETURNING id",
    )
    .bind(user_id)
    .bind(amount_cents)
    .fetch_one(pool)
    .await
}

async fn assert_constraints_are_validated(pool: &PostgresPool) -> Result<(), sqlx::Error> {
    let expected = CHECK_CONSTRAINTS
        .iter()
        .chain(SESSION_CONSTRAINTS)
        .chain(OWNERSHIP_CONSTRAINTS)
        .copied()
        .collect::<Vec<_>>();
    let rows = sqlx::query(
        "SELECT conname, convalidated FROM pg_constraint WHERE conname = ANY($1::text[]) ORDER BY conname",
    )
    .bind(&expected)
    .fetch_all(pool)
    .await?;
    assert_eq!(rows.len(), expected.len());
    assert!(rows.iter().all(|row| row.get::<bool, _>("convalidated")));

    for index in MEMBER_INDEXES {
        let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
            .bind(index)
            .fetch_one(pool)
            .await?;
        assert!(exists, "missing partial unique index {index}");
    }
    Ok(())
}

async fn assert_invalid_states_fail_closed(
    pool: &PostgresPool,
    session_id: i64,
    preview_id: i64,
    group_id: i64,
    operation_id: i64,
    lifecycle_id: i64,
) {
    for (sql, id) in [
        (
            "UPDATE import_sessions SET status='invalid' WHERE id=$1",
            session_id,
        ),
        (
            "UPDATE import_sessions SET import_mode='invalid' WHERE id=$1",
            session_id,
        ),
        (
            "UPDATE import_preview_rows SET operation_kind='invalid' WHERE id=$1",
            preview_id,
        ),
        (
            "UPDATE import_decision_groups SET group_type='invalid' WHERE id=$1",
            group_id,
        ),
        (
            "UPDATE import_decision_groups SET decision_status='invalid' WHERE id=$1",
            group_id,
        ),
        (
            "UPDATE import_confirm_operations SET operation_kind='invalid' WHERE id=$1",
            operation_id,
        ),
        (
            "UPDATE import_confirm_operations SET status='invalid' WHERE id=$1",
            operation_id,
        ),
        (
            "UPDATE import_learning_lifecycle SET status='invalid' WHERE id=$1",
            lifecycle_id,
        ),
    ] {
        let error = sqlx::query(sql)
            .bind(id)
            .execute(pool)
            .await
            .expect_err("invalid lifecycle value must fail closed");
        assert_sqlstate(&error, "23514");
    }
}

async fn assert_cross_user_session_links_fail_closed(
    pool: &PostgresPool,
    other_session_id: i64,
    scoped_rows: &[(&str, i64)],
) {
    for &(table, id) in scoped_rows {
        let sql = format!("UPDATE {table} SET session_id=$1 WHERE id=$2");
        let error = sqlx::query(&sql)
            .bind(other_session_id)
            .bind(id)
            .execute(pool)
            .await
            .expect_err("cross-user session ownership must fail closed");
        assert_sqlstate(&error, "23503");
    }
}

#[tokio::test]
async fn real_postgres_import_schema_invariants_fail_closed_without_rejecting_legal_multi_refs(
) -> Result<(), Box<dyn Error>> {
    let database = strict_isolated_postgres_database("import_schema_invariants").await?;
    let pool = &database.pool;

    assert_constraints_are_validated(pool).await?;

    let owner_id = insert_user(pool, "schema-invariant-owner").await?;
    let other_id = insert_user(pool, "schema-invariant-other").await?;
    let session_id = insert_session(pool, owner_id, "schema-owner").await?;
    let other_session_id = insert_session(pool, other_id, "schema-other").await?;
    let source_id = insert_source(pool, session_id, owner_id, "one").await?;
    let source_two_id = insert_source(pool, session_id, owner_id, "two").await?;
    let standard_id = insert_standard(pool, session_id, source_id, owner_id, 1).await?;
    let standard_two_id = insert_standard(pool, session_id, source_two_id, owner_id, 2).await?;
    let preview_id = insert_preview(pool, session_id, owner_id, "one").await?;
    let preview_two_id = insert_preview(pool, session_id, owner_id, "two").await?;
    let history_bill_id = insert_bill(pool, owner_id, -100).await?;
    let history_two_id = insert_bill(pool, owner_id, -200).await?;

    let group_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_decision_groups (session_id,user_id,group_type,group_key,decision_status) VALUES ($1,$2,'duplicate','schema-group','pending') RETURNING id",
    )
    .bind(session_id)
    .bind(owner_id)
    .fetch_one(pool)
    .await?;
    let materialization_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_history_materializations (session_id,user_id,history_bill_id,history_bill_version,rewrite_reason) VALUES ($1,$2,$3,1,'schema invariant') RETURNING id",
    )
    .bind(session_id)
    .bind(owner_id)
    .bind(history_bill_id)
    .fetch_one(pool)
    .await?;
    let operation_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_confirm_operations (session_id,user_id,operation_kind,status) VALUES ($1,$2,'decision_group','pending') RETURNING id",
    )
    .bind(session_id)
    .bind(owner_id)
    .fetch_one(pool)
    .await?;
    let lifecycle_id: i64 = sqlx::query_scalar(
        "INSERT INTO import_learning_lifecycle (user_id,recommendation_key,recommendation_type,status) VALUES ($1,'schema-rule','classification','yellow') RETURNING id",
    )
    .bind(owner_id)
    .fetch_one(pool)
    .await?;

    for status in [
        "green",
        "auto_applied",
        "downgraded",
        "suppressed",
        "pending",
        "accepted",
        "disabled",
    ] {
        sqlx::query(
            "INSERT INTO import_learning_lifecycle (user_id,recommendation_key,recommendation_type,status) VALUES ($1,$2,'classification',$3)",
        )
        .bind(owner_id)
        .bind(format!("schema-rule-{status}"))
        .bind(status)
        .execute(pool)
        .await?;
    }

    sqlx::query(
        "INSERT INTO import_decision_group_members (group_id,preview_row_id,standard_row_id,history_bill_id,member_role) VALUES ($1,$2,$3,$4,'candidate')",
    )
    .bind(group_id)
    .bind(preview_id)
    .bind(standard_id)
    .bind(history_bill_id)
    .execute(pool)
    .await?;

    for (preview, standard, history) in [
        (preview_id, standard_two_id, history_two_id),
        (preview_two_id, standard_id, history_two_id),
        (preview_two_id, standard_two_id, history_bill_id),
    ] {
        let error = sqlx::query(
            "INSERT INTO import_decision_group_members (group_id,preview_row_id,standard_row_id,history_bill_id,member_role) VALUES ($1,$2,$3,$4,'candidate')",
        )
        .bind(group_id)
        .bind(preview)
        .bind(standard)
        .bind(history)
        .execute(pool)
        .await
        .expect_err("each populated member reference must be unique within its role");
        assert_sqlstate(&error, "23505");
    }

    assert_invalid_states_fail_closed(
        pool,
        session_id,
        preview_id,
        group_id,
        operation_id,
        lifecycle_id,
    )
    .await;
    assert_cross_user_session_links_fail_closed(
        pool,
        other_session_id,
        &[
            ("import_sources", source_id),
            ("import_standard_rows", standard_id),
            ("import_preview_rows", preview_id),
            ("import_decision_groups", group_id),
            ("import_history_materializations", materialization_id),
            ("import_confirm_operations", operation_id),
        ],
    )
    .await;

    database.cleanup().await?;
    Ok(())
}
