use super::*;

pub(super) fn optional_json_body(body: &Bytes) -> Result<Value, String> {
    if body.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let payload: Value =
        serde_json::from_slice(body).map_err(|_| "invalid JSON body".to_string())?;
    if payload.is_object() {
        Ok(payload)
    } else {
        Err("JSON body must be an object".to_string())
    }
}

pub(super) fn backup_sync_config_payload(payload: &Value) -> Value {
    if let Some(config) = payload.get("config").filter(|value| value.is_object()) {
        return config.clone();
    }
    let mut config = payload.as_object().cloned().unwrap_or_default();
    for key in [
        "stepUpToken",
        "step_up_token",
        "currentPassword",
        "current_password",
    ] {
        config.remove(key);
    }
    Value::Object(config)
}
