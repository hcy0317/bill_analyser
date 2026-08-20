use std::collections::BTreeMap;

use bill_analyser_core::budgets::{
    budget_overlaps_period, budget_type_matches_category, build_budget_category_context,
    build_budget_execution_summary, build_budget_export_response, build_budget_forecast_item,
    build_budget_forecast_item_from_input, build_budget_history_filter_summary,
    build_budget_history_item_from_detail, build_budget_period_scope, calculate_avg_backtest_mape,
    calculate_budget_period_progress, calculate_forecast_amount_cents,
    calculate_forecast_backtest_mape, expand_forecast_history_window, get_budget_type_name,
    iter_budget_history_period_ranges, normalize_budget_category_type,
    normalize_budget_query_end_date, parse_budget_csv_int_list, parse_budget_json_int_list,
    resolve_budget_category_info, resolve_budget_category_type, resolve_budget_period_range,
    resolve_forecast_budget_amount_cents, resolve_forecast_confidence, resolve_forecast_trend,
    rollup_parent_amount_cents, rollup_yearly_child_total_cents, select_budget_detail_items,
    select_budget_summary_items, validate_budget_date_range, validate_budget_period_args,
    validate_import_budget_item, BudgetForecastItemInput, BudgetHistoryFilterSummaryInput,
    BudgetPeriodKind, BudgetPeriodScopeInput, BudgetRouteFilters, BUDGET_TYPE_EXPENSE,
    BUDGET_TYPE_INVESTMENT,
};
use bill_analyser_core::TransactionType;
use chrono::NaiveDate;
use serde_json::{json, Value};

fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("test date")
}

fn ids(items: &[Value]) -> Vec<i64> {
    items
        .iter()
        .map(|item| item["id"].as_i64().expect("id"))
        .collect()
}

#[test]
fn budget_period_kind_strictly_owns_period_identity() {
    for raw in ["daily", "weekly", "monthly", "quarterly", "yearly"] {
        let period = BudgetPeriodKind::parse(raw).expect("known budget period");
        assert_eq!(period.as_str(), raw);
    }

    assert_eq!(
        BudgetPeriodKind::parse("fortnight"),
        Err("Invalid period_type: fortnight".to_string())
    );
    assert_eq!(
        BudgetPeriodKind::parse(" monthly "),
        Err("Invalid period_type:  monthly ".to_string())
    );
}

#[test]
fn budget_period_kind_owns_containing_ranges_and_bucket_keys() {
    let cases = [
        (
            BudgetPeriodKind::Daily,
            "2026-05-07",
            "2026-05-07",
            "2026-05-07",
        ),
        (
            BudgetPeriodKind::Weekly,
            "2021-01-01",
            "2020-12-28",
            "2021-01-03",
        ),
        (
            BudgetPeriodKind::Monthly,
            "2024-02-15",
            "2024-02-01",
            "2024-02-29",
        ),
        (
            BudgetPeriodKind::Quarterly,
            "2026-05-07",
            "2026-04-01",
            "2026-06-30",
        ),
        (
            BudgetPeriodKind::Yearly,
            "2026-05-07",
            "2026-01-01",
            "2026-12-31",
        ),
    ];
    for (period, anchor, expected_start, expected_end) in cases {
        let range = period.containing(date(anchor)).expect("containing range");
        assert_eq!(range.start_date, expected_start);
        assert_eq!(range.end_date, expected_end);
    }

    let sample_day = date("2026-05-07");
    assert_eq!(BudgetPeriodKind::Daily.bucket_key(sample_day), "2026-05-07");
    assert_eq!(
        BudgetPeriodKind::Weekly.bucket_key(date("2021-01-01")),
        "2021-00"
    );
    assert_eq!(
        BudgetPeriodKind::Weekly.bucket_key(date("2021-01-10")),
        "2021-01"
    );
    assert_eq!(BudgetPeriodKind::Monthly.bucket_key(sample_day), "2026-05");
    assert_eq!(
        BudgetPeriodKind::Quarterly.bucket_key(sample_day),
        "2026-Q2"
    );
    assert_eq!(BudgetPeriodKind::Yearly.bucket_key(sample_day), "2026");
}

