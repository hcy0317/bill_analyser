// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

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

    fn file_parts(&self) -> Vec<&MultipartPart> {
        self.parts
            .iter()
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

#[derive(Debug, Default)]
struct ParsedCsvBills {
    bills: Vec<StandardBill>,
}

#[derive(Debug, Default)]
struct ImportPreviewRequest {
    temp_path: Option<String>,
    uploaded_file: Option<Vec<u8>>,
    delimiter: Option<String>,
}

#[derive(Debug)]
struct OcrRecognitionInput {
    image_bytes: Vec<u8>,
    mime: String,
    cancelled: bool,
}

#[derive(Debug)]
struct LegacyImportParseResult {
    items: Vec<Value>,
    total_count: usize,
    parser_type: String,
    detected_parser_type: String,
}

#[derive(Debug)]
struct ProviderAuthRefreshError;

fn content_type_from_headers(headers: &HeaderMap) -> String {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

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
        return Ok(OcrRecognitionInput {
            image_bytes: image.0,
            mime: image.1,
            cancelled: form
                .text_value(&["cancelled"])
                .is_some_and(|value| truthy_form_value(&value)),
        });
    }
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

fn truthy_form_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
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

async fn run_ocr_provider(
    config: &OcrConfigContract,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    match config.provider.as_str() {
        "cloud_stub" => Err(build_ocr_error_response(
            "provider_unconfigured",
            Some("cloud_ocr_not_configured"),
        )),
        "tesseract" => run_tesseract_ocr(config.lang.clone(), image_bytes, mime).await,
        "local_json_ocr" => run_local_json_ocr(image_bytes, mime).await,
        NETWORK_OCR_PROVIDER_NAME => run_network_llm_ocr(config, image_bytes, mime).await,
        _ => Err(build_ocr_error_response(
            "provider_unconfigured",
            Some("ocr provider not configured"),
        )),
    }
}

async fn run_tesseract_ocr(
    lang: String,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    match tokio::task::spawn_blocking(move || run_tesseract_ocr_blocking(lang, image_bytes, mime))
        .await
    {
        Ok(result) => result,
        Err(error) => Err(build_ocr_error_response(
            "provider_unconfigured",
            Some(&format!("tesseract provider unavailable: {error}")),
        )),
    }
}

fn run_tesseract_ocr_blocking(
    lang: String,
    image_bytes: Vec<u8>,
    _mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    let program =
        env::var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND").unwrap_or_else(|_| "tesseract".into());
    let mut args = env::var("BILL_ANALYSER_RUST_OCR_TESSERACT_COMMAND_ARGS")
        .ok()
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    args.extend([
        "stdin".to_string(),
        "stdout".to_string(),
        "-l".to_string(),
        lang.clone(),
    ]);
    let timeout = env::var("BILL_ANALYSER_RUST_OCR_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(StdDuration::from_millis)
        .unwrap_or_else(|| StdDuration::from_secs(30));

    let mut child = Command::new(&program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            build_ocr_error_response(
                "provider_unconfigured",
                Some(&format!("tesseract provider unavailable: {error}")),
            )
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&image_bytes)
            .map_err(ocr_io_error_response)?;
    }

    let started_at = Instant::now();
    loop {
        match child.try_wait().map_err(ocr_io_error_response)? {
            Some(status) => {
                let output = child.wait_with_output().map_err(ocr_io_error_response)?;
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(build_ocr_error_response(
                        "provider_unconfigured",
                        Some(&format!(
                            "tesseract provider unavailable{}",
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!(": {stderr}")
                            }
                        )),
                    ));
                }
                let text = String::from_utf8(output.stdout).map_err(|error| {
                    build_ocr_error_response("parse_error", Some(&error.to_string()))
                })?;
                return Ok(OcrProviderTextResult {
                    text,
                    confidence: 0.0,
                    model: "tesseract".to_string(),
                    raw_provider_response: json!({
                        "engine": "tesseract",
                        "lang": lang,
                    }),
                    lines: Vec::new(),
                });
            }
            None if started_at.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(build_ocr_error_response(
                    "timeout",
                    Some("ocr provider timeout"),
                ));
            }
            None => std::thread::sleep(StdDuration::from_millis(5)),
        }
    }
}

