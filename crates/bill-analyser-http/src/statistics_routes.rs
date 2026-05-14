use std::{collections::BTreeMap, time::Duration};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, put},
    Json, Router,
};
use bill_analyser_core::statistics::{
    build_builtin_fallback_exchange_rates, build_overview_result_from_report,
    build_provider_candidate_order, build_provider_exchange_rates_result,
    build_statistics_trend_response, build_user_custom_exchange_rates_result,
    convert_cny_quote_map_to_rates, convert_provider_base_currency, exchange_rate_provider_options,
    normalize_chinese_currency_name, normalize_requested_exchange_rate_provider,
    parse_statistics_timestamp_range, parse_statistics_year_month_range,
    parse_transaction_amount_period_query, validate_asset_trends_span, StatisticsContractError,
    StatisticsTimestampRange, StatisticsYearMonthRange, StatisticsYearMonthRangeMode,
    TARGET_EXCHANGE_CURRENCIES,
};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    delete_user_custom_exchange_rate, find_statistics_all_date_range,
    get_statistics_user_default_currency, list_user_custom_exchange_rates,
    query_asset_trends_payload, query_category_pie_payload, query_category_statistics_payload,
    query_category_trends_payload, query_statistics_analyzer_category_payload,
    query_statistics_analyzer_comparison_payload, query_statistics_analyzer_report_payload,
    query_statistics_analyzer_trends_payload, query_top_merchants_payload,
    query_transaction_amount_period, upsert_user_custom_exchange_rate, SqliteConnectionConfig,
    SqliteDbPath, SqliteRuntime, StatisticsBillFilters,
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
    ("GET", "/api/statistics/overview"),
    ("GET", "/api/statistics/trends"),
    ("GET", "/api/statistics/comparison"),
    ("GET", "/api/statistics/category"),
    ("GET", "/api/statistics/trend"),
    ("GET", "/api/statistics/exchange-rates"),
    ("PUT", "/api/statistics/exchange-rates/custom"),
    ("DELETE", "/api/statistics/exchange-rates/custom/{currency}"),
];

pub const STATISTICS_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

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
        .route("/api/statistics/overview", get(analyzer_overview_handler))
        .route("/api/statistics/trends", get(analyzer_trends_handler))
        .route(
            "/api/statistics/comparison",
            get(analyzer_comparison_handler),
        )
        .route("/api/statistics/category", get(analyzer_category_handler))
        .route("/api/statistics/trend", get(analyzer_trend_handler))
        .route(
            "/api/statistics/exchange-rates",
            get(exchange_rates_handler),
        )
        .route(
            "/api/statistics/exchange-rates/custom",
            put(update_user_custom_exchange_rate_handler),
        )
        .route(
            "/api/statistics/exchange-rates/custom/:currency",
            delete(delete_user_custom_exchange_rate_handler),
        )
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

#[derive(Debug, Default, Deserialize)]
struct AnalyzerStatisticsQuery {
    period: Option<String>,
    category: Option<String>,
    #[serde(rename = "type")]
    compare_type: Option<String>,
    main_category: Option<String>,
    granularity: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ExchangeRatesQuery {
    base_currency: Option<String>,
    base: Option<String>,
    provider: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct UserCustomExchangeRateRequest {
    currency: Option<String>,
    rate: Option<Value>,
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

async fn analyzer_overview_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let period = analyzer_period(query.period.as_deref());
    match query_statistics_analyzer_report_payload(runtime.connection(), user_id, &period) {
        Ok(report) => success_result(StatusCode::OK, build_overview_result_from_report(&report)),
        Err(_) => db_error_response(),
    }
}

async fn analyzer_trends_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let period = analyzer_period(query.period.as_deref());
    match query_statistics_analyzer_trends_payload(
        runtime.connection(),
        user_id,
        &period,
        query.category.as_deref(),
    ) {
        Ok(result) => success_result(StatusCode::OK, result),
        Err(_) => db_error_response(),
    }
}

async fn analyzer_comparison_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let period = analyzer_period(query.period.as_deref());
    let compare_type = query
        .compare_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("category")
        .to_string();
    match query_statistics_analyzer_comparison_payload(
        runtime.connection(),
        user_id,
        &period,
        &compare_type,
    ) {
        Ok(result) => success_result(StatusCode::OK, result),
        Err(_) => db_error_response(),
    }
}

async fn analyzer_category_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let period = analyzer_period(query.period.as_deref());
    match query_statistics_analyzer_category_payload(
        runtime.connection(),
        user_id,
        &period,
        query.main_category.as_deref(),
    ) {
        Ok(result) => success_data(StatusCode::OK, result),
        Err(_) => db_error_response(),
    }
}

