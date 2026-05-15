use std::error::Error;

use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_budget, create_budget_execution_snapshots, delete_budget, export_budgets,
    get_budget_by_id, import_budgets, query_budget_execution_details,
    query_budget_execution_history, query_budget_forecast, query_budgets_for_listing,
    update_budget, BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastFilters,
    BudgetRecord, BudgetUpdateDraft, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use serde_json::{json, Value};

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
        CREATE TABLE budgets (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            category TEXT,
            sub_category TEXT,
            period_type TEXT NOT NULL,
            amount REAL NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            alert_threshold INTEGER DEFAULT 80,
            enabled BOOLEAN DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            icon TEXT,
            color TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            date TEXT NOT NULL,
            main_category TEXT,
            sub_category TEXT,
            source_account_id INTEGER,
            destination_account_id INTEGER
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL
        );
        CREATE TABLE budget_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            budget_id INTEGER NOT NULL,
            period_start TEXT NOT NULL,
            period_end TEXT NOT NULL,
            budget_amount REAL DEFAULT 0,
            spent_amount REAL DEFAULT 0,
            remaining_amount REAL,
            execution_rate REAL DEFAULT 0,
            status TEXT,
            filter_summary TEXT DEFAULT '',
            calculated_at TEXT NOT NULL
        );
        ",
    )?;
    runtime.connection().execute(
        "INSERT INTO users(id, username) VALUES (42, 'owner'), (77, 'other')",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, icon, color, created_at)
         VALUES (1, 42, 1, '餐饮', '', 'folder', '#ffaa00', 'now'),
                (2, 42, 3, '餐饮', '午餐', 'tag', '#ffaa00', 'now'),
                (3, 42, 3, '餐饮', '晚餐', 'tag', '#ffaa00', 'now'),
                (4, 77, 3, '餐饮', '午餐', 'tag', '#000000', 'now')",
        [],
    )?;
    Ok(())
}

fn budget_fields(
    category: &str,
    sub_category: &str,
    amount: f64,
    start_date: &str,
) -> BudgetRecord {
    json!({
        "name": "",
        "category": category,
        "sub_category": sub_category,
        "period_type": "monthly",
        "amount": amount,
        "start_date": start_date,
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": true,
        "created_at": "2026-03-01 00:00:00",
        "updated_at": "2026-03-01 00:00:00"
    })
    .as_object()
    .expect("object")
    .clone()
}

fn text(row: &BudgetRecord, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn amount(row: &BudgetRecord) -> f64 {
    row.get("amount")
        .and_then(Value::as_f64)
        .unwrap_or_default()
}

#[test]
fn budget_crud_is_user_scoped_and_syncs_primary_and_period_hierarchy() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("budgets-crud.db"))?;
    init_schema(&runtime)?;

    let lunch_id = create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "午餐", 100.0, "2026-03-01"),
        },
    )?;
    assert!(get_budget_by_id(runtime.connection(), user_id(77), lunch_id)?.is_none());

    let mut monthly = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("monthly".to_string()),
            budget_type: Some(3),
            ..BudgetFilters::default()
        },
    )?;
    monthly.sort_by_key(|row| text(row, "sub_category"));
    assert_eq!(monthly.len(), 2);
    assert_eq!(amount(&monthly[0]), 100.0);
    assert_eq!(amount(&monthly[1]), 100.0);
    assert_eq!(monthly[1]["category_id"], "2");
    assert_eq!(monthly[1]["type"], 3);

    let primary_id = monthly
        .iter()
        .find(|row| text(row, "sub_category").is_empty())
        .and_then(|row| row.get("id").and_then(Value::as_i64))
        .expect("primary budget id");
    assert!(update_budget(
        runtime.connection_mut(),
        user_id(42),
        primary_id,
        &BudgetUpdateDraft {
            fields: json!({
                "amount": 180.0,
                "updated_at": "2026-03-02 00:00:00"
            })
            .as_object()
            .expect("object")
            .clone(),
        },
    )?);
    create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "晚餐", 70.0, "2026-03-01"),
        },
    )?;
    let monthly = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("monthly".to_string()),
            budget_type: Some(3),
            ..BudgetFilters::default()
        },
    )?;
    let primary = monthly
        .iter()
        .find(|row| text(row, "sub_category").is_empty())
        .expect("primary budget");
    assert_eq!(amount(primary), 180.0_f64.max(170.0));

    create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "夜宵", 30.0, "2026-03-01"),
        },
    )?;
    let monthly = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("monthly".to_string()),
            budget_type: Some(3),
            ..BudgetFilters::default()
        },
    )?;
    let primary = monthly
        .iter()
        .find(|row| text(row, "sub_category").is_empty())
        .expect("primary budget");
    assert_eq!(amount(primary), 200.0);

    let quarterly = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("quarterly".to_string()),
            budget_type: Some(3),
            ..BudgetFilters::default()
        },
    )?;
    assert!(quarterly.iter().any(|row| amount(row) >= 200.0));
    let yearly = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("yearly".to_string()),
            budget_type: Some(3),
            ..BudgetFilters::default()
        },
    )?;
    assert!(yearly.iter().any(|row| amount(row) >= 200.0));

    assert!(delete_budget(
        runtime.connection_mut(),
        user_id(42),
        primary_id
    )?);
    let monthly_after_delete = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("monthly".to_string()),
            ..BudgetFilters::default()
        },
    )?;
    assert!(monthly_after_delete.is_empty());
    let quarterly_after_delete = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("quarterly".to_string()),
            ..BudgetFilters::default()
        },
    )?;
    assert!(quarterly_after_delete.is_empty());
    let yearly_after_delete = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            category: Some("餐饮".to_string()),
            period_type: Some("yearly".to_string()),
            ..BudgetFilters::default()
        },
    )?;
    assert!(yearly_after_delete.is_empty());
    assert!(
        delete_budget(runtime.connection_mut(), user_id(42), primary_id)
            .is_ok_and(|deleted| !deleted)
    );

    Ok(())
}

