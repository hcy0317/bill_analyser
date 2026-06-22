use super::*;

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn run_tesseract_ocr(
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

pub(super) fn ocr_io_error_response(error: io::Error) -> AiRouteResponse {
    if matches!(error.kind(), io::ErrorKind::BrokenPipe) || error.raw_os_error() == Some(299) {
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
