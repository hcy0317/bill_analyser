use std::error::Error;

use bill_analyser_db::{
    batch_create_postgres_bills, create_postgres_bill, get_postgres_bill_by_id,
    get_postgres_bill_tags, query_postgres_bills, BillCategoryFilter, BillCreateDraft, BillFilters,
    PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

#[tokio::test]
async fn bills_postgres_queries_preserve_cents_filters_paging_and_user_scope(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) = postgres_test_support::isolated_postgres_database("bills_contract").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let fixture = seed_bills_fixture(pool).await?;

    let page = query_postgres_bills(pool, fixture.user_id, 1, 2, &BillFilters::default()).await?;
    assert_eq!(
        page.total, 3,
        "only current user's non-deleted bills are counted"
    );
    assert_eq!(page.bills.len(), 2, "page_size bounds the first page");
    assert_eq!(page.bills[0]["id"], fixture.transfer_bill_id);
    assert_eq!(page.bills[0]["amount_cents"], 20000);
    assert_eq!(page.bills[0]["destination_amount_cents"], 20150);
    assert_eq!(
        page.bills[0]["source_account_id"],
        fixture.wallet_account_id
    );
    assert_eq!(
        page.bills[0]["destination_account_id"],
        fixture.bank_account_id
    );
    assert_eq!(page.bills[1]["id"], fixture.coffee_bill_id);

    let second_page =
        query_postgres_bills(pool, fixture.user_id, 2, 1, &BillFilters::default()).await?;
    assert_eq!(second_page.bills[0]["id"], fixture.coffee_bill_id);

    let filtered = query_postgres_bills(
        pool,
        fixture.user_id,
        1,
        10,
        &BillFilters {
            account_ids: vec![fixture.wallet_account_id, -1, fixture.wallet_account_id],
            categories: vec![BillCategoryFilter {
                main: "餐饮".to_string(),
                sub: Some("咖啡".to_string()),
            }],
            tag_ids: vec![fixture.coffee_tag_id, 0],
            amount_filter_cents: Some("between:12000:13000".to_string()),
            keyword: Some("Cafe".to_string()),
            ..BillFilters::default()
        },
    )
    .await?;
    assert_eq!(filtered.total, 1);
    let coffee = filtered.bills.first().expect("coffee bill");
    assert_eq!(coffee["id"], fixture.coffee_bill_id);
    assert_eq!(coffee["amount_cents"], 12345);
    assert_eq!(coffee["destination_amount_cents"], 0);
    assert_eq!(coffee["main_category"], "餐饮");
    assert_eq!(coffee["sub_category"], "咖啡");
    assert_eq!(
        coffee["category_id"],
        fixture.coffee_category_id.to_string()
    );

    let gte_filtered = query_postgres_bills(
        pool,
        fixture.user_id,
        1,
        10,
        &BillFilters {
            amount_filter_cents: Some("gte:20000".to_string()),
            ..BillFilters::default()
        },
    )
    .await?;
    assert_eq!(gte_filtered.total, 1);
    assert_eq!(gte_filtered.bills[0]["id"], fixture.transfer_bill_id);

    assert!(
        get_postgres_bill_by_id(pool, fixture.user_id, fixture.deleted_bill_id)
            .await?
            .is_none(),
        "deleted bills are not projected"
    );
    assert!(
        get_postgres_bill_by_id(pool, fixture.user_id, fixture.other_user_bill_id)
            .await?
            .is_none(),
        "other users' bills are not projected"
    );
    let tags = get_postgres_bill_tags(pool, fixture.user_id, fixture.coffee_bill_id).await?;
    assert_eq!(
        tags,
        vec![
            json!({"id": fixture.coffee_tag_id.to_string(), "name": "咖啡标签", "color": "#663300", "icon": "coffee"})
        ]
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn create_bill_prefers_explicit_category_id_over_same_name_path() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_category_id_contract").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-category-id-contract").await?;
    let account_id = insert_account(pool, user_id, "现金钱包").await?;
    let explicit_category_id = insert_category(pool, user_id, "理财收益", "理财/理财收益").await?;
    let path_category_id = insert_category(pool, user_id, "咖啡", "餐饮/咖啡").await?;

    let bill_id = create_postgres_bill(
        pool,
        user_id,
        &BillCreateDraft {
            fields: serde_json::Map::from_iter([
                ("date".to_string(), json!("2026-05-01 09:00:00")),
                ("type".to_string(), json!("支出")),
                ("amount_cents".to_string(), json!(1234)),
                ("source_account_id".to_string(), json!(account_id)),
                ("category_id".to_string(), json!(explicit_category_id)),
                ("main_category".to_string(), json!("餐饮")),
                ("sub_category".to_string(), json!("咖啡")),
                ("counterparty".to_string(), json!("显式分类")),
                ("description".to_string(), json!("category_id must win")),
                ("payment_method".to_string(), json!("现金")),
            ]),
            tag_ids: Vec::new(),
        },
    )
    .await?;

    let bill = get_postgres_bill_by_id(pool, user_id, bill_id)
        .await?
        .expect("created bill");
    assert_eq!(bill["category_id"], explicit_category_id.to_string());
    assert_ne!(bill["category_id"], path_category_id.to_string());

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn create_bill_persists_camel_category_id_as_canonical_identity() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_camel_category_id_contract")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-camel-category-id-contract").await?;
    let account_id = insert_account(pool, user_id, "现金钱包").await?;
    let category_id = insert_category(pool, user_id, "咖啡", "餐饮/咖啡").await?;

    let bill_id = create_postgres_bill(
        pool,
        user_id,
        &BillCreateDraft {
            fields: serde_json::Map::from_iter([
                ("date".to_string(), json!("2026-05-01 09:00:00")),
                ("type".to_string(), json!("支出")),
                ("amount_cents".to_string(), json!(1234)),
                ("source_account_id".to_string(), json!(account_id)),
                ("categoryId".to_string(), json!(category_id)),
                ("counterparty".to_string(), json!("camel 分类")),
                ("description".to_string(), json!("categoryId must persist")),
            ]),
            tag_ids: Vec::new(),
        },
    )
    .await?;

    let bill = get_postgres_bill_by_id(pool, user_id, bill_id)
        .await?
        .expect("created bill");
    assert_eq!(bill["category_id"], category_id.to_string());

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn batch_create_rolls_back_all_bills_when_any_identity_is_invalid(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_batch_identity_contract").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-batch-identity-contract").await?;
    let account_id = insert_account(pool, user_id, "现金钱包").await?;
    let category_id = insert_category(pool, user_id, "咖啡", "餐饮/咖啡").await?;

    let valid = BillCreateDraft {
        fields: serde_json::Map::from_iter([
            ("date".to_string(), json!("2026-05-01 09:00:00")),
            ("type".to_string(), json!("支出")),
            ("amount_cents".to_string(), json!(1234)),
            ("source_account_id".to_string(), json!(account_id)),
            ("category_id".to_string(), json!(category_id)),
            ("counterparty".to_string(), json!("有效账单")),
            ("description".to_string(), json!("must rollback")),
        ]),
        tag_ids: Vec::new(),
    };
    let invalid = BillCreateDraft {
        fields: serde_json::Map::from_iter([
            ("date".to_string(), json!("2026-05-02 09:00:00")),
            ("type".to_string(), json!("支出")),
            ("amount_cents".to_string(), json!(5678)),
            ("source_account_id".to_string(), json!(account_id)),
            ("category_id".to_string(), json!(9_999_999_i64)),
            ("counterparty".to_string(), json!("无效分类")),
            ("description".to_string(), json!("must fail")),
        ]),
        tag_ids: Vec::new(),
    };

    let error = batch_create_postgres_bills(pool, user_id, &[valid, invalid])
        .await
        .expect_err("invalid category fails the batch");
    assert!(
        error.to_string().contains("category not found"),
        "unexpected error: {error}"
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    assert_eq!(count, 0, "valid row must be rolled back with invalid row");

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn create_bill_rejects_invalid_account_and_category_identity() -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_invalid_identity_contract").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-invalid-identity-contract").await?;
    let account_id = insert_account(pool, user_id, "现金钱包").await?;
    let income_category_id =
        insert_category_with_type(pool, user_id, "工资", "income", "收入/工资").await?;

    let invalid_source = create_postgres_bill(
        pool,
        user_id,
        &BillCreateDraft {
            fields: serde_json::Map::from_iter([
                ("date".to_string(), json!("2026-05-01 09:00:00")),
                ("type".to_string(), json!("支出")),
                ("amount_cents".to_string(), json!(1234)),
                ("source_account_id".to_string(), json!(9_999_999_i64)),
                ("counterparty".to_string(), json!("无效账户")),
                ("description".to_string(), json!("must fail")),
            ]),
            tag_ids: Vec::new(),
        },
    )
    .await
    .expect_err("invalid source account is rejected");
    assert!(
        invalid_source
            .to_string()
            .contains("source_account_id not found"),
        "unexpected error: {invalid_source}"
    );

    let same_transfer_accounts = create_postgres_bill(
        pool,
        user_id,
        &BillCreateDraft {
            fields: serde_json::Map::from_iter([
                ("date".to_string(), json!("2026-05-01 10:00:00")),
                ("type".to_string(), json!("转账")),
                ("amount_cents".to_string(), json!(1234)),
                ("source_account_id".to_string(), json!(account_id)),
                ("destination_account_id".to_string(), json!(account_id)),
                ("counterparty".to_string(), json!("同账户转账")),
                ("description".to_string(), json!("must fail")),
            ]),
            tag_ids: Vec::new(),
        },
    )
    .await
    .expect_err("same transfer accounts are rejected");
    assert!(
        same_transfer_accounts
            .to_string()
            .contains("source and destination accounts must differ"),
        "unexpected error: {same_transfer_accounts}"
    );

    let category_mismatch = create_postgres_bill(
        pool,
        user_id,
        &BillCreateDraft {
            fields: serde_json::Map::from_iter([
                ("date".to_string(), json!("2026-05-01 11:00:00")),
                ("type".to_string(), json!("支出")),
                ("amount_cents".to_string(), json!(1234)),
                ("source_account_id".to_string(), json!(account_id)),
                ("category_id".to_string(), json!(income_category_id)),
                ("counterparty".to_string(), json!("分类类型错误")),
                ("description".to_string(), json!("must fail")),
            ]),
            tag_ids: Vec::new(),
        },
    )
    .await
    .expect_err("category type mismatch is rejected");
    assert!(
        category_mismatch
            .to_string()
            .contains("category type mismatch"),
        "unexpected error: {category_mismatch}"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    assert_eq!(count, 0);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn create_bill_drops_destination_account_for_non_transfer_types() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_non_transfer_destination_contract")
            .await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-non-transfer-destination-contract").await?;
    let other_user_id = insert_user(pool, "bill-non-transfer-destination-other").await?;
    let account_id = insert_account(pool, user_id, "现金钱包").await?;
    let other_account_id = insert_account(pool, other_user_id, "别人账户").await?;
    let category_id = insert_category(pool, user_id, "咖啡", "餐饮/咖啡").await?;

    let bill_id = create_postgres_bill(
        pool,
        user_id,
        &BillCreateDraft {
            fields: serde_json::Map::from_iter([
                ("date".to_string(), json!("2026-05-01 12:00:00")),
                ("type".to_string(), json!("支出")),
                ("amount_cents".to_string(), json!(1234)),
                ("source_account_id".to_string(), json!(account_id)),
                (
                    "destination_account_id".to_string(),
                    json!(other_account_id),
                ),
                ("destinationAccountId".to_string(), json!(other_account_id)),
                ("category_id".to_string(), json!(category_id)),
                ("counterparty".to_string(), json!("普通支出")),
                (
                    "description".to_string(),
                    json!("destination account must be dropped"),
                ),
            ]),
            tag_ids: Vec::new(),
        },
    )
    .await?;

    let row = sqlx::query(
        "SELECT transfer_target_account_id, standard_payload FROM bills WHERE id = $1 AND user_id = $2",
    )
    .bind(bill_id)
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let transfer_target_account_id: Option<i64> = row.try_get("transfer_target_account_id")?;
    let standard_payload: Value = row.try_get("standard_payload")?;
    assert_eq!(transfer_target_account_id, None);
    assert!(standard_payload.get("destination_account_id").is_none());
    assert!(standard_payload.get("destinationAccountId").is_none());

    test_db.cleanup().await?;
    Ok(())
}

