fn bool_field_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    first_value(object, keys).and_then(|value| match value {
        Value::Bool(value) => Some(*value),
        Value::Number(number) => number.as_i64().map(|value| value != 0),
        Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "y" => Some(true),
            "false" | "0" | "no" | "n" => Some(false),
            _ => None,
        },
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    })
}

fn id_list_field_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<Vec<i64>> {
    first_value(object, keys).and_then(|value| {
        let ids = value
            .as_array()?
            .iter()
            .filter_map(value_to_i64)
            .filter(|id| *id > 0)
            .collect::<Vec<_>>();
        Some(ids)
    })
}

fn limited_id_list_field_from_object(
    object: &Map<String, Value>,
    keys: &[&str],
    limit: usize,
) -> Result<Option<Vec<i64>>, ImportV2RouteResponse> {
    let Some(value) = first_value(object, keys) else {
        return Ok(None);
    };
    let Some(values) = value.as_array() else {
        return Err(llm_contract_error_response(
            "ID list fields must be arrays",
            "INVALID_REQUEST",
            400,
        ));
    };
    let mut seen_ids = BTreeSet::new();
    let mut ids = Vec::new();
    for value in values {
        let Some(id) = value_to_i64(value) else {
            return Err(llm_contract_error_response(
                "ID list fields must contain integer IDs",
                "INVALID_REQUEST",
                400,
            ));
        };
        if id <= 0 || !seen_ids.insert(id) {
            continue;
        }
        ids.push(id);
    }
    if ids.len() > limit {
        return Err(preview_selection_too_large_response());
    }
    Ok(Some(ids))
}

fn preview_selection_too_large_response() -> ImportV2RouteResponse {
    llm_contract_error_response(
        "Selected preview rows exceed the maximum batch size",
        "PREVIEW_SELECTION_TOO_LARGE",
        422,
    )
}
