use bill_analyser_core::adapters::transaction::{
    apply_create_category_contract, apply_legacy_modify_preserved_fields,
    apply_manual_create_defaults, batch_create_failure_response,
    batch_create_persist_error_route_response, batch_create_prepare_error_route_response,
    batch_create_success_response, batch_create_success_route_response,
    batch_create_transaction_items, batch_delete_success_payload,
    batch_update_balance_sync_account_ids, batch_update_response,
    calculate_account_balance_from_bills, delete_bill_success_payload,
    frontend_transaction_from_backend, frontend_transaction_mutation_to_backend,
    is_formula_like_export_cell, legacy_delete_bill_success_payload,
    legacy_modify_bill_success_payload, month_date_range, normalize_bill_create_aliases,
    serialize_export_cell, serialize_optional_export_cell, sync_account_ids_for_batch_delete,
    sync_account_ids_for_bill, sync_account_ids_for_update, transaction_list_type_filter,
    validate_batch_route_update_fields, validate_bill_create_fields, validate_bill_update_fields,
    AccountBalanceBill, BackendBillUpdateSnapshot, BackendTransactionView, BillAccountSyncSnapshot,
    FrontendTransactionTag,
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

#[test]
fn frontend_mutation_to_backend_and_create_defaults_match_python_write_adapter() {
    let frontend = json!({
        "type": 3,
        "time": 1_735_758_245,
        "sourceAmount": 1234,
        "destinationAmount": 0,
        "sourceAccountId": "0",
        "destinationAccountId": "",
        "categoryId": 7,
        "tagIds": ["0", "11", 12, ""],
        "comment": "早餐",
        "merchantName": ""
    });

    let (mut backend, metadata) =
        frontend_transaction_mutation_to_backend(&frontend, UtcOffsetMinutes::new(480)).unwrap();

    assert_eq!(backend["type"], "支出");
    assert_eq!(backend["date"], "2025-01-02 03:04:05");
    assert_eq!(backend["amount"], -12.34);
    assert_eq!(backend["destination_amount"], 0.0);
    assert_eq!(backend["source_account_id"], 0);
    assert_eq!(backend["destination_account_id"], 0);
    assert_eq!(backend["description"], "早餐");
    assert_eq!(metadata.category_id, "7");
    assert_eq!(metadata.source_account_id, 0);
    assert_eq!(metadata.destination_account_id, 0);
    assert_eq!(metadata.tag_ids, vec![11, 12]);
    assert!(!metadata.auto_invest_account);

    apply_manual_create_defaults(&mut backend, &frontend, Some(9)).unwrap();

    assert_eq!(backend["description"], "早餐");
    assert_eq!(backend["counterparty"], "早餐");
    assert_eq!(backend["source_account_id"], 9);

    let millisecond_frontend =
        json!({"type": 2, "time": 1_735_758_245_000i64, "sourceAmount": 2500});
    let (backend_from_millis, _) =
        frontend_transaction_mutation_to_backend(&millisecond_frontend, UtcOffsetMinutes::new(480))
            .unwrap();
    assert_eq!(backend_from_millis["type"], "收入");
    assert_eq!(backend_from_millis["date"], "2025-01-02 03:04:05");
    assert_eq!(backend_from_millis["amount"], 25.0);

    let (backend_ignoring_offset, _) =
        frontend_transaction_mutation_to_backend(&frontend, UtcOffsetMinutes::new(0)).unwrap();
    assert_eq!(backend_ignoring_offset["date"], "2025-01-02 03:04:05");

    let (bad_amount_backend, _) = frontend_transaction_mutation_to_backend(
        &json!({"type": 3, "sourceAmount": "bad", "destinationAmount": {"bad": true}}),
        UtcOffsetMinutes::new(480),
    )
    .unwrap();
    assert_eq!(bad_amount_backend["amount"], 0.0);
    assert_eq!(bad_amount_backend["destination_amount"], 0.0);
}

#[test]
fn legacy_modify_aliases_and_create_category_contracts_cover_python_prepare_edges() {
    let frontend = json!({"remark": "原备注", "comment": "新备注"});
    let old_bill = BackendBillUpdateSnapshot {
        transaction_type: "转账".to_string(),
        source_account_id: Some(3),
        destination_account_id: Some(4),
        destination_amount: Some(money("88.00")),
    };
    let (mut backend, _) = frontend_transaction_mutation_to_backend(
        &json!({"sourceAmount": 0, "destinationAmount": 0}),
        UtcOffsetMinutes::new(480),
    )
    .unwrap();

    apply_legacy_modify_preserved_fields(&mut backend, &frontend, &old_bill);

    assert_eq!(backend["description"], "新备注");
    assert_eq!(backend["type"], "转账");
    assert_eq!(backend["source_account_id"], 3);
    assert_eq!(backend["destination_account_id"], 4);
    assert_eq!(backend["destination_amount"], 88.0);
    assert!(backend.get("amount").is_none());
    assert!(backend.get("date").is_none());
    assert_eq!(
        legacy_modify_bill_success_payload(42),
        json!({"success": true, "result": {"id": "42"}})
    );

    let mut aliased = json!({
        "channel": "微信",
        "category": "餐饮",
        "date": "2025-01-02",
        "type": "支出",
        "amount": -12.34,
        "counterparty": "早餐店",
        "description": "早餐"
    })
    .as_object()
    .unwrap()
    .clone();
    normalize_bill_create_aliases(&mut aliased);
    assert_eq!(aliased["payment_method"], "微信");
    assert_eq!(aliased["main_category"], "餐饮");
    assert!(aliased.get("channel").is_none());
    assert!(aliased.get("category").is_none());
    validate_bill_create_fields(aliased.keys().map(String::as_str)).unwrap();

    let mut category_by_id = json!({"type": "支出"}).as_object().unwrap().clone();
    apply_create_category_contract(&mut category_by_id, Some(("餐饮", "早餐")), None);
    assert_eq!(category_by_id["main_category"], "餐饮");
    assert_eq!(category_by_id["sub_category"], "早餐");

    let mut category_by_rule = json!({"type": "收入"}).as_object().unwrap().clone();
    apply_create_category_contract(&mut category_by_rule, None, Some(("奖金", "")));
    assert_eq!(category_by_rule["main_category"], "奖金");
    assert_eq!(category_by_rule["sub_category"], "");

    let mut default_category = json!({"type": "投资"}).as_object().unwrap().clone();
    apply_create_category_contract(&mut default_category, None, None);
    assert_eq!(default_category["main_category"], "投资理财");
    assert_eq!(default_category["sub_category"], "证券投资");
}

#[test]
fn batch_create_items_and_update_field_guards_match_route_and_db_contracts() {
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": [{"id": 1}]}))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"bills": [{"id": 1}]}))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        batch_create_transaction_items(&json!([{"id": 1}]))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": [], "bills": [{"id": 1}]}))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": null, "bills": [{"id": 1}]}))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": 0.0, "bills": [{"id": 1}]}))
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": {"id": 1}, "bills": [{"id": 2}]}))
            .unwrap_err()
            .message,
        "transactions is required"
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": []}))
            .unwrap_err()
            .message,
        "transactions is required"
    );
    assert_eq!(
        batch_create_transaction_items(&json!({"transactions": [false]}))
            .unwrap_err()
            .message,
        "transactions[0] must be an object"
    );

    validate_bill_create_fields(["date", "type", "amount", "counterparty"]).unwrap();
    validate_bill_update_fields(["description", "destination_amount"]).unwrap();
    validate_batch_route_update_fields(["amount", "source_account_id"]).unwrap();
    assert_eq!(
        validate_bill_create_fields(["z_field", "date", "a_field", "z_field"])
            .unwrap_err()
            .message,
        "unsupported bill create fields: a_field, z_field"
    );
    assert_eq!(
        validate_bill_update_fields(["user_id"])
            .unwrap_err()
            .message,
        "unsupported bill update fields: user_id"
    );
    assert_eq!(
        validate_batch_route_update_fields(["batch_id"])
            .unwrap_err()
            .message,
        "unsupported update fields: batch_id"
    );

    let response = serde_json::to_value(batch_update_response(2, 1, vec![99])).unwrap();
    assert_eq!(response["updated_count"], 2);
    assert_eq!(response["failed_count"], 1);
    assert_eq!(response["failed_ids"], json!([99]));

    let success = serde_json::to_value(batch_create_success_response(
        vec![json!({"id": "1"}), json!({"id": "2"})],
        vec!["1".to_string(), "2".to_string()],
    ))
    .unwrap();
    assert_eq!(success["createdCount"], 2);
    assert_eq!(success["items"], json!([{"id": "1"}, {"id": "2"}]));
    assert_eq!(success["ids"], json!(["1", "2"]));
    assert!(success.get("failedIndex").is_none());

    let success_route = batch_create_success_route_response(
        vec![json!({"id": "1"}), json!({"id": "2"})],
        vec!["1".to_string(), "2".to_string()],
    );
    assert_eq!(success_route.status_code, 201);
    assert_eq!(success_route.body["success"], true);
    assert_eq!(success_route.body["result"]["createdCount"], 2);
    assert_eq!(success_route.body["result"]["ids"], json!(["1", "2"]));

    let failure = serde_json::to_value(batch_create_failure_response(
        3,
        vec![json!({"id": "1"})],
        vec!["1".to_string()],
    ))
    .unwrap();
    assert_eq!(failure["failedIndex"], 3);
    assert_eq!(failure["createdCount"], 1);
    assert_eq!(failure["items"], json!([{"id": "1"}]));
    assert_eq!(failure["ids"], json!(["1"]));

    let prepare_error = batch_create_prepare_error_route_response("bad input", 3);
    assert_eq!(prepare_error.status_code, 400);
    assert_eq!(prepare_error.body["success"], false);
    assert_eq!(prepare_error.body["error"], "bad input");
    assert_eq!(prepare_error.body["result"]["failedIndex"], 3);
    assert_eq!(prepare_error.body["result"]["createdCount"], 0);
    assert_eq!(prepare_error.body["result"]["items"], json!([]));
    assert!(prepare_error.body["result"].get("ids").is_none());

    let persist_error = batch_create_persist_error_route_response(
        "db failed",
        2,
        vec![json!({"id": "1"})],
        vec!["1".to_string()],
    );
    assert_eq!(persist_error.status_code, 500);
    assert_eq!(persist_error.body["success"], false);
    assert_eq!(persist_error.body["error"], "db failed");
    assert_eq!(persist_error.body["result"]["failedIndex"], 2);
    assert_eq!(persist_error.body["result"]["createdCount"], 1);
    assert_eq!(persist_error.body["result"]["ids"], json!(["1"]));

    let first_persist_error =
        batch_create_persist_error_route_response("db failed", 0, Vec::new(), Vec::new());
    assert_eq!(first_persist_error.status_code, 500);
    assert_eq!(first_persist_error.body["result"]["createdCount"], 0);
    assert_eq!(first_persist_error.body["result"]["items"], json!([]));
    assert_eq!(first_persist_error.body["result"]["ids"], json!([]));

    assert_eq!(
        delete_bill_success_payload(),
        json!({"success": true, "result": true, "message": "Bill deleted successfully"})
    );
    assert_eq!(
        legacy_delete_bill_success_payload(),
        json!({"success": true})
    );
    assert_eq!(
        batch_delete_success_payload(2),
        json!({"success": true, "result": {"deleted_count": 2}})
    );
    assert_eq!(
        batch_update_balance_sync_account_ids(["amount", "source_account_id"]),
        Vec::<i64>::new()
    );
}

