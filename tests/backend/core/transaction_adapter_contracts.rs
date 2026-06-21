use bill_analyser_core::adapters::transaction::{
    apply_create_category_contract, apply_manual_create_defaults, batch_create_failure_response,
    batch_create_persist_error_route_response, batch_create_prepare_error_route_response,
    batch_create_success_response, batch_create_success_route_response,
    batch_create_transaction_items, batch_delete_success_payload,
    batch_update_balance_sync_account_ids, batch_update_response, build_reconciliation_filters,
    build_reconciliation_transactions, calculate_account_balance_from_bills,
    calculate_reconciliation_summary, delete_bill_success_payload,
    frontend_transaction_from_backend, frontend_transaction_mutation_to_backend,
    invalid_reconciliation_account_id_response, invalid_transaction_picture_file_response,
    is_allowed_transaction_picture_filename, is_formula_like_export_cell,
    missing_reconciliation_parameters_response, missing_transaction_picture_file_response,
    missing_unused_transaction_picture_id_response, month_date_range, parse_reconciliation_query,
    reconciliation_account_not_found_response, reconciliation_category_filters,
    reconciliation_internal_error_response, reconciliation_opening_balance,
    reconciliation_result_payload, reconciliation_success_response, reconciliation_type_filter,
    remove_unused_transaction_picture_success_payload,
    remove_unused_transaction_picture_success_response, secure_picture_file_name,
    serialize_export_cell, serialize_optional_export_cell, sync_account_ids_for_batch_delete,
    sync_account_ids_for_bill, sync_account_ids_for_update, transaction_list_type_filter,
    transaction_picture_data_url_from_base64, transaction_picture_delete_path,
    transaction_picture_extension, transaction_picture_internal_error_response,
    transaction_picture_mime_type, transaction_picture_upload_id,
    transaction_picture_upload_success_payload, transaction_picture_upload_success_response,
    unsupported_transaction_picture_type_message, unsupported_transaction_picture_type_response,
    validate_batch_route_update_fields, validate_bill_create_fields, validate_bill_update_fields,
    AccountBalanceBill, BackendTransactionView, BillAccountSyncSnapshot, FrontendTransactionTag,
    ReconciliationBill, ReconciliationCategoryRecord, ReconciliationOpeningBalanceSnapshot,
};
use bill_analyser_core::primitives::{Money, TransactionType, UtcOffsetMinutes};
use bill_analyser_core::ErrorCode;
use chrono::Local;
use serde_json::json;
use std::path::Path;

fn money(yuan: &str) -> Money {
    Money::from_yuan_str(yuan).unwrap()
}

fn local_date_from_timestamp(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .unwrap()
        .with_timezone(&Local)
        .format("%Y-%m-%d")
        .to_string()
}

