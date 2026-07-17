use super::super::super::*;
use axum::{
    body::Body,
    http::{header, Method, Request},
};
use std::time::Duration;
use tower::ServiceExt;

pub(super) const PREVIEW_TEST_SECRET: &str = "import-preview-test-secret";

pub(super) fn preview_request(content_type: &str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri("/api/bills/import/preview")
        .header(header::CONTENT_TYPE, content_type)
        .header("x-bill-analyser-trusted-user-secret", PREVIEW_TEST_SECRET)
        .header("x-user-id", "9201")
        .body(Body::from(body))
        .expect("preview request")
}

pub(super) async fn preview_file_request(
    filename: &str,
    bytes: &[u8],
    body_limit_bytes: usize,
) -> Response {
    let boundary = "bill-analyser-preview-file-test";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let config = HttpShellConfig::new("", Duration::from_secs(1), body_limit_bytes)
        .expect("preview test HTTP config")
        .with_trusted_user_header_secret(PREVIEW_TEST_SECRET);
    let state = HttpAppState::new(config).expect("preview test state");
    import_runtime_router()
        .with_state(state)
        .oneshot(preview_request(
            &format!("multipart/form-data; boundary={boundary}"),
            body,
        ))
        .await
        .expect("preview file response")
}

pub(super) async fn preview_temp_request(
    state: HttpAppState,
    temp_path: &str,
    session_id: Option<&str>,
    user_id: i64,
) -> Response {
    let boundary = "bill-analyser-preview-temp-test";
    let mut fields = vec![("temp_path", temp_path)];
    if let Some(session_id) = session_id {
        fields.push(("session_id", session_id));
    }
    let body = multipart_text_body(boundary, &fields);
    import_runtime_router()
        .with_state(state)
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/import/preview")
                .header(
                    header::CONTENT_TYPE,
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .header("x-bill-analyser-trusted-user-secret", PREVIEW_TEST_SECRET)
                .header("x-user-id", user_id.to_string())
                .body(Body::from(body))
                .expect("preview temp request"),
        )
        .await
        .expect("preview temp response")
}

fn multipart_text_body(boundary: &str, fields: &[(&str, &str)]) -> Vec<u8> {
    let mut body = String::new();
    for (name, value) in fields {
        body.push_str(&format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
        ));
    }
    body.push_str(&format!("--{boundary}--\r\n"));
    body.into_bytes()
}

pub(super) fn cleanup_temp_fixture(raw_path: &str) {
    let path = temp_import_root().join(raw_path);
    let session_dir = path.parent().map(FsPath::to_path_buf);
    let user_dir = session_dir
        .as_deref()
        .and_then(FsPath::parent)
        .map(FsPath::to_path_buf);
    let _ = fs::remove_file(path);
    if let Some(session_dir) = session_dir {
        let _ = fs::remove_dir(session_dir);
    }
    if let Some(user_dir) = user_dir {
        let _ = fs::remove_dir(user_dir);
    }
}
