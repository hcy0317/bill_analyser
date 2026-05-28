use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    batch_create_bills, batch_delete_bills, batch_update_bills, bind_bill_to_recurring,
    calculate_bill_hash_from_fields, create_bill, delete_bill, get_bill_by_id,
    get_bill_recurring_candidates, get_bill_tags, query_bills, sync_all_account_balances,
    unbind_bill_from_recurring, update_bill, BillCategoryFilter, BillCreateDraft, BillFilters,
    BillRecord, BillUpdateDraft, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
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
        CREATE TABLE recurring_bills (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            template_id INTEGER,
            name TEXT NOT NULL,
            description TEXT,
            type INTEGER,
            category TEXT,
            amount REAL,
            account TEXT,
            counterparty TEXT,
            destination_amount REAL,
            hide_amount INTEGER DEFAULT 0,
            tag TEXT,
            comment TEXT,
            frequency TEXT,
            scheduled_frequency_type INTEGER,
            start_date TEXT,
            end_date TEXT,
            next_date TEXT,
            enabled INTEGER DEFAULT 1,
            auto_create INTEGER DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            utc_offset INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
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

#[allow(clippy::too_many_arguments)]
fn insert_recurring_template(
    runtime: &SqliteRuntime,
    recurring_id: i64,
    name: &str,
    tx_type: i64,
    amount_cents: f64,
    account: &str,
    counterparty: &str,
    frequency_type: i64,
    frequency: &str,
    start_date: &str,
    end_date: &str,
    next_date: &str,
    display_order: i64,
) -> Result<(), Box<dyn Error>> {
    runtime.connection().execute(
        "INSERT INTO recurring_bills(
            id, user_id, name, type, category, amount, account, counterparty,
            destination_amount, hide_amount, tag, comment, frequency,
            scheduled_frequency_type, start_date, end_date, next_date,
            enabled, display_order, hidden, utc_offset, created_at, updated_at
         ) VALUES (?1, 42, ?2, ?3, '1', ?4, ?5, ?6, 0, 0, ?7, 'note',
            ?8, ?9, ?10, ?11, ?12, 1, ?13, 0, 480, 'now', 'now')",
        rusqlite::params![
            recurring_id,
            name,
            tx_type,
            amount_cents,
            account,
            counterparty,
            b"1,2".as_slice(),
            frequency,
            frequency_type,
            start_date,
            end_date,
            next_date,
            display_order
        ],
    )?;
    Ok(())
}

fn recurring_next_date(
    runtime: &SqliteRuntime,
    recurring_id: i64,
) -> Result<String, Box<dyn Error>> {
    Ok(runtime.connection().query_row(
        "SELECT next_date FROM recurring_bills WHERE id = ?1",
        [recurring_id],
        |row| row.get::<_, String>(0),
    )?)
}

#[test]
fn recurring_candidates_bind_and_unbind_match_python_schedule_edges() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-recurring.db"))?;
    init_schema(&runtime)?;

    insert_recurring_template(
        &runtime,
        501,
        "weekly rent",
        3,
        1234.0,
        "10",
        "0",
        1,
        "",
        "2026-05-03",
        "",
        "2026-05-03",
        2,
    )?;
    insert_recurring_template(
        &runtime,
        502,
        "monthly rent",
        3,
        1234.0,
        "10",
        "0",
        2,
        "9",
        "2026-05-01",
        "",
        "2026-05-09",
        1,
    )?;
    insert_recurring_template(
        &runtime,
        503,
        "wrong type",
        2,
        1234.0,
        "10",
        "0",
        2,
        "10",
        "2026-05-01",
        "",
        "2026-05-10",
        3,
    )?;
    insert_recurring_template(
        &runtime,
        504,
        "wrong amount",
        3,
        9999.0,
        "10",
        "0",
        2,
        "10",
        "2026-05-01",
        "",
        "2026-05-10",
        4,
    )?;
    insert_recurring_template(
        &runtime,
        505,
        "inactive",
        3,
        1234.0,
        "10",
        "0",
        2,
        "10",
        "2026-04-01",
        "2026-04-30",
        "2026-04-10",
        5,
    )?;

    let bill_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields: bill_fields("2026-05-10 09:00:00", "支出", 12.34, "Landlord", "Rent", 10),
            tag_ids: vec![],
        },
    )?;

    let candidates = get_bill_recurring_candidates(runtime.connection(), user_id(42), bill_id, 2)?
        .expect("bill exists");
    assert_eq!(candidates.linked_recurring_id, None);
    assert_eq!(candidates.candidates.len(), 2);
    assert_eq!(candidates.candidates[0]["id"], "501");
    assert_eq!(
        candidates.candidates[0]["matchedOccurrenceDate"],
        "2026-05-10"
    );
    assert_eq!(candidates.candidates[0]["scheduledFrequency"], "");
    assert_eq!(candidates.candidates[0]["scheduledEndDate"], "");
    assert_eq!(candidates.candidates[0]["matchedDayOffset"], 0);
    assert_eq!(candidates.candidates[0]["tagIds"], json!(["1", "2"]));
    assert_eq!(candidates.candidates[1]["id"], "502");
    assert_eq!(candidates.candidates[1]["matchedDayOffset"], 1);

    assert!(bind_bill_to_recurring(runtime.connection_mut(), user_id(42), 999, 501)?.is_none());
    assert!(bind_bill_to_recurring(runtime.connection_mut(), user_id(42), bill_id, 999)?.is_none());

    let bound = bind_bill_to_recurring(runtime.connection_mut(), user_id(42), bill_id, 501)?
        .expect("bind succeeds");
    assert_eq!(bound.recurring_id, 501);
    assert_eq!(bound.next_scheduled_date.as_deref(), Some("2026-05-17"));
    assert_eq!(recurring_next_date(&runtime, 501)?, "2026-05-17");

    let rebound = bind_bill_to_recurring(runtime.connection_mut(), user_id(42), bill_id, 502)?
        .expect("rebind succeeds");
    assert_eq!(rebound.recurring_id, 502);
    assert_eq!(rebound.next_scheduled_date.as_deref(), Some("2026-06-09"));
    assert_eq!(recurring_next_date(&runtime, 501)?, "2026-05-03");

    let linked = get_bill_recurring_candidates(runtime.connection(), user_id(42), bill_id, 31)?
        .expect("bill exists");
    assert_eq!(linked.linked_recurring_id, Some(502));
    assert_eq!(linked.linked_recurring_name, "monthly rent");
    assert!(linked
        .candidates
        .iter()
        .any(|candidate| { candidate["id"] == "502" && candidate["linked"] == true }));

    assert!(unbind_bill_from_recurring(runtime.connection_mut(), user_id(42), 999)?.is_none());
    assert_eq!(
        unbind_bill_from_recurring(runtime.connection_mut(), user_id(42), bill_id)?,
        Some(true)
    );
    assert_eq!(recurring_next_date(&runtime, 502)?, "2026-05-09");

    runtime.connection().execute(
        "INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description,
            source_account_id, destination_account_id, created_at, updated_at
         ) VALUES (900, 42, 'bad-date', '支出', 12.34, 'n/a', 'bad date', 10, 0, 'now', 'now')",
        [],
    )?;
    let invalid_date = get_bill_recurring_candidates(runtime.connection(), user_id(42), 900, 31)?
        .expect("bad-date bill exists");
    assert!(invalid_date.candidates.is_empty());

    Ok(())
}