struct BillsFixture {
    user_id: i64,
    wallet_account_id: i64,
    bank_account_id: i64,
    coffee_category_id: i64,
    coffee_tag_id: i64,
    coffee_bill_id: i64,
    transfer_bill_id: i64,
    deleted_bill_id: i64,
    other_user_bill_id: i64,
}

async fn seed_bills_fixture(pool: &PostgresPool) -> Result<BillsFixture, Box<dyn Error>> {
    let user_id = insert_user(pool, "bills-contract").await?;
    let other_user_id = insert_user(pool, "bills-contract-other").await?;
    let wallet_account_id = insert_account(pool, user_id, "现金钱包").await?;
    let bank_account_id = insert_account(pool, user_id, "银行卡").await?;
    let coffee_category_id = insert_category(pool, user_id, "咖啡", "餐饮/咖啡").await?;
    let transfer_category_id = insert_category(pool, user_id, "转账", "转账").await?;
    let coffee_tag_id = insert_tag(pool, user_id, "咖啡标签").await?;

    let older_bill_id = insert_bill(
        pool,
        user_id,
        "2026-04-01T09:00:00Z",
        "expense",
        "expense",
        5000,
        wallet_account_id,
        None,
        coffee_category_id,
        "Older Cafe",
        "旧咖啡",
        json!({"main_category": "餐饮", "sub_category": "咖啡"}),
        false,
    )
    .await?;
    let coffee_bill_id = insert_bill(
        pool,
        user_id,
        "2026-04-02T09:00:00Z",
        "expense",
        "expense",
        12345,
        wallet_account_id,
        None,
        coffee_category_id,
        "Cafe Contract",
        "B007 咖啡账单",
        json!({"main_category": "餐饮", "sub_category": "咖啡"}),
        false,
    )
    .await?;
    link_tag(pool, user_id, coffee_bill_id, coffee_tag_id).await?;
    let transfer_bill_id = insert_bill(
        pool,
        user_id,
        "2026-04-03T09:00:00Z",
        "expense",
        "transfer",
        20000,
        wallet_account_id,
        Some(bank_account_id),
        transfer_category_id,
        "Bank Transfer",
        "内部转账",
        json!({"destination_amount_cents": 20150}),
        false,
    )
    .await?;
    let deleted_bill_id = insert_bill(
        pool,
        user_id,
        "2026-04-04T09:00:00Z",
        "expense",
        "expense",
        99999,
        wallet_account_id,
        None,
        coffee_category_id,
        "Deleted Cafe",
        "已删除",
        json!({}),
        true,
    )
    .await?;
    let other_account_id = insert_account(pool, other_user_id, "其他钱包").await?;
    let other_category_id = insert_category(pool, other_user_id, "其他", "其他").await?;
    let other_user_bill_id = insert_bill(
        pool,
        other_user_id,
        "2026-04-05T09:00:00Z",
        "expense",
        "expense",
        100,
        other_account_id,
        None,
        other_category_id,
        "Other",
        "其他用户",
        json!({}),
        false,
    )
    .await?;

    assert!(older_bill_id > 0);

    Ok(BillsFixture {
        user_id,
        wallet_account_id,
        bank_account_id,
        coffee_category_id,
        coffee_tag_id,
        coffee_bill_id,
        transfer_bill_id,
        deleted_bill_id,
        other_user_bill_id,
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
    name: &str,
    path: &str,
) -> Result<i64, Box<dyn Error>> {
    insert_category_with_type(pool, user_id, name, "expense", path).await
}

async fn insert_category_with_type(
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

async fn insert_tag(pool: &PostgresPool, user_id: i64, name: &str) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        "INSERT INTO tags (user_id, name, color, metadata) VALUES ($1, $2, '#663300', $3) RETURNING id",
    )
    .bind(user_id)
    .bind(name)
    .bind(json!({"icon": "coffee"}))
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}

async fn link_tag(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
    tag_id: i64,
) -> Result<(), Box<dyn Error>> {
    sqlx::query("INSERT INTO bill_tags (user_id, bill_id, tag_id) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(bill_id)
        .bind(tag_id)
        .execute(pool)
        .await?;
    Ok(())
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
    description: &str,
    standard_payload: Value,
    is_deleted: bool,
) -> Result<i64, Box<dyn Error>> {
    Ok(sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, direction, transaction_type, amount_cents,
            account_id, source_account_id, target_account_id, transfer_target_account_id,
            category_id, merchant, payment_method, description, source_hash, standard_payload,
            is_deleted
        )
        VALUES ($1, $2::timestamptz, $3, $4, $5, $6, $6, $7, $7, $8, $9, '现金', $10, $11, $12, $13)
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
    .bind(description)
    .bind(format!("bills-{user_id}-{merchant}-{amount_cents}"))
    .bind(standard_payload)
    .bind(is_deleted)
    .fetch_one(pool)
    .await?
    .try_get("id")?)
}
