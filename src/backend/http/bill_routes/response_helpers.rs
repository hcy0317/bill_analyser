// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。
fn open_postgres_runtime(
    state: &HttpAppState,
    runtime_label: &'static str,
) -> RouteResult<bill_analyser_db::PostgresRepositoryRuntime> {
    state
        .open_postgres_repository_runtime(runtime_label)
        .map_err(|error| {
            Box::new(error_response(
                status_or_internal(error.http_status_code()),
                error.public_message(),
            ))
        })
}

fn open_ledger_queries(
    state: &HttpAppState,
) -> RouteResult<bill_analyser_db::PostgresLedgerQueries> {
    state.ledger_queries().map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.http_status_code()),
            error.public_message(),
        ))
    })
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn response_error_text(response: Response) -> Option<String> {
    let status = response.status();
    if status == StatusCode::BAD_REQUEST {
        Some("Invalid bill payload".to_string())
    } else {
        None
    }
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn error_result(status: StatusCode, message: impl ToString, result: Value) -> Response {
    json_response(
        status,
        json!({
            "success": false,
            "error": message.to_string(),
            "result": result,
        }),
    )
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn not_found(message: impl ToString) -> Response {
    error_response(StatusCode::NOT_FOUND, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BillInternalErrorKind {
    Database,
    Picture,
    Reconciliation,
}

impl BillInternalErrorKind {
    fn label(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Picture => "picture",
            Self::Reconciliation => "reconciliation",
        }
    }

    fn public_message(self) -> &'static str {
        match self {
            Self::Database => "Rust bills route runtime DB error",
            Self::Picture => "Rust bills picture runtime error",
            Self::Reconciliation => "Rust bills reconciliation runtime error",
        }
    }
}

fn bill_internal_error_public_message(
    kind: BillInternalErrorKind,
    operation: &'static str,
    error: impl std::fmt::Display,
) -> &'static str {
    tracing::error!(
        domain = "bills",
        operation,
        failure_kind = kind.label(),
        error = %error,
        "bill route internal failure"
    );
    kind.public_message()
}

fn bill_internal_error_response(
    kind: BillInternalErrorKind,
    operation: &'static str,
    error: impl std::fmt::Display,
) -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        bill_internal_error_public_message(kind, operation, error),
    )
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        BillInternalErrorKind::Database.public_message(),
    )
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod response_helper_tests {
    use super::*;
    use axum::body::to_bytes;

    async fn response_value(response: Response) -> (StatusCode, Value) {
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body bytes");
        (
            status,
            serde_json::from_slice(&body).expect("JSON response body"),
        )
    }

    #[test]
    fn response_helpers_cover_error_text_and_status_mapping() {
        assert_eq!(
            response_error_text(bad_request("invalid")),
            Some("Invalid bill payload".to_string())
        );
        assert_eq!(response_error_text(not_found("missing")), None);
        assert_eq!(
            db_error_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            status_or_internal(0),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn internal_error_kinds_have_closed_log_labels_and_public_messages() {
        for (kind, label, public_message) in [
            (
                BillInternalErrorKind::Database,
                "database",
                "Rust bills route runtime DB error",
            ),
            (
                BillInternalErrorKind::Picture,
                "picture",
                "Rust bills picture runtime error",
            ),
            (
                BillInternalErrorKind::Reconciliation,
                "reconciliation",
                "Rust bills reconciliation runtime error",
            ),
        ] {
            assert_eq!(kind.label(), label);
            assert_eq!(kind.public_message(), public_message);
        }
    }

    #[tokio::test]
    async fn success_and_error_helpers_pin_bill_route_envelopes() {
        let (status, success) =
            response_value(success_result(StatusCode::OK, json!({"items": [1]}))).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(success, json!({"success": true, "result": {"items": [1]}}));

        let (status, error) = response_value(error_response(StatusCode::CONFLICT, "duplicate")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(error, json!({"success": false, "error": "duplicate"}));

        let (status, error_with_result) = response_value(error_result(
            StatusCode::BAD_REQUEST,
            "invalid item",
            json!({"failedIndex": 2}),
        ))
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error_with_result,
            json!({
                "success": false,
                "error": "invalid item",
                "result": {"failedIndex": 2}
            })
        );
    }
}