fn ocr_io_error_response(error: io::Error) -> AiRouteResponse {
    if error.raw_os_error() == Some(299) {
        return build_ocr_error_response(
            "provider_unconfigured",
            Some("tesseract provider unavailable"),
        );
    }
    build_ocr_error_response(
        "provider_unconfigured",
        Some(&format!("tesseract provider unavailable: {error}")),
    )
}

async fn run_local_json_ocr(
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    match tokio::task::spawn_blocking(move || run_local_json_ocr_blocking(image_bytes, mime)).await
    {
        Ok(result) => result,
        Err(error) => Err(build_ocr_error_response(
            "provider_unconfigured",
            Some(&format!("local JSON OCR provider unavailable: {error}")),
        )),
    }
}

fn run_local_json_ocr_blocking(
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    let program = env::var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND").map_err(|_| {
        build_ocr_error_response(
            "provider_unconfigured",
            Some("local JSON OCR provider unavailable"),
        )
    })?;
    let input_mode = env::var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_INPUT_MODE")
        .unwrap_or_else(|_| "stdin".to_string())
        .trim()
        .to_ascii_lowercase();
    let mut args = env::var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND_ARGS")
        .ok()
        .map(|value| {
            value
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut temp_file = None;
    if input_mode == "file" {
        let mut file = tempfile::NamedTempFile::new().map_err(ocr_io_error_response)?;
        file.write_all(&image_bytes).map_err(ocr_io_error_response)?;
        let path = file.path().to_string_lossy().to_string();
        let mut replaced = false;
        for arg in &mut args {
            if arg.contains("{input}") {
                *arg = arg.replace("{input}", &path);
                replaced = true;
            }
            if arg.contains("{mime}") {
                *arg = arg.replace("{mime}", &mime);
            }
        }
        if !replaced {
            args.push(path);
        }
        temp_file = Some(file);
    } else {
        for arg in &mut args {
            if arg.contains("{mime}") {
                *arg = arg.replace("{mime}", &mime);
            }
        }
    }
    let timeout = env::var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_TIMEOUT_MS")
        .or_else(|_| env::var("BILL_ANALYSER_RUST_OCR_TIMEOUT_MS"))
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(StdDuration::from_millis)
        .unwrap_or_else(|| StdDuration::from_secs(30));

    let mut child = Command::new(&program)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            build_ocr_error_response(
                "provider_unconfigured",
                Some(&format!("local JSON OCR provider unavailable: {error}")),
            )
        })?;
    if input_mode != "file" {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&image_bytes)
                .map_err(ocr_local_json_io_error_response)?;
        }
    }

    let started_at = Instant::now();
    loop {
        match child.try_wait().map_err(ocr_local_json_io_error_response)? {
            Some(status) => {
                let output = child
                    .wait_with_output()
                    .map_err(ocr_local_json_io_error_response)?;
                drop(temp_file);
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(build_ocr_error_response(
                        "provider_unconfigured",
                        Some(&format!(
                            "local JSON OCR provider unavailable{}",
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!(": {stderr}")
                            }
                        )),
                    ));
                }
                let stdout = String::from_utf8(output.stdout).map_err(|error| {
                    build_ocr_error_response("parse_error", Some(&error.to_string()))
                })?;
                return parse_local_json_ocr_output(&stdout, &mime);
            }
            None if started_at.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                drop(temp_file);
                return Err(build_ocr_error_response(
                    "timeout",
                    Some("ocr provider timeout"),
                ));
            }
            None => std::thread::sleep(StdDuration::from_millis(5)),
        }
    }
}

fn ocr_local_json_io_error_response(error: io::Error) -> AiRouteResponse {
    build_ocr_error_response(
        "provider_unconfigured",
        Some(&format!("local JSON OCR provider unavailable: {error}")),
    )
}

fn parse_local_json_ocr_output(
    stdout: &str,
    mime: &str,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(build_ocr_error_response(
            "parse_error",
            Some("local JSON OCR provider returned empty output"),
        ));
    }
    let parsed = serde_json::from_str::<Value>(trimmed)
        .or_else(|_| parse_local_json_ocr_json_lines(trimmed))
        .map_err(|error| build_ocr_error_response("parse_error", Some(&error.to_string())))?;
    local_json_ocr_result_from_value(parsed, mime)
}