#[test]
fn route_period_scope_and_filter_parsing_match_budget_contract() {
    let today = date("2026-05-07");

    let explicit = build_budget_period_scope(
        &BudgetPeriodScopeInput {
            budget_type: Some(BUDGET_TYPE_INVESTMENT),
            period_type: Some("monthly".to_string()),
            start_date: Some("2026-02-03".to_string()),
            end_date: Some("2026-02-28".to_string()),
            ..Default::default()
        },
        today,
    )
    .expect("explicit range");
    assert_eq!(explicit.budget_type, BUDGET_TYPE_INVESTMENT);
    assert_eq!(explicit.start_date, "2026-02-03");
    assert_eq!(explicit.end_date, "2026-02-28");

    let weekly = build_budget_period_scope(
        &BudgetPeriodScopeInput {
            period_type: Some("weekly".to_string()),
            ..Default::default()
        },
        today,
    )
    .expect("weekly range");
    assert_eq!(weekly.start_date, "2026-05-04");
    assert_eq!(weekly.end_date, "2026-05-10");

    let monthly = build_budget_period_scope(
        &BudgetPeriodScopeInput {
            year: Some(2026),
            month: Some(2),
            ..Default::default()
        },
        today,
    )
    .expect("monthly range");
    assert_eq!(monthly.period_type, "monthly");
    assert_eq!(monthly.start_date, "2026-02-01");
    assert_eq!(monthly.end_date, "2026-02-28");

    let quarterly = build_budget_period_scope(
        &BudgetPeriodScopeInput {
            period_type: Some("quarterly".to_string()),
            year: Some(2026),
            quarter: Some(4),
            ..Default::default()
        },
        today,
    )
    .expect("quarterly range");
    assert_eq!(quarterly.start_date, "2026-10-01");
    assert_eq!(quarterly.end_date, "2026-12-31");

    let yearly = build_budget_period_scope(
        &BudgetPeriodScopeInput {
            period_type: Some("yearly".to_string()),
            year: Some(2027),
            ..Default::default()
        },
        today,
    )
    .expect("yearly range");
    assert_eq!(yearly.start_date, "2027-01-01");
    assert_eq!(yearly.end_date, "2027-12-31");

    let progress =
        calculate_budget_period_progress("2026-05-01", "2026-05-10", today).expect("progress");
    assert_eq!(progress.elapsed_days, 7);
    assert_eq!(progress.remaining_days, 3);

    assert_eq!(
        parse_budget_csv_int_list(Some(" 1,2,,3 "), "account_ids").expect("csv ids"),
        Some(vec![1, 2, 3])
    );
    assert_eq!(
        parse_budget_json_int_list(Some(&json!(["7", 8, ""])), "tag_ids").expect("json ids"),
        vec![7, 8]
    );
    assert!(parse_budget_json_int_list(Some(&json!("bad")), "tag_ids").is_err());

    let route_filters = BudgetRouteFilters {
        budget_id: Some(9),
        category_id: Some(11),
        account_ids: Some(vec![1, 2]),
        tag_ids: None,
    };
    assert_eq!(route_filters.budget_id, Some(9));
    assert_eq!(route_filters.tag_ids, None);
}

#[test]
fn import_export_and_avg_mape_contracts_match_budget_routes() {
    let import_item = json!({
        "period_type": "monthly",
        "amount_cents": 128825,
        "start_date": "2026-05-01",
        "end_date": "2026-05-31",
        "category": "餐饮"
    });
    validate_import_budget_item(&import_item, 1).expect("valid import item");
    assert!(validate_import_budget_item(&json!({"period_type": "monthly"}), 2).is_err());
    assert!(validate_import_budget_item(
        &json!({
            "period_type": "weekly",
            "amount_cents": 1,
            "start_date": "2026-06-02",
            "end_date": "2026-06-01",
            "category": "交通"
        }),
        3
    )
    .is_err());

    let export = build_budget_export_response(&[json!({
        "name": "五月餐饮",
        "category": "餐饮",
        "sub_category": "外卖",
        "period_type": "monthly",
        "amount_cents": 80000,
        "start_date": "2026-05-01",
        "end_date": "2026-05-31",
        "alert_threshold": 80,
        "enabled": true,
        "created_at": "ignored"
    })]);
    assert_eq!(export["success"], true);
    assert_eq!(export["result"][0]["category"], "餐饮");
    assert!(export["result"][0].get("created_at").is_none());

    let avg = calculate_avg_backtest_mape(&[
        json!({"backtest_mape": 8.335}),
        json!({"backtest_mape": null}),
        json!({"backtest_mape": 11.665}),
    ]);
    assert_eq!(avg, Some(10.0));
}

