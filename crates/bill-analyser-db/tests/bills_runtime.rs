use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    batch_create_bills, batch_delete_bills, batch_update_bills, calculate_bill_hash_from_fields,
    create_bill, delete_bill, get_bill_by_id, get_bill_tags, query_bills, update_bill,
    BillCategoryFilter, BillCreateDraft, BillFilters, BillRecord, BillUpdateDraft,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use serde_json::{json, Map, Value};

fn runtime_for(path: &std::path::Path) -> Result<SqliteRuntime, Box<dyn Error>> {
    let db_path = SqliteDbPath::temporary_file(path)?;
    Ok(SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: std::time::Duration::from_secs(1),
    })?)
}

fn user_id(value: u64) -> UserId {
    UserId::new(value).expect("positive test user id")
}

fn init_schema(runtime: &SqliteRuntime) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL
        );
        CREATE TABLE accounts(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            type INTEGER NOT NULL,
            category INTEGER,
            currency TEXT DEFAULT 'CNY',
            icon TEXT,
            color TEXT,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0,
            hidden BOOLEAN DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            comment TEXT,
            aliases TEXT,
            parent_id INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            payment_method TEXT DEFAULT '',
            main_category TEXT,
            sub_category TEXT,
            batch_id TEXT,
            hash TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX idx_bills_user_hash_unique ON bills(user_id, hash);
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            display_order INTEGER DEFAULT 0,
            hidden BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, name),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (bill_id, tag_id),
            FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE,
            FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
        );
        CREATE TABLE bill_pair_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            pair_type TEXT NOT NULL,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL,
            source TEXT NOT NULL DEFAULT 'manual',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            CHECK(left_bill_id < right_bill_id),
            UNIQUE(user_id, pair_type, left_bill_id, right_bill_id)
        );
        CREATE TABLE bill_transfer_pair_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK(left_bill_id < right_bill_id),
            UNIQUE(user_id, left_bill_id, right_bill_id)
        );
        CREATE TABLE bill_investment_pair_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            CHECK(left_bill_id < right_bill_id),
            UNIQUE(user_id, left_bill_id, right_bill_id)
        );
        CREATE TABLE bill_learning_rule_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            bill_id INTEGER NOT NULL,
            rule_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, bill_id, rule_id)
        );
        ",
    )?;
    runtime.connection().execute(
        "INSERT INTO users(id, username) VALUES (42, 'owner'), (77, 'other')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO accounts(id, user_id, name, type, balance, initial_balance, created_at, updated_at)
         VALUES (10, 42, 'cash', 1, 100.0, 100.0, 'now', 'now'),
                (20, 42, 'broker', 1, 0.0, 0.0, 'now', 'now'),
                (90, 77, 'other-cash', 1, 100.0, 100.0, 'now', 'now')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO tags(id, user_id, name, created_at, updated_at)
         VALUES (1, 42, 'food', 'now', 'now'),
                (2, 42, 'salary', 'now', 'now'),
                (3, 77, 'other-tag', 'now', 'now')",
        [],
    )?;
    Ok(())
}

fn bill_fields(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
    source_account_id: i64,
) -> BillRecord {
    json!({
        "date": date,
        "type": bill_type,
        "amount": amount,
        "counterparty": counterparty,
        "description": description,
        "payment_method": "manual",
        "main_category": "工资",
        "sub_category": "",
        "source_account_id": source_account_id,
        "destination_account_id": 0,
        "destination_amount": 0.0
    })
    .as_object()
    .expect("object")
    .clone()
}

fn balance(runtime: &SqliteRuntime, account_id: i64) -> Result<f64, Box<dyn Error>> {
    Ok(runtime.connection().query_row(
        "SELECT balance FROM accounts WHERE id = ?1",
        [account_id],
        |row| row.get::<_, f64>(0),
    )?)
}

fn table_count(runtime: &SqliteRuntime, table: &str) -> Result<i64, Box<dyn Error>> {
    Ok(runtime
        .connection()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?)
}

