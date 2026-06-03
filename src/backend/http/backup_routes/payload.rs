// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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
