fn first_value<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn push_text_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_text) {
        changes.push((field, ImportPreviewPatchValue::Text(value)));
    }
}

fn push_preview_type_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_preview_type_text) {
        changes.push((
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(value),
        ));
    }
}

fn push_real_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_f64) {
        changes.push((field, ImportPreviewPatchValue::Real(value)));
    }
}

fn push_i64_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_i64) {
        changes.push((field, ImportPreviewPatchValue::Integer(value)));
    }
}

fn push_minor_units_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys).and_then(value_to_minor_units) {
        changes.push((field, ImportPreviewPatchValue::Integer(value)));
    }
}

fn push_nullable_i64_change(
    changes: &mut Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)>,
    object: &Map<String, Value>,
    keys: &[&str],
    field: ImportPreviewPatchField,
) {
    if let Some(value) = first_value(object, keys) {
        let patch_value = match value_to_i64(value) {
            Some(value) if value > 0 => ImportPreviewPatchValue::Integer(value),
            _ => ImportPreviewPatchValue::Null,
        };
        changes.push((field, patch_value));
    }
}

fn value_to_preview_type_text(value: &Value) -> Option<String> {
    if let Some(label) = value_to_i64(value).and_then(transaction_type_label_from_i64) {
        return Some(label.to_string());
    }
    let text = value_to_text(value)?;
    normalize_transaction_type_text(&text)
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_transaction_type_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = match trimmed.to_ascii_lowercase().as_str() {
        "expense" => "支出",
        "income" => "收入",
        "transfer" => "转账",
        "investment" => "投资",
        _ => trimmed,
    };
    Some(normalized.to_string())
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Bool(_) => None,
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_minor_units(value: &Value) -> Option<i64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<i64>().ok()
            }
        }
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Null => None,
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                text.parse::<f64>().ok()
            }
        }
        Value::Bool(value) => Some(f64::from(u8::from(*value))),
        Value::Array(_) | Value::Object(_) => None,
    }
}
