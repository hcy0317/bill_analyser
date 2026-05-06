use bill_analyser_core::adapters::transaction::{
    frontend_transaction_from_backend, is_formula_like_export_cell, month_date_range,
    serialize_export_cell, serialize_optional_export_cell, transaction_list_type_filter,
    BackendTransactionView, FrontendTransactionTag,
};
use bill_analyser_core::primitives::{Money, TransactionType, UtcOffsetMinutes};
use serde_json::json;

fn money(yuan: &str) -> Money {
    Money::from_yuan_str(yuan).unwrap()
}

#[test]
fn backend_transaction_view_serializes_like_python_frontend_adapter() {
    let bill = BackendTransactionView {
        id: "42".to_string(),
        time_sequence_id: Some("seq-42".to_string()),
        transaction_type: TransactionType::Expense,
        category_id: Some("7".to_string()),
        main_category: "餐饮".to_string(),
        sub_category: "咖啡".to_string(),
        date: "2025-01-02 03:04:05".to_string(),
        amount: money("-12.34"),
        destination_amount: None,
        source_account_id: Some(3),
        destination_account_id: None,
        utc_offset: UtcOffsetMinutes::new(480),
        hide_amount: true,
        tag_ids: vec!["11".to_string(), "12".to_string()],
        tags: vec![
            FrontendTransactionTag {
                id: "11".to_string(),
                name: "早餐".to_string(),
            },
            FrontendTransactionTag {
                id: "12".to_string(),
                name: "咖啡".to_string(),
            },
        ],
        category: Some(json!({"id": "7", "name": "咖啡", "parentId": "virtual_餐饮"})),
        source_account: Some(json!({"id": "3", "name": "钱包"})),
        destination_account: None,
        description: "早餐".to_string(),
    };

    let value = serde_json::to_value(frontend_transaction_from_backend(&bill)).unwrap();

    assert_eq!(value["id"], "42");
    assert_eq!(value["timeSequenceId"], "seq-42");
    assert_eq!(value["type"], 3);
    assert_eq!(value["categoryId"], "7");
    assert_eq!(value["categoryName"], "餐饮");
    assert_eq!(value["subCategoryName"], "咖啡");
    assert_eq!(value["time"], 1_735_758_245);
    assert_eq!(value["utcOffset"], 480);
    assert_eq!(value["sourceAccountId"], "3");
    assert_eq!(value["destinationAccountId"], "0");
    assert_eq!(value["amount"], 1234);
    assert_eq!(value["sourceAmount"], 1234);
    assert_eq!(value["destinationAmount"], 1234);
    assert_eq!(value["hideAmount"], true);
    assert_eq!(value["tagIds"], json!(["11", "12"]));
    assert_eq!(
        value["tags"],
        json!([{"id": "11", "name": "早餐"}, {"id": "12", "name": "咖啡"}])
    );
    assert_eq!(value["category"]["id"], "7");
    assert_eq!(value["sourceAccount"]["id"], "3");
    assert!(value.get("destinationAccount").is_none());
    assert_eq!(value["comment"], "早餐");
    assert_eq!(value["editable"], true);
    assert_eq!(value["gregorianCalendarYearDashMonthDashDay"], "2025-01-02");
    assert_eq!(value["gregorianCalendarDayOfMonth"], 2);
    assert_eq!(value["displayDayOfWeek"], 5);
}

#[test]
fn transfer_destination_amount_uses_backend_destination_yuan_when_present() {
    let bill = BackendTransactionView {
        id: "99".to_string(),
        transaction_type: TransactionType::Transfer,
        amount: money("-88.00"),
        destination_amount: Some(money("87.50")),
        source_account_id: Some(1),
        destination_account_id: Some(2),
        ..Default::default()
    };

    let value = serde_json::to_value(frontend_transaction_from_backend(&bill)).unwrap();

    assert_eq!(value["type"], 4);
    assert_eq!(value["sourceAmount"], 8800);
    assert_eq!(value["destinationAmount"], 8750);
    assert_eq!(value["sourceAccountId"], "1");
    assert_eq!(value["destinationAccountId"], "2");
}

#[test]
fn zero_destination_amount_falls_back_to_source_amount_like_python_adapter() {
    let bill = BackendTransactionView {
        id: "100".to_string(),
        transaction_type: TransactionType::Expense,
        amount: money("-19.90"),
        destination_amount: Some(Money::ZERO),
        ..Default::default()
    };

    let value = serde_json::to_value(frontend_transaction_from_backend(&bill)).unwrap();

    assert_eq!(value["amount"], 1990);
    assert_eq!(value["destinationAmount"], 1990);
}

#[test]
fn list_type_filter_and_month_range_match_bills_routes() {
    assert_eq!(transaction_list_type_filter(None), None);
    assert_eq!(transaction_list_type_filter(Some("0")), None);
    assert_eq!(
        transaction_list_type_filter(Some("2")),
        Some("收入".to_string())
    );
    assert_eq!(
        transaction_list_type_filter(Some("3")),
        Some("支出".to_string())
    );
    assert_eq!(
        transaction_list_type_filter(Some("4")),
        Some("转账".to_string())
    );
    assert_eq!(
        transaction_list_type_filter(Some("5")),
        Some("投资".to_string())
    );
    assert_eq!(
        transaction_list_type_filter(Some("支出")),
        Some("支出".to_string())
    );
    assert_eq!(transaction_list_type_filter(Some("9")), None);

    assert_eq!(
        month_date_range(2026, 5).unwrap(),
        ("2026-05-01".to_string(), "2026-06-01".to_string())
    );
    assert_eq!(
        month_date_range(2026, 12).unwrap(),
        ("2026-12-01".to_string(), "2027-01-01".to_string())
    );
    assert!(month_date_range(2026, 0).is_err());
}

#[test]
fn export_cells_escape_formula_like_text_columns_only() {
    assert!(is_formula_like_export_cell(" =1+1"));
    assert!(is_formula_like_export_cell("@cmd"));
    assert_eq!(serialize_export_cell("description", " =1+1"), "' =1+1");
    assert_eq!(
        serialize_export_cell("counterparty", "-SUM(A1)"),
        "'-SUM(A1)"
    );
    assert_eq!(serialize_export_cell("amount", "-12.34"), "-12.34");
    assert_eq!(serialize_export_cell("description", "normal"), "normal");
    assert_eq!(
        serialize_optional_export_cell::<String>("description", None),
        ""
    );
}
