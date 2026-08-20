#[cfg(test)]
mod response_payload_tests {
    use super::*;
    use bill_analyser_core::Money;
    use zip::{write::SimpleFileOptions, ZipWriter};

    #[test]
    fn import_response_contract_keeps_error_message_and_status_envelopes() {
        let missing_session_id = import_v2_error_response(400, "Missing session_id");
        assert_eq!(missing_session_id.status_code, 400);
        assert_eq!(
            missing_session_id.body,
            json!({"success": false, "error": "Missing session_id"})
        );
        assert_eq!(
            import_v2_error_response(400, "Invalid request").body,
            json!({"success": false, "error": "Invalid request"})
        );
        assert_eq!(
            preview_state_conflict_response().body,
            json!({"success": false, "error": "Preview state changed, please refresh"})
        );
        assert_eq!(preview_state_conflict_response().status_code, 409);
        assert_eq!(import_session_not_found_response().status_code, 404);
        assert_eq!(
            import_session_cancel_missing_response().body,
            json!({"success": false, "message": "Session not found"})
        );
        assert_eq!(
            import_session_cancel_success_response().body,
            json!({"success": true, "message": "Session cleared"})
        );

        let invalid_status = route_response(import_v2_error_response(99, "invalid status"));
        assert_eq!(invalid_status.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn import_stage_response_contract_keeps_typed_data_envelopes() {
        let parse = import_stage_parse_success(ImportStageParseData {
            session_id: "sess-parse".to_string(),
            parsed_count: 2,
            files: vec![json!({"file": "a.csv", "success": true})],
            unmatched_files: vec![json!({"file": "unknown.csv"})],
            errors: vec![],
        });
        assert_eq!(parse.body["data"]["parsed_count"], 2);
        assert_eq!(
            parse.body["data"]["unmatched_files"][0]["file"],
            "unknown.csv"
        );

        let dedup = import_stage_dedup_success(ImportStageDedupData {
            session_id: "sess-preview".to_string(),
            preview: vec![json!({"id": 1})],
            preview_included: true,
            total: 2,
            after_dedup: 1,
            dedup_stats: json!({"removed": 1}),
            match_stats: json!({"transfer": 0}),
        });
        assert_eq!(dedup.body["data"]["preview_included"], true);
        assert_eq!(dedup.body["data"]["dedup_stats"]["removed"], 1);

        let confirm = import_stage_confirm_success(ImportStageConfirmData {
            imported_count: 1,
            skipped_count: 1,
            errors: vec!["duplicate".to_string()],
        });
        assert_eq!(confirm.body["data"]["imported_count"], 1);
        assert_eq!(confirm.body["data"]["errors"][0], "duplicate");
    }

    #[test]
    fn import_success_response_contract_keeps_session_and_preview_keys() {
        let session = ImportSessionSummary {
            session_id: "sess-1".to_string(),
            session_version: 7,
            status: "previewing".to_string(),
            created_at: "2026-05-01 08:00:00".to_string(),
            parsed_count: 3,
            preview_count: 2,
            file_paths: json!("a.csv"),
        };
        let response = import_session_success(session);
        assert_eq!(response.status_code, 200);
        assert_eq!(response.body["data"]["session_id"], "sess-1");
        assert_eq!(response.body["data"]["session_version"], 7);
        assert_eq!(response.body["data"]["preview_count"], 2);
        assert_eq!(response.body["data"]["file_paths"], "a.csv");

        let page = import_preview_page_success(ImportPreviewPageData {
            preview: vec![json!({"id": 1})],
            total: 1,
            page: 1,
            page_size: 50,
            query: Some(json!({"page": 1, "page_size": 50})),
            metadata: Some(json!({"counts": {"total": 1}})),
        });
        assert_eq!(
            page.body,
            json!({
                "success": true,
                "data": {
                    "preview": [{"id": 1}],
                    "total": 1,
                    "page": 1,
                    "page_size": 50,
                    "query": {"page": 1, "page_size": 50},
                    "metadata": {"counts": {"total": 1}}
                }
            })
        );

        let preview_value = json!({"id": 1, "preview_type": "expense"});
        let index_item = build_import_preview_filter_index_item(
            preview_value.as_object().expect("preview object"),
            &BTreeMap::<i64, CategoryLookup>::new(),
            &BTreeMap::<i64, AccountLookup>::new(),
        );
        let index = import_preview_index_success(ImportPreviewIndexData {
            items: vec![index_item],
            total: 1,
        });
        assert_eq!(index.body["data"]["items"][0]["id"], 1);
        assert_eq!(index.body["data"]["total"], 1);
    }

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

    #[test]
    fn stage1_response_exposes_component_server_timing() {
        let mut response = route_response(import_v2_data_response(json!({})));
        attach_import_parse_server_timing(
            &mut response,
            ImportParseServerTiming {
                multipart_ms: 12,
                parser_ms: 34,
                staging_ms: 56,
                total_ms: 78,
            },
        );

        assert_eq!(
            response
                .headers()
                .get("server-timing")
                .and_then(|value| value.to_str().ok()),
            Some("multipart;dur=12, parser;dur=34, staging;dur=56, total;dur=78")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stage1_json_runtime_response_attaches_server_timing_after_staging() {
        let postgres_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL")
            .expect("BILL_ANALYSER_TEST_POSTGRES_URL is required for the staging response test");
        let state = HttpAppState::new(
            crate::config::HttpShellConfig::default()
                .with_postgres_url(postgres_url)
                .expect("test postgres URL")
                .with_trusted_user_header_secret("import-test-secret"),
        )
        .expect("test state");
        let runtime = state
            .open_postgres_repository_runtime("stage1-server-timing-test")
            .expect("postgres runtime");
        bill_analyser_db::run_postgres_migrations(runtime.pool())
            .await
            .expect("postgres migrations");
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let username = format!("stage1-timing-{nonce}");
        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id",
        )
        .bind(&username)
        .bind(format!("{username}@example.test"))
        .fetch_one(runtime.pool())
        .await
        .expect("insert test user");
        let mut headers = HeaderMap::new();
        headers.insert(
            TRUSTED_USER_SECRET_HEADER,
            axum::http::HeaderValue::from_static("import-test-secret"),
        );
        headers.insert(
            "x-user-id",
            axum::http::HeaderValue::from_str(&user_id.to_string()).expect("user header"),
        );
        let payload = json!({
            "bills": [{
                "date": "2026-08-15 12:00:00",
                "amount": "12.34",
                "transaction_type": "支出",
                "description": "stage1 timing contract"
            }]
        });

        let response = import_parse_json_runtime_response(&state, &headers, &payload, false);

        assert_eq!(response.status(), StatusCode::OK);
        let timing = response
            .headers()
            .get("server-timing")
            .and_then(|value| value.to_str().ok())
            .expect("stage1 server timing header");
        for metric in ["multipart", "parser", "staging", "total"] {
            assert!(
                timing
                    .split(',')
                    .any(|entry| entry.trim().starts_with(&format!("{metric};dur="))),
                "missing {metric} metric in {timing}"
            );
        }
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
