use std::collections::BTreeMap;

use bill_analyser_core::statistics::{
    build_asset_trend_legend, build_asset_trends, build_builtin_fallback_exchange_rates,
    build_calendar_events_data, build_calendar_events_response, build_category_pie_data,
    build_category_statistics_items, build_category_statistics_response,
    build_category_trend_statistics, build_insight_anomaly_summary, build_net_worth_snapshot,
    build_net_worth_snapshot_response, build_overview_result_from_report,
    build_provider_candidate_order, build_provider_exchange_rates_result,
    build_statistics_analyzer_category_result, build_statistics_analyzer_comparison_result,
    build_statistics_analyzer_report, build_statistics_analyzer_trend_bucket,
    build_statistics_analyzer_trends_result, build_statistics_report_chart_plan,
    build_statistics_trend_response, build_top_merchants_data,
    build_transaction_amount_period_result, build_transaction_amounts_response,
    build_user_custom_exchange_rates_result, convert_cny_quote_map_to_rates,
    convert_provider_base_currency, exchange_rate_provider_options, extract_numeric_values,
    normalize_chinese_currency_name, normalize_requested_exchange_rate_provider,
    parse_statistics_timestamp_range, parse_statistics_year_month_range,
    parse_transaction_amount_period_query, statistics_analyzer_period_range,
    validate_asset_trends_span, AssetTrendDay, CategoryStatisticItem, RecurringRuleInput,
    StatisticsAccountInput, StatisticsBillInput, StatisticsCategoryInput, StatisticsTimestampRange,
    StatisticsYearMonthRangeMode, UserCustomExchangeRateInput,
};
use chrono::NaiveDate;
use serde_json::{json, Value};

fn date(value: &str) -> NaiveDate {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").expect("test date")
}

fn cents(value: &str) -> i64 {
    (value.parse::<f64>().expect("test amount") * 100.0).round() as i64
}

fn bill(id: i64, date: &str, bill_type: &str, amount: &str) -> StatisticsBillInput {
    StatisticsBillInput {
        id: Some(id),
        date: date.to_string(),
        bill_type: bill_type.to_string(),
        amount_cents: cents(amount),
        channel: "现金".to_string(),
        source_account_id: Some(10),
        main_category: "餐饮".to_string(),
        sub_category: "早餐".to_string(),
        counterparty: "早餐店".to_string(),
        description: String::new(),
        ..Default::default()
    }
}

fn account(id: i64, name: &str, balance: &str) -> StatisticsAccountInput {
    StatisticsAccountInput {
        id,
        name: name.to_string(),
        account_type: "cash".to_string(),
        balance_cents: cents(balance),
        initial_balance_cents: cents(balance),
        currency: Some("CNY".to_string()),
        ..Default::default()
    }
}

#[test]
fn range_provider_and_asset_validation_contracts_match_statistics_routes() {
    assert_eq!(
        parse_statistics_timestamp_range(Some("0"), Some("0")).expect("all range"),
        StatisticsTimestampRange::All
    );
    assert_eq!(
        parse_statistics_timestamp_range(None, None)
            .expect_err("missing timestamps are rejected by current route validation")
            .error,
        "Invalid timestamp format"
    );

    let reverse = parse_statistics_timestamp_range(Some("1740873599"), Some("1740787200"))
        .expect_err("reverse range");
    assert_eq!(reverse.error, "Invalid time range");
    assert_eq!(
        reverse.message,
        "startTime must be less than or equal to endTime"
    );

    let asset_span = validate_asset_trends_span(0, 366 * 86_400, false)
        .expect_err("too wide non-all asset range");
    assert_eq!(
        asset_span.error,
        "资产趋势查询最多支持365天范围，请缩小时间范围"
    );

    match parse_statistics_year_month_range(Some("2026-02"), Some("202603"))
        .expect("bounded year-month range")
    {
        StatisticsYearMonthRangeMode::Bounded(range) => {
            assert_eq!(range.start_date, "2026-02-01");
            assert_eq!(range.end_date, "2026-03-31");
        }
        StatisticsYearMonthRangeMode::All => panic!("expected bounded range"),
    }

    assert_eq!(
        parse_statistics_year_month_range(Some("197001"), Some("0")).expect("all months"),
        StatisticsYearMonthRangeMode::All
    );
    let reverse_month = parse_statistics_year_month_range(Some("202603"), Some("202602"))
        .expect_err("reverse month");
    assert_eq!(reverse_month.error, "Invalid year-month range");

    assert_eq!(
        normalize_requested_exchange_rate_provider(Some(" ECB ")),
        "ecb"
    );
    assert_eq!(
        build_provider_candidate_order("cmb_cn"),
        vec!["cmb_cn", "boc_cn", "ecb", "rba"]
    );
    assert!(build_provider_candidate_order("bad-provider").is_empty());
    assert_eq!(
        exchange_rate_provider_options()["boc_cn"].reference_url,
        "https://www.boc.cn/sourcedb/whpj/"
    );
}