fn parse_local_json_ocr_json_lines(text: &str) -> serde_json::Result<Value> {
    let mut items = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        items.push(serde_json::from_str::<Value>(line)?);
    }
    Ok(Value::Array(items))
}

fn local_json_ocr_result_from_value(
    value: Value,
    mime: &str,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    let mut model = "local_json_ocr".to_string();
    let mut explicit_text = None;
    let mut explicit_confidence = None;
    let mut line_source = value.clone();
    if let Some(object) = value.as_object() {
        if let Some(text) = first_text_from_object(object, &["text", "full_text", "fullText"]) {
            explicit_text = Some(text);
        }
        if let Some(value) = first_value(object, &["confidence", "score"]) {
            explicit_confidence = value.as_f64();
        }
        if let Some(raw_model) = first_text_from_object(object, &["model", "engine"]) {
            model = raw_model;
        }
        if let Some(lines) = first_value(object, &["lines", "items", "results"]) {
            line_source = lines.clone();
        }
    }
    let lines = local_json_ocr_lines_from_value(&line_source)?;
    let text = explicit_text.unwrap_or_else(|| {
        lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    });
    let confidence = explicit_confidence
        .or_else(|| average_ocr_line_confidence(&lines))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    Ok(OcrProviderTextResult {
        text,
        confidence,
        model,
        raw_provider_response: json!({
            "engine": "local_json_ocr",
            "mime": mime,
            "response": value,
        }),
        lines,
    })
}

fn local_json_ocr_lines_from_value(value: &Value) -> Result<Vec<OcrProviderTextLine>, AiRouteResponse> {
    let values = match value {
        Value::Array(items) => items.clone(),
        Value::Object(_) | Value::String(_) => vec![value.clone()],
        _ => Vec::new(),
    };
    let mut lines = Vec::new();
    for value in values {
        if let Some(text) = value.as_str() {
            if !text.trim().is_empty() {
                lines.push(OcrProviderTextLine {
                    text: text.trim().to_string(),
                    confidence: None,
                    bbox: None,
                });
            }
            continue;
        }
        let Some(object) = value.as_object() else {
            continue;
        };
        let Some(text) = first_text_from_object(object, &["text", "value", "label"]) else {
            continue;
        };
        let confidence = first_value(object, &["confidence", "score"]).and_then(Value::as_f64);
        let bbox = first_value(object, &["bbox", "box", "points"]).cloned();
        lines.push(OcrProviderTextLine {
            text,
            confidence,
            bbox,
        });
    }
    if lines.is_empty() {
        return Err(build_ocr_error_response(
            "parse_error",
            Some("local JSON OCR provider returned no text lines"),
        ));
    }
    Ok(lines)
}

fn average_ocr_line_confidence(lines: &[OcrProviderTextLine]) -> Option<f64> {
    let scores = lines
        .iter()
        .filter_map(|line| line.confidence)
        .collect::<Vec<_>>();
    (!scores.is_empty()).then(|| scores.iter().sum::<f64>() / scores.len() as f64)
}

#[cfg(test)]
mod ocr_io_error_tests {
    use super::*;

    #[test]
    fn ocr_io_error_response_normalizes_windows_partial_copy() {
        let response = ocr_io_error_response(io::Error::from_raw_os_error(299));
        assert_eq!(response.status_code, 501);
        assert_eq!(response.body["errorCode"], "provider_unconfigured");
        assert_eq!(
            response.body["message"],
            "tesseract provider unavailable"
        );
    }
}

fn first_text_from_object(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    first_value(object, keys)
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

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
            "available_providers".to_string(),
            build_llm_config_get_response(config).body["data"]["available_providers"].clone(),
        );
    }
    Value::Object(response)
}