fn value_text(record: &Map<String, Value>, key: &str) -> String {
    record
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[test]
fn create_bill_is_user_scoped_hashes_tags_and_syncs_balance() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-create.db"))?;
    init_schema(&runtime)?;

    let owner_bill_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-08 10:00:00", "收入", 25.0, "ACME", "Salary", 10),
            tag_ids: vec![1, 2],
        },
    )?;

    let owner_bill = get_bill_by_id(runtime.connection(), user_id(42), owner_bill_id)?
        .expect("owner can read created bill");
    assert_eq!(
        value_text(&owner_bill, "hash"),
        calculate_bill_hash_from_fields("2026-05-08 10:00:00", "收入", 25.0, "ACME", "Salary")
    );
    assert_eq!(
        get_bill_tags(runtime.connection(), user_id(42), owner_bill_id)?.len(),
        2
    );
    assert_eq!(balance(&runtime, 10)?, 125.0);
    assert!(get_bill_by_id(runtime.connection(), user_id(77), owner_bill_id)?.is_none());

    let duplicate = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-08 10:00:00", "收入", 25.0, "ACME", "Salary", 10),
            tag_ids: vec![],
        },
    );
    assert!(duplicate.is_err());

    let other_user_bill_id = create_bill(
        runtime.connection_mut(),
        user_id(77),
        &BillCreateDraft {
            fields: bill_fields("2026-05-08 10:00:00", "收入", 25.0, "ACME", "Salary", 90),
            tag_ids: vec![3],
        },
    )?;
    assert!(other_user_bill_id > owner_bill_id);

    let page = query_bills(
        runtime.connection(),
        user_id(42),
        1,
        20,
        &BillFilters {
            keyword: Some("Salary".to_string()),
            tag_ids: vec![2],
            categories: vec![BillCategoryFilter {
                main: "工资".to_string(),
                sub: None,
            }],
            ..BillFilters::default()
        },
    )?;
    assert_eq!(page.total, 1);
    assert_eq!(page.bills.len(), 1);
    assert_eq!(
        page.bills[0].get("id").and_then(Value::as_i64),
        Some(owner_bill_id)
    );
    Ok(())
}

#[test]
fn create_bill_rolls_back_row_tags_and_balance_when_tag_write_fails() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-create-rollback.db"))?;
    init_schema(&runtime)?;

    let result = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-08 11:00:00", "收入", 30.0, "ACME", "Bonus", 10),
            tag_ids: vec![999],
        },
    );

    assert!(result.is_err());
    assert_eq!(table_count(&runtime, "bills")?, 0);
    assert_eq!(table_count(&runtime, "bill_tags")?, 0);
    assert_eq!(balance(&runtime, 10)?, 100.0);
    Ok(())
}

#[test]
fn batch_create_bills_uses_one_transaction_for_rows_tags_and_balances() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-batch-create.db"))?;
    init_schema(&runtime)?;

    let ids = batch_create_bills(
        runtime.connection_mut(),
        user_id(42),
        &[
            BillCreateDraft {
                fields: bill_fields("2026-05-08 12:00:00", "收入", 10.0, "ACME", "Salary", 10),
                tag_ids: vec![1],
            },
            BillCreateDraft {
                fields: bill_fields("2026-05-08 13:00:00", "收入", 15.0, "ACME", "Bonus", 10),
                tag_ids: vec![2],
            },
        ],
    )?;
    assert_eq!(ids.len(), 2);
    assert_eq!(table_count(&runtime, "bills")?, 2);
    assert_eq!(table_count(&runtime, "bill_tags")?, 2);
    assert_eq!(balance(&runtime, 10)?, 125.0);

    let mut rollback_runtime =
        runtime_for(&temp_dir.path().join("bills-batch-create-rollback.db"))?;
    init_schema(&rollback_runtime)?;
    let result = batch_create_bills(
        rollback_runtime.connection_mut(),
        user_id(42),
        &[
            BillCreateDraft {
                fields: bill_fields("2026-05-08 14:00:00", "收入", 10.0, "ACME", "Salary", 10),
                tag_ids: vec![1],
            },
            BillCreateDraft {
                fields: bill_fields("2026-05-08 15:00:00", "收入", 15.0, "ACME", "Bonus", 10),
                tag_ids: vec![999],
            },
        ],
    );
    assert!(result.is_err());
    assert_eq!(table_count(&rollback_runtime, "bills")?, 0);
    assert_eq!(table_count(&rollback_runtime, "bill_tags")?, 0);
    assert_eq!(balance(&rollback_runtime, 10)?, 100.0);
    Ok(())
}

