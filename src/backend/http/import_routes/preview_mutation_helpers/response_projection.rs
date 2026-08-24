fn preview_row_to_value(row: ImportPreviewRow) -> Value {
    let mut value = serde_json::to_value(row).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        attach_import_preview_matching_payload(object);
        attach_import_preview_state_snapshot_to_canonical_row(object);
    }
    value
}

fn preview_row_version_conflict_response(
    expected_row_version: i64,
    latest_row: ImportPreviewRow,
) -> ImportV2RouteResponse {
    let actual_row_version = latest_row.version;
    ImportV2RouteResponse {
        status_code: 409,
        body: json!({
            "success": false,
            "error": "Preview row changed, please refresh",
            "code": "PREVIEW_ROW_VERSION_CONFLICT",
            "data": {
                "expected_row_version": expected_row_version,
                "actual_row_version": actual_row_version,
                "previewItem": preview_row_to_value(latest_row),
            },
        }),
    }
}

fn preview_row_is_categorized(row: &&ImportPreviewRow) -> bool {
    !row.preview_main_category.trim().is_empty() || !row.preview_sub_category.trim().is_empty()
}

fn preview_row_has_account(row: &&ImportPreviewRow) -> bool {
    row.preview_source_account_id.is_some() || row.preview_destination_account_id.is_some()
}

#[cfg(test)]
mod preview_state_response_projection_tests {
    use super::*;

    #[test]
    fn canonical_preview_response_includes_the_typed_state_snapshot() {
        let contract: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/fixtures/import_preview_row_version_contract.json"
        )))
        .expect("row version contract fixture");
        let contract_row = &contract["current_server_row"];
        let value = preview_row_to_value(ImportPreviewRow {
            id: contract_row["id"].as_i64().expect("fixture preview id"),
            version: contract_row["row_version"]
                .as_i64()
                .expect("fixture row version"),
            session_id: "session-1".to_string(),
            user_id: 2,
            preview_date: "2026-08-16T00:00:00Z".to_string(),
            preview_type: String::new(),
            preview_amount_cents: 100,
            preview_destination_amount_cents: 0,
            category_id: None,
            preview_main_category: String::new(),
            preview_sub_category: String::new(),
            preview_source_account_id: None,
            preview_destination_account_id: None,
            preview_counterparty: String::new(),
            preview_payment_method: String::new(),
            preview_description: String::new(),
            preview_parser_id: "wechat".to_string(),
            preview_parser_tags: vec!["parser:wechat".to_string()],
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: true,
            dedup_type: String::new(),
            dedup_source_ids: Vec::new(),
            preview_matching_feedback: json!({}),
            created_at: "2026-08-16T00:00:00Z".to_string(),
        });

        assert_eq!(value["matching"]["parser"]["id"], json!("wechat"));
        assert_eq!(value["row_version"], contract_row["row_version"]);
        assert!(contract["legacy_server_row"].get("row_version").is_none());
        assert_eq!(value["preview_state"]["projection_version"], json!(1));
        assert_eq!(value["preview_state"]["signals"], json!(["parser"]));
    }
}
