use std::error::Error;

use bill_analyser_core::{LedgerListQuery, Money, TransactionType, UserId};
use bill_analyser_db::taxonomy::postgres_reads::{
    clear_postgres_account_transactions, list_postgres_categories,
    move_all_postgres_account_transactions, query_postgres_category_statistics,
};
use bill_analyser_db::{
    batch_create_postgres_bills, batch_update_postgres_bills, create_postgres_bill,
    delete_postgres_bill, get_postgres_bill_by_id, get_postgres_bill_tags,
    list_postgres_reconciliation_categories, list_postgres_user_data_categories,
    query_postgres_bills, resolve_postgres_category_by_id, sync_all_postgres_account_balances,
    update_postgres_bill, BillCategoryFilter, BillCreateDraft, BillFilters, BillUpdateDraft,
    PostgresLedgerQueries, PostgresPool,
};
use serde_json::{json, Value};
use sqlx::Row;

mod postgres_test_support;

async fn required_isolated_postgres_database(
    prefix: &str,
) -> Result<postgres_test_support::IsolatedPostgres, Box<dyn Error>> {
    postgres_test_support::isolated_postgres_database(prefix)
        .await?
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "BILL_ANALYSER_TEST_POSTGRES_URL is required for Ledger list boundary tests",
            )
            .into()
        })
}