fn llm_config_update_from_map(object: &Map<String, Value>) -> LlmConfigUpdate {
    let api_key = object
        .get("api_key")
        .and_then(value_to_text)
        .and_then(|value| {
            if value == "********" {
                None
            } else {
                Some(value)
            }
        });
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

async fn refresh_provider_auth_profile(
    client: &reqwest::Client,
    credential_config: &Value,
    provider_base_url: &str,
) -> Result<Value, ProviderAuthRefreshError> {
    let token_endpoint = credential_config
        .get("token_endpoint")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(ProviderAuthRefreshError)?;
    validate_provider_token_endpoint(token_endpoint, provider_base_url)
        .map_err(|_| ProviderAuthRefreshError)?;

    let mut url = Url::parse(token_endpoint).map_err(|_| ProviderAuthRefreshError)?;
    if let Some(params) = credential_config
        .get("refresh_params")
        .and_then(Value::as_object)
        .or_else(|| {
            credential_config
                .get("request_params")
                .and_then(Value::as_object)
        })
    {
        for (key, value) in params {
            if let Some(value) = header_value_text(value) {
                url.query_pairs_mut().append_pair(key, &value);
            }
        }
    }
    let mut request = client.post(url);
    if let Some(headers) = credential_config
        .get("refresh_headers")
        .and_then(Value::as_object)
    {
        for (key, value) in headers {
            if let Some(value) = header_value_text(value) {
                request = request.header(key, value);
            }
        }
    }

    let mut body = credential_config
        .get("refresh_body")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if !body.contains_key("grant_type") {
        body.insert("grant_type".to_string(), json!("refresh_token"));
    }
    if !body.contains_key("refresh_token") {
        let refresh_token =
            provider_auth_refresh_token(credential_config).ok_or(ProviderAuthRefreshError)?;
        body.insert("refresh_token".to_string(), json!(refresh_token));
    }

    let response = request
        .header("content-type", "application/json")
        .body(Value::Object(body).to_string())
        .send()
        .await
        .map_err(|_| ProviderAuthRefreshError)?;
    if !response.status().is_success() {
        return Err(ProviderAuthRefreshError);
    }
    let raw_text = read_limited_llm_provider_body(response)
        .await
        .map_err(|_| ProviderAuthRefreshError)?;
    let response_json =
        serde_json::from_str::<Value>(&raw_text).map_err(|_| ProviderAuthRefreshError)?;
    merge_refreshed_provider_auth_profile(credential_config, &response_json)
        .ok_or(ProviderAuthRefreshError)
}

fn merge_refreshed_provider_auth_profile(current: &Value, response: &Value) -> Option<Value> {
    let mut merged = current.as_object().cloned().unwrap_or_default();
    let refreshed = normalize_provider_auth_config(Some(response));
    let refreshed_object = refreshed.as_object()?;
    for key in [
        "access_token",
        "refresh_token",
        "expires_at",
        "credential_mode",
        "credential_json",
    ] {
        if let Some(value) = refreshed_object.get(key) {
            merged.insert(key.to_string(), value.clone());
        }
    }
    if !merged.contains_key("refresh_token") {
        if let Some(refresh_token) = provider_auth_refresh_token(current) {
            merged.insert("refresh_token".to_string(), json!(refresh_token));
        }
    }
    if !merged.contains_key("token_endpoint") {
        if let Some(token_endpoint) = current.get("token_endpoint").cloned() {
            merged.insert("token_endpoint".to_string(), token_endpoint);
        }
    }
    let normalized = normalize_provider_auth_config(Some(&Value::Object(merged)));
    provider_auth_access_token(&normalized)?;
    Some(normalized)
}

fn validate_provider_token_endpoint(
    token_endpoint: &str,
    provider_base_url: &str,
) -> Result<(), String> {
    let parsed = parse_provider_runtime_url(token_endpoint)?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("token endpoint must not contain credentials".to_string());
    }
    if provider_url_is_same_origin(&parsed, provider_base_url)
        || provider_url_is_allowlisted(&parsed, "BILL_ANALYSER_LLM_TOKEN_URL_ALLOWLIST")
    {
        if parsed.scheme() == "https" || provider_url_is_local_http(&parsed) {
            return Ok(());
        }
        return Err("token endpoint must use https unless it is local".to_string());
    }
    Err("token endpoint is not allowed".to_string())
}

fn parse_provider_runtime_url(value: &str) -> Result<Url, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('\\') || trimmed.chars().any(char::is_control) {
        return Err("provider url is not allowed".to_string());
    }
    let parsed = Url::parse(trimmed).map_err(|_| "provider url must be valid".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err("provider url must use http or https with a host".to_string());
    }
    Ok(parsed)
}

