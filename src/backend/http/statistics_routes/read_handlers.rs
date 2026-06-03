// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use std::collections::BTreeMap;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use bill_analyser_core::statistics::parse_transaction_amount_period_query;
use bill_analyser_db::{
    find_postgres_statistics_all_date_range, query_postgres_asset_trends_payload,
    query_postgres_category_pie_payload, query_postgres_category_statistics_payload,
    query_postgres_category_trends_payload, query_postgres_top_merchants_payload,
    query_postgres_transaction_amount_period, StatisticsBillFilters,
};
use serde_json::json;

use crate::state::HttpAppState;

use super::{
    query::{
        asset_date_range_from_all_range, asset_date_range_from_timestamp_range,
        bill_filters_for_timestamp_range, date_from_timestamp, default_year_month_query_values,
        non_empty_string, timestamp_range_from_query, year_month_range_from_values,
        BasicStatisticsQuery, CategoryStatisticsQuery, CategoryTrendsQuery,
    },
    response::{
        bad_request, db_error_response, internal_error, open_postgres_runtime, success_data,
        success_result, user_id_from_headers,
    },
};
#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn category_statistics_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "category_statistics_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state) {
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
    return match query_postgres_category_statistics_payload(runtime.pool(), user_id, &filters).await
    {
        Ok(items) => success_result(
            StatusCode::OK,
            json!({
                "startTime": range.start_time,
                "endTime": range.end_time,
                "items": items
            }),
        ),
        Err(_) => db_error_response(),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn category_trends_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryTrendsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "category_trends_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let (start_raw, end_raw) = default_year_month_query_values(
        query
            .start_year_month_camel
            .as_deref()
            .or(query.start_year_month.as_deref()),
        query
            .end_year_month_camel
            .as_deref()
            .or(query.end_year_month.as_deref()),
    );
    let all_range = if start_raw.trim() == "0" && end_raw.trim() == "0" {
        match find_postgres_statistics_all_date_range(runtime.pool(), user_id).await {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        }
    } else {
        None
    };
    let range = match year_month_range_from_values(&start_raw, &end_raw, all_range) {
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
    return match query_postgres_category_trends_payload(runtime.pool(), user_id, &filters, &range)
        .await
    {
        Ok(items) => success_result(StatusCode::OK, items),
        Err(_) => db_error_response(),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn asset_trends_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "asset_trends_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let range = if matches!(
        (
            query
                .start_time_camel
                .as_deref()
                .or(query.start_time.as_deref()),
            query
                .end_time_camel
                .as_deref()
                .or(query.end_time.as_deref())
        ),
        (Some("0"), Some("0"))
    ) {
        match find_postgres_statistics_all_date_range(runtime.pool(), user_id).await {
            Ok(value) => match asset_date_range_from_all_range(value) {
                Ok(value) => value,
                Err(response) => return *response,
            },
            Err(_) => return db_error_response(),
        }
    } else {
        let timestamp_range = match timestamp_range_from_query(
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
        match asset_date_range_from_timestamp_range(timestamp_range) {
            Ok(value) => value,
            Err(response) => return *response,
        }
    };
    let Some((start_date, end_date)) = range else {
        return success_result(StatusCode::OK, json!([]));
    };
    return match query_postgres_asset_trends_payload(runtime.pool(), user_id, start_date, end_date)
        .await
    {
        Ok(payload) => success_result(
            StatusCode::OK,
            payload.get("items").cloned().unwrap_or_else(|| json!([])),
        ),
        Err(_) => db_error_response(),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn category_pie_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BasicStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "category_pie_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
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

    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_category_pie_payload(runtime.pool(), user_id, &filters).await {
        Ok(items) => success_data(StatusCode::OK, json!(items)),
        Err(_) => db_error_response(),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn top_merchants_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BasicStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "top_merchants_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
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

    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    return match query_postgres_top_merchants_payload(runtime.pool(), user_id, &filters, limit)
        .await
    {
        Ok(items) => success_data(StatusCode::OK, json!(items)),
        Err(_) => db_error_response(),
    };
}

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn transaction_amounts_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BasicStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "transaction_amounts_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let query_text = non_empty_string(query.query.as_ref())
        .or_else(|| non_empty_string(query.periods.as_ref()))
        .unwrap_or_default();
    if query_text.is_empty() {
        return bad_request("Missing query parameter");
    }

    let runtime = match open_postgres_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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
        let result = match query_postgres_transaction_amount_period(
            runtime.pool(),
            user_id,
            start_time,
            end_time,
            start_date.to_string(),
            end_date.to_string(),
        )
        .await
        {
            Ok(value) => value,
            Err(_) => return db_error_response(),
        };
        results.insert(period_name, result);
    }
    return success_result(StatusCode::OK, json!(results));
}