#[test]
fn category_statistics_and_trend_contracts_preserve_cents_signs_and_empty_months() {
    let categories = vec![StatisticsCategoryInput {
        id: 1,
        main_category: "餐饮".to_string(),
        sub_category: "早餐".to_string(),
    }];
    let accounts = vec![account(10, "现金", "0")];
    let mut bills = vec![
        bill(1, "2026-03-01T08:00:00", "支出", "18.50"),
        bill(2, "2026-03-01T09:00:00", "收入", "5.00"),
    ];
    bills.push(StatisticsBillInput {
        id: Some(3),
        date: "2026-03-02T10:00:00".to_string(),
        bill_type: "转账".to_string(),
        amount_cents: 300,
        channel: "现金".to_string(),
        source_account_id: Some(999),
        destination_account: "银行卡".to_string(),
        main_category: "餐饮".to_string(),
        sub_category: "早餐".to_string(),
        counterparty: "银行卡".to_string(),
        ..Default::default()
    });

    let items = build_category_statistics_items(&bills, &categories, &accounts);
    assert_eq!(
        items,
        vec![CategoryStatisticItem {
            category_id: "1".to_string(),
            account_id: "10".to_string(),
            amount_cents: -1650,
        }]
    );
    assert_eq!(
        build_category_statistics_response(1740787200, 1740873599, &items),
        json!({
            "success": true,
            "result": {
                "startTime": 1740787200,
                "endTime": 1740873599,
                "items": [{"categoryId": "1", "accountId": "10", "amountCents": -1650}],
            },
        })
    );

    let range = match parse_statistics_year_month_range(Some("202603"), Some("202604"))
        .expect("trend range")
    {
        StatisticsYearMonthRangeMode::Bounded(range) => range,
        StatisticsYearMonthRangeMode::All => panic!("expected bounded range"),
    };
    let trend = build_category_trend_statistics(&bills, &categories, &accounts, &range);
    assert_eq!(trend.len(), 2);
    assert_eq!(trend[0].year, 2026);
    assert_eq!(trend[0].month, 3);
    assert_eq!(trend[0].items[0].amount_cents, -1650);
    assert!(trend[1].items.is_empty());
}

#[test]
fn asset_trends_contract_preserves_balance_math_and_filters_empty_legends() {
    let mut cash = account(10, "现金", "100.00");
    cash.initial_balance_cents = 10000;
    let mut bank = account(20, "银行卡", "50.00");
    bank.initial_balance_cents = 5000;
    let zero = account(30, "空账户", "0.00");
    let accounts = vec![cash, bank, zero];
    let balances_before = BTreeMap::from([(10, 500)]);
    let bills = vec![
        StatisticsBillInput {
            source_account_id: Some(10),
            ..bill(1, "2026-03-01T08:00:00", "支出", "10.00")
        },
        StatisticsBillInput {
            source_account_id: Some(20),
            ..bill(2, "2026-03-01T09:00:00", "收入", "20.00")
        },
        StatisticsBillInput {
            source_account_id: Some(20),
            destination_account_id: Some(10),
            destination_amount_cents: Some(1600),
            ..bill(3, "2026-03-01T10:00:00", "转账", "15.00")
        },
    ];

    let trends = build_asset_trends(
        &bills,
        &accounts,
        &balances_before,
        date("2026-03-01"),
        date("2026-03-02"),
    );
    assert_eq!(trends.len(), 2);
    assert_asset_day(
        &trends[0],
        json!([
            {"accountId": "10", "accountOpeningBalanceCents": 10500, "accountClosingBalanceCents": 11100},
            {"accountId": "20", "accountOpeningBalanceCents": 5000, "accountClosingBalanceCents": 5500},
            {"accountId": "30", "accountOpeningBalanceCents": 0, "accountClosingBalanceCents": 0},
        ]),
    );
    assert_asset_day(
        &trends[1],
        json!([
            {"accountId": "10", "accountOpeningBalanceCents": 11100, "accountClosingBalanceCents": 11100},
            {"accountId": "20", "accountOpeningBalanceCents": 5500, "accountClosingBalanceCents": 5500},
            {"accountId": "30", "accountOpeningBalanceCents": 0, "accountClosingBalanceCents": 0},
        ]),
    );

    let legend = build_asset_trend_legend(&accounts, &trends);
    assert_eq!(legend.len(), 2);
    assert_eq!(legend[0].id, "10");
    assert_eq!(legend[1].id, "20");
}

