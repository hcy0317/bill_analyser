use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bill_analyser_core::budgets::{
    build_budget_execution_summary, build_budget_period_scope, calculate_avg_backtest_mape,
    calculate_budget_period_progress, parse_budget_csv_int_list, parse_budget_json_int_list,
    validate_budget_date_range, validate_budget_period_args, BudgetPeriodScopeInput,
};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    create_budget, create_budget_execution_snapshots, delete_budget, export_budgets,
    get_budget_by_id, import_budgets, query_budget_execution_details,
    query_budget_execution_history, query_budget_forecast, query_budgets_for_listing,
    update_budget, BudgetCreateDraft, BudgetExecutionFilters, BudgetFilters, BudgetForecastFilters,
    BudgetRecord, BudgetUpdateDraft, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, proxy::ProxyState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

type RouteResult<T> = Result<T, Box<Response>>;

pub const BUDGET_CRUD_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/budgets/execution"),
    ("GET", "/api/budgets/forecast"),
    ("GET", "/api/budgets/history"),
    ("POST", "/api/budgets/history/snapshot"),
    ("POST", "/api/budgets/import"),
    ("GET", "/api/budgets"),
    ("GET", "/api/budgets/"),
    ("POST", "/api/budgets"),
    ("POST", "/api/budgets/"),
    ("GET", "/api/budgets/{budget_id}"),
    ("PUT", "/api/budgets/{budget_id}"),
    ("DELETE", "/api/budgets/{budget_id}"),
    ("GET", "/api/budgets/export"),
];

pub const BUDGET_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

pub fn budget_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route("/api/budgets/execution", get(get_budget_execution_handler))
        .route("/api/budgets/forecast", get(get_budget_forecast_handler))
        .route("/api/budgets/history", get(get_budget_history_handler))
        .route(
            "/api/budgets/history/snapshot",
            post(create_budget_history_snapshot_handler),
        )
        .route("/api/budgets/import", post(import_budgets_handler))
        .route("/api/budgets/export", get(export_budgets_handler))
        .route(
            "/api/budgets",
            get(list_budgets_handler).post(create_budget_handler),
        )
        .route(
            "/api/budgets/",
            get(list_budgets_handler).post(create_budget_handler),
        )
        .route(
            "/api/budgets/:budget_id",
            get(get_budget_handler)
                .put(update_budget_handler)
                .delete(delete_budget_handler),
        )
}