#[test]
fn budget_filters_update_edges_and_export_contract() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("budgets-filter.db"))?;
    init_schema(&runtime)?;

    let budget_id = create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "", 88.0, "2026-04-01"),
        },
    )?;
    assert!(!update_budget(
        runtime.connection_mut(),
        user_id(42),
        999,
        &BudgetUpdateDraft {
            fields: json!({"amount": 1.0}).as_object().expect("object").clone(),
        },
    )?);
    assert!(update_budget(
        runtime.connection_mut(),
        user_id(42),
        budget_id,
        &BudgetUpdateDraft {
            fields: json!({
                "enabled": false,
                "updated_at": "2026-04-02 00:00:00"
            })
            .as_object()
            .expect("object")
            .clone(),
        },
    )?);
    let disabled = query_budgets_for_listing(
        runtime.connection(),
        user_id(42),
        &BudgetFilters {
            enabled: Some(false),
            period_type: Some("monthly".to_string()),
            ..BudgetFilters::default()
        },
    )?;
    assert_eq!(disabled.len(), 1);
    assert_eq!(disabled[0]["enabled"], 0);

    let exported = export_budgets(runtime.connection(), user_id(42))?;
    assert_eq!(exported.len(), 3);
    assert!(exported
        .iter()
        .any(|item| item["category"] == "餐饮" && item["amount"] == 88.0));

    Ok(())
}

#[test]
fn budget_import_upserts_by_name_counts_item_errors_and_exports_rows() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("budgets-import.db"))?;
    init_schema(&runtime)?;

    let result = import_budgets(
        runtime.connection_mut(),
        user_id(42),
        &[
            json!({
                "name": "导入预算A",
                "category": "导入预算分类",
                "sub_category": "",
                "period_type": "monthly",
                "amount": 40.0,
                "start_date": "2026-04-01",
                "end_date": "2026-04-30",
                "alert_threshold": 70,
                "enabled": true
            })
            .as_object()
            .expect("object")
            .clone(),
            json!({
                "name": "导入预算A",
                "category": "导入预算分类",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 55.0,
                "start_date": "2026-04-01",
                "end_date": "2026-04-30",
                "enabled": false
            })
            .as_object()
            .expect("object")
            .clone(),
            json!({
                "name": "",
                "category": "缺失名称分类",
                "period_type": "monthly",
                "amount": 10.0,
                "start_date": "2026-04-01"
            })
            .as_object()
            .expect("object")
            .clone(),
        ],
    )?;

    assert_eq!(result["created"], 1);
    assert_eq!(result["updated"], 1);
    assert_eq!(result["errors"], 1);
    assert_eq!(
        result["error_details"][0],
        "第3条: 缺少必填字段(name或amount)"
    );

    let exported = export_budgets(runtime.connection(), user_id(42))?;
    let imported = exported
        .iter()
        .find(|item| item["name"] == "导入预算A")
        .expect("imported budget");
    assert_eq!(imported["amount"], 55.0);
    assert_eq!(imported["sub_category"], "午餐");
    assert_eq!(imported["alert_threshold"], 80);
    assert_eq!(imported["enabled"], 0);
    assert!(!exported.iter().any(|item| item["name"] == ""));

    Ok(())
}