#[test]
fn exchange_rate_contracts_cover_provider_conversion_custom_and_fallback_shapes() {
    assert_eq!(normalize_chinese_currency_name(" 美元 (USD) "), "USD");
    assert_eq!(
        extract_numeric_values(&["100", "1,234.56", "08:30", "-", "NaN", "text", "0.75"]),
        vec![100.0, 1234.56, 0.75]
    );

    let quote_map = BTreeMap::from([("USD".to_string(), 730.0), ("EUR".to_string(), 800.0)]);
    let cny_rates =
        convert_cny_quote_map_to_rates(&quote_map, "CNY", &["USD".to_string(), "EUR".to_string()]);
    assert_eq!(cny_rates["USD"], 100.0 / 730.0);

    let usd_rates =
        convert_cny_quote_map_to_rates(&quote_map, "USD", &["CNY".to_string(), "EUR".to_string()]);
    assert_eq!(usd_rates["CNY"], 7.3);
    assert_eq!(usd_rates["EUR"], 730.0 / 800.0);

    let base_rates = BTreeMap::from([
        ("EUR".to_string(), 1.0),
        ("USD".to_string(), 1.1),
        ("CNY".to_string(), 7.7),
    ]);
    let converted = convert_provider_base_currency(
        &base_rates,
        "EUR",
        "CNY",
        &["USD".to_string(), "EUR".to_string(), "CNY".to_string()],
        "base_to_target",
    );
    assert_eq!(converted["USD"], 1.1 / 7.7);
    assert_eq!(converted["EUR"], 1.0 / 7.7);
    assert_eq!(converted["CNY"], 1.0);

    let custom = build_user_custom_exchange_rates_result(
        "CNY",
        &[UserCustomExchangeRateInput {
            to_currency: "USD".to_string(),
            rate: "7.12".to_string(),
            effective_date: Some("2026-03-01T12:00:00+00:00".to_string()),
            effective_timestamp: Some(1_777_777_777),
        }],
        100,
    );
    assert_eq!(custom.provider_key, "user_custom");
    assert_eq!(custom.update_time, 1_777_777_777);
    assert_eq!(custom.exchange_rates[1].rate, "7.12");

    let provider = build_provider_exchange_rates_result(
        "CNY",
        "cmb_cn",
        "ecb",
        &BTreeMap::from([("USD".to_string(), 0.139)]),
        123,
    );
    assert_eq!(provider.provider_key, "ecb");
    assert!(provider.fallback_used);
    assert_eq!(provider.exchange_rates[0].rate, "1.0");
    assert_eq!(provider.exchange_rates[1].rate, "0.139");

    let fallback = build_builtin_fallback_exchange_rates("USD", 456);
    assert_eq!(fallback.provider_key, "fallback");
    assert_eq!(fallback.base_currency, "USD");
    assert_eq!(fallback.exchange_rates[0].currency, "USD");
    assert!(fallback
        .exchange_rates
        .iter()
        .any(|rate| rate.currency == "CNY"));
    assert!(build_builtin_fallback_exchange_rates("CNY", 456)
        .exchange_rates
        .iter()
        .any(|rate| rate.currency == "VND" && rate.rate == "3425.0"));
}

