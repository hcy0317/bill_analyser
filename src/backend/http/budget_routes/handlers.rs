// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。


async fn list_budgets_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BudgetListQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = match filters_from_query(&query) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_budgets_for_listing(runtime.connection(), user_id, &filters) {
        Ok(budgets) => success_result(
            StatusCode::OK,
            Value::Array(budgets.into_iter().map(Value::Object).collect()),
        ),
        Err(_) => db_error_response(),
    }
}

async fn get_budget_execution_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BudgetExecutionQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let scope = match budget_scope_from_execution_query(&query) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = match execution_filters_from_query(&query, &scope) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_budget_execution_details(runtime.connection(), user_id, &filters) {
        Ok(items) => success_result(
            StatusCode::OK,
            json!({
                "items": items,
                "summary": build_budget_execution_summary(&items),
                "period_start": scope.start_date,
                "period_end": scope.end_date
            }),
        ),
        Err(_) => db_error_response(),
    }
}

async fn get_budget_forecast_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BudgetForecastQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let (scope, history_periods) = match budget_scope_from_forecast_query(&query) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let forecast_strategy = non_empty_string(query.forecast_strategy.as_ref())
        .unwrap_or_else(|| "historical_average".to_string());
    let filters = BudgetForecastFilters {
        budget_type: scope.budget_type,
        period_type: scope.period_type.clone(),
        start_date: scope.start_date.clone(),
        end_date: scope.end_date.clone(),
        forecast_strategy: forecast_strategy.clone(),
        history_periods: i64::from(history_periods),
    };
    match query_budget_forecast(runtime.connection(), user_id, &filters) {
        Ok(items) => {
            let progress = match calculate_budget_period_progress(
                &scope.start_date,
                &scope.end_date,
                Utc::now().date_naive(),
            ) {
                Ok(value) => value,
                Err(error) => return bad_request(error),
            };
            let total_forecast = round2(items.iter().map(forecast_amount).sum::<f64>());
            let item_count = items.len();
            let avg_backtest_mape = calculate_avg_backtest_mape(&items);
            success_result(
                StatusCode::OK,
                json!({
                    "items": items,
                    "period_start": scope.start_date,
                    "period_end": scope.end_date,
                    "periodStart": scope.start_date,
                    "periodEnd": scope.end_date,
                    "daysElapsed": progress.elapsed_days,
                    "daysRemaining": progress.remaining_days,
                    "summary": {
                        "total_forecast": total_forecast,
                        "count": item_count,
                        "forecast_strategy": forecast_strategy,
                        "history_periods": history_periods,
                        "avg_backtest_mape": avg_backtest_mape,
                        "days_elapsed": progress.elapsed_days,
                        "days_remaining": progress.remaining_days
                    }
                }),
            )
        }
        Err(_) => db_error_response(),
    }
}

async fn get_budget_history_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BudgetExecutionQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let scope = match budget_scope_from_execution_query(&query) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = match execution_filters_from_query(&query, &scope) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_budget_execution_history(runtime.connection(), user_id, &filters) {
        Ok(items) => {
            let count = items.len();
            success_result(
                StatusCode::OK,
                json!({
                    "items": items,
                    "summary": {
                        "count": count,
                        "period_start": scope.start_date,
                        "period_end": scope.end_date
                    }
                }),
            )
        }
        Err(_) => db_error_response(),
    }
}

async fn create_budget_history_snapshot_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let object = match payload.as_object() {
        Some(value) => value,
        None => return bad_request("Invalid JSON body. Expected object."),
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let scope = match budget_scope_from_payload(object) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = match execution_filters_from_payload(object, &scope) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match create_budget_execution_snapshots(runtime.connection_mut(), user_id, &filters) {
        Ok(result) => success_result(StatusCode::OK, result),
        Err(_) => db_error_response(),
    }
}

async fn get_budget_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(budget_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_budget_by_id(runtime.connection(), user_id, budget_id) {
        Ok(Some(budget)) => success_result(StatusCode::OK, Value::Object(budget)),
        Ok(None) => not_found("Budget not found"),
        Err(_) => db_error_response(),
    }
}

async fn create_budget_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut fields = match create_fields_from_payload(&payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let now = now_text();
    fields.insert("created_at".to_string(), Value::String(now.clone()));
    fields.insert("updated_at".to_string(), Value::String(now));
    let budget_id = match create_budget(
        runtime.connection_mut(),
        user_id,
        &BudgetCreateDraft { fields },
    ) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match get_budget_by_id(runtime.connection(), user_id, budget_id) {
        Ok(Some(budget)) => success_result(StatusCode::CREATED, Value::Object(budget)),
        Ok(None) | Err(_) => db_error_response(),
    }
}

async fn update_budget_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(budget_id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let existing_budget = match get_budget_by_id(runtime.connection(), user_id, budget_id) {
        Ok(Some(value)) => value,
        Ok(None) => return not_found("Budget not found"),
        Err(_) => return db_error_response(),
    };
    let fields = match update_fields_from_payload(&payload, &existing_budget) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match update_budget(
        runtime.connection_mut(),
        user_id,
        budget_id,
        &BudgetUpdateDraft { fields },
    ) {
        Ok(true) => json_response(
            StatusCode::OK,
            json!({"success": true, "message": "Budget updated successfully"}),
        ),
        Ok(false) => not_found("Budget not found"),
        Err(error) => bad_request(error.to_string()),
    }
}

async fn delete_budget_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(budget_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match delete_budget(runtime.connection_mut(), user_id, budget_id) {
        Ok(true) => json_response(
            StatusCode::OK,
            json!({"success": true, "message": "Budget deleted successfully"}),
        ),
        Ok(false) => not_found("Budget not found"),
        Err(_) => db_error_response(),
    }
}

async fn export_budgets_handler(State(state): State<HttpAppState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match export_budgets(runtime.connection(), user_id) {
        Ok(rows) => success_result(StatusCode::OK, Value::Array(rows)),
        Err(_) => db_error_response(),
    }
}

async fn import_budgets_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = match serde_json::from_slice::<Value>(&body) {
        Ok(value) => value,
        Err(_) => return invalid_budget_import_data_response(),
    };
    let items = match import_budget_records_from_payload(&payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match import_budgets(runtime.connection_mut(), user_id, &items) {
        Ok(result) => success_result(StatusCode::OK, result),
        Err(_) => db_error_response(),
    }
}
