use super::multipart_ocr_provider_runtime::OcrProviderFailure;
use bill_analyser_core::OcrProviderTextResult;
use serde_json::json;
use std::{
    env, io,
    io::Write,
    process::{Command, Stdio},
    time::{Duration as StdDuration, Instant},
};

/// 运行 Tesseract OCR 的异步入口，通过 blocking 线程隔离外部命令执行。
#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn run_tesseract_ocr(
    lang: String,
    image_bytes: Vec<u8>,
    mime: String,
) -> Result<OcrProviderTextResult, OcrProviderFailure> {
    match tokio::task::spawn_blocking(move || run_tesseract_ocr_blocking(lang, image_bytes, mime))
        .await
    {
        Ok(result) => result,
        Err(error) => Err(OcrProviderFailure::unavailable(format!(
            "tesseract provider unavailable: {error}"
        ))),
    }
}

/// 调用 Tesseract CLI 并写入图片字节到 stdin，负责超时、stderr 和 UTF-8 输出处理。
fn run_tesseract_ocr_blocking(
    lang: String,
    image_bytes: Vec<u8>,
    _mime: String,
) -> Result<OcrProviderTextResult, OcrProviderFailure> {
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
            OcrProviderFailure::unavailable(format!("tesseract provider unavailable: {error}"))
        })?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(&image_bytes)
            .map_err(ocr_io_failure)?;
    }

    let started_at = Instant::now();
    loop {
        match child.try_wait().map_err(ocr_io_failure)? {
            Some(status) => {
                let output = child.wait_with_output().map_err(ocr_io_failure)?;
                if !status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(OcrProviderFailure::unavailable(format!(
                            "tesseract provider unavailable{}",
                            if stderr.is_empty() {
                                String::new()
                            } else {
                                format!(": {stderr}")
                            }
                        )));
                }
                let text = String::from_utf8(output.stdout)
                    .map_err(|error| OcrProviderFailure::invalid_output(error.to_string()))?;
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
                return Err(OcrProviderFailure::timed_out("ocr provider timeout"));
            }
            None => std::thread::sleep(StdDuration::from_millis(5)),
        }
    }
}

/// 将 Tesseract IO 错误归一为 typed provider failure，兼容 Windows partial copy 错误。
fn ocr_io_failure(error: io::Error) -> OcrProviderFailure {
    if matches!(error.kind(), io::ErrorKind::BrokenPipe) || error.raw_os_error() == Some(299) {
        return OcrProviderFailure::unavailable("tesseract provider unavailable");
    }
    OcrProviderFailure::unavailable(format!("tesseract provider unavailable: {error}"))
}

#[cfg(test)]
mod ocr_io_error_tests {
    use super::*;

    #[test]
    fn ocr_io_failure_normalizes_windows_partial_copy() {
        let failure = ocr_io_failure(io::Error::from_raw_os_error(299));
        assert_eq!(
            failure,
            OcrProviderFailure::Unavailable {
                message: "tesseract provider unavailable".to_string()
            }
        );
    }

    #[test]
    fn ocr_io_failure_preserves_other_error_context() {
        let failure = ocr_io_failure(io::Error::other("disk"));
        assert_eq!(
            failure,
            OcrProviderFailure::Unavailable {
                message: "tesseract provider unavailable: disk".to_string()
            }
        );
    }
}