#[derive(Debug, Default, Deserialize)]
struct BudgetListQuery {
    period_type: Option<String>,
    enabled: Option<String>,
    category: Option<String>,
    budget_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BudgetExecutionQuery {
    budget_type: Option<String>,
    period_type: Option<String>,
    year: Option<String>,
    month: Option<String>,
    quarter: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    budget_id: Option<String>,
    category_id: Option<String>,
    account_ids: Option<String>,
    tag_ids: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BudgetForecastQuery {
    budget_type: Option<String>,
    period_type: Option<String>,
    year: Option<String>,
    month: Option<String>,
    quarter: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    forecast_strategy: Option<String>,
    months_history: Option<String>,
}

async fn list_budgets_handler(
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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
    State(state): State<ProxyState>,
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

async fn export_budgets_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
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
    State(state): State<ProxyState>,
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

fn filters_from_query(query: &BudgetListQuery) -> RouteResult<BudgetFilters> {
    Ok(BudgetFilters {
        period_type: non_empty_string(query.period_type.as_ref()),
        enabled: query.enabled.as_deref().and_then(parse_enabled),
        category: non_empty_string(query.category.as_ref()),
        budget_type: match query
            .budget_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) => Some(parse_i32(value, "budget_type")?),
            None => None,
        },
    })
}

fn budget_scope_from_execution_query(
    query: &BudgetExecutionQuery,
) -> RouteResult<bill_analyser_core::budgets::BudgetPeriodScope> {
    let input = BudgetPeriodScopeInput {
        budget_type: parse_optional_i32(query.budget_type.as_deref(), "budget_type")?,
        period_type: query.period_type.clone(),
        start_date: non_empty_string(query.start_date.as_ref()),
        end_date: non_empty_string(query.end_date.as_ref()),
        year: parse_optional_i32(query.year.as_deref(), "year")?,
        month: parse_optional_u32(query.month.as_deref(), "month")?,
        quarter: parse_optional_u32(query.quarter.as_deref(), "quarter")?,
    };
    build_budget_period_scope(&input, Utc::now().date_naive())
        .map_err(|error| Box::new(bad_request(error)) as Box<Response>)
}

fn budget_scope_from_forecast_query(
    query: &BudgetForecastQuery,
) -> RouteResult<(bill_analyser_core::budgets::BudgetPeriodScope, i32)> {
    let months_history =
        parse_optional_i32(query.months_history.as_deref(), "months_history")?.unwrap_or(6);
    let input = BudgetPeriodScopeInput {
        budget_type: parse_optional_i32(query.budget_type.as_deref(), "budget_type")?,
        period_type: query.period_type.clone(),
        start_date: non_empty_string(query.start_date.as_ref()),
        end_date: non_empty_string(query.end_date.as_ref()),
        year: parse_optional_i32(query.year.as_deref(), "year")?,
        month: parse_optional_u32(query.month.as_deref(), "month")?,
        quarter: parse_optional_u32(query.quarter.as_deref(), "quarter")?,
    };
    validate_budget_period_args(&input, Some(i64::from(months_history)))
        .map_err(|error| Box::new(bad_request(error)))?;
    let scope = build_budget_period_scope(&input, Utc::now().date_naive())
        .map_err(|error| Box::new(bad_request(error)))?;
    Ok((scope, months_history))
}

fn budget_scope_from_payload(
    payload: &Map<String, Value>,
) -> RouteResult<bill_analyser_core::budgets::BudgetPeriodScope> {
    let input = BudgetPeriodScopeInput {
        budget_type: parse_optional_i32_value(payload.get("budget_type"), "budget_type")?,
        period_type: non_empty_value_string(payload.get("period_type")),
        start_date: non_empty_value_string(payload.get("start_date")),
        end_date: non_empty_value_string(payload.get("end_date")),
        year: parse_optional_i32_value(payload.get("year"), "year")?,
        month: parse_optional_u32_value(payload.get("month"), "month")?,
        quarter: parse_optional_u32_value(payload.get("quarter"), "quarter")?,
    };
    build_budget_period_scope(&input, Utc::now().date_naive())
        .map_err(|error| Box::new(bad_request(error)) as Box<Response>)
}

fn execution_filters_from_query(
    query: &BudgetExecutionQuery,
    scope: &bill_analyser_core::budgets::BudgetPeriodScope,
) -> RouteResult<BudgetExecutionFilters> {
    Ok(BudgetExecutionFilters {
        budget_type: scope.budget_type,
        period_type: Some(scope.period_type.clone()),
        start_date: Some(scope.start_date.clone()),
        end_date: Some(scope.end_date.clone()),
        budget_id: parse_optional_i64(query.budget_id.as_deref(), "budget_id")?,
        category_id: parse_optional_i64(query.category_id.as_deref(), "category_id")?,
        account_ids: parse_budget_csv_int_list(query.account_ids.as_deref(), "account_ids")
            .map_err(|error| Box::new(bad_request(error)))?,
        tag_ids: parse_budget_csv_int_list(query.tag_ids.as_deref(), "tag_ids")
            .map_err(|error| Box::new(bad_request(error)))?,
    })
}

fn execution_filters_from_payload(
    payload: &Map<String, Value>,
    scope: &bill_analyser_core::budgets::BudgetPeriodScope,
) -> RouteResult<BudgetExecutionFilters> {
    let account_ids = parse_budget_json_int_list(payload.get("account_ids"), "account_ids")
        .map_err(|error| Box::new(bad_request(error)))?;
    let tag_ids = parse_budget_json_int_list(payload.get("tag_ids"), "tag_ids")
        .map_err(|error| Box::new(bad_request(error)))?;
    Ok(BudgetExecutionFilters {
        budget_type: scope.budget_type,
        period_type: Some(scope.period_type.clone()),
        start_date: Some(scope.start_date.clone()),
        end_date: Some(scope.end_date.clone()),
        budget_id: parse_optional_i64_value(payload.get("budget_id"), "budget_id")?,
        category_id: parse_optional_i64_value(payload.get("category_id"), "category_id")?,
        account_ids: (!account_ids.is_empty()).then_some(account_ids),
        tag_ids: (!tag_ids.is_empty()).then_some(tag_ids),
    })
}

fn import_budget_records_from_payload(payload: &Value) -> RouteResult<Vec<BudgetRecord>> {
    let Some(items) = payload.as_array().filter(|items| !items.is_empty()) else {
        return Err(Box::new(invalid_budget_import_data_response()));
    };
    let mut records = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(Box::new(bad_request(format!(
                "Invalid budget item at index {index}"
            ))));
        };
        validate_import_budget_record(object, index)?;
        records.push(object.clone());
    }
    Ok(records)
}