#[test]
fn backend_transaction_view_serializes_for_frontend_adapter() {
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
    assert_eq!(value["amountCents"], 1234);
    assert_eq!(value["sourceAmountCents"], 1234);
    assert_eq!(value["destinationAmountCents"], 1234);
    assert!(value.get("amount").is_none());
    assert!(value.get("sourceAmount").is_none());
    assert!(value.get("destinationAmount").is_none());
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
fn transfer_destination_amount_uses_backend_destination_cents_when_present() {
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
    assert_eq!(value["sourceAmountCents"], 8800);
    assert_eq!(value["destinationAmountCents"], 8750);
    assert_eq!(value["sourceAccountId"], "1");
    assert_eq!(value["destinationAccountId"], "2");
}

#[test]
fn zero_destination_amount_falls_back_to_source_amount() {
    let bill = BackendTransactionView {
        id: "100".to_string(),
        transaction_type: TransactionType::Expense,
        amount: money("-19.90"),
        destination_amount: Some(Money::ZERO),
        ..Default::default()
    };

    let value = serde_json::to_value(frontend_transaction_from_backend(&bill)).unwrap();

    assert_eq!(value["amountCents"], 1990);
    assert_eq!(value["destinationAmountCents"], 1990);
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
    assert_eq!(
        serialize_export_cell("source_account", "=HYPERLINK(\"http://example.test\")"),
        "'=HYPERLINK(\"http://example.test\")"
    );
    assert_eq!(serialize_export_cell("tags", "@cmd"), "'@cmd");
    assert_eq!(serialize_export_cell("amount", "-12.34"), "-12.34");
    assert_eq!(serialize_export_cell("description", "normal"), "normal");
    assert_eq!(
        serialize_optional_export_cell::<String>("description", None),
        ""
    );
}

#[test]
fn frontend_mutation_to_backend_and_create_defaults_match_current_write_adapter() {
    let frontend = json!({
        "type": 3,
        "time": 1_735_758_245,
        "sourceAmountCents": 1234,
        "destinationAmountCents": 0,
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
    assert_eq!(backend["amount_cents"], -1234);
    assert_eq!(backend["destination_amount_cents"], 0);
    assert!(backend.get("amount").is_none());
    assert!(backend.get("destination_amount").is_none());
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
        json!({"type": 2, "time": 1_735_758_245_000i64, "sourceAmountCents": 2500});
    let (backend_from_millis, _) =
        frontend_transaction_mutation_to_backend(&millisecond_frontend, UtcOffsetMinutes::new(480))
            .unwrap();
    assert_eq!(backend_from_millis["type"], "收入");
    assert_eq!(backend_from_millis["date"], "2025-01-02 03:04:05");
    assert_eq!(backend_from_millis["amount_cents"], 2500);

    let (backend_ignoring_offset, _) =
        frontend_transaction_mutation_to_backend(&frontend, UtcOffsetMinutes::new(0)).unwrap();
    assert_eq!(backend_ignoring_offset["date"], "2025-01-02 03:04:05");

    let bad_amount_error = frontend_transaction_mutation_to_backend(
        &json!({"type": 3, "sourceAmountCents": "bad", "destinationAmountCents": {"bad": true}}),
        UtcOffsetMinutes::new(480),
    )
    .expect_err("invalid explicit cents are rejected");
    assert_eq!(bad_amount_error.code, ErrorCode::InvalidInput);

    let bool_amount_error = frontend_transaction_mutation_to_backend(
        &json!({"type": 3, "sourceAmountCents": true}),
        UtcOffsetMinutes::new(480),
    )
    .expect_err("boolean cents are rejected");
    assert_eq!(bool_amount_error.code, ErrorCode::InvalidInput);
}

#[test]
fn frontend_mutation_to_backend_preserves_transfer_and_investment_destination_contracts() {
    let transfer = json!({
        "type": 4,
        "time": 1_735_758_245,
        "sourceAmountCents": 12345,
        "destinationAmountCents": 12300,
        "sourceAccountId": "10",
        "destinationAccountId": "11",
        "categoryId": "22",
        "tagIds": ["31", "32"],
        "comment": "内部转账"
    });

    let (transfer_backend, transfer_metadata) =
        frontend_transaction_mutation_to_backend(&transfer, UtcOffsetMinutes::new(480)).unwrap();

    assert_eq!(transfer_backend["type"], "转账");
    assert_eq!(transfer_backend["amount_cents"], -12345);
    assert_eq!(transfer_backend["destination_amount_cents"], 12300);
    assert_eq!(transfer_backend["source_account_id"], 10);
    assert_eq!(transfer_backend["destination_account_id"], 11);
    assert_eq!(transfer_backend["description"], "内部转账");
    assert_eq!(transfer_metadata.category_id, "22");
    assert_eq!(transfer_metadata.source_account_id, 10);
    assert_eq!(transfer_metadata.destination_account_id, 11);
    assert_eq!(transfer_metadata.tag_ids, vec![31, 32]);

    let investment = json!({
        "type": 5,
        "time": 1_735_758_245,
        "sourceAmountCents": 100000,
        "destinationAmountCents": 99888,
        "sourceAccountId": 12,
        "destinationAccountId": 13,
        "categoryId": "88",
        "tagIds": [90],
        "comment": "基金买入"
    });

    let (investment_backend, investment_metadata) =
        frontend_transaction_mutation_to_backend(&investment, UtcOffsetMinutes::new(480)).unwrap();

    assert_eq!(investment_backend["type"], "投资");
    assert_eq!(investment_backend["amount_cents"], -100000);
    assert_eq!(investment_backend["destination_amount_cents"], 99888);
    assert_eq!(investment_backend["source_account_id"], 12);
    assert_eq!(investment_backend["destination_account_id"], 13);
    assert_eq!(investment_backend["description"], "基金买入");
    assert_eq!(investment_metadata.category_id, "88");
    assert_eq!(investment_metadata.source_account_id, 12);
    assert_eq!(investment_metadata.destination_account_id, 13);
    assert_eq!(investment_metadata.tag_ids, vec![90]);
}

#[test]
fn create_category_contracts_cover_current_rest_edges() {
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

    validate_bill_create_fields(["date", "type", "amount_cents", "counterparty"]).unwrap();
    validate_bill_update_fields(["description", "destination_amount_cents"]).unwrap();
    validate_batch_route_update_fields(["amount_cents", "source_account_id"]).unwrap();
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
        batch_delete_success_payload(2),
        json!({"success": true, "result": {"deleted_count": 2}})
    );
    assert_eq!(
        batch_update_balance_sync_account_ids(["amount_cents", "source_account_id"]),
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

#[test]
fn reconciliation_query_filters_and_error_envelopes_match_bills_route_contract() {
    let missing = parse_reconciliation_query(None, Some(0), Some(0), None, None, None)
        .expect_err("missing account_id should produce route error");
    assert_eq!(missing.status_code, 400);
    assert_eq!(
        missing.body,
        json!({"success": false, "error": "Missing required parameters: account_id, start_time, end_time"})
    );
    assert_eq!(missing, missing_reconciliation_parameters_response());

    let invalid = parse_reconciliation_query(Some("abc"), Some(0), Some(0), None, None, None)
        .expect_err("non-integer account_id should produce route error");
    assert_eq!(invalid.status_code, 400);
    assert_eq!(
        invalid.body,
        json!({"success": false, "error": "Invalid account_id: abc"})
    );
    assert_eq!(invalid, invalid_reconciliation_account_id_response("abc"));

    let timestamp_error =
        parse_reconciliation_query(Some("abc"), Some(i64::MAX), Some(0), None, None, None)
            .expect_err("date conversion should run before account_id parsing");
    assert_eq!(timestamp_error.status_code, 500);
    assert_eq!(
        timestamp_error.body,
        json!({
            "success": false,
            "error": "invalid reconciliation start_time",
            "message": "Failed to retrieve reconciliation statements"
        })
    );

    assert_eq!(
        reconciliation_account_not_found_response().body,
        json!({"success": false, "error": "Account not found"})
    );
    let internal = reconciliation_internal_error_response("reconciliation boom");
    assert_eq!(internal.status_code, 500);
    assert_eq!(
        internal.body,
        json!({
            "success": false,
            "error": "reconciliation boom",
            "message": "Failed to retrieve reconciliation statements"
        })
    );

    let params = parse_reconciliation_query(
        Some("1"),
        Some(0),
        Some(0),
        Some("10,11"),
        Some(4),
        Some("早餐"),
    )
    .unwrap();
    assert_eq!(params.account_id, "1");
    assert_eq!(params.account_id_int, 1);
    assert_eq!(params.start_date, None);
    assert_eq!(params.end_date, None);
    assert_eq!(reconciliation_type_filter(Some(1)), Some("收入"));
    assert_eq!(reconciliation_type_filter(Some(2)), Some("支出"));
    assert_eq!(reconciliation_type_filter(Some(3)), Some("转账"));
    assert_eq!(reconciliation_type_filter(Some(4)), Some("投资"));
    assert_eq!(reconciliation_type_filter(Some(5)), None);

    let categories = vec![
        ReconciliationCategoryRecord {
            id: 9,
            main_category: "忽略".to_string(),
            sub_category: "其他".to_string(),
        },
        ReconciliationCategoryRecord {
            id: 10,
            main_category: "餐饮".to_string(),
            sub_category: "早餐".to_string(),
        },
        ReconciliationCategoryRecord {
            id: 11,
            main_category: "学习".to_string(),
            sub_category: "课本".to_string(),
        },
    ];
    let category_filters =
        reconciliation_category_filters(params.category_ids.as_deref(), &categories);
    assert_eq!(
        serde_json::to_value(&category_filters).unwrap(),
        json!([
            {"main": "餐饮", "sub": "早餐"},
            {"main": "学习", "sub": "课本"}
        ])
    );
    assert!(reconciliation_category_filters(Some("10,bad"), &categories).is_empty());

    let filters = build_reconciliation_filters(&params, &category_filters);
    assert_eq!(
        filters,
        json!({
            "account_ids": [1],
            "categories": [
                {"main": "餐饮", "sub": "早餐"},
                {"main": "学习", "sub": "课本"}
            ],
            "type": "投资",
            "keyword": "早餐"
        })
    );

    let date_params = parse_reconciliation_query(
        Some("1"),
        Some(1_709_251_200),
        Some(1_709_337_600),
        None,
        None,
        None,
    )
    .unwrap();
    let start_date = local_date_from_timestamp(1_709_251_200);
    let end_date = local_date_from_timestamp(1_709_337_600);
    assert_eq!(date_params.start_date.as_deref(), Some(start_date.as_str()));
    assert_eq!(date_params.end_date.as_deref(), Some(end_date.as_str()));
    assert_eq!(
        build_reconciliation_filters(&date_params, &[]),
        json!({
            "account_ids": [1],
            "start_date": start_date,
            "end_date": end_date
        })
    );
}

#[test]
fn reconciliation_opening_balance_preserves_all_time_and_filtered_fallbacks() {
    let all_time =
        parse_reconciliation_query(Some("1"), Some(0), Some(0), None, None, None).unwrap();
    assert_eq!(
        reconciliation_opening_balance(&all_time, money("20.00"), &[]).to_cents(),
        2000
    );

    let filtered = parse_reconciliation_query(
        Some("1"),
        Some(1_709_251_200),
        Some(1_709_337_600),
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(
        reconciliation_opening_balance(&filtered, money("20.00"), &[]).to_cents(),
        0
    );
    assert_eq!(
        reconciliation_opening_balance(
            &filtered,
            money("20.00"),
            &[ReconciliationOpeningBalanceSnapshot {
                account_balance: money("55.00")
            }]
        )
        .to_cents(),
        5500
    );
}

#[test]
fn reconciliation_summary_transactions_and_payload_pin_current_balance_trace() {
    let account_id = 1;
    let bills = vec![
        ReconciliationBill {
            id: "expense".to_string(),
            date: "2026-03-02 08:00:00".to_string(),
            transaction_type: Some(TransactionType::Expense),
            amount: money("-8.00"),
            source_account_id: Some(account_id),
            destination_account_id: None,
        },
        ReconciliationBill {
            id: "income".to_string(),
            date: "2026-03-01 08:00:00".to_string(),
            transaction_type: Some(TransactionType::Income),
            amount: money("12.00"),
            source_account_id: Some(account_id),
            destination_account_id: None,
        },
        ReconciliationBill {
            id: "transfer-in".to_string(),
            date: "2026-03-03 08:00:00".to_string(),
            transaction_type: Some(TransactionType::Transfer),
            amount: money("5.00"),
            source_account_id: Some(9),
            destination_account_id: Some(account_id),
        },
        ReconciliationBill {
            id: "transfer-mismatch".to_string(),
            date: "2026-03-04 08:00:00".to_string(),
            transaction_type: Some(TransactionType::Transfer),
            amount: money("99.00"),
            source_account_id: Some(8),
            destination_account_id: Some(9),
        },
        ReconciliationBill {
            id: "investment-mismatch".to_string(),
            date: "2026-03-05 08:00:00".to_string(),
            transaction_type: Some(TransactionType::Investment),
            amount: money("7.00"),
            source_account_id: Some(8),
            destination_account_id: Some(9),
        },
        ReconciliationBill {
            id: "unknown".to_string(),
            date: "2026-03-06 08:00:00".to_string(),
            transaction_type: None,
            amount: money("99.00"),
            source_account_id: Some(account_id),
            destination_account_id: None,
        },
    ];

    let summary = calculate_reconciliation_summary(account_id, money("20.00"), &bills).unwrap();
    assert_eq!(summary.opening_balance.to_cents(), 2000);
    assert_eq!(summary.closing_balance.to_cents(), 2900);
    assert_eq!(summary.total_inflows.to_cents(), 1700);
    assert_eq!(summary.total_outflows.to_cents(), 800);
    assert_eq!(
        summary.balance_history["income"],
        bill_analyser_core::adapters::transaction::ReconciliationBalanceEntry {
            opening: money("20.00"),
            closing: money("32.00")
        }
    );
    assert_eq!(summary.balance_history["expense"].opening.to_cents(), 3200);
    assert_eq!(summary.balance_history["expense"].closing.to_cents(), 2400);
    assert!(!summary.balance_history.contains_key("transfer-mismatch"));
    assert!(!summary.balance_history.contains_key("unknown"));
    assert_eq!(
        summary.balance_history["investment-mismatch"]
            .opening
            .to_cents(),
        2900
    );
    assert_eq!(
        summary.balance_history["investment-mismatch"]
            .closing
            .to_cents(),
        2900
    );

    let same_date_summary = calculate_reconciliation_summary(
        account_id,
        money("10.00"),
        &[
            ReconciliationBill {
                id: "z-first".to_string(),
                date: "2026-03-07 08:00:00".to_string(),
                transaction_type: Some(TransactionType::Income),
                amount: money("1.00"),
                source_account_id: Some(account_id),
                destination_account_id: None,
            },
            ReconciliationBill {
                id: "a-second".to_string(),
                date: "2026-03-07 08:00:00".to_string(),
                transaction_type: Some(TransactionType::Expense),
                amount: money("2.00"),
                source_account_id: Some(account_id),
                destination_account_id: None,
            },
        ],
    )
    .unwrap();
    assert_eq!(
        same_date_summary.balance_history["z-first"]
            .opening
            .to_cents(),
        1000
    );
    assert_eq!(
        same_date_summary.balance_history["z-first"]
            .closing
            .to_cents(),
        1100
    );
    assert_eq!(
        same_date_summary.balance_history["a-second"]
            .opening
            .to_cents(),
        1100
    );

    let transactions = build_reconciliation_transactions(
        vec![
            json!({"id": "income", "time": 10}),
            json!({"id": "expense", "time": 30}),
            json!({"id": "transfer-in", "time": 20}),
            json!({"id": "investment-mismatch", "time": 5}),
            json!({"id": "transfer-mismatch", "time": 100}),
        ],
        &summary.balance_history,
    );
    assert_eq!(transactions.len(), 4);
    assert_eq!(transactions[0]["id"], "expense");
    assert_eq!(transactions[0]["accountOpeningBalanceCents"], 3200);
    assert_eq!(transactions[0]["accountClosingBalanceCents"], 2400);
    assert_eq!(transactions[1]["id"], "transfer-in");
    assert_eq!(transactions[1]["accountOpeningBalanceCents"], 2400);
    assert_eq!(transactions[1]["accountClosingBalanceCents"], 2900);

    let params = parse_reconciliation_query(Some("1"), Some(0), Some(0), None, None, None).unwrap();
    let payload =
        reconciliation_result_payload(&params, "现金账户", &summary, transactions.clone()).unwrap();
    assert_eq!(payload["accountId"], "1");
    assert_eq!(payload["accountName"], "现金账户");
    assert_eq!(payload["startTime"], 0);
    assert_eq!(payload["endTime"], 0);
    assert_eq!(payload["openingBalanceCents"], 2000);
    assert_eq!(payload["closingBalanceCents"], 2900);
    assert_eq!(payload["totalInflowsCents"], 1700);
    assert_eq!(payload["totalOutflowsCents"], 800);
    assert_eq!(payload["netFlowCents"], 900);
    assert_eq!(payload["itemCount"], 4);
    assert_eq!(payload["transactions"][0]["id"], "expense");

    let response =
        reconciliation_success_response(&params, "现金账户", &summary, transactions).unwrap();
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["success"], true);
    assert_eq!(response.body["result"], payload);
}

#[test]
fn transaction_picture_upload_contract_matches_rest_route_envelope_and_validation() {
    assert!(is_allowed_transaction_picture_filename("avatar.png"));
    assert!(is_allowed_transaction_picture_filename("avatar.JPG"));
    assert!(is_allowed_transaction_picture_filename(
        "avatar.with.dots.webp"
    ));
    assert!(is_allowed_transaction_picture_filename(
        "C:\\temp\\avatar.bmp"
    ));
    assert!(!is_allowed_transaction_picture_filename("avatar"));
    assert!(!is_allowed_transaction_picture_filename("avatar.txt"));
    assert!(is_allowed_transaction_picture_filename(".png"));
    assert!(!is_allowed_transaction_picture_filename(""));
    assert_eq!(
        transaction_picture_extension("avatar.JPEG").as_deref(),
        Some("jpeg")
    );

    let message = unsupported_transaction_picture_type_message();
    assert_eq!(
        message,
        "Picture type not allowed. Supported: bmp, gif, jpeg, jpg, png, webp"
    );

    assert_eq!(
        transaction_picture_upload_id("0123456789abcdef0123456789ABCDEF", "receipt.PNG").unwrap(),
        "0123456789abcdef0123456789abcdef.png"
    );
    assert_eq!(
        transaction_picture_upload_id("0123456789abcdef0123456789abcdef", "ümlaut.png").unwrap(),
        "0123456789abcdef0123456789abcdef.png"
    );
    assert_eq!(
        transaction_picture_upload_id("abcdefabcdefabcdefabcdefabcdefab", "../测试.PNG").unwrap(),
        "abcdefabcdefabcdefabcdefabcdefab"
    );
    assert_eq!(
        transaction_picture_upload_id("bad-uuid", "receipt.png")
            .unwrap_err()
            .message,
        "invalid picture uuid"
    );
    assert_eq!(
        transaction_picture_upload_id("0123456789abcdef0123456789abcdef", "receipt.exe")
            .unwrap_err()
            .message,
        message
    );

    assert_eq!(secure_picture_file_name("../../evil.png"), "evil.png");
    assert_eq!(secure_picture_file_name("my receipt.png"), "my_receipt.png");
    assert_eq!(secure_picture_file_name("ümlaut.png"), "umlaut.png");
    assert_eq!(secure_picture_file_name("CON.png"), "_CON.png");
    assert_eq!(secure_picture_file_name("NUL.jpg"), "_NUL.jpg");
    assert_eq!(transaction_picture_mime_type("receipt.png"), "image/png");
    assert_eq!(
        transaction_picture_mime_type("receipt.unknown"),
        "application/octet-stream"
    );
    assert_eq!(
        transaction_picture_data_url_from_base64("receipt.png", "YWJj"),
        "data:image/png;base64,YWJj"
    );

    let success = transaction_picture_upload_success_response(
        "pic.png",
        transaction_picture_data_url_from_base64("pic.png", "YWJj"),
    );
    assert_eq!(success.status_code, 200);
    assert_eq!(
        success.body,
        json!({"success": true, "result": {"pictureId": "pic.png", "originalUrl": "data:image/png;base64,YWJj"}})
    );
    assert_eq!(
        transaction_picture_upload_success_payload("pic.webp", "data:image/webp;base64,abc"),
        json!({"success": true, "result": {"pictureId": "pic.webp", "originalUrl": "data:image/webp;base64,abc"}})
    );

    let missing = missing_transaction_picture_file_response();
    assert_eq!(missing.status_code, 400);
    assert_eq!(
        missing.body,
        json!({"success": false, "error": "Missing picture file"})
    );
    assert_eq!(
        invalid_transaction_picture_file_response().body,
        json!({"success": false, "error": "Invalid picture file"})
    );
    assert_eq!(
        unsupported_transaction_picture_type_response().status_code,
        400
    );
    assert_eq!(
        unsupported_transaction_picture_type_response().body,
        json!({"success": false, "error": message})
    );
    assert_eq!(
        transaction_picture_internal_error_response("picture boom").body,
        json!({"success": false, "error": "picture boom"})
    );
    assert_eq!(
        transaction_picture_internal_error_response("picture boom").status_code,
        500
    );
}

#[test]
fn unused_transaction_picture_delete_contract_is_best_effort_and_filename_sanitized() {
    let upload_root = Path::new("data/uploads");
    assert_eq!(
        transaction_picture_delete_path(upload_root, "../stale.png"),
        upload_root.join("stale.png")
    );
    assert_eq!(
        transaction_picture_delete_path(upload_root, "nested\\stale.webp"),
        upload_root.join("nested_stale.webp")
    );
    assert_eq!(
        transaction_picture_delete_path(upload_root, "LPT1.webp"),
        upload_root.join("_LPT1.webp")
    );

    let missing = missing_unused_transaction_picture_id_response();
    assert_eq!(missing.status_code, 400);
    assert_eq!(
        missing.body,
        json!({"success": false, "error": "Missing picture id"})
    );

    let success = remove_unused_transaction_picture_success_response();
    assert_eq!(success.status_code, 200);
    assert_eq!(success.body, json!({"success": true, "result": true}));
    assert_eq!(
        remove_unused_transaction_picture_success_payload(),
        json!({"success": true, "result": true})
    );
}
