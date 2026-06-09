use super::{import_v2_error_response, ImportV2RouteResponse};
use axum::http::{self, HeaderMap};
use serde_json::Value;

const OCR_IMAGE_MAX_BYTES: usize = 10 * 1024 * 1024;
const OCR_LLM_MAX_TOKENS_CAP: i64 = 4_000;

pub(super) fn content_type_from_headers(headers: &HeaderMap) -> String {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

pub(super) fn validate_ocr_image_size(image_bytes: &[u8]) -> Result<(), ImportV2RouteResponse> {
    if image_bytes.len() > OCR_IMAGE_MAX_BYTES {
        return Err(import_v2_error_response(
            413,
            "OCR image exceeds maximum size",
        ));
    }
    Ok(())
}

pub(super) fn normalize_ocr_llm_max_tokens(parameters: &Value) -> i64 {
    parameters
        .get("max_tokens")
        .and_then(Value::as_i64)
        .unwrap_or(1200)
        .clamp(1, OCR_LLM_MAX_TOKENS_CAP)
}

pub(super) fn truthy_form_value(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
