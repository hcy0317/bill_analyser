#[tracing::instrument(level = "debug", skip_all)]
fn generate_import_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let counter = IMPORT_SESSION_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("rust-import-{nanos}-{counter}")
}

fn usize_to_i64(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn preview_update_items_from_payload(
    payload: &Value,
) -> Result<Vec<&Map<String, Value>>, ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let Some(updates) = first_value(object, &["preview_updates", "previewUpdates"]) else {
        return Ok(Vec::new());
    };
    let updates = updates
        .as_array()
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))?;
    let mut items = Vec::with_capacity(updates.len());
    for item in updates {
        items.push(payload_object(item)?);
    }
    Ok(items)
}

fn limited_preview_update_items_from_payload(
    payload: &Value,
    limit: usize,
) -> Result<Vec<&Map<String, Value>>, ImportV2RouteResponse> {
    let items = preview_update_items_from_payload(payload)?;
    if items.len() > limit {
        return Err(preview_selection_too_large_response());
    }
    Ok(items)
}

fn preview_id_from_payload(object: &Map<String, Value>) -> Result<i64, ImportV2RouteResponse> {
    first_value(object, &["id", "preview_id", "previewId"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
        .ok_or_else(|| import_v2_error_response(400, "Missing bill id"))
}

fn expected_row_version_from_payload(
    object: &Map<String, Value>,
) -> Result<i64, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &[
            "expected_row_version",
            "expectedRowVersion",
            "row_version",
            "rowVersion",
        ],
    ) else {
        return Err(import_version_required_response("row_version"));
    };
    if value.is_null() {
        return Err(import_version_required_response("row_version"));
    }
    value_to_i64(value)
        .filter(|version| *version > 0)
        .ok_or_else(|| import_v2_error_response(400, "Invalid expected_row_version"))
}
