use super::*;

pub(super) async fn run_local_json_ocr(
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
