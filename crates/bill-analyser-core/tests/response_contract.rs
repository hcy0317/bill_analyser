use bill_analyser_core::{ApiError, ApiResponse, ErrorCode, RuntimeError};
use serde_json::json;

#[test]
fn success_response_serializes_runtime_shell_envelope_shape() {
    let response = ApiResponse::success(json!({
        "runtime": "rust",
        "business_migration": "none",
    }));

    let serialized = serde_json::to_value(&response).expect("response serializes");

    assert_eq!(
        serialized,
        json!({
            "success": true,
            "data": {
                "runtime": "rust",
                "business_migration": "none",
            }
        })
    );
}

#[test]
fn error_response_serializes_runtime_shell_code_and_message_without_data() {
    let error = RuntimeError::new(ErrorCode::InvalidInput, "runtime identity is required");
    let response: ApiResponse<()> = ApiResponse::failure(error);

    let serialized = serde_json::to_value(&response).expect("response serializes");

    assert_eq!(
        serialized,
        json!({
            "success": false,
            "error": {
                "code": "invalid_input",
                "message": "runtime identity is required",
            }
        })
    );
}

#[test]
fn api_error_can_be_constructed_without_exposing_internal_types() {
    let api_error = ApiError::new("runtime_unavailable", "Rust runtime is unavailable");

    assert_eq!(api_error.code(), "runtime_unavailable");
    assert_eq!(api_error.message(), "Rust runtime is unavailable");
}