#[test]
fn execution_summary_avoids_double_counting_synchronized_primary_budget() {
    let items = vec![
        json!({
            "id": 1,
            "category": "餐饮",
            "sub_category": "",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 30000,
            "spent_amount_cents": 12000,
            "remaining_amount_cents": 18000
        }),
        json!({
            "id": 2,
            "category": "餐饮",
            "sub_category": "外卖",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 10000,
            "spent_amount_cents": 5000,
            "remaining_amount_cents": 5000
        }),
        json!({
            "id": 3,
            "category": "餐饮",
            "sub_category": "堂食",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 20000,
            "spent_amount_cents": 7000,
            "remaining_amount_cents": 13000
        }),
    ];

    assert_eq!(ids(&select_budget_detail_items(&items)), vec![2, 3]);
    assert_eq!(ids(&select_budget_summary_items(&items)), vec![1]);

    let summary = build_budget_execution_summary(&items);
    assert_eq!(summary["total_budget_cents"], 30000);
    assert_eq!(summary["total_spent_cents"], 12000);
    assert_eq!(summary["total_remaining_cents"], 18000);
    assert_eq!(summary["overall_execution_rate"], 40.0);
    assert_eq!(summary["count"], 1);
}

#[test]
fn execution_summary_preserves_manual_primary_headroom_and_ambiguous_primaries() {
    let manual_primary = vec![
        json!({
            "id": 10,
            "category": "交通",
            "sub_category": "",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 50000,
            "spent_amount_cents": 18000
        }),
        json!({
            "id": 11,
            "category": "交通",
            "sub_category": "地铁",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 10000,
            "spent_amount_cents": 6000
        }),
    ];
    assert_eq!(ids(&select_budget_detail_items(&manual_primary)), vec![10]);

    let duplicate_primaries = vec![
        json!({
            "id": 20,
            "category": "娱乐",
            "sub_category": "",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 10000
        }),
        json!({
            "id": 21,
            "category": "娱乐",
            "sub_category": "",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 12000
        }),
        json!({
            "id": 22,
            "category": "娱乐",
            "sub_category": "电影",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 8000
        }),
    ];
    assert_eq!(
        ids(&select_budget_detail_items(&duplicate_primaries)),
        vec![20, 21, 22]
    );
    assert_eq!(
        ids(&select_budget_summary_items(&duplicate_primaries)),
        vec![20, 21, 22]
    );

    let first_seen_groups = vec![
        json!({
            "id": 30,
            "category": "b-group",
            "sub_category": "",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 10001,
            "spent_amount_cents": 3333
        }),
        json!({
            "id": 31,
            "category": "a-group",
            "sub_category": "",
            "period_type": "monthly",
            "start_date": "2026-05-01",
            "budget_amount_cents": 5000,
            "spent_amount_cents": 1000
        }),
    ];
    assert_eq!(
        ids(&select_budget_summary_items(&first_seen_groups)),
        vec![30, 31]
    );

    let decimal_summary = build_budget_execution_summary(&first_seen_groups);
    assert_eq!(decimal_summary["total_budget_cents"], 15001);
    assert_eq!(decimal_summary["total_spent_cents"], 4333);
    assert_eq!(decimal_summary["overall_execution_rate"], 28.88);
}