async fn analyzer_trend_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let period = analyzer_period(query.granularity.as_deref());
    match query_statistics_analyzer_trends_payload(
        runtime.connection(),
        user_id,
        &period,
        query.category.as_deref(),
    ) {
        Ok(result) => json_response(StatusCode::OK, build_statistics_trend_response(&result)),
        Err(_) => db_error_response(),
    }
}

async fn exchange_rates_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<ExchangeRatesQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let requested_provider = normalize_requested_exchange_rate_provider(query.provider.as_deref());
    if !exchange_rate_provider_options().contains_key(&requested_provider) {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "success": false,
                "error": format!("Unsupported exchange rate provider: {requested_provider}")
            }),
        );
    }

    let base_currency =
        match normalized_exchange_base_currency(&query, runtime.connection(), user_id) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let custom_rates =
        match list_user_custom_exchange_rates(runtime.connection(), user_id, &base_currency) {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
    let now = chrono::Utc::now().timestamp();
    if requested_provider == "auto" && !custom_rates.is_empty() {
        return success_result(
            StatusCode::OK,
            json!(build_user_custom_exchange_rates_result(
                &base_currency,
                &custom_rates,
                now,
            )),
        );
    }

    let target_currencies = target_exchange_currencies(&base_currency);
    let provider_result = fetch_exchange_rates_from_providers(
        &base_currency,
        &target_currencies,
        &requested_provider,
    )
    .await;
    let result = match provider_result {
        Some((provider_key, rates)) if !rates.is_empty() => build_provider_exchange_rates_result(
            &base_currency,
            &requested_provider,
            &provider_key,
            &rates,
            now,
        ),
        _ => build_builtin_fallback_exchange_rates(&base_currency, now),
    };
    success_result(StatusCode::OK, json!(result))
}

async fn update_user_custom_exchange_rate_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Json(body): Json<UserCustomExchangeRateRequest>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let currency = body
        .currency
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_uppercase();
    if currency.is_empty() || is_missing_json_value(body.rate.as_ref()) {
        return invalid_request("currency and rate are required");
    }
    let rate = match parse_exchange_rate_value(body.rate.as_ref()) {
        Ok(value) => value,
        Err(message) => return invalid_request(message),
    };
    if rate <= 0.0 {
        return invalid_request("rate must be greater than 0");
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let base_currency = match get_statistics_user_default_currency(runtime.connection(), user_id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match upsert_user_custom_exchange_rate(
        runtime.connection(),
        user_id,
        &base_currency,
        &currency,
        rate,
    ) {
        Ok(result) => success_result(
            StatusCode::OK,
            json!({
                "currency": currency,
                "rate": format_route_rate_value(rate),
                "updateTime": result.update_time
            }),
        ),
        Err(error) => {
            let message = match &error {
                bill_analyser_db::DbError::InvalidOperation(_) => {
                    "Failed to update user custom exchange rate".to_string()
                }
                _ => error.to_string(),
            };
            json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "success": false,
                    "error": "Internal Server Error",
                    "message": message
                }),
            )
        }
    }
}

