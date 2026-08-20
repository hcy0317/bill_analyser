// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn list_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsListQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "list_bills_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let queries = match open_ledger_queries(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let page = match queries.list(user_id, query.into_ledger_list_query()).await {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match ledger_page_to_frontend(page) {
        Ok(value) => json_response(StatusCode::OK, value),
        Err(_) => db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn bills_by_month_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsByMonthQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "bills_by_month_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let (start_date, end_date) =
        match bill_analyser_core::adapters::transaction::month_date_range(query.year, query.month) {
            Ok(value) => value,
            Err(error) => return bad_request(error.to_string()),
        };
    let mut ledger_query = query.common.into_ledger_list_query();
    ledger_query.date_from = Some(start_date);
    ledger_query.date_to = Some(end_date);
    let queries = match open_ledger_queries(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let page = match queries.list_month(user_id, ledger_query).await {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match ledger_page_to_frontend(page) {
        Ok(value) => json_response(StatusCode::OK, value),
        Err(_) => db_error_response(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn create_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "create_bill_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if let Err(response) = validate_bill_money_payload(&payload) {
        return *response;
    }

        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let draft = match create_draft_from_payload_postgres(runtime.pool(), user_id, &payload).await
        {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let bill_id = match create_postgres_bill(runtime.pool(), user_id.get() as i64, &draft).await
        {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        return match get_postgres_frontend_bill(runtime.pool(), user_id, bill_id).await {
            Ok(Some(value)) => success_result(StatusCode::CREATED, value),
            Ok(None) => db_error_response(),
            Err(_) => db_error_response(),
        };
}


#[tracing::instrument(level = "debug", skip_all)]
async fn get_bill_query_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillIdQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "get_bill_query_handler", "business operation entered");
    let Some(bill_id) = query.id.as_deref().and_then(parse_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    get_bill_response(state, headers, bill_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "get_bill_path_handler", "business operation entered");
    get_bill_response(state, headers, bill_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn get_bill_response(state: HttpAppState, headers: HeaderMap, bill_id: i64) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match get_postgres_frontend_bill(runtime.pool(), user_id, bill_id).await {
            Ok(Some(value)) => success_result(StatusCode::OK, value),
            Ok(None) => not_found("Bill not found"),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn update_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "update_bill_path_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if let Err(response) = validate_bill_money_payload(&payload) {
        return *response;
    }

        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        if get_postgres_bill_by_id(runtime.pool(), user_id.get() as i64, bill_id)
            .await
            .map_err(|_| ())
            .ok()
            .flatten()
            .is_none()
        {
            return not_found("Bill not found");
        }
        let draft = match update_draft_from_payload_postgres(runtime.pool(), user_id, &payload).await
        {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match update_postgres_bill(runtime.pool(), user_id.get() as i64, bill_id, &draft)
            .await
        {
            Ok(true) => match get_postgres_frontend_bill(runtime.pool(), user_id, bill_id).await {
                Ok(Some(value)) => success_result(StatusCode::OK, value),
                Ok(None) => not_found("Bill not found"),
                Err(_) => db_error_response(),
            },
            Ok(false) => not_found("Bill not found or update failed"),
            Err(_) => db_error_response(),
        };
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(domain = "bills", operation = "delete_bill_path_handler", "business operation entered");
    delete_bill_response(state, headers, bill_id, delete_bill_success_payload()).await
}

#[tracing::instrument(level = "debug", skip_all)]
async fn delete_bill_response(
    state: HttpAppState,
    headers: HeaderMap,
    bill_id: i64,
    success_body: Value,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

        let runtime = match open_postgres_runtime(&state, "bills") {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match delete_postgres_bill(runtime.pool(), user_id.get() as i64, bill_id).await {
            Ok(true) => json_response(StatusCode::OK, success_body),
            Ok(false) => not_found("Bill not found"),
            Err(_) => db_error_response(),
        };
}

#[cfg(test)]
mod crud_handler_tests {
    use super::*;

    #[tokio::test]
    async fn crud_list_handler_rejects_missing_auth_before_db_open() {
        let config = HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            std::time::Duration::from_secs(1),
            1024,
            crate::config::ImportRouteMode::ImportDbRuntime,
        )
        .expect("config builds");
        let state = HttpAppState::new(config).expect("state builds");
        let response =
            list_bills_handler(State(state), HeaderMap::new(), Query(BillsListQuery::default()))
                .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn list_handlers_depend_on_ledger_boundary_instead_of_postgres_rows() {
        let source = include_str!("crud_handlers.rs");
        let list_start = source
            .find("async fn list_bills_handler")
            .expect("list handler source");
        let month_start = source[list_start..]
            .find("async fn bills_by_month_handler")
            .map(|offset| list_start + offset)
            .expect("next handler source");
        let month_end = source[month_start..]
            .find("async fn create_bill_handler")
            .map(|offset| month_start + offset)
            .expect("handler after month source");
        let list_handler = &source[list_start..month_start];
        let month_handler = &source[month_start..month_end];

        assert!(list_handler.contains("open_ledger_queries"));
        assert!(list_handler.contains(".list(user_id, query.into_ledger_list_query())"));
        assert!(list_handler.contains("ledger_page_to_frontend"));
        assert!(month_handler.contains("open_ledger_queries"));
        assert!(month_handler.contains(".list_month(user_id, ledger_query)"));
        assert!(month_handler.contains("ledger_page_to_frontend"));
        for forbidden in [
            "open_postgres_runtime",
            ".pool()",
            "query_postgres_bills",
            "BillRecord",
            "page_to_frontend_postgres",
        ] {
            assert!(
                !list_handler.contains(forbidden),
                "list handler leaked forbidden dependency: {forbidden}"
            );
            assert!(
                !month_handler.contains(forbidden),
                "month handler leaked forbidden dependency: {forbidden}"
            );
        }
        assert!(!include_str!("query.rs").contains("fn postgres_filters_from_query"));
        assert!(!include_str!("record_presenters.rs").contains("fn page_to_frontend_postgres"));
    }
}