#[test]
fn recurring_transfer_candidates_score_destination_and_one_time_schedule(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-recurring-transfer.db"))?;
    init_schema(&runtime)?;
    insert_recurring_template(
        &runtime,
        601,
        "card payment",
        4,
        2000.0,
        "10",
        "20",
        0,
        "",
        "2026-05-12",
        "",
        "2026-05-12",
        0,
    )?;

    let mut fields = bill_fields(
        "2026-05-12 08:00:00",
        "转账",
        20.0,
        "Credit Card",
        "Card payment",
        10,
    );
    fields.insert("destination_account_id".to_string(), json!(20));
    let bill_id = create_bill(
        runtime.connection_mut(),
        user_id(42),
        &BillCreateDraft {
            fields,
            tag_ids: vec![],
        },
    )?;

    let candidates = get_bill_recurring_candidates(runtime.connection(), user_id(42), bill_id, 0)?
        .expect("bill exists");
    assert_eq!(candidates.candidates.len(), 1);
    assert_eq!(candidates.candidates[0]["id"], "601");
    assert_eq!(
        candidates.candidates[0]["matchReasons"],
        json!([
            "type",
            "amount",
            "schedule",
            "source_account",
            "destination_account"
        ])
    );
    assert_eq!(candidates.candidates[0]["matchScore"], 110);

    Ok(())
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
fn sync_all_account_balances_reports_discrepancies_errors_and_user_scope(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("bills-sync-all-balances.db"))?;
    init_schema(&runtime)?;

    runtime.connection().execute(
        "INSERT INTO bills(
            id, user_id, date, type, amount, counterparty, description,
            source_account_id, destination_account_id, destination_amount, created_at, updated_at
         ) VALUES
            (1001, 42, '2026-05-08 10:00:00', '收入', 25.0, 'ACME', 'Salary', 10, 0, 0, 'now', 'now'),
            (1002, 42, '2026-05-08 11:00:00', '未知', 5.0, 'Bad', 'Unsupported type', 20, 0, 0, 'now', 'now'),
            (1003, 77, '2026-05-08 12:00:00', '收入', 50.0, 'Other', 'Other user', 90, 0, 0, 'now', 'now')",
        [],
    )?;

    let result = sync_all_account_balances(runtime.connection_mut(), user_id(42))?;

    assert_eq!(result.total_accounts, 2);
    assert_eq!(result.synced_accounts, 1);
    assert_eq!(result.discrepancies.len(), 1);
    assert_eq!(result.discrepancies[0].account_id, 10);
    assert_eq!(result.discrepancies[0].name, "cash");
    assert_eq!(result.discrepancies[0].old_balance, 100.0);
    assert_eq!(result.discrepancies[0].new_balance, 125.0);
    assert_eq!(result.discrepancies[0].diff, 25.0);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("broker"));
    assert_eq!(balance(&runtime, 10)?, 125.0);
    assert_eq!(balance(&runtime, 20)?, 0.0);
    assert_eq!(balance(&runtime, 90)?, 100.0);

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
