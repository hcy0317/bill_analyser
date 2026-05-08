use std::collections::BTreeMap;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use bill_analyser_core::statistics::{
    parse_statistics_timestamp_range, parse_statistics_year_month_range,
    parse_transaction_amount_period_query, validate_asset_trends_span, StatisticsContractError,
    StatisticsTimestampRange, StatisticsYearMonthRange, StatisticsYearMonthRangeMode,
};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    find_statistics_all_date_range, query_asset_trends_payload, query_category_pie_payload,
    query_category_statistics_payload, query_category_trends_payload, query_top_merchants_payload,
    query_transaction_amount_period, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
    StatisticsBillFilters,
};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, proxy::ProxyState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

type RouteResult<T> = Result<T, Box<Response>>;

pub const STATISTICS_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/statistics/category-statistics"),
    ("GET", "/api/statistics/category-statistics/trends"),
    ("GET", "/api/statistics/asset-trends"),
    ("GET", "/api/statistics/category-pie"),
    ("GET", "/api/statistics/top-merchants"),
    ("GET", "/api/statistics/amounts"),
];

pub const STATISTICS_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/statistics/overview"),
    ("GET", "/api/statistics/trends"),
    ("GET", "/api/statistics/comparison"),
    ("GET", "/api/statistics/category"),
    ("GET", "/api/statistics/trend"),
    ("GET", "/api/statistics/exchange-rates"),
    ("PUT", "/api/statistics/exchange-rates/custom"),
    ("DELETE", "/api/statistics/exchange-rates/custom/{currency}"),
];

pub fn statistics_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route(
            "/api/statistics/category-statistics",
            get(category_statistics_handler),
        )
        .route(
            "/api/statistics/category-statistics/trends",
            get(category_trends_handler),
        )
        .route("/api/statistics/asset-trends", get(asset_trends_handler))
        .route("/api/statistics/category-pie", get(category_pie_handler))
        .route("/api/statistics/top-merchants", get(top_merchants_handler))
        .route("/api/statistics/amounts", get(transaction_amounts_handler))
}

#[derive(Debug, Default, Deserialize)]
struct CategoryStatisticsQuery {
    #[serde(rename = "startTime")]
    start_time_camel: Option<String>,
    #[serde(rename = "endTime")]
    end_time_camel: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CategoryTrendsQuery {
    #[serde(rename = "startYearMonth")]
    start_year_month_camel: Option<String>,
    #[serde(rename = "endYearMonth")]
    end_year_month_camel: Option<String>,
    start_year_month: Option<String>,
    end_year_month: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BasicStatisticsQuery {
    #[serde(rename = "type")]
    transaction_type: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    limit: Option<String>,
    query: Option<String>,
    periods: Option<String>,
}

async fn category_statistics_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let range = match timestamp_range_from_query(
        query
            .start_time_camel
            .as_deref()
            .or(query.start_time.as_deref()),
        query
            .end_time_camel
            .as_deref()
            .or(query.end_time.as_deref()),
        true,
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = bill_filters_for_timestamp_range(&range, query.keyword.as_deref());
    match query_category_statistics_payload(runtime.connection(), user_id, &filters) {
        Ok(items) => success_result(
            StatusCode::OK,
            json!({
                "startTime": range.start_time,
                "endTime": range.end_time,
                "items": items
            }),
        ),
        Err(_) => db_error_response(),
    }
}

async fn category_trends_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<CategoryTrendsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let range = match year_month_range_from_query(
        query
            .start_year_month_camel
            .as_deref()
            .or(query.start_year_month.as_deref()),
        query
            .end_year_month_camel
            .as_deref()
            .or(query.end_year_month.as_deref()),
        runtime.connection(),
        user_id,
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(range) = range else {
        return success_result(StatusCode::OK, json!([]));
    };
    let filters = StatisticsBillFilters {
        start_date: Some(range.start_date.clone()),
        end_date: Some(range.end_date.clone()),
        keyword: non_empty_string(query.keyword.as_ref()),
        ..StatisticsBillFilters::default()
    };
    match query_category_trends_payload(runtime.connection(), user_id, &filters, &range) {
        Ok(items) => success_result(StatusCode::OK, items),
        Err(_) => db_error_response(),
    }
}

async fn asset_trends_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let range = match asset_date_range_from_query(
        query
            .start_time_camel
            .as_deref()
            .or(query.start_time.as_deref()),
        query
            .end_time_camel
            .as_deref()
            .or(query.end_time.as_deref()),
        runtime.connection(),
        user_id,
    ) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some((start_date, end_date)) = range else {
        return success_result(StatusCode::OK, json!([]));
    };
    match query_asset_trends_payload(runtime.connection(), user_id, start_date, end_date) {
        Ok(payload) => success_result(
            StatusCode::OK,
            payload.get("items").cloned().unwrap_or_else(|| json!([])),
        ),
        Err(_) => db_error_response(),
    }
}

async fn category_pie_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BasicStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = StatisticsBillFilters {
        start_date: non_empty_string(query.start_date.as_ref()),
        end_date: non_empty_string(query.end_date.as_ref()),
        transaction_type: non_empty_string(query.transaction_type.as_ref())
            .or_else(|| Some("支出".to_string())),
        ..StatisticsBillFilters::default()
    };
    match query_category_pie_payload(runtime.connection(), user_id, &filters) {
        Ok(items) => success_data(StatusCode::OK, json!(items)),
        Err(_) => db_error_response(),
    }
}