#[test]
fn category_type_context_normalizes_current_expense_and_keeps_investment_separate() {
    let categories = vec![
        json!({"id": 1, "main_category": "基金", "sub_category": "", "type": 1, "icon": "old", "color": "#aaa"}),
        json!({"id": 2, "main_category": "基金", "sub_category": "", "type": 5, "icon": "fund", "color": "#0a0"}),
        json!({"id": 3, "main_category": "餐饮", "sub_category": "外卖", "type": 3, "icon": "meal", "color": "#f80"}),
        json!({"id": 4, "main_category": "餐饮", "sub_category": "", "type": 3, "icon": "food", "color": "#f00"}),
    ];
    let context = build_budget_category_context(&categories);

    assert_eq!(
        normalize_budget_category_type(Some(1)),
        Some(TransactionType::Expense)
    );
    assert_eq!(
        resolve_budget_category_type(&context, "基金", None, Some(BUDGET_TYPE_EXPENSE)),
        Some(TransactionType::Expense)
    );
    assert_eq!(
        resolve_budget_category_type(&context, "基金", None, Some(BUDGET_TYPE_INVESTMENT)),
        Some(TransactionType::Investment)
    );
    assert_eq!(
        resolve_budget_category_type(&context, "基金", None, None),
        Some(TransactionType::Expense)
    );
    assert_eq!(
        resolve_budget_category_type(&context, "餐饮", Some("不存在"), Some(3)),
        None
    );
    assert!(budget_type_matches_category(
        &context,
        "餐饮",
        Some("外卖"),
        BUDGET_TYPE_EXPENSE
    ));
    assert!(!budget_type_matches_category(
        &context,
        "餐饮",
        Some("外卖"),
        BUDGET_TYPE_INVESTMENT
    ));

    let info = resolve_budget_category_info(&context, "餐饮", Some("外卖"), Some(3));
    assert_eq!(info["id"], 3);
    assert_eq!(info["type"], 3);
    assert_eq!(info["main_category"], "餐饮");
    assert_eq!(info["sub_category"], "外卖");
    assert_eq!(get_budget_type_name(3), "支出");
    assert_eq!(get_budget_type_name(5), "投资");
}

#[test]
fn date_helpers_pin_query_end_window_history_and_parent_rollup_semantics() {
    assert_eq!(
        normalize_budget_query_end_date(Some("2026-05-01")),
        Some("2026-05-01 23:59:59".to_string())
    );
    assert_eq!(
        normalize_budget_query_end_date(Some("2026-05-01 10:30:00")),
        Some("2026-05-01 10:30:00".to_string())
    );

    let forecast_window =
        expand_forecast_history_window(BudgetPeriodKind::Monthly, "2026-05-01", "2026-05-31", 6)
            .expect("forecast window");
    assert_eq!(forecast_window.start_date, "2025-12-01");
    assert_eq!(forecast_window.end_date, "2026-05-31");

    assert_eq!(
        BudgetPeriodKind::Monthly.rollup_parent_kinds(),
        &[BudgetPeriodKind::Quarterly, BudgetPeriodKind::Yearly]
    );
    assert_eq!(
        BudgetPeriodKind::Quarterly.rollup_parent_kinds(),
        &[BudgetPeriodKind::Yearly]
    );
    assert!(BudgetPeriodKind::Weekly.rollup_parent_kinds().is_empty());

    let quarterly_parent = BudgetPeriodKind::Quarterly
        .containing(date("2026-05-18"))
        .expect("quarterly parent");
    assert_eq!(quarterly_parent.start_date, "2026-04-01");
    assert_eq!(quarterly_parent.end_date, "2026-06-30");

    let yearly_parent = BudgetPeriodKind::Yearly
        .containing(date("2026-04-01"))
        .expect("yearly parent");
    assert_eq!(yearly_parent.start_date, "2026-01-01");
    assert_eq!(yearly_parent.end_date, "2026-12-31");

    assert_eq!(rollup_parent_amount_cents(50000, 62013), 62013);
    assert_eq!(rollup_parent_amount_cents(70000, 62013), 70000);

    let quarterly = BTreeMap::from([(1, 100000), (2, 60000)]);
    let monthly = BTreeMap::from([(1, 90000), (2, 75013), (4, 10000)]);
    assert_eq!(
        rollup_yearly_child_total_cents(&quarterly, &monthly),
        185013
    );
}

