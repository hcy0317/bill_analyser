// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use ocr_security::{
    content_type_from_headers, normalize_ocr_llm_max_tokens, truthy_form_value,
    validate_ocr_image_size,
};
#[path = "multipart_and_ocr/multipart_form.rs"]
mod multipart_form;
#[path = "multipart_and_ocr/network_llm_ocr.rs"]
mod multipart_network_llm_ocr;
#[path = "multipart_and_ocr/ocr_local_json.rs"]
mod multipart_ocr_local_json;
#[path = "multipart_and_ocr/ocr_tesseract.rs"]
mod multipart_ocr_tesseract;
#[path = "multipart_and_ocr/provider_auth_refresh.rs"]
mod multipart_provider_auth_refresh;
#[path = "multipart_and_ocr/provider_runtime.rs"]
mod multipart_ocr_provider_runtime;

use multipart_form::{decode_import_text, parse_multipart_form_data};
use multipart_ocr_provider_runtime::{run_ocr_provider, OcrProviderFailure};
use multipart_provider_auth_refresh::refresh_provider_auth_profile;

#[derive(Debug, Clone)]
struct MultipartPart {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    body: Vec<u8>,
}

#[derive(Debug, Default)]
struct MultipartForm {
    parts: Vec<MultipartPart>,
}

impl MultipartForm {
    fn text_value(&self, keys: &[&str]) -> Option<String> {
        keys.iter().find_map(|key| {
            self.parts
                .iter()
                .find(|part| part.name == *key && part.filename.is_none())
                .map(|part| decode_import_text(&part.body).trim().to_string())
                .filter(|value| !value.is_empty())
        })
    }

    fn into_file_parts(self) -> Vec<MultipartPart> {
        self.parts
            .into_iter()
            .filter(|part| part.filename.is_some() || part.name == "file" || part.name == "files")
            .filter(|part| !part.body.is_empty())
            .collect()
    }

    fn first_file_part_named(&self, keys: &[&str]) -> Option<&MultipartPart> {
        keys.iter().find_map(|key| {
            self.parts
                .iter()
                .find(|part| part.name == *key && !part.body.is_empty())
        })
    }
}

#[derive(Debug)]
struct OcrRecognitionInput {
    image_bytes: Vec<u8>,
    mime: String,
    cancelled: bool,
}

#[derive(Debug)]
struct ProviderAuthRefreshError;

/// 从请求头和 body 构造 OCR 输入，兼容 multipart 上传和原始二进制图片两种入口。
fn ocr_recognition_input_from_request(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<OcrRecognitionInput, ImportV2RouteResponse> {
    let content_type = content_type_from_headers(headers);
    if content_type
        .to_ascii_lowercase()
        .contains("multipart/form-data")
    {
        let form = parse_multipart_form_data(&content_type, body)?;
        let image = form
            .first_file_part_named(&["image", "file", "files"])
            .map(|part| {
                (
                    part.body.clone(),
                    part.content_type
                        .clone()
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                )
            })
            .unwrap_or_else(|| (Vec::new(), "application/octet-stream".to_string()));
        validate_ocr_image_size(&image.0)?;
        return Ok(OcrRecognitionInput {
            image_bytes: image.0,
            mime: image.1,
            cancelled: form
                .text_value(&["cancelled"])
                .is_some_and(|value| truthy_form_value(&value)),
        });
    }
    validate_ocr_image_size(body)?;
    Ok(OcrRecognitionInput {
        image_bytes: body.to_vec(),
        mime: if content_type.trim().is_empty() {
            "application/octet-stream".to_string()
        } else {
            content_type
        },
        cancelled: false,
    })
}

fn ocr_rate_limit_try_acquire(user_id: i64) -> bool {
    let buckets = OCR_RATE_LIMIT_BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut buckets) = buckets.lock() else {
        return false;
    };
    let now = Instant::now();
    let bucket = buckets.entry(user_id).or_default();
    let cutoff = now - StdDuration::from_secs(60);
    while bucket.front().is_some_and(|timestamp| *timestamp < cutoff) {
        bucket.pop_front();
    }
    if bucket.len() >= 10 {
        return false;
    }
    bucket.push_back(now);
    true
}

