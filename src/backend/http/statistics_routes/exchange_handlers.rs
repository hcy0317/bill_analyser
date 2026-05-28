// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    Json,
};
use bill_analyser_core::{
    statistics::{
        build_builtin_fallback_exchange_rates, build_provider_exchange_rates_result,
        build_user_custom_exchange_rates_result, exchange_rate_provider_options,
        normalize_requested_exchange_rate_provider, ExchangeRatesResult,
    },
    UserId,
};
use bill_analyser_db::{
    delete_postgres_user_custom_exchange_rate, delete_user_custom_exchange_rate,
    get_postgres_statistics_user_default_currency, get_statistics_user_default_currency,
    list_postgres_user_custom_exchange_rates, list_user_custom_exchange_rates,
    upsert_postgres_user_custom_exchange_rate, upsert_user_custom_exchange_rate,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use crate::state::HttpAppState;

use super::{
    exchange_providers::{
        fetch_exchange_rates_from_providers, json_number, target_exchange_currencies,
    },
    query::{ExchangeRatesQuery, UserCustomExchangeRateRequest},
    response::{
        db_error_response, json_response, open_postgres_runtime, open_runtime, success_result,
        user_id_from_headers,
    },
};
#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn exchange_rates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<ExchangeRatesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "exchange_rates_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
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

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let base_currency =
            match normalized_postgres_exchange_base_currency(&query, runtime.pool(), user_id).await
            {
                Ok(value) => value,
                Err(_) => return db_error_response(),
            };
        let custom_rates =
            match list_postgres_user_custom_exchange_rates(runtime.pool(), user_id, &base_currency)
                .await
            {
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
        let result =
            build_exchange_rates_result(&base_currency, &requested_provider, provider_result, now);
        return success_result(StatusCode::OK, json!(result));
    }

    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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
    let result =
        build_exchange_rates_result(&base_currency, &requested_provider, provider_result, now);
    success_result(StatusCode::OK, json!(result))
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_exchange_rates_result(
    base_currency: &str,
    requested_provider: &str,
    provider_result: Option<(String, BTreeMap<String, f64>)>,
    now: i64,
) -> ExchangeRatesResult {
    match provider_result {
        Some((provider_key, rates)) if !rates.is_empty() => build_provider_exchange_rates_result(
            base_currency,
            requested_provider,
            &provider_key,
            &rates,
            now,
        ),
        _ => build_builtin_fallback_exchange_rates(base_currency, now),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn update_user_custom_exchange_rate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(body): Json<UserCustomExchangeRateRequest>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "update_user_custom_exchange_rate_handler",
        "business operation entered"
    );
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

    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let base_currency =
            match get_postgres_statistics_user_default_currency(runtime.pool(), user_id).await {
                Ok(value) => value,
                Err(_) => return db_error_response(),
            };
        return match upsert_postgres_user_custom_exchange_rate(
            runtime.pool(),
            user_id,
            &base_currency,
            &currency,
            rate,
        )
        .await
        {
            Ok(result) => success_result(
                StatusCode::OK,
                json!({
                    "currency": currency,
                    "rate": format_route_rate_value(rate),
                    "updateTime": result.update_time
                }),
            ),
            Err(error) => json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "success": false,
                    "error": "Internal Server Error",
                    "message": error.to_string()
                }),
            ),
        };
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn delete_user_custom_exchange_rate_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(currency): Path<String>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "delete_user_custom_exchange_rate_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let normalized_currency = currency.trim().to_uppercase();
    if normalized_currency.is_empty() {
        return invalid_request("currency is required");
    }
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let base_currency =
            match get_postgres_statistics_user_default_currency(runtime.pool(), user_id).await {
                Ok(value) => value,
                Err(_) => return db_error_response(),
            };
        return match delete_postgres_user_custom_exchange_rate(
            runtime.pool(),
            user_id,
            &base_currency,
            &normalized_currency,
        )
        .await
        {
            Ok(deleted) => success_result(StatusCode::OK, json!(deleted)),
            Err(error) => json_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({
                    "success": false,
                    "error": "Internal Server Error",
                    "message": error.to_string()
                }),
            ),
        };
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

async fn normalized_postgres_exchange_base_currency(
    query: &ExchangeRatesQuery,
    pool: &bill_analyser_db::PostgresPool,
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
    get_postgres_statistics_user_default_currency(pool, user_id).await
}

fn is_missing_json_value(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(_) => false,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn builds_provider_and_fallback_exchange_rate_results_without_network() {
        let provider_result = build_exchange_rates_result(
            "CNY",
            "boc_cn",
            Some((
                "boc_cn".to_string(),
                BTreeMap::from([("USD".to_string(), 0.14)]),
            )),
            123,
        );
        assert_eq!(provider_result.provider_key, "boc_cn");
        assert_eq!(provider_result.requested_provider, "boc_cn");
        assert!(!provider_result.fallback_used);
        assert!(provider_result
            .exchange_rates
            .iter()
            .any(|rate| rate.currency == "USD" && rate.rate == "0.14"));

        let fallback_result = build_exchange_rates_result("USD", "auto", None, 456);
        assert_eq!(fallback_result.provider_key, "fallback");
        assert!(fallback_result.fallback_used);
        assert_eq!(fallback_result.base_currency, "USD");
        assert!(fallback_result
            .exchange_rates
            .iter()
            .any(|rate| rate.currency == "CNY"));
    }

    #[test]
    fn validates_and_formats_custom_exchange_rate_inputs() {
        assert!(is_missing_json_value(None));
        assert!(is_missing_json_value(Some(&Value::Null)));
        assert!(is_missing_json_value(Some(&json!("  "))));
        assert!(!is_missing_json_value(Some(&json!(7.2))));

        assert_eq!(
            parse_exchange_rate_value(Some(&json!("1,234.50"))).expect("numeric string"),
            1234.5
        );
        assert_eq!(
            parse_exchange_rate_value(Some(&json!("not-a-number"))),
            Err("rate must be numeric")
        );
        assert_eq!(
            parse_exchange_rate_value(None),
            Err("currency and rate are required")
        );

        assert_eq!(format_route_rate_value(2.0), "2.0");
        assert_eq!(format_route_rate_value(8.5), "8.5");
    }

    #[test]
    fn resolves_exchange_base_currency_from_query_before_user_default() {
        let connection = rusqlite::Connection::open_in_memory().expect("connection");
        connection
            .execute_batch(
                "
                CREATE TABLE users(
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL,
                    default_currency TEXT DEFAULT 'CNY'
                );
                INSERT INTO users(id, username, default_currency)
                VALUES (42, 'owner', 'USD');
                ",
            )
            .expect("schema");
        let user_id = UserId::new(42).expect("user id");

        let query = ExchangeRatesQuery {
            base: Some(" eur ".to_string()),
            ..ExchangeRatesQuery::default()
        };
        assert_eq!(
            normalized_exchange_base_currency(&query, &connection, user_id).expect("query base"),
            "EUR"
        );

        assert_eq!(
            normalized_exchange_base_currency(&ExchangeRatesQuery::default(), &connection, user_id)
                .expect("user default"),
            "USD"
        );
    }
}
