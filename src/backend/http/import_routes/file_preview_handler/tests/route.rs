use super::super::super::*;
use super::helpers::*;
use axum::{body::to_bytes, http::StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn preview_route_rejects_media_form_and_file_selection_errors() {
    let state = HttpAppState::new(
        HttpShellConfig::default().with_trusted_user_header_secret(PREVIEW_TEST_SECRET),
    )
    .unwrap();
    let router = import_runtime_router().with_state(state);

    let response = router
        .clone()
        .oneshot(preview_request("application/json", b"{}".to_vec()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);

    let response = router
        .clone()
        .oneshot(preview_request("multipart/form-data", b"broken".to_vec()))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let boundary = "empty-preview-form";
    let response = router
        .oneshot(preview_request(
            &format!("multipart/form-data; boundary={boundary}"),
            format!("--{boundary}--\r\n").into_bytes(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn direct_preview_preserves_envelope_and_status_contract() {
    let response = preview_file_request(
        "generic.csv",
        b"date,amount\n2026-01-15,-12.50\n",
        1024 * 1024,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["success"], true);
    assert_eq!(payload["result"]["headers"], json!(["date", "amount"]));
    assert_eq!(payload["result"]["totalRows"], 1);

    let response = preview_file_request("oversized.csv", &vec![b'x'; 1025], 1024).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let response = preview_file_request("spoofed.xlsx", b"not-an-xlsx", 1024 * 1024).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let response = preview_file_request("payload.exe", b"x", 1024 * 1024).await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let wide_html = format!("<table><tr>{}</tr></table>", "<td>x</td>".repeat(129));
    let response = preview_file_request("wide.xls", wide_html.as_bytes(), 1024 * 1024).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let response = preview_file_request(
        "legacy.xls",
        b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1legacy",
        1024 * 1024,
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}