async fn delete_user_custom_exchange_rate_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(currency): Path<String>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let normalized_currency = currency.trim().to_uppercase();
    if normalized_currency.is_empty() {
        return invalid_request("currency is required");
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let base_currency = match get_statistics_user_default_currency(runtime.connection(), user_id) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match delete_user_custom_exchange_rate(
        runtime.connection(),
        user_id,
        &base_currency,
        &normalized_currency,
    ) {
        Ok(deleted) => success_result(StatusCode::OK, json!(deleted)),
        Err(error) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({
                "success": false,
                "error": "Internal Server Error",
                "message": error.to_string()
            }),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTimestampRange {
    start_time: i64,
    end_time: i64,
    start_date: Option<String>,
    end_date: Option<String>,
}

fn normalized_exchange_base_currency(
    query: &ExchangeRatesQuery,
    connection: &rusqlite::Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<String> {
    let requested = query
        .base_currency
        .as_deref()
        .or(query.base.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_uppercase);
    if let Some(value) = requested {
        return Ok(value);
    }
    get_statistics_user_default_currency(connection, user_id)
}

fn target_exchange_currencies(base_currency: &str) -> Vec<String> {
    TARGET_EXCHANGE_CURRENCIES
        .iter()
        .copied()
        .filter(|currency| *currency != base_currency)
        .map(str::to_string)
        .collect()
}

async fn fetch_exchange_rates_from_providers(
    base_currency: &str,
    target_currencies: &[String],
    requested_provider: &str,
) -> Option<(String, BTreeMap<String, f64>)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .ok()?;
    for provider_key in build_provider_candidate_order(requested_provider) {
        let rates = fetch_exchange_rates_from_provider(
            &client,
            &provider_key,
            base_currency,
            target_currencies,
        )
        .await;
        if let Some(rates) = rates.filter(|rates| !rates.is_empty()) {
            return Some((provider_key, rates));
        }
    }
    None
}

async fn fetch_exchange_rates_from_provider(
    client: &reqwest::Client,
    provider_key: &str,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    match provider_key {
        "boc_cn" => fetch_boc_china_rates(client, base_currency, target_currencies).await,
        "cmb_cn" => fetch_cmb_china_rates(client, base_currency, target_currencies).await,
        "ecb" => fetch_ecb_rates(client, base_currency, target_currencies).await,
        "rba" => fetch_rba_rates(client, base_currency, target_currencies).await,
        _ => None,
    }
}

async fn fetch_boc_china_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let html = fetch_text(client, "https://www.boc.cn/sourcedb/whpj/").await?;
    let quote_map = parse_boc_quote_map(&html);
    Some(convert_cny_quote_map_to_rates(
        &quote_map,
        base_currency,
        target_currencies,
    ))
}

async fn fetch_cmb_china_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let quote_map =
        if let Some(payload) = fetch_text(client, "https://fx.cmbchina.com/api/v1/fx/rate").await {
            serde_json::from_str::<Value>(&payload)
                .ok()
                .map(|value| parse_cmb_quote_map_from_api(&value))
                .unwrap_or_default()
        } else {
            BTreeMap::new()
        };
    let quote_map = if quote_map.is_empty() {
        fetch_text(client, "https://fx.cmbchina.com/hq/")
            .await
            .map(|html| parse_cmb_quote_map_from_html(&html))
            .unwrap_or_default()
    } else {
        quote_map
    };
    Some(convert_cny_quote_map_to_rates(
        &quote_map,
        base_currency,
        target_currencies,
    ))
}

async fn fetch_ecb_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let xml = fetch_text(
        client,
        "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml",
    )
    .await?;
    let mut rates = BTreeMap::from([("EUR".to_string(), 1.0)]);
    for fragment in xml.split("<Cube").skip(1) {
        let Some(currency) = xml_attr(fragment, "currency") else {
            continue;
        };
        let Some(rate) = xml_attr(fragment, "rate").and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        rates.insert(currency, rate);
    }
    Some(convert_provider_base_currency(
        &rates,
        "EUR",
        base_currency,
        target_currencies,
        "base_to_target",
    ))
}

async fn fetch_rba_rates(
    client: &reqwest::Client,
    base_currency: &str,
    target_currencies: &[String],
) -> Option<BTreeMap<String, f64>> {
    let xml = fetch_text(
        client,
        "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml",
    )
    .await?;
    let mut rates = BTreeMap::from([("AUD".to_string(), 1.0)]);
    for item in xml.split("<item").skip(1) {
        let Some(currency) = between(item, "<cb:targetCurrency>", "</cb:targetCurrency>") else {
            continue;
        };
        let Some(rate) =
            between(item, "<cb:value>", "</cb:value>").and_then(|value| value.parse::<f64>().ok())
        else {
            continue;
        };
        rates.insert(currency, rate);
    }
    Some(convert_provider_base_currency(
        &rates,
        "AUD",
        base_currency,
        target_currencies,
        "base_to_target",
    ))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    client
        .get(url)
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .text()
        .await
        .ok()
}

fn parse_boc_quote_map(html: &str) -> BTreeMap<String, f64> {
    let mut quote_map = BTreeMap::new();
    for cells in html_table_rows(html) {
        let Some(currency) = cells
            .first()
            .map(|value| normalize_chinese_currency_name(value))
        else {
            continue;
        };
        if currency.is_empty() || currency == "CNY" {
            continue;
        }
        let numeric_values = numeric_values_from_cells(&cells[1..]);
        if let Some(quote) = numeric_values.last().copied().filter(|value| *value > 0.0) {
            quote_map.insert(currency, quote);
        }
    }
    quote_map
}

fn parse_cmb_quote_map_from_api(payload: &Value) -> BTreeMap<String, f64> {
    let mut quote_map = BTreeMap::new();
    let Some(rows) = payload.get("body").and_then(Value::as_array) else {
        return quote_map;
    };
    for row in rows {
        let currency = cmb_currency_from_api_row(row);
        if currency.is_empty() || currency == "CNY" {
            continue;
        }
        let quotes = ["rthOfr", "rthBid", "rtcOfr", "rtcBid", "rtbBid"]
            .iter()
            .filter_map(|field| row.get(*field).and_then(json_number))
            .filter(|value| *value > 0.0)
            .collect::<Vec<_>>();
        if !quotes.is_empty() {
            quote_map.insert(currency, quotes.iter().sum::<f64>() / quotes.len() as f64);
        }
    }
    quote_map
}