#[tokio::test]
async fn ledger_queries_list_returns_typed_page_with_cents_tags_and_user_scope(
) -> Result<(), Box<dyn Error>> {
    let test_db = required_isolated_postgres_database("ledger_list_boundary").await?;
    let fixture = seed_bills_fixture(&test_db.pool).await?;
    let principal = UserId::new(u64::try_from(fixture.user_id)?)?;

    let queries = PostgresLedgerQueries::new(&test_db.pool);
    let page = queries
        .list(
            principal,
            LedgerListQuery {
                page: 1,
                page_size: 10,
                account_ids: vec![fixture.wallet_account_id],
                category_ids: vec![fixture.coffee_category_id],
                tag_ids: vec![fixture.coffee_tag_id],
                amount_filter_cents: Some("between:12000:13000".to_string()),
                keyword: Some("Cafe".to_string()),
                ..LedgerListQuery::default()
            },
        )
        .await?;

    assert_eq!(page.total, 1);
    assert_eq!(page.page, 1);
    assert_eq!(page.page_size, 10);
    let entry = page.items.first().expect("typed ledger entry");
    assert_eq!(entry.id, fixture.coffee_bill_id);
    assert_eq!(entry.transaction_type, TransactionType::Expense);
    assert_eq!(entry.amount, Money::from_cents(12_345));
    assert_eq!(entry.destination_amount, Money::ZERO);
    assert_eq!(entry.category_id, Some(fixture.coffee_category_id));
    assert_eq!(entry.main_category, "餐饮");
    assert_eq!(entry.sub_category, "咖啡");
    assert_eq!(entry.source_account_id, Some(fixture.wallet_account_id));
    assert_eq!(entry.destination_account_id, None);
    assert_eq!(
        entry.tags,
        vec![bill_analyser_core::LedgerTag {
            id: fixture.coffee_tag_id,
            name: "咖啡标签".to_string(),
        }]
    );

    let full_page = queries
        .list(
            principal,
            LedgerListQuery {
                page_size: 10,
                ..LedgerListQuery::default()
            },
        )
        .await?;
    assert_eq!(full_page.total, 3);
    assert!(
        full_page
            .items
            .iter()
            .all(|entry| entry.id != fixture.other_user_bill_id),
        "typed Ledger query must enforce principal scope"
    );

    let empty_page = queries
        .list(
            principal,
            LedgerListQuery {
                page: 0,
                page_size: 999,
                keyword: Some("does-not-exist".to_string()),
                ..LedgerListQuery::default()
            },
        )
        .await?;
    assert_eq!(empty_page.total, 0);
    assert!(empty_page.items.is_empty());
    assert_eq!(empty_page.page, 1);
    assert_eq!(empty_page.page_size, 500);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn category_path_projection_is_shared_across_ledger_taxonomy_and_export_adapters(
) -> Result<(), Box<dyn Error>> {
    let test_db = required_isolated_postgres_database("category_path_projection").await?;
    let user_id = insert_user(&test_db.pool, "category-path-projection").await?;
    let principal = UserId::new(u64::try_from(user_id)?)?;
    let account_id = insert_account(&test_db.pool, user_id, "分类投影账户").await?;
    let category_id = insert_category(
        &test_db.pool,
        user_id,
        "午餐 fallback",
        "\t\u{a0}餐饮\u{a0} //\n 工作日 / 午餐\u{3000}\t",
    )
    .await?;
    let bill_id = insert_bill(
        &test_db.pool,
        user_id,
        "2026-08-19T12:00:00Z",
        "expense",
        "expense",
        2_500,
        account_id,
        None,
        category_id,
        "午餐",
        "分类路径投影合同",
        json!({}),
        false,
    )
    .await?;

    let ledger = PostgresLedgerQueries::new(&test_db.pool)
        .list(principal, LedgerListQuery::default())
        .await?;
    let ledger_entry = ledger
        .items
        .iter()
        .find(|entry| entry.id == bill_id)
        .expect("ledger entry");
    assert_eq!(ledger_entry.main_category, "餐饮");
    assert_eq!(ledger_entry.sub_category, "工作日/午餐");

    let filtered_ledger = PostgresLedgerQueries::new(&test_db.pool)
        .list(
            principal,
            LedgerListQuery {
                main_category: Some("餐饮".to_string()),
                sub_category: Some("工作日/午餐".to_string()),
                ..LedgerListQuery::default()
            },
        )
        .await?;
    assert_eq!(
        filtered_ledger
            .items
            .iter()
            .map(|entry| entry.id)
            .collect::<Vec<_>>(),
        vec![bill_id]
    );

    let taxonomy = list_postgres_categories(&test_db.pool, user_id).await?;
    let taxonomy_entry = taxonomy
        .iter()
        .find(|entry| entry.get("id").and_then(Value::as_i64) == Some(category_id))
        .expect("taxonomy category");
    assert_eq!(
        taxonomy_entry.get("main_category").and_then(Value::as_str),
        Some("餐饮")
    );
    assert_eq!(
        taxonomy_entry.get("sub_category").and_then(Value::as_str),
        Some("工作日/午餐")
    );

    let exported = list_postgres_user_data_categories(&test_db.pool, principal).await?;
    let export_entry = exported
        .iter()
        .find(|entry| entry.id == category_id)
        .expect("export category");
    assert_eq!(export_entry.main_category, "餐饮");
    assert_eq!(export_entry.sub_category, "工作日/午餐");

    assert_eq!(
        resolve_postgres_category_by_id(&test_db.pool, user_id, category_id).await?,
        Some(("餐饮".to_string(), "工作日/午餐".to_string()))
    );
    let reconciliation_categories =
        list_postgres_reconciliation_categories(&test_db.pool, user_id).await?;
    let reconciliation_category = reconciliation_categories
        .iter()
        .find(|entry| entry.id == category_id)
        .expect("reconciliation category");
    assert_eq!(reconciliation_category.main_category, "餐饮");
    assert_eq!(reconciliation_category.sub_category, "工作日/午餐");

    let statistics = query_postgres_category_statistics(
        &test_db.pool,
        Some("2026-08-19"),
        Some("2026-08-19"),
        user_id,
    )
    .await?;
    assert_eq!(
        statistics,
        vec![bill_analyser_db::taxonomy::categories::CategoryStatistic {
            main_category: "餐饮".to_string(),
            sub_category: "工作日/午餐".to_string(),
            count: 1,
            total_amount_cents: 2_500,
        }]
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn account_balance_sync_replays_authoritative_bills_with_user_scope(
) -> Result<(), Box<dyn Error>> {
    let test_db = required_isolated_postgres_database("account_balance_sync").await?;
    let fixture = seed_bills_fixture(&test_db.pool).await?;
    let unchanged_account_id = insert_account(&test_db.pool, fixture.user_id, "无流水账户").await?;
    let other_account_id: i64 =
        sqlx::query_scalar("SELECT source_account_id FROM bills WHERE id = $1")
            .bind(fixture.other_user_bill_id)
            .fetch_one(&test_db.pool)
            .await?;

    sqlx::query("UPDATE accounts SET balance_cents = $1, metadata = $2 WHERE id = $3")
        .bind(1_i64)
        .bind(json!({"initial_balance_cents": "100000"}))
        .bind(fixture.wallet_account_id)
        .execute(&test_db.pool)
        .await?;
    sqlx::query("UPDATE accounts SET balance_cents = $1, metadata = $2 WHERE id = $3")
        .bind(5_000_i64)
        .bind(json!({"initial_balance_cents": "invalid"}))
        .bind(fixture.bank_account_id)
        .execute(&test_db.pool)
        .await?;
    sqlx::query("UPDATE accounts SET balance_cents = $1, metadata = $2 WHERE id = $3")
        .bind(7_777_i64)
        .bind(json!({"initial_balance_cents": 7_777}))
        .bind(unchanged_account_id)
        .execute(&test_db.pool)
        .await?;

    let before_versions = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, version FROM accounts WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![
        fixture.wallet_account_id,
        fixture.bank_account_id,
        unchanged_account_id,
    ])
    .fetch_all(&test_db.pool)
    .await?;

    let result = sync_all_postgres_account_balances(&test_db.pool, fixture.user_id).await?;

    assert_eq!(result.total_accounts, 3);
    assert_eq!(result.synced_accounts, 2);
    assert!(result.errors.is_empty());
    assert_eq!(result.discrepancies.len(), 2);
    let wallet = result
        .discrepancies
        .iter()
        .find(|item| item.account_id == fixture.wallet_account_id)
        .expect("wallet discrepancy");
    assert_eq!(wallet.old_balance_cents, 1);
    assert_eq!(wallet.new_balance_cents, 62_655);
    assert_eq!(wallet.diff_cents, 62_654);
    let bank = result
        .discrepancies
        .iter()
        .find(|item| item.account_id == fixture.bank_account_id)
        .expect("bank discrepancy");
    assert_eq!(bank.old_balance_cents, 5_000);
    assert_eq!(bank.new_balance_cents, 25_150);
    assert_eq!(bank.diff_cents, 20_150);

    let after_accounts = sqlx::query_as::<_, (i64, i64, i64)>(
        "SELECT id, balance_cents, version FROM accounts WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![
        fixture.wallet_account_id,
        fixture.bank_account_id,
        unchanged_account_id,
    ])
    .fetch_all(&test_db.pool)
    .await?;
    for ((before_id, before_version), (after_id, _, after_version)) in
        before_versions.iter().zip(&after_accounts)
    {
        assert_eq!(before_id, after_id);
        let expected_increment = i64::from(*after_id != unchanged_account_id);
        assert_eq!(*after_version, *before_version + expected_increment);
    }
    assert_eq!(
        after_accounts
            .iter()
            .find(|(id, _, _)| *id == fixture.wallet_account_id)
            .map(|(_, balance, _)| *balance),
        Some(62_655)
    );
    assert_eq!(
        after_accounts
            .iter()
            .find(|(id, _, _)| *id == fixture.bank_account_id)
            .map(|(_, balance, _)| *balance),
        Some(25_150)
    );
    assert_eq!(
        after_accounts
            .iter()
            .find(|(id, _, _)| *id == unchanged_account_id)
            .map(|(_, balance, _)| *balance),
        Some(7_777)
    );
    assert_eq!(
        account_balance_cents(&test_db.pool, other_account_id).await?,
        0
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn account_transaction_move_rewrites_active_user_scoped_references_only(
) -> Result<(), Box<dyn Error>> {
    let test_db = required_isolated_postgres_database("account_transaction_move").await?;
    let fixture = seed_bills_fixture(&test_db.pool).await?;

    sqlx::query("UPDATE accounts SET balance_cents = CASE WHEN id = $1 THEN 111 ELSE 222 END WHERE id = ANY($2)")
        .bind(fixture.wallet_account_id)
        .bind(vec![fixture.wallet_account_id, fixture.bank_account_id])
        .execute(&test_db.pool)
        .await?;

    let result = move_all_postgres_account_transactions(
        &test_db.pool,
        fixture.wallet_account_id,
        fixture.bank_account_id,
        fixture.user_id,
    )
    .await?;

    assert!(result.success);
    assert_eq!(result.message, "Transactions moved successfully");
    assert_eq!(result.moved_count, 3);

    let active_rows = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, i64)>(
        r#"
        SELECT id, account_id, source_account_id, target_account_id,
               transfer_target_account_id, version
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
        ORDER BY id
        "#,
    )
    .bind(fixture.user_id)
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(active_rows.len(), 3);
    for (
        id,
        account_id,
        source_account_id,
        target_account_id,
        transfer_target_account_id,
        version,
    ) in &active_rows
    {
        assert_eq!(*account_id, fixture.bank_account_id, "bill {id} account");
        assert_eq!(
            *source_account_id,
            Some(fixture.bank_account_id),
            "bill {id} source"
        );
        if *id == fixture.transfer_bill_id {
            assert_eq!(*target_account_id, Some(fixture.bank_account_id));
            assert_eq!(*transfer_target_account_id, Some(fixture.bank_account_id));
        } else {
            assert_eq!(*target_account_id, None);
            assert_eq!(*transfer_target_account_id, None);
        }
        assert_eq!(*version, 2, "bill {id} version");
    }

    let deleted_row = sqlx::query_as::<_, (i64, Option<i64>, i64)>(
        "SELECT account_id, source_account_id, version FROM bills WHERE id = $1",
    )
    .bind(fixture.deleted_bill_id)
    .fetch_one(&test_db.pool)
    .await?;
    assert_eq!(
        deleted_row,
        (
            fixture.wallet_account_id,
            Some(fixture.wallet_account_id),
            1
        )
    );
    let other_user_source: Option<i64> =
        sqlx::query_scalar("SELECT source_account_id FROM bills WHERE id = $1")
            .bind(fixture.other_user_bill_id)
            .fetch_one(&test_db.pool)
            .await?;
    assert_ne!(other_user_source, Some(fixture.bank_account_id));

    let balances = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, balance_cents FROM accounts WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![fixture.wallet_account_id, fixture.bank_account_id])
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(
        balances,
        vec![
            (fixture.wallet_account_id, 111),
            (fixture.bank_account_id, 222)
        ]
    );

    let same_account = move_all_postgres_account_transactions(
        &test_db.pool,
        fixture.bank_account_id,
        fixture.bank_account_id,
        fixture.user_id,
    )
    .await?;
    assert_eq!(
        same_account.message,
        "Source and target accounts must be different"
    );
    assert_eq!(same_account.moved_count, 0);

    let missing_source = move_all_postgres_account_transactions(
        &test_db.pool,
        i64::MAX,
        fixture.bank_account_id,
        fixture.user_id,
    )
    .await?;
    assert_eq!(missing_source.message, "Source account not found");
    assert_eq!(missing_source.moved_count, 0);

    let missing_target = move_all_postgres_account_transactions(
        &test_db.pool,
        fixture.bank_account_id,
        i64::MAX,
        fixture.user_id,
    )
    .await?;
    assert_eq!(missing_target.message, "Target account not found");
    assert_eq!(missing_target.moved_count, 0);

    let versions_after_failures = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, version FROM bills WHERE user_id = $1 AND is_deleted = false ORDER BY id",
    )
    .bind(fixture.user_id)
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(
        versions_after_failures,
        active_rows
            .iter()
            .map(|(id, _, _, _, _, version)| (*id, *version))
            .collect::<Vec<_>>()
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test]
async fn account_transaction_clear_soft_deletes_active_user_scoped_bills_and_tags(
) -> Result<(), Box<dyn Error>> {
    let test_db = required_isolated_postgres_database("account_transaction_clear").await?;
    let fixture = seed_bills_fixture(&test_db.pool).await?;

    sqlx::query("UPDATE accounts SET balance_cents = CASE WHEN id = $1 THEN 333 ELSE 444 END WHERE id = ANY($2)")
        .bind(fixture.wallet_account_id)
        .bind(vec![fixture.wallet_account_id, fixture.bank_account_id])
        .execute(&test_db.pool)
        .await?;
    let affected_before = sqlx::query_as::<_, (i64, i64)>(
        r#"
        SELECT id, version
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        ORDER BY id
        "#,
    )
    .bind(fixture.user_id)
    .bind(fixture.wallet_account_id)
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(affected_before.len(), 3);

    let result = clear_postgres_account_transactions(
        &test_db.pool,
        fixture.wallet_account_id,
        fixture.user_id,
    )
    .await?;

    assert!(result.success);
    assert_eq!(result.message, "Transactions deleted successfully");
    assert_eq!(result.deleted_count, 3);
    let affected_after = sqlx::query_as::<_, (i64, bool, i64)>(
        "SELECT id, is_deleted, version FROM bills WHERE id = ANY($1) ORDER BY id",
    )
    .bind(
        affected_before
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
    )
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(affected_after.len(), 3);
    for ((before_id, before_version), (after_id, is_deleted, after_version)) in
        affected_before.iter().zip(&affected_after)
    {
        assert_eq!(before_id, after_id);
        assert!(*is_deleted);
        assert_eq!(*after_version, *before_version + 1);
    }

    let coffee_tag_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bill_tags WHERE bill_id = $1")
            .bind(fixture.coffee_bill_id)
            .fetch_one(&test_db.pool)
            .await?;
    assert_eq!(coffee_tag_count, 0);
    let prior_deleted =
        sqlx::query_as::<_, (bool, i64)>("SELECT is_deleted, version FROM bills WHERE id = $1")
            .bind(fixture.deleted_bill_id)
            .fetch_one(&test_db.pool)
            .await?;
    assert_eq!(prior_deleted, (true, 1));
    let other_user_row =
        sqlx::query_as::<_, (bool, i64)>("SELECT is_deleted, version FROM bills WHERE id = $1")
            .bind(fixture.other_user_bill_id)
            .fetch_one(&test_db.pool)
            .await?;
    assert_eq!(other_user_row, (false, 1));

    let balances = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, balance_cents FROM accounts WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![fixture.wallet_account_id, fixture.bank_account_id])
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(
        balances,
        vec![
            (fixture.wallet_account_id, 333),
            (fixture.bank_account_id, 444)
        ]
    );

    let missing =
        clear_postgres_account_transactions(&test_db.pool, i64::MAX, fixture.user_id).await?;
    assert!(!missing.success);
    assert_eq!(missing.message, "Account not found");
    assert_eq!(missing.deleted_count, 0);

    let versions_after_failure = sqlx::query_as::<_, (i64, i64)>(
        "SELECT id, version FROM bills WHERE id = ANY($1) ORDER BY id",
    )
    .bind(
        affected_after
            .iter()
            .map(|(id, _, _)| *id)
            .collect::<Vec<_>>(),
    )
    .fetch_all(&test_db.pool)
    .await?;
    assert_eq!(
        versions_after_failure,
        affected_after
            .iter()
            .map(|(id, _, version)| (*id, *version))
            .collect::<Vec<_>>()
    );

    test_db.cleanup().await?;
    Ok(())
}

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
async fn postgres_bill_mutations_reject_i64_min_without_writing_rows() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_i64_min_contract").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-i64-min-contract").await?;

    let invalid_amount = BillCreateDraft {
        fields: serde_json::Map::from_iter([
            ("date".to_string(), json!("2026-05-03 09:00:00")),
            ("type".to_string(), json!("支出")),
            ("amount_cents".to_string(), json!(i64::MIN)),
        ]),
        tag_ids: Vec::new(),
    };
    let error = create_postgres_bill(pool, user_id, &invalid_amount)
        .await
        .expect_err("i64::MIN amount must fail before PostgreSQL mutation");
    assert!(
        error.to_string().contains("invalid bill amount_cents"),
        "unexpected error: {error}"
    );

    let invalid_destination_amount = BillCreateDraft {
        fields: serde_json::Map::from_iter([
            ("date".to_string(), json!("2026-05-03 10:00:00")),
            ("type".to_string(), json!("转账")),
            ("amount_cents".to_string(), json!(100)),
            ("destination_amount_cents".to_string(), json!(i64::MIN)),
        ]),
        tag_ids: Vec::new(),
    };
    let error = batch_create_postgres_bills(pool, user_id, &[invalid_destination_amount])
        .await
        .expect_err("i64::MIN destination amount must fail the batch");
    assert!(
        error
            .to_string()
            .contains("invalid bill destination_amount_cents"),
        "unexpected error: {error}"
    );

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM bills WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await?;
    assert_eq!(count, 0, "extreme amounts must not write partial rows");

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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_bill_updates_keep_account_balance_equal_to_committed_bill(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_concurrent_update_update").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-concurrent-update-update").await?;
    let account_id = insert_account(pool, user_id, "并发钱包").await?;
    let category_id = insert_category(pool, user_id, "并发支出", "支出/并发支出").await?;
    let bill_id = create_postgres_bill(
        pool,
        user_id,
        &expense_bill_draft(account_id, category_id, 1_000, "update-update"),
    )
    .await?;

    let mut blocker = pool.begin().await?;
    sqlx::query("SELECT id FROM bills WHERE id=$1 FOR UPDATE")
        .bind(bill_id)
        .fetch_one(&mut *blocker)
        .await?;
    let first = spawn_bill_update(pool.clone(), user_id, bill_id, 1_500);
    let second = spawn_bill_update(pool.clone(), user_id, bill_id, 2_200);
    wait_for_bill_lock_waiters(pool, 2).await?;
    blocker.commit().await?;
    assert!(first.await??, "first update must commit exactly once");
    assert!(second.await??, "second update must commit exactly once");

    assert_balance_matches_authoritative_bills(pool, user_id, account_id).await?;
    let version: i64 = sqlx::query_scalar("SELECT version FROM bills WHERE id=$1")
        .bind(bill_id)
        .fetch_one(pool)
        .await?;
    assert_eq!(
        version, 3,
        "two serialized updates each advance one version"
    );

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_bill_update_and_delete_leave_no_balance_residue() -> Result<(), Box<dyn Error>>
{
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_concurrent_update_delete").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-concurrent-update-delete").await?;
    let account_id = insert_account(pool, user_id, "并发删除钱包").await?;
    let category_id = insert_category(pool, user_id, "并发删除", "支出/并发删除").await?;
    let bill_id = create_postgres_bill(
        pool,
        user_id,
        &expense_bill_draft(account_id, category_id, 1_000, "update-delete"),
    )
    .await?;

    let mut blocker = pool.begin().await?;
    sqlx::query("SELECT id FROM bills WHERE id=$1 FOR UPDATE")
        .bind(bill_id)
        .fetch_one(&mut *blocker)
        .await?;
    let update = spawn_bill_update(pool.clone(), user_id, bill_id, 2_500);
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let delete_pool = pool.clone();
    let delete =
        tokio::spawn(async move { delete_postgres_bill(&delete_pool, user_id, bill_id).await });
    wait_for_bill_lock_waiters(pool, 2).await?;
    blocker.commit().await?;
    let _update_committed = update.await??;
    assert!(delete.await??, "delete must commit once");

    assert!(get_postgres_bill_by_id(pool, user_id, bill_id)
        .await?
        .is_none());
    assert_eq!(account_balance_cents(pool, account_id).await?, 0);

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_batch_and_single_update_preserve_authoritative_balance_sum(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_concurrent_batch_single").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-concurrent-batch-single").await?;
    let account_id = insert_account(pool, user_id, "批量并发钱包").await?;
    let category_id = insert_category(pool, user_id, "批量并发", "支出/批量并发").await?;
    let first_bill_id = create_postgres_bill(
        pool,
        user_id,
        &expense_bill_draft(account_id, category_id, 1_000, "batch-first"),
    )
    .await?;
    let second_bill_id = create_postgres_bill(
        pool,
        user_id,
        &expense_bill_draft(account_id, category_id, 2_000, "batch-second"),
    )
    .await?;

    let mut blocker = pool.begin().await?;
    sqlx::query("SELECT id FROM bills WHERE id=$1 FOR UPDATE")
        .bind(first_bill_id)
        .fetch_one(&mut *blocker)
        .await?;
    let batch_pool = pool.clone();
    let batch = tokio::spawn(async move {
        batch_update_postgres_bills(
            &batch_pool,
            user_id,
            &[first_bill_id, second_bill_id],
            &serde_json::Map::from_iter([("amount_cents".to_string(), json!(3_000))]),
        )
        .await
    });
    let single = spawn_bill_update(pool.clone(), user_id, first_bill_id, 4_500);
    wait_for_bill_lock_waiters(pool, 2).await?;
    blocker.commit().await?;
    let batch_result = batch.await??;
    assert_eq!(batch_result.success_count, 2);
    assert!(single.await??, "single update must commit once");

    assert_balance_matches_authoritative_bills(pool, user_id, account_id).await?;

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reverse_transfer_updates_use_one_account_lock_order(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_reverse_transfer_accounts").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-reverse-transfer-accounts").await?;
    let account_a = insert_account(pool, user_id, "反向账户 A").await?;
    let account_b = insert_account(pool, user_id, "反向账户 B").await?;
    let category_a =
        insert_category_with_type(pool, user_id, "反向转账 A", "transfer", "转账/反向 A").await?;
    let category_b =
        insert_category_with_type(pool, user_id, "反向转账 B", "transfer", "转账/反向 B").await?;
    let bill_ab = create_postgres_bill(
        pool,
        user_id,
        &transfer_bill_draft(account_a, account_b, category_a, 1_000, "A-to-B"),
    )
    .await?;
    let bill_ba = create_postgres_bill(
        pool,
        user_id,
        &transfer_bill_draft(account_b, account_a, category_b, 2_000, "B-to-A"),
    )
    .await?;

    let mut blocker = pool.begin().await?;
    sqlx::query("SELECT id FROM accounts WHERE id IN ($1, $2) ORDER BY id FOR UPDATE")
        .bind(account_a)
        .bind(account_b)
        .fetch_all(&mut *blocker)
        .await?;
    let update_ab = spawn_bill_update_fields(
        pool.clone(),
        user_id,
        bill_ab,
        serde_json::Map::from_iter([
            ("source_account_id".to_string(), json!(account_b)),
            ("destination_account_id".to_string(), json!(account_a)),
        ]),
    );
    let update_ba = spawn_bill_update_fields(
        pool.clone(),
        user_id,
        bill_ba,
        serde_json::Map::from_iter([
            ("source_account_id".to_string(), json!(account_a)),
            ("destination_account_id".to_string(), json!(account_b)),
        ]),
    );
    wait_for_relation_lock_waiters(pool, "accounts", 2).await?;
    blocker.commit().await?;

    let (update_ab, update_ba) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(update_ab, update_ba)
    })
    .await
    .map_err(|_| "reverse transfer updates exceeded the bounded lock deadline")?;
    let update_ab = update_ab?;
    let update_ba = update_ba?;
    assert!(
        matches!(update_ab, Ok(true)) && matches!(update_ba, Ok(true)),
        "reverse transfer updates must both serialize without a deadlock: {update_ab:?}, {update_ba:?}"
    );
    assert_balance_matches_transfer_bills(pool, user_id, account_a).await?;
    assert_balance_matches_transfer_bills(pool, user_id, account_b).await?;

    test_db.cleanup().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reverse_category_updates_use_one_category_lock_order(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("bill_reverse_categories").await?
    else {
        return Ok(());
    };
    let pool = &test_db.pool;
    let user_id = insert_user(pool, "bill-reverse-categories").await?;
    let account_a = insert_account(pool, user_id, "分类账户 A").await?;
    let account_b = insert_account(pool, user_id, "分类账户 B").await?;
    let category_x = insert_category(pool, user_id, "分类 X", "支出/分类 X").await?;
    let category_y = insert_category(pool, user_id, "分类 Y", "支出/分类 Y").await?;
    let bill_x = create_postgres_bill(
        pool,
        user_id,
        &expense_bill_draft(account_a, category_x, 1_100, "category-X"),
    )
    .await?;
    let bill_y = create_postgres_bill(
        pool,
        user_id,
        &expense_bill_draft(account_b, category_y, 2_200, "category-Y"),
    )
    .await?;

    let mut blocker = pool.begin().await?;
    sqlx::query("SELECT id FROM categories WHERE id IN ($1, $2) ORDER BY id FOR UPDATE")
        .bind(category_x)
        .bind(category_y)
        .fetch_all(&mut *blocker)
        .await?;
    let update_x = spawn_bill_update_fields(
        pool.clone(),
        user_id,
        bill_x,
        serde_json::Map::from_iter([("category_id".to_string(), json!(category_y))]),
    );
    let update_y = spawn_bill_update_fields(
        pool.clone(),
        user_id,
        bill_y,
        serde_json::Map::from_iter([("category_id".to_string(), json!(category_x))]),
    );
    wait_for_relation_lock_waiters(pool, "categories", 2).await?;
    blocker.commit().await?;

    let (update_x, update_y) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(update_x, update_y)
    })
    .await
    .map_err(|_| "reverse category updates exceeded the bounded lock deadline")?;
    let update_x = update_x?;
    let update_y = update_y?;
    assert!(
        matches!(update_x, Ok(true)) && matches!(update_y, Ok(true)),
        "reverse category updates must both serialize without a deadlock: {update_x:?}, {update_y:?}"
    );
    assert_balance_matches_authoritative_bills(pool, user_id, account_a).await?;
    assert_balance_matches_authoritative_bills(pool, user_id, account_b).await?;

    test_db.cleanup().await?;
    Ok(())
}

