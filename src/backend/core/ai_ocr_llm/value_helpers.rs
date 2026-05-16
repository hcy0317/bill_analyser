use serde_json::{Map, Value};

pub(super) fn first_non_empty_field(
    object: Option<&Map<String, Value>>,
    key: &str,
) -> Option<String> {
    object
        .and_then(|item| item.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn string_field_or(
    object: Option<&Map<String, Value>>,
    key: &str,
    default: &str,
) -> String {
    first_non_empty_field(object, key).unwrap_or_else(|| default.to_string())
}