async fn top_merchants_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BasicStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let limit = match query.limit.as_deref().unwrap_or("10").parse::<usize>() {
        Ok(value) => value,
        Err(_) => return internal_error("invalid digit found in string"),
    };
    let filters = StatisticsBillFilters {
        start_date: non_empty_string(query.start_date.as_ref()),
        end_date: non_empty_string(query.end_date.as_ref()),
        ..StatisticsBillFilters::default()
    };
    match query_top_merchants_payload(runtime.connection(), user_id, &filters, limit) {
        Ok(items) => success_data(StatusCode::OK, json!(items)),
        Err(_) => db_error_response(),
    }
}

async fn transaction_amounts_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BasicStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let query_text = non_empty_string(query.query.as_ref())
        .or_else(|| non_empty_string(query.periods.as_ref()))
        .unwrap_or_default();
    if query_text.is_empty() {
        return bad_request("Missing query parameter");
    }

    let mut results = BTreeMap::new();
    for period_query in query_text.split('|') {
        let Some((period_name, start_time, end_time)) =
            parse_transaction_amount_period_query(period_query)
        else {
            continue;
        };
        let Some(start_date) = date_from_timestamp(start_time) else {
            return internal_error("invalid timestamp");
        };
        let Some(end_date) = date_from_timestamp(end_time) else {
            return internal_error("invalid timestamp");
        };
        let result = match query_transaction_amount_period(
            runtime.connection(),
            user_id,
            start_time,
            end_time,
            start_date.to_string(),
            end_date.to_string(),
        ) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        results.insert(period_name, result);
    }
    success_result(StatusCode::OK, json!(results))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTimestampRange {
    start_time: i64,
    end_time: i64,
    start_date: Option<String>,
    end_date: Option<String>,
}

fn timestamp_range_from_query(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
    default_current_month: bool,
) -> RouteResult<ResolvedTimestampRange> {
    let (start_raw, end_raw) = match (start_raw, end_raw) {
        (Some(start), Some(end)) => (start.to_string(), end.to_string()),
        _ if default_current_month => {
            let (start, end) = current_month_timestamp_range();
            (start.to_string(), end.to_string())
        }
        _ => (String::new(), String::new()),
    };
    match parse_statistics_timestamp_range(Some(&start_raw), Some(&end_raw)) {
        Ok(StatisticsTimestampRange::All) => Ok(ResolvedTimestampRange {
            start_time: 0,
            end_time: 0,
            start_date: None,
            end_date: None,
        }),
        Ok(StatisticsTimestampRange::Bounded {
            start_time,
            end_time,
        }) => {
            let start_date = date_from_timestamp(start_time).ok_or_else(|| {
                Box::new(bad_request(
                    "Invalid timestamp format: timestamp out of range",
                ))
            })?;
            let end_date = date_from_timestamp(end_time).ok_or_else(|| {
                Box::new(bad_request(
                    "Invalid timestamp format: timestamp out of range",
                ))
            })?;
            Ok(ResolvedTimestampRange {
                start_time,
                end_time,
                start_date: Some(start_date.to_string()),
                end_date: Some(end_date.to_string()),
            })
        }
        Err(error) => Err(Box::new(statistics_error_response(error, false))),
    }
}

