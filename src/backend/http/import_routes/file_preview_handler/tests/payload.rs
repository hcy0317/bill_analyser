use super::super::super::*;
use super::super::{payload::*, text::*};

#[test]
fn csv_preview_starts_at_known_headers_and_preserves_minor_unit_text() {
    let payload = import_file_preview_payload(
        "generic.csv",
        "说明\n日期,金额,备注\n2026-01-15,-12.50,早餐\n".as_bytes(),
        None,
        "auto",
    )
    .expect("CSV preview");

    assert_eq!(payload["headers"], json!(["日期", "金额", "备注"]));
    assert_eq!(payload["sampleData"][1][1], "-12.50");
    assert_eq!(payload["totalRows"], 1);
    assert_eq!(payload["delimiter"], ",");
}

#[test]
fn csv_preview_honors_explicit_encoding_and_rejects_unknown_labels() {
    let (gbk_bytes, _, had_errors) = GBK.encode("日期,金额,备注\n2026-01-15,-12.50,早餐\n");
    assert!(!had_errors, "GBK fixture must encode without replacement");

    let payload = import_file_preview_payload("generic.csv", &gbk_bytes, None, "gbk")
        .expect("explicit GBK preview");
    assert_eq!(payload["headers"], json!(["日期", "金额", "备注"]));
    assert_eq!(payload["encoding"], "gbk");

    let response = import_file_preview_payload("generic.csv", b"date,amount\n", None, "utf-16")
        .expect_err("unsupported encoding must fail closed");
    assert_eq!(response.status_code, 400);
}

#[test]
fn preview_maps_supported_and_semantic_spreadsheet_errors() {
    for (filename, bytes) in [
        (
            "statement.xls",
            b"<html><body><table><tr><th>date</th><th>amount</th></tr><tr><td>2026-01-15</td><td>-12.50</td></tr></table></body></html>"
                .as_slice(),
        ),
        (
            "statement.xlsx",
            include_bytes!("../../../../../../tests/fixtures/import_samples/wechat_statement_sample.xlsx")
                .as_slice(),
        ),
    ] {
        let payload = import_file_preview_payload(filename, bytes, None, "auto")
            .unwrap_or_else(|_| panic!("{filename} preview must succeed"));
        assert!(payload["headers"]
            .as_array()
            .is_some_and(|headers| !headers.is_empty()));
        assert_eq!(payload["encoding"], "auto");
    }

    let unsupported = import_file_preview_payload("payload.exe", b"x", None, "auto")
        .expect_err("unsupported extension");
    assert_eq!(unsupported.status_code, 415);

    let malformed = import_file_preview_payload("spoofed.xlsx", b"not-an-xlsx", None, "auto")
        .expect_err("malformed spreadsheet");
    assert_eq!(malformed.status_code, 400);

    let xls =
        include_bytes!("../../../../../../tests/fixtures/import_samples/abc_statement_sample.xls");
    let payload = import_file_preview_payload("statement.xls", xls, None, "auto")
        .expect("binary XLS preview");
    assert_eq!(payload["headers"][0], "交易日期");
    assert_eq!(payload["totalRows"], 2);
}

#[test]
fn text_preview_limits_and_helpers_fail_closed() {
    let too_many_columns = (0..129)
        .map(|index| format!("column-{index}"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        import_file_preview_payload(
            "wide.csv",
            format!("{too_many_columns}\n").as_bytes(),
            None,
            "auto",
        )
        .expect_err("wide preview")
        .status_code,
        400
    );
    let oversized_cell = "x".repeat(16_385);
    assert_eq!(
        import_file_preview_payload(
            "large-cell.csv",
            format!("date,comment\n2026-01-15,{oversized_cell}\n").as_bytes(),
            None,
            "auto",
        )
        .expect_err("oversized cell")
        .status_code,
        400
    );
    assert!(import_file_preview_payload("binary.csv", b"a\0b", None, "auto").is_err());
    assert!(import_file_preview_payload("empty.csv", b"", None, "auto").is_err());

    for (input, expected) in [
        ("", "auto"),
        ("UTF_8", "utf-8"),
        ("cp936", "gbk"),
        ("GB18030", "gb18030"),
    ] {
        assert_eq!(normalize_preview_encoding(input).unwrap(), expected);
    }
    assert!(normalize_preview_encoding("utf-16").is_err());
    assert_eq!(
        decode_preview_text("\u{feff}date,amount".as_bytes(), "utf-8").unwrap(),
        ("date,amount".to_string(), "utf-8".to_string())
    );
    assert_eq!(
        decode_preview_text(b"date,amount", "auto").unwrap().1,
        "utf-8"
    );
    assert!(decode_preview_text(&[0xff], "utf-8").is_err());
    assert!(decode_preview_text(&[0x81], "gbk").is_err());
    assert!(decode_preview_text(&[0x81], "gb18030").is_err());
    assert!(decode_preview_text(b"x", "utf-16").is_err());

    let (trimmed, offset) = trim_preview_rows_to_header(vec![
        vec!["notice".to_string()],
        vec!["date".to_string(), "amount".to_string()],
        vec!["2026-01-15".to_string(), "1".to_string()],
    ]);
    assert_eq!(offset, 1);
    assert_eq!(trimmed.len(), 2);
}