#[test]
fn history_filter_summary_and_on_demand_ranges_are_stable() {
    let summary = build_budget_history_filter_summary(&BudgetHistoryFilterSummaryInput {
        budget_type: BUDGET_TYPE_EXPENSE,
        period_type: Some("monthly".to_string()),
        budget_id: Some(9),
        category_id: None,
        account_ids: Some(vec![5, 4, 5]),
        tag_ids: Some(vec![2, 1]),
    });
    assert_eq!(
        summary,
        "{\"account_ids\": [4, 5, 5], \"budget_id\": 9, \"budget_type\": 3, \"category_id\": null, \"period_type\": \"monthly\", \"tag_ids\": [1, 2]}"
    );

    let weekly =
        iter_budget_history_period_ranges(BudgetPeriodKind::Weekly, "2026-05-07", "2026-05-20")
            .expect("weekly");
    assert_eq!(weekly[0].start_date, "2026-05-04");
    assert_eq!(weekly[0].end_date, "2026-05-10");
    assert_eq!(weekly[2].start_date, "2026-05-18");
    assert_eq!(weekly[2].end_date, "2026-05-24");

    let quarterly =
        iter_budget_history_period_ranges(BudgetPeriodKind::Quarterly, "2026-05-07", "2026-11-20")
            .expect("quarterly");
    assert_eq!(quarterly[0].start_date, "2026-04-01");
    assert_eq!(quarterly[0].end_date, "2026-06-30");
    assert_eq!(quarterly[2].start_date, "2026-10-01");
    assert_eq!(quarterly[2].end_date, "2026-12-31");

    assert!(
        budget_overlaps_period("2026-05-01", Some("2026-05-31"), "2026-05-15", "2026-05-20")
            .expect("overlap")
    );
    assert!(
        !budget_overlaps_period("2026-06-01", None, "2026-05-15", "2026-05-20")
            .expect("no overlap")
    );
}

#[test]
fn on_demand_history_item_and_forecast_helpers_preserve_budget_shapes() {
    let detail = json!({
        "id": 42,
        "name": "五月餐饮",
        "category": "餐饮",
        "sub_category": "外卖",
        "category_id": "3",
        "budget_amount_cents": 30000,
        "spent_amount_cents": 36000,
        "category_info": {"id": 3, "type": 3},
        "type": 3,
        "period_type": "monthly",
        "alert_threshold": 90,
        "enabled": 1
    });
    let history_item = build_budget_history_item_from_detail(
        &detail,
        &bill_analyser_core::budgets::BudgetPeriodRange {
            start_date: "2026-05-01".to_string(),
            end_date: "2026-05-31".to_string(),
        },
    );
    assert_eq!(history_item["id"], "42_2026-05-01_2026-05-31");
    assert_eq!(history_item["status"], "over_budget");
    assert_eq!(history_item["execution_rate"], 120.0);
    assert_eq!(history_item["filter_summary"], "");
    assert_eq!(history_item["calculated_at"], "");
    assert_eq!(history_item["name"], "五月餐饮");
    assert_eq!(history_item["category_id"], "3");
    assert_eq!(history_item["type"], 3);
    assert_eq!(history_item["period_type"], "monthly");
    assert_eq!(history_item["alert_threshold"], 90);
    assert_eq!(history_item["enabled"], 1);

    let samples = [10000, 12000, 9000, 15000];
    assert_eq!(
        calculate_forecast_amount_cents(&samples, "historical_average"),
        Some(11500)
    );
    assert_eq!(
        calculate_forecast_amount_cents(&samples, "moving_average"),
        Some(12000)
    );
    assert_eq!(
        calculate_forecast_backtest_mape(&samples, "moving_average"),
        Some(23.33)
    );
    assert_eq!(resolve_forecast_confidence(Some(9.9)), "high");
    assert_eq!(resolve_forecast_confidence(Some(15.0)), "medium");
    assert_eq!(resolve_forecast_confidence(None), "low");
    assert_eq!(resolve_forecast_trend(&samples), "up");
    assert_eq!(resolve_forecast_budget_amount_cents(50000, 80000), 50000);
    assert_eq!(resolve_forecast_budget_amount_cents(0, 80000), 80000);

    let period_labels = vec![
        "2026-02".to_string(),
        "2026-03".to_string(),
        "2026-04".to_string(),
        "2026-05".to_string(),
    ];
    let forecast = build_budget_forecast_item_from_input(BudgetForecastItemInput {
        category: "餐饮",
        category_info: json!({"id": 3, "type": 3}),
        amounts_cents: &samples,
        current_spent_cents: 8000,
        primary_budget_amount_cents: 0,
        sub_budget_total_cents: 11000,
        strategy: "moving_average",
        period_count: 6,
        period_labels: Some(&period_labels),
    });
    assert_eq!(forecast["category"], "餐饮");
    assert_eq!(forecast["budget_amount_cents"], 11000);
    assert_eq!(forecast["forecast_amount_cents"], 12000);
    assert_eq!(forecast["projected_over_budget"], true);
    assert_eq!(forecast["forecast_strategy"], "moving_average");
    assert_eq!(
        forecast["strategy_explanation"],
        "基于最近3个周期的移动平均"
    );
    assert_eq!(forecast["period_count"], 6);
    assert_eq!(
        forecast["periods"][0],
        json!({"period": "2026-02", "amount_cents": 10000})
    );
    assert_eq!(forecast["trend"], "up");

    let default_forecast =
        build_budget_forecast_item("交通", Value::Null, &samples, 5000, 50000, 0, "");
    assert_eq!(default_forecast["forecast_strategy"], "historical_average");
}

