use super::super::super::*;
use super::super::payload::import_file_preview_payload;
use super::helpers::cleanup_temp_fixture;

#[test]
fn spreadsheet_error_kinds_map_to_stable_http_envelopes() {
    let oversized = vec![b'x'; 10 * 1024 * 1024 + 1];
    let cases = [
        (
            bill_analyser_parsers::validate_spreadsheet_payload(b"not-a-spreadsheet")
                .expect_err("invalid spreadsheet"),
            400,
            "Import preview spreadsheet content is invalid",
        ),
        (
            bill_analyser_parsers::validate_spreadsheet_payload(&oversized)
                .expect_err("oversized spreadsheet"),
            413,
            "Import preview file exceeds the size limit",
        ),
        (
            bill_analyser_parsers::validate_spreadsheet_payload(
                b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1",
            )
            .expect_err("legacy OLE spreadsheet"),
            415,
            "Legacy binary XLS preview is not supported; convert the file to XLSX or CSV",
        ),
    ];

    for (error, expected_status, expected_message) in cases {
        let response = spreadsheet_validation_error_response(error);
        assert_eq!(response.status_code, expected_status);
        assert_eq!(response.body["success"], false);
        assert_eq!(response.body["error"], expected_message);
    }
}

#[test]
fn csv_mapping_enforces_bounded_table_contracts() {
    let rows = csv_rows_from_text(" notice \n date , amount \n 2026-01-15 , -12.50 ", None)
        .expect("bounded CSV mapping");
    assert_eq!(
        rows,
        vec![
            vec!["date".to_string(), "amount".to_string()],
            vec!["2026-01-15".to_string(), "-12.50".to_string()],
        ]
    );

    let too_many_rows = std::iter::repeat_n("date,amount", 50_001)
        .collect::<Vec<_>>()
        .join("\n");
    let response =
        csv_rows_from_text(&too_many_rows, Some(",")).expect_err("row limit must be enforced");
    assert_eq!(response.status_code, 413);
    assert_eq!(
        response.body["error"],
        "Generic import table exceeds the row limit"
    );

    let too_many_columns = std::iter::repeat_n("value", 129)
        .collect::<Vec<_>>()
        .join(",");
    let response = csv_rows_from_text(&too_many_columns, Some(","))
        .expect_err("column limit must be enforced");
    assert_eq!(response.status_code, 413);
    assert_eq!(
        response.body["error"],
        "Generic import table exceeds the column limit"
    );

    let row_at_column_limit = std::iter::repeat_n("x", 128).collect::<Vec<_>>().join(",");
    let too_many_cells = std::iter::repeat_n(row_at_column_limit.as_str(), 7_813)
        .collect::<Vec<_>>()
        .join("\n");
    let response = csv_rows_from_text(&too_many_cells, Some(","))
        .expect_err("aggregate cell limit must be enforced");
    assert_eq!(response.status_code, 413);
    assert_eq!(
        response.body["error"],
        "Generic import table exceeds the cell limit"
    );

    let oversized_cell = "x".repeat(GENERIC_TEXT_IMPORT_MAX_CELL_BYTES + 1);
    let response = csv_rows_from_text(&format!("date,comment\n2026-01-15,{oversized_cell}"), None)
        .expect_err("cell byte limit must be enforced");
    assert_eq!(response.status_code, 413);
    assert_eq!(
        response.body["error"],
        "Generic import table contains an oversized cell"
    );
}

#[test]
fn generic_mapping_helpers_fail_closed_and_preserve_fallback_fields() {
    assert_eq!(
        detect_csv_table_start("notice\n交易日期,金额", true),
        Some((1, ','))
    );
    assert_eq!(detect_csv_table_start("a,b", true), None);
    assert_eq!(detect_csv_table_start("a,b", false), Some((0, ',')));
    assert!(looks_like_import_headers(
        ["\u{feff}\" trade_time \"", "source_amount"].into_iter()
    ));
    assert_eq!(normalized_header_key("'\u{feff} A B\t'"), "ab");

    assert!(column_mapping_from_payload(json!({}).as_object().unwrap()).is_err());
    assert!(
        column_mapping_from_payload(json!({"column_mapping": []}).as_object().unwrap()).is_err()
    );
    assert!(column_mapping_from_payload(
        json!({"column_mapping": {"bad": -1, "0": 1}})
            .as_object()
            .unwrap()
    )
    .is_err());

    let payload = json!({
        "column_mapping": "{\"1\":0,\"3\":1,\"4\":2,\"5\":3,\"6\":4,\"8\":5,\"14\":6}",
        "transactionTypeMapping": {"credit": 2, "debit": 3, "ignored": 99},
        "hasHeaderLine": "yes"
    });
    let object = payload.as_object().unwrap();
    let mapping = column_mapping_from_payload(object).unwrap();
    let types = transaction_type_mapping_from_payload(object);
    assert_eq!(types.get("credit").map(String::as_str), Some("收入"));
    assert_eq!(types.get("debit").map(String::as_str), Some("支出"));
    assert!(!types.contains_key("ignored"));

    let row = vec!["2026-01-15", "credit", "餐饮", "早餐", "现金", "12.50", ""]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let (raw, main, sub) = raw_bill_from_column_mapped_row(&row, &mapping, &types).unwrap();
    assert_eq!(raw.transaction_type, "收入");
    assert_eq!(raw.description, "餐饮");
    assert_eq!((main.as_str(), sub.as_str()), ("餐饮", "早餐"));
    assert_eq!(mapped_column_text(&row, &mapping, 999), "");
}