#[test]
fn basic_statistics_collection_contracts_preserve_current_route_shapes() {
    let bills = vec![
        StatisticsBillInput {
            main_category: "餐饮".to_string(),
            counterparty: "早餐店".to_string(),
            ..bill(1, "2026-03-01T08:00:00", "支出", "18.50")
        },
        StatisticsBillInput {
            main_category: "餐饮".to_string(),
            counterparty: "早餐店".to_string(),
            ..bill(2, "2026-03-02T08:00:00", "支出", "-6.50")
        },
        StatisticsBillInput {
            main_category: "交通".to_string(),
            counterparty: "地铁".to_string(),
            ..bill(3, "2026-03-03T08:00:00", "收入", "10.01")
        },
        StatisticsBillInput {
            main_category: String::new(),
            counterparty: String::new(),
            ..bill(4, "2026-03-04T08:00:00", "支出", "1.45")
        },
    ];

    let expense_bills = bills
        .iter()
        .filter(|bill| bill.bill_type == "支出")
        .cloned()
        .collect::<Vec<_>>();
    let pie = build_category_pie_data(&expense_bills);
    assert_eq!(pie[0].name, "餐饮");
    assert_eq!(pie[0].value_cents, 2500);
    assert_eq!(pie[1].name, "");
    assert_eq!(pie[1].value_cents, 145);

    let merchants = build_top_merchants_data(&bills, 2);
    assert_eq!(merchants.len(), 2);
    assert_eq!(merchants[0].name, "早餐店");
    assert_eq!(merchants[0].amount_cents, 2500);
    assert_eq!(merchants[0].count, 2);
    assert_eq!(merchants[1].name, "地铁");

    assert_eq!(
        parse_transaction_amount_period_query("range_1740787200_1740873599"),
        Some(("range".to_string(), 1740787200, 1740873599))
    );
    assert_eq!(parse_transaction_amount_period_query("bad-segment"), None);

    let amounts = build_transaction_amount_period_result(1740787200, 1740873599, &bills);
    assert_eq!(amounts.amounts[0].currency, "CNY");
    assert_eq!(amounts.amounts[0].income_amount_cents, 1001);
    assert_eq!(amounts.amounts[0].expense_amount_cents, 2645);
    assert_eq!(
        build_transaction_amounts_response(&BTreeMap::from([("range".to_string(), amounts)])),
        json!({
            "success": true,
            "result": {
                "range": {
                    "startTime": 1740787200,
                    "endTime": 1740873599,
                    "amounts": [{"currency": "CNY", "incomeAmountCents": 1001, "expenseAmountCents": 2645}],
                },
            },
        })
    );

    assert_eq!(
        build_statistics_trend_response(&json!({
            "trends": [{"period": "2026-03", "incomeCents": 10013, "expenseCents": 5056, "netCents": 4957}]
        })),
        json!({
            "success": true,
            "data": [{"date": "2026-03", "incomeCents": 10013, "expenseCents": 5056, "netCents": 4957}]
        })
    );
}