#[test]
fn validation_error_and_period_edge_contracts_are_pinned() {
    let today = date("2026-05-07");

    assert_eq!(
        validate_budget_period_args(
            &BudgetPeriodScopeInput {
                period_type: Some("fortnight".to_string()),
                ..Default::default()
            },
            None,
        ),
        Err("Invalid period_type: fortnight".to_string())
    );
    assert_eq!(
        validate_budget_period_args(
            &BudgetPeriodScopeInput {
                month: Some(13),
                ..Default::default()
            },
            None,
        ),
        Err("month must be between 1 and 12".to_string())
    );
    assert_eq!(
        validate_budget_period_args(
            &BudgetPeriodScopeInput {
                quarter: Some(5),
                ..Default::default()
            },
            None,
        ),
        Err("quarter must be between 1 and 4".to_string())
    );
    assert_eq!(
        validate_budget_period_args(&BudgetPeriodScopeInput::default(), Some(0)),
        Err("months_history must be >= 1".to_string())
    );
    validate_budget_date_range(None, Some("2026-05-31")).expect("open start range");
    validate_budget_date_range(Some("2026-05-01"), None).expect("open end range");

    let daily = resolve_budget_period_range(
        &BudgetPeriodScopeInput {
            period_type: Some("daily".to_string()),
            ..Default::default()
        },
        today,
    )
    .expect("daily range");
    assert_eq!(daily.start_date, "2026-05-07");
    assert_eq!(daily.end_date, "2026-05-07");

    assert_eq!(
        parse_budget_csv_int_list(Some(" ,, "), "ids").expect("empty csv"),
        None
    );
    assert_eq!(
        parse_budget_json_int_list(None, "ids").expect("missing json ids"),
        Vec::<i64>::new()
    );
    assert!(parse_budget_json_int_list(Some(&json!([null])), "ids").is_err());
    assert!(parse_budget_json_int_list(Some(&json!([true])), "ids").is_err());

    assert!(validate_import_budget_item(&json!("bad"), 4).is_err());
    assert!(validate_import_budget_item(
        &json!({
            "period_type": "monthly",
            "amount_cents": 10,
            "start_date": "2026-05-01",
            "category": ""
        }),
        5,
    )
    .is_err());
    assert!(validate_import_budget_item(
        &json!({
            "period_type": "bad",
            "amount_cents": 10,
            "start_date": "2026-05-01",
            "category": "餐饮"
        }),
        6,
    )
    .is_err());

    let before =
        calculate_budget_period_progress("2026-06-01", "2026-06-10", today).expect("before period");
    assert_eq!(before.elapsed_days, 0);
    assert_eq!(before.remaining_days, 10);
    let after =
        calculate_budget_period_progress("2026-04-01", "2026-04-30", today).expect("after period");
    assert_eq!(after.elapsed_days, 30);
    assert_eq!(after.remaining_days, 0);
}

