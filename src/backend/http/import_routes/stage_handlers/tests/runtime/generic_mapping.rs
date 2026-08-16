fn generic_mapping_multipart_body(boundary: &str, session_id: &str) -> Bytes {
    let csv = "日期,收支类型,金额,备注\n2026-01-15 08:00:00,收入,12.50,通用样本-早餐\n2026-01-16 09:30:00,支出,8.00,通用样本-公交\n";
    let mut body = Vec::new();

    for (name, value) in [("session_id", session_id), ("parser_type", "auto")] {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }

    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"files\"; filename=\"generic_statement_sample.csv\"\r\nContent-Type: text/csv\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(csv.as_bytes());
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Bytes::from(body)
}

fn remove_generic_mapping_temp_file(temp_path: &str) {
    let path = temp_import_root().join(temp_path);
    let session_dir = path.parent().map(FsPath::to_path_buf);
    let user_dir = session_dir
        .as_deref()
        .and_then(FsPath::parent)
        .map(FsPath::to_path_buf);

    let _ = fs::remove_file(path);
    if let Some(directory) = session_dir {
        let _ = fs::remove_dir(directory);
    }
    if let Some(directory) = user_dir {
        let _ = fs::remove_dir(directory);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn dedicated_no_match_flows_through_generic_mapping_into_standard_rows() {
    let Some((state, user_id, session_id)) = import_postgres_test_state().await else {
        return;
    };
    let boundary = "bill-analyser-generic-mapping-contract";
    let mut headers = import_test_headers(user_id);
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}"))
            .expect("multipart content type should be valid"),
    );

    let stage_one_response = import_parse_runtime_handler(
        State(state.clone()),
        headers.clone(),
        generic_mapping_multipart_body(boundary, &session_id),
    )
    .await;
    let (stage_one_status, stage_one_body) = import_test_response(stage_one_response).await;

    assert_eq!(stage_one_status, StatusCode::OK, "{stage_one_body}");
    assert_eq!(stage_one_body["data"]["parsed_count"], json!(0));
    assert_eq!(
        stage_one_body["data"]["unmatched_files"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        stage_one_body["data"]["unmatched_files"][0]["parser_decision"]["status"],
        json!("no_match")
    );
    assert!(stage_one_body["data"]["unmatched_files"][0]["parser_decision"]
        ["selected_parser_id"]
        .is_null());
    let temp_path = stage_one_body["data"]["unmatched_files"][0]["temp_path"]
        .as_str()
        .expect("unmatched file should expose a temporary path")
        .to_owned();

    let generic_response = import_parse_generic_runtime_handler(
        State(state.clone()),
        headers,
        Json(json!({
            "session_id": session_id,
            "temp_path": temp_path,
            "column_mapping": {
                "1": 0,
                "3": 1,
                "8": 2,
                "14": 3
            },
            "has_header_line": true,
            "file_encoding": "utf-8"
        })),
    )
    .await;
    let (generic_status, generic_body) = import_test_response(generic_response).await;

    assert_eq!(generic_status, StatusCode::OK, "{generic_body}");
    assert_eq!(generic_body["data"]["parsed_count"], json!(2));
    assert_eq!(
        generic_body["data"]["files"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        generic_body["data"]["unmatched_files"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let runtime = state
        .open_postgres_repository_runtime("generic-mapping-contract")
        .expect("PostgreSQL runtime should open");
    let canonical_user = UserId::new(user_id as u64).expect("test user id should be valid");
    let rows = get_import_standard_rows_by_session(runtime.pool(), &session_id, canonical_user)
        .expect("generic mapping should persist standard rows");

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].source_index, 0);
    assert_eq!(rows[1].source_index, 0);
    assert_eq!(rows[0].source_row_index, 0);
    assert_eq!(rows[1].source_row_index, 1);
    assert_eq!(rows[0].parser_id, "rust-import");
    assert_eq!(rows[1].parser_id, "rust-import");
    assert_eq!(rows[0].amount_cents, 1_250);
    assert_eq!(rows[1].amount_cents, -800);
    assert_eq!(rows[0].direction, "income");
    assert_eq!(rows[1].direction, "expense");
    assert!(rows[0].description.contains("通用样本-早餐"));
    assert!(rows[1].description.contains("通用样本-公交"));
    assert!(rows[0].occurred_at.starts_with("2026-01-15"));
    assert!(rows[1].occurred_at.starts_with("2026-01-16"));

    clear_session_data(runtime.pool(), &session_id, canonical_user)
        .expect("generic mapping contract session should be cleaned up");
    remove_generic_mapping_temp_file(&temp_path);
}