fn validate_import_budget_record(payload: &BudgetRecord, index: usize) -> RouteResult<()> {
    for field in ["period_type", "amount", "start_date"] {
        if !payload.contains_key(field) {
            return Err(Box::new(bad_request(format!(
                "Missing required field at index {index}: {field}"
            ))));
        }
    }
    if value_string(payload.get("category")).is_none() {
        return Err(Box::new(bad_request(format!(
            "Missing required field at index {index}: category"
        ))));
    }

    let period_type = value_string(payload.get("period_type")).unwrap_or_default();
    validate_budget_period_args(
        &BudgetPeriodScopeInput {
            period_type: Some(period_type),
            ..BudgetPeriodScopeInput::default()
        },
        None,
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    validate_budget_date_range(
        value_string(payload.get("start_date")).as_deref(),
        value_string(payload.get("end_date")).as_deref(),
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    Ok(())
}

fn create_fields_from_payload(payload: &Value) -> RouteResult<BudgetRecord> {
    let mut fields = payload_object(payload)?.clone();
    for field in ["period_type", "amount", "start_date"] {
        if missing_required_field(&fields, field) {
            return Err(Box::new(bad_request(format!(
                "Missing required field: {field}"
            ))));
        }
    }
    if missing_required_field(&fields, "category") {
        return Err(Box::new(bad_request("Missing required field: category")));
    }
    let period_type = value_string(fields.get("period_type")).unwrap_or_default();
    validate_budget_period_args(
        &BudgetPeriodScopeInput {
            period_type: Some(period_type.clone()),
            ..BudgetPeriodScopeInput::default()
        },
        None,
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    validate_budget_date_range(
        value_string(fields.get("start_date")).as_deref(),
        value_string(fields.get("end_date")).as_deref(),
    )
    .map_err(|error| Box::new(bad_request(error)))?;
    fields
        .entry("name".to_string())
        .or_insert_with(|| Value::String(String::new()));
    fields
        .entry("enabled".to_string())
        .or_insert_with(|| Value::Bool(true));
    fields
        .entry("alert_threshold".to_string())
        .or_insert_with(|| json!(80));
    Ok(fields)
}

fn update_fields_from_payload(
    payload: &Value,
    existing_budget: &BudgetRecord,
) -> RouteResult<BudgetRecord> {
    let mut fields = payload_object(payload)?.clone();
    if let Some(period_type) = fields
        .get("period_type")
        .and_then(|value| value_string(Some(value)))
    {
        validate_budget_period_args(
            &BudgetPeriodScopeInput {
                period_type: Some(period_type),
                ..BudgetPeriodScopeInput::default()
            },
            None,
        )
        .map_err(|error| Box::new(bad_request(error)))?;
    }
    let start_date = value_string(fields.get("start_date"))
        .or_else(|| value_string(existing_budget.get("start_date")));
    let end_date = value_string(fields.get("end_date"))
        .or_else(|| value_string(existing_budget.get("end_date")));
    validate_budget_date_range(start_date.as_deref(), end_date.as_deref())
        .map_err(|error| Box::new(bad_request(error)))?;
    fields.insert("updated_at".to_string(), Value::String(now_text()));
    Ok(fields)
}

fn payload_object(payload: &Value) -> RouteResult<&Map<String, Value>> {
    payload
        .as_object()
        .filter(|object| !object.is_empty())
        .ok_or_else(|| Box::new(bad_request("No data provided")))
}

fn missing_required_field(payload: &BudgetRecord, field: &str) -> bool {
    payload
        .get(field)
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(|text| text.is_empty()))
}

fn parse_i32(value: &str, field_name: &str) -> RouteResult<i32> {
    value.trim().parse::<i32>().map_err(|_| {
        Box::new(bad_request(format!(
            "Invalid integer for {field_name}: {value}"
        )))
    })
}

fn parse_optional_i32(value: Option<&str>, field_name: &str) -> RouteResult<Option<i32>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_i32(value, field_name))
        .transpose()
}

fn parse_optional_i32_value(value: Option<&Value>, field_name: &str) -> RouteResult<Option<i32>> {
    let value = value_string(value);
    parse_optional_i32(value.as_deref(), field_name)
}

fn parse_optional_i64(value: Option<&str>, field_name: &str) -> RouteResult<Option<i64>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.trim().parse::<i64>().map_err(|_| {
                Box::new(bad_request(format!(
                    "Invalid integer for {field_name}: {value}"
                ))) as Box<Response>
            })
        })
        .transpose()
}

fn parse_optional_i64_value(value: Option<&Value>, field_name: &str) -> RouteResult<Option<i64>> {
    let value = value_string(value);
    parse_optional_i64(value.as_deref(), field_name)
}

fn parse_optional_u32(value: Option<&str>, field_name: &str) -> RouteResult<Option<u32>> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            value.trim().parse::<u32>().map_err(|_| {
                Box::new(bad_request(format!(
                    "Invalid integer for {field_name}: {value}"
                ))) as Box<Response>
            })
        })
        .transpose()
}

fn parse_optional_u32_value(value: Option<&Value>, field_name: &str) -> RouteResult<Option<u32>> {
    let value = value_string(value);
    parse_optional_u32(value.as_deref(), field_name)
}

fn parse_enabled(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}

fn value_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.trim().to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn non_empty_value_string(value: Option<&Value>) -> Option<String> {
    value_string(value)
}

fn forecast_amount(item: &Value) -> f64 {
    item.get("forecast_amount")
        .and_then(Value::as_f64)
        .unwrap_or_default()
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn open_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust budget DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        ))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
        ))
    })?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: state.config.timeout,
    })
    .map_err(|_| Box::new(db_error_response()))
}

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn invalid_budget_import_data_response() -> Response {
    bad_request("Invalid data format. Expected array of budgets.")
}

fn not_found(message: impl ToString) -> Response {
    error_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust budget route runtime DB error",
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

fn now_text() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