fn provider_url_is_same_origin(parsed: &Url, other_url: &str) -> bool {
    Url::parse(other_url)
        .map(|other| {
            parsed.scheme() == other.scheme()
                && parsed.host_str().map(str::to_ascii_lowercase)
                    == other.host_str().map(str::to_ascii_lowercase)
                && parsed.port_or_known_default() == other.port_or_known_default()
        })
        .unwrap_or(false)
}

fn provider_url_is_allowlisted(parsed: &Url, env_key: &str) -> bool {
    let origin = provider_url_origin(parsed);
    let full = parsed.as_str().trim_end_matches('/').to_ascii_lowercase();
    env::var(env_key)
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.trim_end_matches('/').to_ascii_lowercase())
        .any(|entry| entry == full || entry == origin)
}

fn provider_url_is_local_http(parsed: &Url) -> bool {
    parsed.scheme() == "http"
        && parsed.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host.ends_with(".localhost")
                || host == "127.0.0.1"
                || host == "::1"
        })
}

fn provider_url_origin(parsed: &Url) -> String {
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    match parsed.port() {
        Some(port) => format!("{}://{}:{port}", parsed.scheme(), host),
        None => format!("{}://{}", parsed.scheme(), host),
    }
}

fn header_value_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.trim().to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

async fn run_network_llm_ocr(
    config: &OcrConfigContract,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, AiRouteResponse> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(60))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| build_ocr_error_response("provider_unconfigured", Some("OCR provider unavailable")))?;
    let mut credential_config = config.credential_config.clone();
    if provider_auth_is_expired(&credential_config, Utc::now()) {
        credential_config = refresh_provider_auth_profile(&client, &credential_config, &config.base_url)
            .await
            .map_err(|_| {
                build_ocr_error_response(
                    "provider_relogin_required",
                    Some("OCR provider authorization expired; sign in again or refresh credentials"),
                )
            })?;
    }
    let Some(access_token) = provider_auth_access_token(&credential_config) else {
        return Err(build_ocr_error_response(
            "provider_relogin_required",
            Some("OCR provider authorization is missing; sign in again or refresh credentials"),
        ));
    };
    let base_url = config.base_url.trim_end_matches('/');
    if base_url.is_empty() {
        return Err(build_ocr_error_response(
            "provider_unconfigured",
            Some("OCR provider base URL is required"),
        ));
    }
    let url = format!("{base_url}/chat/completions");
    validate_provider_token_endpoint(&url, base_url).map_err(|_| {
        build_ocr_error_response("provider_unconfigured", Some("OCR provider base URL is not allowed"))
    })?;
    let encoded = general_purpose::STANDARD.encode(&image_bytes);
    let model = if config.model.trim().is_empty() {
        "gpt-4o-mini".to_string()
    } else {
        config.model.clone()
    };
    let payload = json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                {"type": "text", "text": "Extract all visible receipt or payment screenshot text. Return plain text only."},
                {"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{encoded}")}}
            ]
        }],
        "temperature": config.parameters.get("temperature").and_then(Value::as_f64).unwrap_or(0.0),
        "max_tokens": config.parameters.get("max_tokens").and_then(Value::as_i64).unwrap_or(1200),
    });
    let response = client
        .post(&url)
        .header("authorization", format!("Bearer {access_token}"))
        .header("content-type", "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|_| build_ocr_error_response("provider_unconfigured", Some("OCR provider request failed")))?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(build_ocr_error_response(
            "provider_relogin_required",
            Some("OCR provider authorization expired; sign in again or refresh credentials"),
        ));
    }
    if !response.status().is_success() {
        return Err(build_ocr_error_response(
            "provider_unconfigured",
            Some("OCR provider returned an error"),
        ));
    }
    let raw_text = read_limited_llm_provider_body(response)
        .await
        .map_err(|_| build_ocr_error_response("parse_error", Some("OCR provider returned invalid response")))?;
    let raw_response = serde_json::from_str::<Value>(&raw_text)
        .map_err(|_| build_ocr_error_response("parse_error", Some("OCR provider returned invalid JSON")))?;
    let text = raw_response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(build_ocr_error_response(
            "parse_error",
            Some("OCR provider returned empty text"),
        ));
    }
    Ok(OcrProviderTextResult {
        text,
        confidence: 0.8,
        model,
        raw_provider_response: json!({
            "engine": NETWORK_OCR_PROVIDER_NAME,
            "mime": mime,
            "provider": raw_response,
        }),
        lines: Vec::new(),
    })
}