fn first_text_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    first_value(object, keys)
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// 解析 JSON 请求体，空 body 等价为空对象，错误时保持 import v2 400 响应。
fn json_body_or_empty(body: &[u8]) -> Result<Value, ImportV2RouteResponse> {
    if body.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice::<Value>(body)
        .map_err(|_| import_v2_error_response(400, "Invalid JSON request"))
}

fn text_from_map(object: &Map<String, Value>, key: &str) -> String {
    object.get(key).and_then(value_to_text).unwrap_or_default()
}

fn text_from_map_or(object: &Map<String, Value>, key: &str, default: &str) -> String {
    let text = text_from_map(object, key);
    if text.trim().is_empty() {
        default.to_string()
    } else {
        text
    }
}

/// 基于请求增量更新 runtime LLM 配置，并通过 copy_runtime_llm_config 统一规范字段。
#[tracing::instrument(level = "debug", skip_all)]
fn update_runtime_llm_config_payload(base_config: &Value, object: &Map<String, Value>) -> Value {
    let mut config = copy_runtime_llm_config(base_config)
        .as_object()
        .cloned()
        .unwrap_or_default();
    if let Some(enabled) = object.get("enabled").and_then(Value::as_bool) {
        config.insert("enabled".to_string(), json!(enabled));
    }
    if let Some(provider) = object.get("provider").and_then(Value::as_str) {
        config.insert("provider".to_string(), json!(provider));
    }
    if let Some(provider_config) = object.get("provider_config").and_then(Value::as_object) {
        config.insert(
            "provider_config".to_string(),
            Value::Object(provider_config.clone()),
        );
    }
    if let Some(advanced_settings) = object.get("advanced_settings") {
        config.insert("advanced_settings".to_string(), advanced_settings.clone());
    }
    copy_runtime_llm_config(&Value::Object(config))
}

/// 构建 runtime LLM config 响应数据，只返回前端需要字段并可选带 provider 列表。
fn llm_runtime_config_response_data(config: &Value, include_available_providers: bool) -> Value {
    let copied = copy_runtime_llm_config(config);
    let object = copied.as_object();
    let provider_config = object
        .and_then(|item| item.get("provider_config"))
        .and_then(Value::as_object);
    let mut response = Map::new();
    response.insert(
        "enabled".to_string(),
        json!(object
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false)),
    );
    response.insert(
        "provider".to_string(),
        json!(object
            .and_then(|item| item.get("provider"))
            .and_then(Value::as_str)
            .unwrap_or("openai")),
    );
    response.insert(
        "model".to_string(),
        json!(provider_config
            .and_then(|item| item.get("model"))
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()),
    );
    response.insert(
        "advanced_settings".to_string(),
        copied
            .get("advanced_settings")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    if include_available_providers {
        response.insert(
            "credential_config".to_string(),
            copied
                .get("credential_config")
                .map(redact_provider_auth_config)
                .unwrap_or_else(|| json!({})),
        );
        response.insert(
            "available_providers".to_string(),
            json!(llm_available_providers()),
        );
    }
    Value::Object(response)
}

/// 将前端保存配置 payload 转为数据库 update DTO，保留脱敏占位符代表“不修改密钥”语义。
fn llm_config_update_from_map(object: &Map<String, Value>) -> LlmConfigUpdate {
    let api_key = object
        .get("api_key")
        .and_then(value_to_text)
        .filter(|value| value != "********");
    LlmConfigUpdate {
        name: object.get("name").and_then(value_to_text),
        provider: object.get("provider").and_then(value_to_text),
        model: object.get("model").and_then(value_to_text),
        api_key,
        base_url: object.get("base_url").and_then(value_to_text),
        credential_config: first_value(
            object,
            &["credential_config", "credentialConfig", "auth_profile", "authProfile"],
        )
        .cloned(),
        advanced_settings: object.get("advanced_settings").cloned(),
        is_active: object.get("is_active").and_then(Value::as_bool),
    }
}

#[tracing::instrument(level = "debug", skip_all)]

fn llm_not_found_response(message: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 404,
        body: json!({"success": false, "error": message}),
    }
}
