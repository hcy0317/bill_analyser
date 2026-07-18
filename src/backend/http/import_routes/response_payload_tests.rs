#[cfg(test)]
mod response_payload_tests {
    use super::*;
    use bill_analyser_core::Money;
    use zip::{write::SimpleFileOptions, ZipWriter};

    #[test]
    fn request_budget_rejects_aggregate_materialized_bill_output() {
        let bill = StandardBill {
            date: "2026-07-18 00:00:00".to_string(),
            amount: Money::ZERO,
            transaction_type: "支出".to_string(),
            description: "x".repeat(256),
            source_account_id: String::new(),
            counterparty: String::new(),
            payment_method: String::new(),
            parser_tags: vec!["tag".to_string()],
            original_type: String::new(),
            original_category: String::new(),
            transaction_id: String::new(),
            merchant_id: String::new(),
            status: String::new(),
            main_category: String::new(),
            sub_category: String::new(),
        };
        let one_bill_bytes = standard_bill_materialized_bytes(&bill);
        let mut budget = ImportParseRequestBudget::new(one_bill_bytes);

        budget.consume_bills(std::slice::from_ref(&bill)).expect("first bill fits");
        let error = budget.consume_bills(&[bill]).expect_err("aggregate output must be bounded");

        assert_eq!(error.status_code, 413);
        assert_eq!(
            error.body.get("error").and_then(Value::as_str),
            Some("Import parser request exceeds the materialized output limit")
        );
    }

    #[tokio::test]
    async fn bounded_multipart_parse_preserves_order_and_defaults_missing_filename() {
        let body = include_bytes!(
            "../../../../tests/fixtures/import_samples/abc_statement_sample.csv"
        )
        .to_vec();
        let file_parts = vec![
            MultipartPart {
                name: "files".to_string(),
                filename: Some("abc_statement_sample.csv".to_string()),
                content_type: None,
                body: body.clone(),
            },
            MultipartPart {
                name: "file".to_string(),
                filename: None,
                content_type: None,
                body,
            },
        ];

        let results = parse_multipart_import_files_bounded(file_parts, "auto")
            .await
            .expect("bounded multipart parsing");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 0);
        assert_eq!(results[1].index, 1);
        assert_eq!(results[0].original_name, "abc_statement_sample.csv");
        assert_eq!(results[1].original_name, "import-file.csv");
        assert!(results[0].parsed.is_some());
    }

    #[test]
    fn unmatched_payload_maps_malformed_binary_xls_to_invalid_spreadsheet() {
        let result = parse_multipart_import_file(ImportMultipartFileParseInput {
            index: 0,
            original_name: "legacy.xls".to_string(),
            body: b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1".to_vec(),
            requested_parser: "auto".to_string(),
        });
        let payload = unmatched_import_file_payload(
            &result.original_name,
            "user-1/session/file.xls",
            "rust-import",
            &result.decision,
        );

        assert_eq!(
            payload.get("error_code").and_then(Value::as_str),
            Some("invalid_spreadsheet")
        );
        assert_eq!(
            payload.get("errorCode").and_then(Value::as_str),
            Some("invalid_spreadsheet")
        );
        assert!(payload.get("reason").and_then(Value::as_str).is_some());
    }

    #[test]
    fn stage1_auto_rejects_hostile_xlsx_before_dedicated_parser_materialization() {
        let result = parse_multipart_import_file(ImportMultipartFileParseInput {
            index: 0,
            original_name: "far-reference.xlsx".to_string(),
            body: hostile_far_reference_xlsx(),
            requested_parser: "auto".to_string(),
        });

        assert!(result.parsed.is_none());
        assert_eq!(result.decision.status, "no_match");
        assert_eq!(
            result.decision.reason,
            "Import preview worksheet cell reference exceeds the row or column limit"
        );
    }

    fn hostile_far_reference_xlsx() -> Vec<u8> {
        let cursor = io::Cursor::new(Vec::new());
        let mut writer = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (name, contents) in [
            ("[Content_Types].xml", "<Types/>"),
            (
                "xl/workbook.xml",
                r#"<workbook><sheets><sheet name="Sheet1" r:id="rId1"/></sheets></workbook>"#,
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#,
            ),
            (
                "xl/worksheets/sheet1.xml",
                r#"<worksheet><dimension ref="A1:A1"/><sheetData><row r="1"><c r="XFD1048576"><v>1</v></c></row></sheetData></worksheet>"#,
            ),
        ] {
            writer.start_file(name, options).expect("start XLSX entry");
            writer
                .write_all(contents.as_bytes())
                .expect("write XLSX entry");
        }
        writer.finish().expect("finish XLSX").into_inner()
    }
}