fn llm_not_found_response(message: &str) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 404,
        body: json!({"success": false, "error": message}),
    }
}

fn parse_multipart_form_data(
    content_type: &str,
    body: &[u8],
) -> Result<MultipartForm, ImportV2RouteResponse> {
    let boundary = multipart_boundary_from_content_type(content_type)
        .ok_or_else(|| import_v2_error_response(400, "Missing multipart boundary"))?;
    let marker = format!("--{boundary}");
    let mut form = MultipartForm::default();
    for raw_part in split_bytes(body, marker.as_bytes()) {
        let raw_part = trim_part_boundary(raw_part);
        if raw_part.is_empty() || raw_part == b"--" {
            continue;
        }
        let Some((headers, part_body)) =
            split_once_bytes(raw_part, b"\r\n\r\n").or_else(|| split_once_bytes(raw_part, b"\n\n"))
        else {
            continue;
        };
        let headers_text = String::from_utf8_lossy(headers);
        let Some(disposition) = headers_text.lines().find(|line| {
            line.to_ascii_lowercase()
                .starts_with("content-disposition:")
        }) else {
            continue;
        };
        let Some(name) = disposition_param(disposition, "name") else {
            continue;
        };
        let filename = disposition_param(disposition, "filename")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let content_type = headers_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.trim().eq_ignore_ascii_case("content-type") {
                    Some(value.trim().to_string())
                } else {
                    None
                }
            })
            .filter(|value| !value.is_empty());
        form.parts.push(MultipartPart {
            name,
            filename,
            content_type,
            body: strip_trailing_newline(part_body).to_vec(),
        });
    }
    Ok(form)
}

fn multipart_boundary_from_content_type(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let part = part.trim();
        let value = part.strip_prefix("boundary=")?;
        Some(value.trim_matches('"').to_string())
    })
}

fn disposition_param(disposition: &str, param: &str) -> Option<String> {
    let prefix = format!("{param}=");
    disposition.split(';').find_map(|part| {
        let part = part.trim();
        let raw_value = part.strip_prefix(&prefix)?;
        Some(raw_value.trim_matches('"').to_string())
    })
}

fn split_bytes<'a>(body: &'a [u8], marker: &[u8]) -> Vec<&'a [u8]> {
    if marker.is_empty() {
        return vec![body];
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while let Some(offset) = find_bytes(&body[start..], marker) {
        parts.push(&body[start..start + offset]);
        start += offset + marker.len();
    }
    parts.push(&body[start..]);
    parts
}

fn split_once_bytes<'a>(body: &'a [u8], marker: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let offset = find_bytes(body, marker)?;
    Some((&body[..offset], &body[offset + marker.len()..]))
}

fn find_bytes(body: &[u8], marker: &[u8]) -> Option<usize> {
    if marker.is_empty() || marker.len() > body.len() {
        return None;
    }
    body.windows(marker.len())
        .position(|window| window == marker)
}

fn trim_part_boundary(mut part: &[u8]) -> &[u8] {
    while part.starts_with(b"\r\n") {
        part = &part[2..];
    }
    while part.starts_with(b"\n") {
        part = &part[1..];
    }
    part
}

fn strip_trailing_newline(mut body: &[u8]) -> &[u8] {
    if body.ends_with(b"\r\n") {
        body = &body[..body.len().saturating_sub(2)];
    } else if body.ends_with(b"\n") {
        body = &body[..body.len().saturating_sub(1)];
    }
    body
}

fn decode_import_text(bytes: &[u8]) -> String {
    let decoded = if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_string()
    } else {
        let (text, _, _) = GBK.decode(bytes);
        text.into_owned()
    };
    decoded.trim_start_matches('\u{feff}').to_string()
}