fn year_month_range_from_query(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
    connection: &rusqlite::Connection,
    user_id: UserId,
) -> RouteResult<Option<StatisticsYearMonthRange>> {
    let (start_raw, end_raw) = match (start_raw, end_raw) {
        (Some(start), Some(end)) => (start.to_string(), end.to_string()),
        _ => {
            let now = Local::now();
            (format!("{}01", now.year()), format!("{}12", now.year()))
        }
    };
    match parse_statistics_year_month_range(Some(&start_raw), Some(&end_raw)) {
        Ok(StatisticsYearMonthRangeMode::Bounded(range)) => Ok(Some(range)),
        Ok(StatisticsYearMonthRangeMode::All) => {
            let Some(range) = find_statistics_all_date_range(connection, user_id)
                .map_err(|_| Box::new(db_error_response()))?
            else {
                return Ok(None);
            };
            let start = range.start_date.chars().take(7).collect::<String>();
            let end = range.end_date.chars().take(7).collect::<String>();
            match parse_statistics_year_month_range(Some(&start), Some(&end)) {
                Ok(StatisticsYearMonthRangeMode::Bounded(range)) => Ok(Some(range)),
                Ok(StatisticsYearMonthRangeMode::All) => Ok(None),
                Err(error) => Err(Box::new(statistics_error_response(error, true))),
            }
        }
        Err(error) => Err(Box::new(statistics_error_response(error, true))),
    }
}

fn asset_date_range_from_query(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
    connection: &rusqlite::Connection,
    user_id: UserId,
) -> RouteResult<Option<(NaiveDate, NaiveDate)>> {
    if matches!((start_raw, end_raw), (Some("0"), Some("0"))) {
        let Some(range) = find_statistics_all_date_range(connection, user_id)
            .map_err(|_| Box::new(db_error_response()))?
        else {
            return Ok(None);
        };
        return Ok(Some((
            parse_date_prefix(&range.start_date)?,
            parse_date_prefix(&range.end_date)?,
        )));
    }
    let range = timestamp_range_from_query(start_raw, end_raw, true)?;
    let (Some(start_date), Some(end_date)) = (range.start_date, range.end_date) else {
        return Ok(None);
    };
    validate_asset_trends_span(range.start_time, range.end_time, false)
        .map_err(|error| Box::new(asset_trends_error_response(error)))?;
    Ok(Some((
        parse_date_prefix(&start_date)?,
        parse_date_prefix(&end_date)?,
    )))
}

fn bill_filters_for_timestamp_range(
    range: &ResolvedTimestampRange,
    keyword: Option<&str>,
) -> StatisticsBillFilters {
    StatisticsBillFilters {
        start_date: range.start_date.clone(),
        end_date: range.end_date.clone(),
        keyword: keyword
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        ..StatisticsBillFilters::default()
    }
}

fn parse_date_prefix(value: &str) -> RouteResult<NaiveDate> {
    NaiveDate::parse_from_str(
        value
            .get(..10)
            .ok_or_else(|| Box::new(bad_request("Invalid timestamp format: invalid date")))?,
        "%Y-%m-%d",
    )
    .map_err(|_| Box::new(bad_request("Invalid timestamp format: invalid date")))
}

fn current_month_timestamp_range() -> (i64, i64) {
    let now = Local::now();
    let month_start = Local
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .unwrap_or(now);
    let next_month = if now.month() == 12 {
        Local.with_ymd_and_hms(now.year() + 1, 1, 1, 0, 0, 0)
    } else {
        Local.with_ymd_and_hms(now.year(), now.month() + 1, 1, 0, 0, 0)
    }
    .single()
    .unwrap_or(now);
    (month_start.timestamp(), next_month.timestamp() - 1)
}

fn date_from_timestamp(value: i64) -> Option<NaiveDate> {
    Local
        .timestamp_opt(value, 0)
        .single()
        .map(|datetime| datetime.date_naive())
}

fn open_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust statistics DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
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

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn success_data(status: StatusCode, data: Value) -> Response {
    json_response(status, json!({ "success": true, "data": data }))
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn internal_error(message: impl ToString) -> Response {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust statistics route runtime DB error",
    )
}

fn statistics_error_response(error: StatisticsContractError, year_month: bool) -> Response {
    if year_month && error.error == "Invalid year-month format" {
        return bad_request(error.message);
    }
    json_response(
        StatusCode::BAD_REQUEST,
        json!({
            "success": false,
            "error": error.error,
            "message": error.message
        }),
    )
}

fn asset_trends_error_response(error: StatisticsContractError) -> Response {
    if error.error == "资产趋势查询最多支持365天范围，请缩小时间范围" {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "success": false,
                "errorMessage": error.error,
                "error": error.error,
                "errorCode": 400
            }),
        );
    }
    statistics_error_response(error, false)
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}