fn parse_cmb_quote_map_from_html(html: &str) -> BTreeMap<String, f64> {
    let mut quote_map = BTreeMap::new();
    for cells in html_table_rows(html) {
        let Some(currency) = cells
            .first()
            .map(|value| normalize_chinese_currency_name(value))
        else {
            continue;
        };
        if currency.is_empty() || currency == "CNY" {
            continue;
        }
        let numeric_values = numeric_values_from_cells(&cells[1..]);
        let quote_candidates =
            if numeric_values.len() >= 5 && (numeric_values[0] - 100.0).abs() < 0.001 {
                numeric_values[1..5].to_vec()
            } else {
                numeric_values.iter().copied().take(4).collect()
            };
        let valid_quotes = quote_candidates
            .into_iter()
            .filter(|value| *value > 0.0)
            .collect::<Vec<_>>();
        if !valid_quotes.is_empty() {
            quote_map.insert(
                currency,
                valid_quotes.iter().sum::<f64>() / valid_quotes.len() as f64,
            );
        }
    }
    quote_map
}

fn cmb_currency_from_api_row(row: &Value) -> String {
    if let Some(text) = row.get("ccyNbrEng").and_then(Value::as_str) {
        for token in text.split_whitespace().rev() {
            let token = token.trim();
            if token.len() == 3 && token.chars().all(|ch| ch.is_ascii_uppercase()) {
                return token.to_string();
            }
        }
    }
    row.get("ccyNbr")
        .and_then(Value::as_str)
        .map(normalize_chinese_currency_name)
        .unwrap_or_default()
}

fn html_table_rows(html: &str) -> Vec<Vec<String>> {
    html.split("<tr")
        .skip(1)
        .filter_map(|row| row.split("</tr>").next())
        .map(|row| {
            let mut cells = html_cells(row, "td");
            if cells.is_empty() {
                cells = html_cells(row, "th");
            }
            cells
        })
        .filter(|cells| !cells.is_empty())
        .collect()
}

fn html_cells(row: &str, tag: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut rest = row;
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    while let Some(start) = rest.find(&open) {
        rest = &rest[start + open.len()..];
        let Some(content_start) = rest.find('>') else {
            break;
        };
        rest = &rest[content_start + 1..];
        let Some(content_end) = rest.find(&close) else {
            break;
        };
        cells.push(clean_html_cell(&rest[..content_end]));
        rest = &rest[content_end + close.len()..];
    }
    cells
}

fn clean_html_cell(value: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn numeric_values_from_cells(cells: &[String]) -> Vec<f64> {
    cells
        .iter()
        .filter_map(|value| {
            let text = value.trim();
            if text.is_empty() || matches!(text, "-" | "--" | "nan" | "NaN") || text.contains(':') {
                return None;
            }
            let normalized = text.replace(',', "");
            if normalized
                .chars()
                .all(|ch| ch.is_ascii_digit() || matches!(ch, '-' | '.'))
            {
                normalized.parse::<f64>().ok()
            } else {
                None
            }
        })
        .collect()
}

fn xml_attr(fragment: &str, attr: &str) -> Option<String> {
    let double = format!("{attr}=\"");
    let single = format!("{attr}='");
    if let Some(start) = fragment.find(&double) {
        let value = &fragment[start + double.len()..];
        return value.split('"').next().map(ToString::to_string);
    }
    let start = fragment.find(&single)?;
    let value = &fragment[start + single.len()..];
    value.split('\'').next().map(ToString::to_string)
}

fn between(value: &str, start_marker: &str, end_marker: &str) -> Option<String> {
    let start = value.find(start_marker)? + start_marker.len();
    let rest = &value[start..];
    let end = rest.find(end_marker)?;
    Some(rest[..end].trim().to_string())
}

fn json_number(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.replace(',', "").trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn is_missing_json_value(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(_) => false,
    }
}

fn parse_exchange_rate_value(value: Option<&Value>) -> Result<f64, &'static str> {
    json_number(value.ok_or("currency and rate are required")?).ok_or("rate must be numeric")
}

fn invalid_request(message: &str) -> Response {
    json_response(
        StatusCode::BAD_REQUEST,
        json!({
            "success": false,
            "error": "Invalid request",
            "message": message
        }),
    )
}

fn format_route_rate_value(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
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

fn analyzer_period(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("month")
        .to_string()
}