#[test]
fn category_context_fallback_and_unknown_edges_are_pinned() {
    let categories = vec![
        json!("ignored"),
        json!({"name": "", "type": 3}),
        json!({"name": "坏类型", "type": 9}),
        json!({"id": 31, "name": "外卖", "parent_name": "餐饮", "type": 3, "icon": "takeout", "color": "#fa0"}),
        json!({"id": 32, "name": "餐饮", "type": 3, "icon": "", "color": ""}),
        json!({"id": 33, "main_category": "基金", "sub_category": "指数", "type": 5, "icon": "fund", "color": "#0a0"}),
    ];
    let context = build_budget_category_context(&categories);

    assert_eq!(
        resolve_budget_category_type(&context, "未知", None, Some(BUDGET_TYPE_EXPENSE)),
        Some(TransactionType::Expense)
    );
    assert_eq!(
        resolve_budget_category_type(&context, "未知", None, None),
        None
    );
    assert_eq!(
        resolve_budget_category_type(&context, "餐饮", Some("外卖"), None),
        Some(TransactionType::Expense)
    );
    assert_eq!(
        resolve_budget_category_type(&context, "基金", Some("指数"), None),
        Some(TransactionType::Investment)
    );

    let merged_primary = resolve_budget_category_info(&context, "餐饮", None, Some(3));
    assert_eq!(merged_primary["id"], 32);
    assert_eq!(merged_primary["icon"], "takeout");
    assert_eq!(merged_primary["color"], "#fa0");

    let sub_info = resolve_budget_category_info(&context, "基金", Some("指数"), Some(5));
    assert_eq!(sub_info["id"], 33);
    assert_eq!(
        resolve_budget_category_info(&context, "基金", Some("债券"), Some(5)),
        Value::Null
    );
    assert_eq!(
        resolve_budget_category_info(&context, "不存在", None, Some(3)),
        Value::Null
    );
}

#[test]
fn history_and_forecast_edge_branches_are_pinned() {
    assert_eq!(
        expand_forecast_history_window(BudgetPeriodKind::Monthly, "2026-05-01", "2026-05-31", 1,)
            .expect("single window")
            .start_date,
        "2026-05-01"
    );
    assert_eq!(
        expand_forecast_history_window(BudgetPeriodKind::Yearly, "2026-05-01", "2026-05-31", 3,)
            .expect("yearly window")
            .start_date,
        "2024-05-01"
    );

    let sample_day = date("2026-05-07");
    assert_eq!(BudgetPeriodKind::Daily.bucket_key(sample_day), "2026-05-07");
    assert_eq!(BudgetPeriodKind::Weekly.bucket_key(sample_day), "2026-18");
    assert_eq!(BudgetPeriodKind::Yearly.bucket_key(sample_day), "2026");
    assert_eq!(BudgetPeriodKind::Daily.rollup_parent_kinds(), &[]);

    assert!(iter_budget_history_period_ranges(
        BudgetPeriodKind::Monthly,
        "2026-06-01",
        "2026-05-01",
    )
    .expect("empty history")
    .is_empty());
    let daily =
        iter_budget_history_period_ranges(BudgetPeriodKind::Daily, "2026-05-01", "2026-05-02")
            .expect("daily history");
    assert_eq!(daily.len(), 2);
    let yearly =
        iter_budget_history_period_ranges(BudgetPeriodKind::Yearly, "2025-05-01", "2026-05-02")
            .expect("yearly history");
    assert_eq!(yearly[0].start_date, "2025-01-01");
    assert_eq!(yearly[1].end_date, "2026-12-31");

    let within = build_budget_history_item_from_detail(
        &json!({"id": 7, "category": "交通", "budget_amount_cents": 30000, "spent_amount_cents": 20000}),
        &bill_analyser_core::budgets::BudgetPeriodRange {
            start_date: "2026-05-01".to_string(),
            end_date: "2026-05-31".to_string(),
        },
    );
    assert_eq!(within["status"], "within_budget");

    assert_eq!(calculate_forecast_amount_cents(&[], "moving_average"), None);
    assert_eq!(
        calculate_forecast_backtest_mape(&[0, 0], "moving_average"),
        None
    );
    assert_eq!(resolve_forecast_trend(&[1000]), "stable");
    assert_eq!(resolve_forecast_trend(&[0, 500]), "stable");
    assert_eq!(resolve_forecast_trend(&[10000, 8000]), "down");
    assert_eq!(resolve_forecast_trend(&[10000, 10300]), "stable");

    let only_secondary = vec![json!({
        "id": 70,
        "category": "餐饮",
        "sub_category": "外卖",
        "period_type": "monthly",
        "start_date": "2026-05-01",
        "budget_amount_cents": 10000
    })];
    assert_eq!(ids(&select_budget_detail_items(&only_secondary)), vec![70]);
}
