fn response_mode_is_preview_item(object: &Map<String, Value>) -> bool {
    first_value(object, &["responseMode", "response_mode"]).is_some_and(|value| {
        value
            .as_str()
            .is_some_and(|text| text.eq_ignore_ascii_case("preview-item"))
    })
}

fn preview_row_to_value(row: ImportPreviewRow) -> Value {
    let mut value = serde_json::to_value(row).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        attach_import_preview_matching_payload(object);
    }
    value
}

fn preview_row_is_categorized(row: &&ImportPreviewRow) -> bool {
    !row.preview_main_category.trim().is_empty() || !row.preview_sub_category.trim().is_empty()
}

fn preview_row_has_account(row: &&ImportPreviewRow) -> bool {
    row.preview_source_account_id.is_some() || row.preview_destination_account_id.is_some()
}