#[test]
fn budget_execution_details_are_user_scoped_filtered_and_deduped() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("budgets-execution.db"))?;
    init_schema(&runtime)?;

    create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "午餐", 100.0, "2026-03-01"),
        },
    )?;
    create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "晚餐", 80.0, "2026-03-01"),
        },
    )?;
    runtime.connection().execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -35.5, '2026-03-15 12:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -20.0, '2026-03-31 23:00:00', '餐饮', '晚餐', NULL, 11),
                (42, '支出', -7.0, '2026-04-01 00:00:00', '餐饮', '午餐', 10, NULL),
                (77, '支出', -99.0, '2026-03-15 12:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO bill_tags(bill_id, tag_id) VALUES (1, 8), (2, 9), (3, 8), (4, 8)",
        [],
    )?;

    let items = query_budget_execution_details(
        runtime.connection(),
        user_id(42),
        &BudgetExecutionFilters {
            budget_type: 3,
            period_type: Some("monthly".to_string()),
            start_date: Some("2026-03-01".to_string()),
            end_date: Some("2026-03-31".to_string()),
            account_ids: Some(vec![10, 11]),
            ..BudgetExecutionFilters::default()
        },
    )?;
    assert_eq!(items.len(), 3);
    assert!(items.iter().any(|item| {
        item["sub_category"] == "午餐"
            && item["spent_amount"] == 35.5
            && item["remaining_amount"] == 64.5
            && item["execution_rate"] == 35.5
    }));
    assert!(items
        .iter()
        .any(|item| item["sub_category"] == "晚餐" && item["spent_amount"] == 20.0));
    assert!(items
        .iter()
        .any(|item| item["sub_category"] == "" && item["spent_amount"] == 55.5));

    let tag_filtered = query_budget_execution_details(
        runtime.connection(),
        user_id(42),
        &BudgetExecutionFilters {
            budget_type: 3,
            period_type: Some("monthly".to_string()),
            start_date: Some("2026-03-01".to_string()),
            end_date: Some("2026-03-31".to_string()),
            tag_ids: Some(vec![8]),
            ..BudgetExecutionFilters::default()
        },
    )?;
    assert!(tag_filtered
        .iter()
        .any(|item| item["sub_category"] == "午餐" && item["spent_amount"] == 35.5));
    assert!(tag_filtered
        .iter()
        .any(|item| item["sub_category"] == "晚餐" && item["spent_amount"] == 0.0));

    let missing_category = query_budget_execution_details(
        runtime.connection(),
        user_id(42),
        &BudgetExecutionFilters {
            budget_type: 3,
            category_id: Some(404),
            ..BudgetExecutionFilters::default()
        },
    )?;
    assert!(missing_category.is_empty());

    Ok(())
}