#[test]
fn account_sync_ids_and_balance_formula_cover_crud_update_delete_paths() {
    let old_bill = BillAccountSyncSnapshot {
        source_account_id: Some(1),
        destination_account_id: Some(2),
    };
    let new_bill = BillAccountSyncSnapshot {
        source_account_id: Some(2),
        destination_account_id: Some(3),
    };

    assert_eq!(sync_account_ids_for_bill(old_bill), vec![1, 2]);
    assert_eq!(
        sync_account_ids_for_update(old_bill, new_bill),
        vec![1, 2, 3]
    );
    assert_eq!(
        sync_account_ids_for_batch_delete(&[
            old_bill,
            new_bill,
            BillAccountSyncSnapshot {
                source_account_id: Some(0),
                destination_account_id: None,
            },
        ]),
        vec![1, 2, 3]
    );

    let primary = 1;
    let peer = 2;
    let bills = vec![
        AccountBalanceBill {
            transaction_type: TransactionType::Income,
            amount: money("500.00"),
            destination_amount: Money::ZERO,
            source_account_id: Some(primary),
            destination_account_id: None,
        },
        AccountBalanceBill {
            transaction_type: TransactionType::Expense,
            amount: money("200.00"),
            destination_amount: Money::ZERO,
            source_account_id: Some(primary),
            destination_account_id: None,
        },
        AccountBalanceBill {
            transaction_type: TransactionType::Transfer,
            amount: money("300.00"),
            destination_amount: money("300.00"),
            source_account_id: Some(primary),
            destination_account_id: Some(peer),
        },
        AccountBalanceBill {
            transaction_type: TransactionType::Transfer,
            amount: money("100.00"),
            destination_amount: money("100.00"),
            source_account_id: Some(peer),
            destination_account_id: Some(primary),
        },
        AccountBalanceBill {
            transaction_type: TransactionType::Investment,
            amount: money("150.00"),
            destination_amount: money("150.00"),
            source_account_id: Some(primary),
            destination_account_id: Some(peer),
        },
        AccountBalanceBill {
            transaction_type: TransactionType::Investment,
            amount: money("50.00"),
            destination_amount: money("50.00"),
            source_account_id: Some(peer),
            destination_account_id: Some(primary),
        },
    ];

    assert_eq!(
        calculate_account_balance_from_bills(primary, money("1000.00"), &bills, Money::ZERO)
            .unwrap()
            .to_yuan_string(),
        "1000.00"
    );
    assert_eq!(
        calculate_account_balance_from_bills(peer, money("200.00"), &bills, Money::ZERO)
            .unwrap()
            .to_yuan_string(),
        "500.00"
    );
    assert_eq!(
        calculate_account_balance_from_bills(primary, money("1000.00"), &[], money("7.00"))
            .unwrap()
            .to_yuan_string(),
        "1007.00"
    );

    let frontend_signed_bills = vec![
        AccountBalanceBill {
            transaction_type: TransactionType::Expense,
            amount: money("-200.00"),
            destination_amount: Money::ZERO,
            source_account_id: Some(primary),
            destination_account_id: None,
        },
        AccountBalanceBill {
            transaction_type: TransactionType::Transfer,
            amount: money("-300.00"),
            destination_amount: money("100.00"),
            source_account_id: Some(primary),
            destination_account_id: Some(primary),
        },
    ];
    assert_eq!(
        calculate_account_balance_from_bills(
            primary,
            money("1000.00"),
            &frontend_signed_bills,
            Money::ZERO
        )
        .unwrap()
        .to_yuan_string(),
        "1600.00"
    );
}