#[test]
fn networth_calendar_insights_and_chart_shapes_match_current_runtime() {
    let mut liability = account(20, "信用卡", "-20.00");
    liability.account_type = "credit_card".to_string();
    let mut hidden = account(30, "隐藏账户", "999.00");
    hidden.hidden = true;
    let snapshot = build_net_worth_snapshot(&[account(10, "现金", "100.00"), liability, hidden]);
    assert_eq!(snapshot.total_assets_cents, 10000);
    assert_eq!(snapshot.total_liabilities_cents, 2000);
    assert_eq!(snapshot.net_worth_cents, 8000);
    assert_eq!(snapshot.account_count, 2);
    assert_eq!(
        build_net_worth_snapshot_response(&snapshot)["data"]["netWorthCents"],
        json!(8000)
    );

    let calendar = build_calendar_events_data(
        &[
            bill(1, "2026-03-01T08:00:00", "支出", "18.50"),
            bill(2, "2026-03-01T09:00:00", "收入", "5.00"),
            bill(3, "2026-03-02T09:00:00", "转账", "3.00"),
        ],
        &[RecurringRuleInput {
            id: Some(77),
            name: "房租".to_string(),
            amount_cents: 200000,
            bill_type: "支出".to_string(),
            frequency: "weekly".to_string(),
            next_date: "2026-03-02".to_string(),
        }],
        date("2026-03-01"),
        date("2026-03-10"),
    );
    assert_eq!(calendar.events[0].date, "2026-03-01");
    assert_eq!(calendar.events[0].income_cents, 500);
    assert_eq!(calendar.events[0].expense_cents, 1850);
    assert_eq!(calendar.events[0].net_cents, -1350);
    assert_eq!(calendar.recurring_projections.len(), 2);
    assert_eq!(
        build_calendar_events_response(&calendar)["data"]["recurringProjections"][0]["type"],
        "recurring_projection"
    );

    let insight_bills = vec![
        anomaly_bill(1, "2026-01-01", "餐饮", "60.00", "早餐店"),
        anomaly_bill(2, "2026-02-01", "餐饮", "60.00", "早餐店"),
        anomaly_bill(3, "2026-03-01", "餐饮", "200.00", "早餐店"),
        anomaly_bill(4, "2026-03-05", "健身", "80.00", "Gym"),
        anomaly_bill(5, "2026-03-07", "健身", "80.00", "Gym"),
        anomaly_bill(6, "2026-03-08", "奢侈", "10.00", "Shop"),
        anomaly_bill(7, "2026-03-09", "奢侈", "10.00", "Shop"),
        anomaly_bill(8, "2026-03-10", "奢侈", "10.00", "Shop"),
        anomaly_bill(9, "2026-03-11", "奢侈", "1000.00", "Shop"),
    ];
    let insights = build_insight_anomaly_summary(&insight_bills, 6, "2026-01-01", "2026-03-31");
    let anomalies = insights["data"]["anomalies"].as_array().expect("anomalies");
    assert!(anomalies
        .iter()
        .any(|item| item["type"] == "category_spike" && item["category"] == "餐饮"));
    assert!(anomalies
        .iter()
        .any(|item| item["type"] == "duplicate_charge" && item["counterparty"] == "Gym"));
    assert!(anomalies
        .iter()
        .any(|item| item["type"] == "large_transaction" && item["category"] == "奢侈"));

    let report = json!({
        "summary": {"total_income_cents": 12013, "total_expense_cents": -3557},
        "total_records": 3,
        "by_category": {"餐饮": {"total_cents": -3557}},
        "by_type": {"支出": {"total_cents": -3557}},
        "trend": [{"date": "2026-03-01", "income_cents": 100, "expense_cents": 200, "net_cents": -100}],
        "top_expenses": [{"amount_cents": 3557}],
        "top_income": [{"amount_cents": 12013}],
        "period": "month",
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
    });
    let overview = build_overview_result_from_report(&report);
    assert_eq!(overview["total_income_cents"], 12013);
    assert_eq!(overview["total_expense_cents"], 3557);
    assert_eq!(overview["net_income_cents"], 8456);
    assert_eq!(
        build_statistics_report_chart_plan(&report),
        vec![
            "trend",
            "category_pie",
            "top_expenses",
            "comparison",
            "dashboard"
        ]
    );

    let analyzer_range = statistics_analyzer_period_range("month", date("2026-03-15"));
    assert_eq!(analyzer_range.start_date, "2026-03-01");
    assert_eq!(analyzer_range.end_date, "2026-04-01");
    let analyzer_bills = vec![
        bill(20, "2026-03-01T08:00:00", "支出", "-12.34"),
        bill(21, "2026-03-02T08:00:00", "收入", "100.00"),
        bill(22, "2026-03-03T08:00:00", "支出", "-50.00"),
    ];
    let analyzer_report =
        build_statistics_analyzer_report("month", &analyzer_range, &analyzer_bills, "now");
    assert_eq!(analyzer_report["summary"]["total_income_cents"], 10000);
    assert_eq!(analyzer_report["summary"]["total_expense_cents"], -6234);
    assert_eq!(analyzer_report["summary"]["net_income_cents"], 3766);
    assert_eq!(analyzer_report["by_category"]["餐饮"]["count"], 3);
    assert_eq!(analyzer_report["top_expenses"][0]["category"], "餐饮");
    assert_eq!(analyzer_report["top_expenses"][0]["amount_cents"], -5000);

    let bucket = build_statistics_analyzer_trend_bucket("2026-03", &analyzer_bills);
    assert_eq!(bucket.income_cents, 10000);
    assert_eq!(bucket.expense_cents, -6234);
    assert_eq!(bucket.net_cents, 3766);
    assert_eq!(
        build_statistics_analyzer_trends_result("month", Some("餐饮"), &[bucket])["category"],
        "餐饮"
    );
    assert_eq!(
        build_statistics_analyzer_comparison_result("month", "category", &analyzer_bills)
            ["comparison"][0]["count"],
        3
    );
    assert_eq!(
        build_statistics_analyzer_category_result("month", Some("餐饮"), &analyzer_bills)
            ["sub_categories"][0]["percentage"],
        100.0
    );
}

fn assert_asset_day(day: &AssetTrendDay, expected_items: Value) {
    assert_eq!(
        serde_json::to_value(&day.items).expect("asset items"),
        expected_items
    );
}

fn anomaly_bill(
    id: i64,
    date: &str,
    main_category: &str,
    amount: &str,
    counterparty: &str,
) -> StatisticsBillInput {
    StatisticsBillInput {
        id: Some(id),
        date: date.to_string(),
        bill_type: "支出".to_string(),
        amount_cents: cents(amount),
        main_category: main_category.to_string(),
        counterparty: counterparty.to_string(),
        ..Default::default()
    }
}