#[test]
fn budget_history_snapshots_prefer_exact_rows_and_fall_back_on_demand() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("budgets-history.db"))?;
    init_schema(&runtime)?;

    let lunch_id = create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "午餐", 100.0, "2026-02-01"),
        },
    )?;
    runtime.connection().execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -35.5, '2026-03-15 12:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -20.0, '2026-02-15 12:00:00', '餐饮', '午餐', 10, NULL),
                (77, '支出', -999.0, '2026-03-15 12:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;
    runtime.connection().execute(
        "INSERT INTO bill_tags(bill_id, tag_id) VALUES (1, 8), (2, 8), (3, 8)",
        [],
    )?;

    let march_filters = BudgetExecutionFilters {
        budget_type: 3,
        period_type: Some("monthly".to_string()),
        start_date: Some("2026-03-01".to_string()),
        end_date: Some("2026-03-31".to_string()),
        tag_ids: Some(vec![8]),
        ..BudgetExecutionFilters::default()
    };
    let snapshot_result =
        create_budget_execution_snapshots(runtime.connection_mut(), user_id(42), &march_filters)?;
    assert_eq!(snapshot_result["created_count"], 2);
    assert_eq!(snapshot_result["period_start"], "2026-03-01");
    assert!(snapshot_result["filter_summary"]
        .as_str()
        .unwrap_or_default()
        .contains("\"tag_ids\": [8]"));

    runtime.connection().execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -40.0, '2026-03-20 12:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;
    runtime
        .connection()
        .execute("INSERT INTO bill_tags(bill_id, tag_id) VALUES (4, 8)", [])?;

    let stored_history =
        query_budget_execution_history(runtime.connection(), user_id(42), &march_filters)?;
    let stored_lunch = stored_history
        .iter()
        .find(|item| item["budget_id"] == lunch_id)
        .expect("stored lunch snapshot");
    assert_eq!(stored_lunch["spent_amount"], 35.5);
    assert_eq!(stored_lunch["status"], "within_budget");
    assert_eq!(stored_lunch["category_info"]["id"], 2);

    let replaced =
        create_budget_execution_snapshots(runtime.connection_mut(), user_id(42), &march_filters)?;
    assert_eq!(replaced["created_count"], 2);
    let updated_history =
        query_budget_execution_history(runtime.connection(), user_id(42), &march_filters)?;
    let updated_lunch = updated_history
        .iter()
        .find(|item| item["budget_id"] == lunch_id)
        .expect("updated lunch snapshot");
    assert_eq!(updated_lunch["spent_amount"], 75.5);

    let february_history = query_budget_execution_history(
        runtime.connection(),
        user_id(42),
        &BudgetExecutionFilters {
            start_date: Some("2026-02-01".to_string()),
            end_date: Some("2026-02-28".to_string()),
            ..march_filters
        },
    )?;
    let february_lunch = february_history
        .iter()
        .find(|item| item["budget_id"] == lunch_id)
        .expect("on-demand lunch history");
    assert_eq!(february_lunch["id"], "1_2026-02-01_2026-02-28");
    assert_eq!(february_lunch["spent_amount"], 20.0);

    Ok(())
}

#[test]
fn budget_forecast_uses_history_window_budget_map_and_current_spend() -> Result<(), Box<dyn Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let mut runtime = runtime_for(&temp_dir.path().join("budgets-forecast.db"))?;
    init_schema(&runtime)?;

    create_budget(
        runtime.connection_mut(),
        user_id(42),
        &BudgetCreateDraft {
            fields: budget_fields("餐饮", "午餐", 100.0, "2026-03-01"),
        },
    )?;
    runtime.connection().execute(
        "INSERT INTO bills(user_id, type, amount, date, main_category, sub_category, source_account_id, destination_account_id)
         VALUES (42, '支出', -10.0, '2026-01-15 08:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -20.0, '2026-02-15 08:00:00', '餐饮', '午餐', 10, NULL),
                (42, '支出', -35.0, '2026-03-31 23:00:00', '餐饮', '午餐', 10, NULL),
                (42, '投资', -99.0, '2026-03-15 08:00:00', '餐饮', '午餐', 10, NULL),
                (77, '支出', -999.0, '2026-03-15 08:00:00', '餐饮', '午餐', 10, NULL)",
        [],
    )?;

    let items = query_budget_forecast(
        runtime.connection(),
        user_id(42),
        &BudgetForecastFilters {
            budget_type: 3,
            period_type: "monthly".to_string(),
            start_date: "2026-03-01".to_string(),
            end_date: "2026-03-31".to_string(),
            forecast_strategy: "historical_average".to_string(),
            history_periods: 3,
        },
    )?;

    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["category"], "餐饮");
    assert_eq!(item["period_count"], 3);
    assert_eq!(item["sample_periods"], 3);
    assert_eq!(item["current_spent"], 35.0);
    assert_eq!(item["budget_amount"], 100.0);
    assert_eq!(item["forecast_amount"], 21.67);
    assert_eq!(item["total_amount"], 65.0);
    assert_eq!(item["periods"][0]["period"], "2026-01");
    assert_eq!(item["periods"][2]["period"], "2026-03");
    assert_eq!(item["category_info"]["id"], 1);

    Ok(())
}
