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
        advanced_settings: object.get("advanced_settings").cloned(),
        is_active: object.get("is_active").and_then(Value::as_bool),
    }
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