#[test]
fn update_bill_transaction_rolls_back_partial_row_tag_and_balance_changes(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-update-rollback.db"))?;
    init_schema(&runtime)?;
    let bill_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-08 12:00:00", "收入", 10.0, "ACME", "Salary", 10),
            tag_ids: vec![2],
        },
    )?;
    assert_eq!(balance(&runtime, 10)?, 110.0);

    let result = update_bill(
        runtime.connection_mut(),
        user_id(42),
        bill_id,
        &BillUpdateDraft {
            fields: json!({"amount": 50.0, "description": "Raised salary"})
                .as_object()
                .expect("object")
                .clone(),
            tag_ids: Some(vec![999]),
        },
    );

    assert!(result.is_err());
    let bill =
        get_bill_by_id(runtime.connection(), user_id(42), bill_id)?.expect("bill should remain");
    assert_eq!(bill.get("amount").and_then(Value::as_f64), Some(10.0));
    assert_eq!(value_text(&bill, "description"), "Salary");
    assert_eq!(
        get_bill_tags(runtime.connection(), user_id(42), bill_id)?.len(),
        1
    );
    assert_eq!(balance(&runtime, 10)?, 110.0);
    Ok(())
}

#[test]
fn update_delete_and_batch_mutations_cleanup_matching_state_and_sync_balances(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-mutations.db"))?;
    init_schema(&runtime)?;
    let first_bill_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-08 12:00:00", "收入", 10.0, "ACME", "Salary", 10),
            tag_ids: vec![2],
        },
    )?;
    let second_bill_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-09 12:00:00", "收入", 20.0, "ACME", "Bonus", 10),
            tag_ids: vec![2],
        },
    )?;
    runtime.connection().execute(
        "INSERT INTO bill_pair_links(user_id, pair_type, left_bill_id, right_bill_id, source, created_at, updated_at)
         VALUES (42, 'transfer', ?1, ?2, 'manual', 'now', 'now')",
        (first_bill_id, second_bill_id),
    )?;
    runtime.connection().execute(
        "INSERT INTO bill_transfer_pair_suppressions(user_id, left_bill_id, right_bill_id, created_at)
         VALUES (42, ?1, ?2, 'now')",
        (first_bill_id, second_bill_id),
    )?;
    runtime.connection().execute(
        "INSERT INTO bill_learning_rule_suppressions(user_id, bill_id, rule_id, created_at)
         VALUES (42, ?1, 7, 'now')",
        [first_bill_id],
    )?;
    assert_eq!(balance(&runtime, 10)?, 130.0);

    assert!(update_bill(
        runtime.connection_mut(),
        user_id(42),
        first_bill_id,
        &BillUpdateDraft {
            fields: json!({"amount": 15.0, "description": "Adjusted salary"})
                .as_object()
                .expect("object")
                .clone(),
            tag_ids: Some(vec![1]),
        },
    )?);
    assert_eq!(table_count(&runtime, "bill_pair_links")?, 0);
    assert_eq!(table_count(&runtime, "bill_transfer_pair_suppressions")?, 0);
    assert_eq!(table_count(&runtime, "bill_learning_rule_suppressions")?, 0);
    assert_eq!(balance(&runtime, 10)?, 135.0);

    let result = batch_update_bills(
        runtime.connection_mut(),
        user_id(42),
        &[first_bill_id, second_bill_id, 999],
        json!({"main_category": "收入", "sub_category": "工资"})
            .as_object()
            .expect("object"),
    )?;
    assert_eq!(result.success_count, 2);
    assert_eq!(result.failed_count, 1);
    assert_eq!(result.failed_ids, vec![999]);

    assert_eq!(
        batch_delete_bills(
            runtime.connection_mut(),
            user_id(42),
            &[first_bill_id, second_bill_id]
        )?,
        2
    );
    assert_eq!(balance(&runtime, 10)?, 100.0);

    let other_bill_id = create_bill(
        runtime.connection_mut(),
        user_id(77),
        &BillCreateDraft {
            fields: bill_fields("2026-05-10 12:00:00", "收入", 20.0, "ACME", "Other", 90),
            tag_ids: vec![3],
        },
    )?;
    assert!(!delete_bill(
        runtime.connection_mut(),
        user_id(42),
        other_bill_id
    )?);
    assert!(get_bill_by_id(runtime.connection(), user_id(77), other_bill_id)?.is_some());
    Ok(())
}

