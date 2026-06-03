use bill_analyser_core::adapters::{
    api::PageResponse,
    category::virtual_parent_id,
    transaction::{
        backend_transaction_type_name, frontend_transaction_type_from_backend,
        signed_backend_amount,
    },
};
use bill_analyser_core::primitives::{
    normalize_bill_date_text, AuthContext, CurrencyCode, Money, Pagination, SortField, SortOrder,
    TransactionType, UnixTimestampSeconds, UserId, UtcOffsetMinutes,
};
use serde_json::json;

#[test]
fn money_keeps_cents_exact_and_rounds_yuan_half_up() {
    let cases = [
        ("100.50", 10050, "100.50"),
        ("1.005", 101, "1.01"),
        ("-1.005", -101, "-1.01"),
        ("0.004", 0, "0.00"),
        ("999999999.99", 99_999_999_999, "999999999.99"),
    ];

    for (yuan_text, expected_cents, expected_yuan_text) in cases {
        let money = Money::from_yuan_str(yuan_text).expect("valid yuan text");
        assert_eq!(money.to_cents(), expected_cents);
        assert_eq!(money.to_yuan_string(), expected_yuan_text);
        assert_eq!(Money::from_cents(expected_cents), money);
    }

    assert_eq!(
        Money::from_cents_text("00123").unwrap().to_yuan_string(),
        "1.23"
    );
    assert!(Money::from_yuan_str("not-money").is_err());
    assert!(Money::from_cents_text("12.3").is_err());
    assert!(!Money::ZERO.is_valid_transaction_amount());
    assert!(Money::MIN_TRANSACTION_AMOUNT.is_valid_transaction_amount());
    assert!(Money::MAX_TRANSACTION_AMOUNT.is_valid_transaction_amount());
    assert!(!Money::from_cents(100_000_000_000).is_valid_transaction_amount());
    assert!(!Money::from_cents(-1).is_valid_transaction_amount());
}

#[test]
fn bill_dates_match_current_normalization_formats() {
    let cases = [
        ("2025-01-02", "2025-01-02 00:00:00"),
        ("2025-01-02 03:04", "2025-01-02 03:04:00"),
        ("2025/01/02 03:04:05", "2025-01-02 03:04:05"),
        ("2025.01.02", "2025-01-02 00:00:00"),
        ("2025.01.02 03:04:05", "2025-01-02 03:04:05"),
        ("2025年01月02日 03:04", "2025-01-02 03:04:00"),
        ("2025-01-02T03:04:05+08:00", "2025-01-02 03:04:05"),
    ];

    for (raw, expected) in cases {
        assert_eq!(normalize_bill_date_text(raw), expected);
    }

    assert_eq!(normalize_bill_date_text("  not-a-date  "), "not-a-date");
    assert_eq!(
        UnixTimestampSeconds::from_frontend_value("1735689600000").as_i64(),
        1_735_689_600
    );
    assert_eq!(UnixTimestampSeconds::from_frontend_value("bad").as_i64(), 0);
    assert_eq!(
        UnixTimestampSeconds::from_frontend_value("-1735689600000").as_i64(),
        -1_735_689_600
    );
    assert_eq!(
        normalize_bill_date_text("2025-01-02")
            .parse::<bill_analyser_core::primitives::BillDateTime>()
            .unwrap()
            .display_day_of_week(),
        5
    );
}

#[test]
fn currency_ids_pagination_sorting_and_auth_context_use_explicit_types() {
    let currency = CurrencyCode::parse(" usd ").unwrap();
    assert_eq!(currency.as_str(), "USD");
    assert_eq!(currency.symbol(), "$");
    assert_eq!(CurrencyCode::default().as_str(), "CNY");
    assert_eq!(UtcOffsetMinutes::default().as_i32(), 480);

    let user_id = UserId::parse_positive(" 42 ").unwrap();
    assert_eq!(user_id.get(), 42);
    assert!(UserId::parse_positive("0").is_err());

    let pagination = Pagination::from_request(Some(0), Some(999));
    assert_eq!(pagination.page(), 1);
    assert_eq!(pagination.page_size(), 500);

    let page = PageResponse::new(vec![json!({"id": "1"})], 5, Pagination::new(2, 10).unwrap());
    let page_json = serde_json::to_value(page).unwrap();
    assert_eq!(page_json["success"], true);
    assert_eq!(page_json["result"]["totalCount"], 5);
    assert_eq!(page_json["result"]["pageSize"], 10);
    assert_eq!(page_json["result"]["total"], 5);

    assert_eq!(
        SortField::parse("created_at").unwrap().as_str(),
        "created_at"
    );
    assert_eq!(SortOrder::parse("DESC").unwrap(), SortOrder::Desc);
    assert!(SortField::parse("raw_sql").is_err());

    let auth_json = serde_json::to_value(
        AuthContext::new(user_id)
            .with_session_id("session-1")
            .with_email("user@example.com"),
    )
    .unwrap();
    assert_eq!(auth_json["userId"], 42);
    assert_eq!(auth_json["sessionId"], "session-1");
    assert_eq!(auth_json["email"], "user@example.com");

    assert!(serde_json::from_value::<AuthContext>(json!({"userId": 0})).is_err());
    assert!(serde_json::from_value::<Pagination>(json!({"page": 0, "pageSize": 10})).is_err());
    assert!(serde_json::from_value::<CurrencyCode>(json!("")).is_err());
}

#[test]
fn adapter_helpers_match_current_contract_edges() {
    assert_eq!(virtual_parent_id("餐饮"), "virtual_餐饮");
    assert_eq!(
        backend_transaction_type_name(TransactionType::Expense),
        "支出"
    );
    assert_eq!(
        frontend_transaction_type_from_backend("投资").unwrap(),
        TransactionType::Investment
    );
    assert_eq!(
        frontend_transaction_type_from_backend("transfer").unwrap(),
        TransactionType::Transfer
    );
    assert_eq!(
        frontend_transaction_type_from_backend("3").unwrap(),
        TransactionType::Expense
    );
    assert_eq!(serde_json::to_value(TransactionType::Expense).unwrap(), 3);

    let positive = Money::from_cents(1234);
    assert_eq!(
        signed_backend_amount(TransactionType::Expense, positive).to_cents(),
        -1234
    );
    assert_eq!(
        signed_backend_amount(TransactionType::Transfer, positive).to_cents(),
        -1234
    );
    assert_eq!(
        signed_backend_amount(TransactionType::Income, positive).to_cents(),
        1234
    );
    assert_eq!(
        signed_backend_amount(TransactionType::Expense, Money::from_cents(-1234)).to_cents(),
        -1234
    );
}