#[test]
fn xlsx_preview_rows_continue_through_generic_column_mapping() {
    let user_id = UserId::new(9205).expect("positive test user id");
    let session_id = "preview-xlsx-generic";
    let bytes = include_bytes!(
        "../../../../../../tests/fixtures/import_samples/wechat_statement_sample.xlsx"
    );
    let preview =
        import_file_preview_payload("statement.xlsx", bytes, None, "auto").expect("XLSX preview");
    assert_eq!(preview["headers"][0], "交易时间");

    let temp_path = save_unmatched_import_file(user_id, session_id, "statement.xlsx", bytes)
        .expect("persist XLSX fixture");
    let payload = json!({
        "column_mapping": {"1": 0, "3": 4, "8": 5, "14": 3},
        "has_header_line": true,
        "file_encoding": "auto"
    });
    let bills = standard_bills_from_temp_path_payload(
        payload.as_object().expect("mapping payload"),
        &temp_path,
        user_id,
        session_id,
    )
    .expect("XLSX generic mapping must share preview rows");
    assert_eq!(bills.len(), 2);
    assert_eq!(bills[0].date, "2026-01-15 08:01:00");
    cleanup_temp_fixture(&temp_path);
}

#[test]
fn generic_spreadsheet_mapping_materializes_rows_beyond_preview_sample() {
    let user_id = UserId::new(9207).expect("positive test user id");
    let session_id = "generic-spreadsheet-full-rows";
    let mut html = String::from("<table><tr><th>date</th><th>amount</th></tr>");
    for day in 0..513 {
        html.push_str(&format!(
            "<tr><td>2026-01-{:02}</td><td>{}.00</td></tr>",
            day % 28 + 1,
            day + 1
        ));
    }
    html.push_str("</table>");
    let temp_path =
        save_unmatched_import_file(user_id, session_id, "statement.xlsx", html.as_bytes())
            .expect("persist bounded spreadsheet fixture");
    let payload = json!({
        "column_mapping": {"1": 0, "8": 1},
        "has_header_line": true
    });

    let bills = standard_bills_from_temp_path_payload(
        payload.as_object().expect("mapping payload"),
        &temp_path,
        user_id,
        session_id,
    )
    .expect("generic mapping must materialize every bounded row");

    assert_eq!(bills.len(), 513);
    cleanup_temp_fixture(&temp_path);
}

#[test]
fn table_text_from_line_returns_a_borrowed_suffix() {
    let text = "metadata\r\nmore metadata\nheader,amount\n2026-01-01,1.00\n";
    let table = table_text_from_line(text, 2);

    assert_eq!(table, "header,amount\n2026-01-01,1.00\n");
    assert_eq!(
        table.as_ptr() as usize - text.as_ptr() as usize,
        text.find("header,amount").expect("header offset")
    );
}

#[test]
fn temp_mapping_decodes_csv_and_rejects_unsupported_extensions() {
    let user_id = UserId::new(9206).expect("positive test user id");
    let session_id = "preview-csv-generic";
    let csv = b"date,amount\n2026-01-15,-12.50\n";
    let temp_path = save_unmatched_import_file(user_id, session_id, "statement.csv", csv)
        .expect("persist CSV fixture");
    let payload = json!({
        "column_mapping": {"1": 0, "8": 1},
        "has_header_line": true
    });
    let bills = standard_bills_from_temp_path_payload(
        payload.as_object().expect("mapping payload"),
        &temp_path,
        user_id,
        session_id,
    )
    .expect("CSV generic mapping must decode the persisted bytes");
    assert_eq!(bills.len(), 1);
    assert_eq!(bills[0].amount.to_yuan_string(), "-12.50");
    cleanup_temp_fixture(&temp_path);

    let unsupported = save_unmatched_import_file(
        user_id,
        session_id,
        "statement.bin",
        b"not a supported table",
    )
    .expect("persist unsupported fixture");
    let response = standard_bills_from_temp_path_payload(
        payload.as_object().expect("mapping payload"),
        &unsupported,
        user_id,
        session_id,
    )
    .expect_err("unsupported persisted extension must fail at the HTTP boundary");
    assert_eq!(response.status_code, 415);
    assert_eq!(
        response.body["error"],
        "Generic column mapping file type is not supported"
    );
    cleanup_temp_fixture(&unsupported);
}