#[test]
fn query_filters_empty_mutations_and_investment_pnl_balance_edges() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-filter-edges.db"))?;
    init_schema(&runtime)?;

    assert!(batch_create_bills(runtime.connection_mut(), user_id(42), &[])?.is_empty());
    assert!(!update_bill(
        runtime.connection_mut(),
        user_id(42),
        999,
        &BillUpdateDraft {
            fields: Map::new(),
            tag_ids: None,
        },
    )?);
    assert_eq!(
        batch_update_bills(runtime.connection_mut(), user_id(42), &[], &Map::new())?,
        Default::default()
    );
    assert_eq!(
        batch_delete_bills(runtime.connection_mut(), user_id(42), &[])?,
        0
    );
    assert!(!delete_bill(runtime.connection_mut(), user_id(42), 999)?);

    let mut may_salary = bill_fields(
        "2026-05-08 10:00:00",
        "收入",
        25.0,
        "ACME",
        "May Salary",
        10,
    );
    may_salary.insert("batch_id".to_string(), Value::String("batch-a".to_string()));
    let salary_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: may_salary,
            tag_ids: vec![1],
        },
    )?;
    let mut june_bonus = bill_fields(
        "2026-06-08 10:00:00",
        "收入",
        40.0,
        "ACME",
        "June Bonus",
        10,
    );
    june_bonus.insert("batch_id".to_string(), Value::String("batch-b".to_string()));
    create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: june_bonus,
            tag_ids: vec![2],
        },
    )?;

    let page = query_bills(
        runtime.connection(),
        user_id(42),
        1,
        20,
        &BillFilters {
            id: Some(salary_id),
            date_from: Some("2026-05-01".to_string()),
            date_to: Some("2026-05-31".to_string()),
            transaction_type: Some("收入".to_string()),
            main_category: Some("工资".to_string()),
            sub_category: Some(String::new()),
            batch_id: Some("batch-a".to_string()),
            counterparty: Some("AC".to_string()),
            description: Some("Salary".to_string()),
            keyword: Some("May".to_string()),
            account_ids: vec![10],
            categories: vec![BillCategoryFilter {
                main: "工资".to_string(),
                sub: None,
            }],
            tag_ids: vec![1],
            min_amount: Some(20.0),
            max_amount: Some(30.0),
            amount_filter: Some("between:24:26".to_string()),
        },
    )?;
    assert_eq!(page.total, 1);
    assert_eq!(
        page.bills[0].get("id").and_then(Value::as_i64),
        Some(salary_id)
    );

    for filter in [
        "eq:25",
        "ne:40",
        "gt:24",
        "lt:26",
        "gte:25",
        "lte:25",
        "bad",
        "unknown:25",
    ] {
        let page = query_bills(
            runtime.connection(),
            user_id(42),
            1,
            20,
            &BillFilters {
                id: Some(salary_id),
                amount_filter: Some(filter.to_string()),
                ..BillFilters::default()
            },
        )?;
        assert_eq!(page.total, 1, "{filter}");
    }
    let invalid_amount = query_bills(
        runtime.connection(),
        user_id(42),
        1,
        20,
        &BillFilters {
            id: Some(salary_id),
            amount_filter: Some("eq:not-a-number".to_string()),
            ..BillFilters::default()
        },
    )?;
    assert_eq!(invalid_amount.total, 1);
    assert_eq!(
        invalid_amount.bills[0].get("id").and_then(Value::as_i64),
        Some(salary_id)
    );

    let mut investment_gain = bill_fields(
        "2026-05-09 10:00:00",
        "投资",
        5.0,
        "基金平台",
        "基金收益",
        10,
    );
    investment_gain.insert("destination_account_id".to_string(), json!(10));
    investment_gain.insert("destination_amount".to_string(), json!(0.0));
    create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: investment_gain,
            tag_ids: vec![],
        },
    )?;
    assert_eq!(balance(&runtime, 10)?, 170.0);

    Ok(())
}
