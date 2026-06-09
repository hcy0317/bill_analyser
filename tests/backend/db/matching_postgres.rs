use std::error::Error;

use bill_analyser_core::{
    matching::{MANUAL_PAIR_SOURCE, TRANSFER_PAIR_TYPE},
    UserId,
};
use bill_analyser_db::{
    create_postgres_manual_matching_pair, delete_postgres_manual_matching_pair,
    query_postgres_matching_bill_candidates_payload, query_postgres_matching_bill_feedback_payload,
    query_postgres_matching_pairs_payload, query_postgres_matching_session_candidates_payload,
    query_postgres_reconciliation_candidates_payload, PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

#[tokio::test]
async fn matching_postgres_manual_pairs_feedback_and_unprojected_materializations_round_trip(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("matching_contract").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let fixture = seed_matching_fixture(pool).await?;
    let user_id = UserId::new(fixture.user_id as u64).expect("positive user id");

    let session_candidates = query_postgres_matching_session_candidates_payload(
        pool,
        user_id,
        &fixture.session_id.to_string(),
    )
    .await?
    .expect("existing import session has an empty current projection");
    assert_eq!(
        session_candidates,
        json!({
            "session_id": fixture.session_id.to_string(),
            "summary": {
                "preview_count": 0,
                "candidate_count": 0,
                "counts_by_kind": {},
            },
            "candidates": [],
        })
    );
    assert!(
        query_postgres_matching_session_candidates_payload(pool, user_id, "not-a-session")
            .await?
            .is_none()
    );

    let materialized_candidates =
        query_postgres_reconciliation_candidates_payload(pool, user_id).await?;
    assert_eq!(
        materialized_candidates,
        json!({"candidates": []}),
        "materialized import history rows and suppressions are intentionally not projected before the B007 refactor gate"
    );

    let created = create_postgres_manual_matching_pair(
        pool,
        user_id,
        fixture.right_bill_id,
        fixture.left_bill_id,
        TRANSFER_PAIR_TYPE,
    )
    .await?;
    let pair = &created["pair"];
    let pair_id = pair["id"].as_i64().expect("pair id");
    assert_eq!(pair["pairType"], TRANSFER_PAIR_TYPE);
    assert_eq!(pair["source"], MANUAL_PAIR_SOURCE);
    assert_eq!(pair["leftBillId"], fixture.left_bill_id);
    assert_eq!(pair["rightBillId"], fixture.right_bill_id);

    let conflict = create_postgres_manual_matching_pair(
        pool,
        user_id,
        fixture.left_bill_id,
        fixture.right_bill_id,
        TRANSFER_PAIR_TYPE,
    )
    .await
    .expect_err("active pair prevents duplicate manual links");
    assert_eq!(conflict.status_code(), 409);

    let linked_payload =
        query_postgres_matching_bill_candidates_payload(pool, user_id, fixture.left_bill_id)
            .await?
            .expect("bill exists");
    assert_eq!(linked_payload["billId"], fixture.left_bill_id);
    assert_eq!(linked_payload["linkedPair"]["id"], pair_id);
    assert_eq!(
        linked_payload["linkedPair"]["otherBillId"],
        fixture.right_bill_id
    );
    assert_eq!(linked_payload["candidates"], json!([]));
    assert_eq!(linked_payload["reconciliation"], Value::Null);

    let pairs = query_postgres_matching_pairs_payload(pool, user_id).await?;
    let listed_pair = pairs["pairs"]
        .as_array()
        .expect("pairs array")
        .first()
        .expect("manual pair listed");
    assert_eq!(listed_pair["id"], pair_id);
    assert_eq!(listed_pair["leftBill"]["amount"], 15.75);
    assert_eq!(listed_pair["leftBill"]["mainCategory"], "转账");
    assert_eq!(listed_pair["leftBill"]["subCategory"], "转出");
    assert_eq!(
        listed_pair["leftBill"]["sourceAccountId"],
        fixture.wallet_account_id
    );
    assert_eq!(
        listed_pair["leftBill"]["destinationAccountId"],
        fixture.bank_account_id
    );
    assert_eq!(listed_pair["rightBill"]["amount"], 15.75);

    insert_feedback_events(pool, &fixture, pair_id).await?;
    let feedback =
        query_postgres_matching_bill_feedback_payload(pool, user_id, fixture.left_bill_id)
            .await?
            .expect("feedback bill exists");
    let events = feedback["events"].as_array().expect("feedback events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["action"], "reject");
    assert_eq!(events[0]["candidateId"], "bill:transfer:later");
    assert_eq!(events[1]["action"], "accept");
    assert_eq!(events[1]["candidateId"], "bill:transfer:first");

    let deleted = delete_postgres_manual_matching_pair(pool, user_id, pair_id).await?;
    assert_eq!(deleted["pair"]["id"], pair_id);
    assert_eq!(
        query_postgres_matching_pairs_payload(pool, user_id).await?,
        json!({"pairs": []})
    );
    let unlinked =
        query_postgres_matching_bill_candidates_payload(pool, user_id, fixture.left_bill_id)
            .await?
            .expect("bill still exists");
    assert_eq!(unlinked["linkedPair"], Value::Null);

    test_db.cleanup().await?;
    Ok(())
}

struct MatchingFixture {
    user_id: i64,
    wallet_account_id: i64,
    bank_account_id: i64,
    left_bill_id: i64,
    right_bill_id: i64,
    session_id: i64,
}

async fn seed_matching_fixture(pool: &PostgresPool) -> Result<MatchingFixture, Box<dyn Error>> {
    let user_id = insert_user(pool, "matching-contract").await?;
    let wallet_account_id = insert_account(pool, user_id, "现金钱包").await?;
    let bank_account_id = insert_account(pool, user_id, "银行卡").await?;
    let category_id = insert_category(pool, user_id, "转账/转出").await?;
    let left_bill_id = insert_bill(
        pool,
        user_id,
        "2026-04-02T09:00:00Z",
        "expense",
        "transfer",
        1575,
        wallet_account_id,
        Some(bank_account_id),
        category_id,
        "转账转出",
        json!({"main_category": "转账", "sub_category": "转出"}),
    )
    .await?;
    let right_bill_id = insert_bill(
        pool,
        user_id,
        "2026-04-02T09:01:00Z",
        "income",
        "transfer",
        1575,
        bank_account_id,
        Some(wallet_account_id),
        category_id,
        "转账转入",
        json!({"main_category": "转账", "sub_category": "转入"}),
    )
    .await?;
    let session_id = insert_import_session(pool, user_id).await?;
    insert_materialized_candidate(pool, user_id, session_id, left_bill_id).await?;
    insert_suppression(pool, user_id, left_bill_id, right_bill_id).await?;

    Ok(MatchingFixture {
        user_id,
        wallet_account_id,
        bank_account_id,
        left_bill_id,
        right_bill_id,
        session_id,
    })
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
    path: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query("INSERT INTO categories (user_id, name, category_type, path) VALUES ($1, $2, 'expense', $3) RETURNING id")
            .bind(user_id)
            .bind(path.rsplit('/').next().unwrap_or(path))
            .bind(path)
            .fetch_one(pool)
            .await?
            .try_get("id")?,
    )
}

#[allow(clippy::too_many_arguments)]
async fn insert_bill(
    pool: &PostgresPool,
    user_id: i64,
    occurred_at: &str,
    direction: &str,
    transaction_type: &str,
    amount_cents: i64,
    account_id: i64,
    target_account_id: Option<i64>,
    category_id: i64,
    merchant: &str,
    standard_payload: Value,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, target_account_id, transfer_target_account_id,
            category_id, merchant, payment_method, description, source_hash, standard_payload
        )
        VALUES ($1, $2::timestamptz, $3, $4, $5, $6, $6, $7, $7, $8, $9, '现金', $10, $11, $12)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(occurred_at)
    .bind(direction)
    .bind(transaction_type)
    .bind(amount_cents)
    .bind(account_id)
    .bind(target_account_id)
    .bind(category_id)
    .bind(merchant)
    .bind(format!("{merchant} 合同测试"))
    .bind(format!("matching-{user_id}-{merchant}-{amount_cents}"))
    .bind(standard_payload)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_import_session(pool: &PostgresPool, user_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO import_sessions (user_id, session_key, status) VALUES ($1, $2, 'preview') RETURNING id",
    )
    .bind(user_id)
    .bind(format!("matching-session-{user_id}"))
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn insert_materialized_candidate(
    pool: &PostgresPool,
    user_id: i64,
    session_id: i64,
    bill_id: i64,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        INSERT INTO import_history_materializations (
            session_id, user_id, history_bill_id, history_bill_version, materialized_payload, rewrite_reason
        )
        VALUES ($1, $2, $3, 1, $4, 'matching-contract')
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(bill_id)
    .bind(json!({"candidate": "currently-unprojected"}))
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_suppression(
    pool: &PostgresPool,
    user_id: i64,
    left_bill_id: i64,
    right_bill_id: i64,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        INSERT INTO matching_suppressions (
            user_id, left_bill_id, right_bill_id, suppression_key, pair_type, reason
        )
        VALUES ($1, $2, $3, $4, $5, 'matching-contract')
        "#,
    )
    .bind(user_id)
    .bind(left_bill_id)
    .bind(right_bill_id)
    .bind(format!("{left_bill_id}:{right_bill_id}:transfer"))
    .bind(TRANSFER_PAIR_TYPE)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_feedback_events(
    pool: &PostgresPool,
    fixture: &MatchingFixture,
    pair_id: i64,
) -> Result<(), Box<dyn Error>> {
    for (event_type, created_at, payload) in [
        (
            "accept",
            "2026-04-02T10:00:00Z",
            json!({
                "billId": fixture.left_bill_id,
                "candidateBillId": fixture.right_bill_id,
                "candidateId": "bill:transfer:first",
            }),
        ),
        (
            "reject",
            "2026-04-02T11:00:00Z",
            json!({
                "leftBillId": fixture.left_bill_id,
                "rightBillId": fixture.right_bill_id,
                "candidateId": "bill:transfer:later",
            }),
        ),
        (
            "accept",
            "2026-04-02T12:00:00Z",
            json!({
                "billId": fixture.right_bill_id + 1000,
                "candidateId": "bill:transfer:unrelated",
            }),
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO matching_feedback_events (
                user_id, matching_pair_id, event_type, actor, payload, created_at
            )
            VALUES ($1, $2, $3, 'user', $4, $5::timestamptz)
            "#,
        )
        .bind(fixture.user_id)
        .bind(pair_id)
        .bind(event_type)
        .bind(payload)
        .bind(created_at)
        .execute(pool)
        .await?;
    }
    Ok(())
}