fn spawn_bill_update(
    pool: PostgresPool,
    user_id: i64,
    bill_id: i64,
    amount_cents: i64,
) -> tokio::task::JoinHandle<bill_analyser_db::DbResult<bool>> {
    spawn_bill_update_fields(
        pool,
        user_id,
        bill_id,
        serde_json::Map::from_iter([("amount_cents".to_string(), json!(amount_cents))]),
    )
}

fn spawn_bill_update_fields(
    pool: PostgresPool,
    user_id: i64,
    bill_id: i64,
    fields: serde_json::Map<String, Value>,
) -> tokio::task::JoinHandle<bill_analyser_db::DbResult<bool>> {
    tokio::spawn(async move {
        update_postgres_bill(
            &pool,
            user_id,
            bill_id,
            &BillUpdateDraft {
                fields,
                tag_ids: None,
            },
        )
        .await
    })
}

async fn wait_for_bill_lock_waiters(
    pool: &PostgresPool,
    expected: i64,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..100 {
        let waiting: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM pg_stat_activity
            WHERE datname = current_database()
              AND wait_event_type = 'Lock'
              AND query ILIKE '%bills%'
            "#,
        )
        .fetch_one(pool)
        .await?;
        if waiting >= expected {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Err(format!("timed out waiting for {expected} blocked bill mutations").into())
}

async fn wait_for_relation_lock_waiters(
    pool: &PostgresPool,
    relation_name: &str,
    expected: i64,
) -> Result<(), Box<dyn Error>> {
    let query_pattern = format!("%{relation_name}%");
    for _ in 0..100 {
        let waiting: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)::BIGINT
            FROM pg_stat_activity
            WHERE datname = current_database()
              AND pid <> pg_backend_pid()
              AND wait_event_type = 'Lock'
              AND query ILIKE $1
            "#,
        )
        .bind(&query_pattern)
        .fetch_one(pool)
        .await?;
        if waiting >= expected {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Err(format!("timed out waiting for {expected} blocked {relation_name} mutations").into())
}

fn expense_bill_draft(
    account_id: i64,
    category_id: i64,
    amount_cents: i64,
    description: &str,
) -> BillCreateDraft {
    BillCreateDraft {
        fields: serde_json::Map::from_iter([
            ("date".to_string(), json!("2026-07-14 12:00:00")),
            ("type".to_string(), json!("支出")),
            ("amount_cents".to_string(), json!(amount_cents)),
            ("source_account_id".to_string(), json!(account_id)),
            ("category_id".to_string(), json!(category_id)),
            ("counterparty".to_string(), json!("并发测试商户")),
            ("description".to_string(), json!(description)),
            ("payment_method".to_string(), json!("现金")),
        ]),
        tag_ids: Vec::new(),
    }
}

fn transfer_bill_draft(
    source_account_id: i64,
    destination_account_id: i64,
    category_id: i64,
    amount_cents: i64,
    description: &str,
) -> BillCreateDraft {
    BillCreateDraft {
        fields: serde_json::Map::from_iter([
            ("date".to_string(), json!("2026-07-14 12:00:00")),
            ("type".to_string(), json!("转账")),
            ("amount_cents".to_string(), json!(amount_cents)),
            ("destination_amount_cents".to_string(), json!(amount_cents)),
            ("source_account_id".to_string(), json!(source_account_id)),
            (
                "destination_account_id".to_string(),
                json!(destination_account_id),
            ),
            ("category_id".to_string(), json!(category_id)),
            ("counterparty".to_string(), json!("反向转账测试")),
            ("description".to_string(), json!(description)),
            ("payment_method".to_string(), json!("内部转账")),
        ]),
        tag_ids: Vec::new(),
    }
}

async fn account_balance_cents(
    pool: &PostgresPool,
    account_id: i64,
) -> Result<i64, Box<dyn Error>> {
    Ok(
        sqlx::query_scalar("SELECT balance_cents FROM accounts WHERE id=$1")
            .bind(account_id)
            .fetch_one(pool)
            .await?,
    )
}

async fn assert_balance_matches_authoritative_bills(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
) -> Result<(), Box<dyn Error>> {
    let expected: i64 = sqlx::query_scalar(
        "SELECT -COALESCE(SUM(amount_cents), 0)::BIGINT FROM bills WHERE user_id=$1 AND source_account_id=$2 AND is_deleted=false",
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(account_balance_cents(pool, account_id).await?, expected);
    Ok(())
}

async fn assert_balance_matches_transfer_bills(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
) -> Result<(), Box<dyn Error>> {
    let expected: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(SUM(
            CASE WHEN source_account_id = $2 THEN -amount_cents ELSE 0 END
            + CASE WHEN transfer_target_account_id = $2 THEN
                COALESCE((standard_payload->>'destination_amount_cents')::BIGINT, amount_cents)
              ELSE 0 END
        ), 0)::BIGINT
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(account_balance_cents(pool, account_id).await?, expected);
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
