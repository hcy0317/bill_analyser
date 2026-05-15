use std::collections::BTreeMap;

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use bill_analyser_core::statistics::parse_transaction_amount_period_query;
use bill_analyser_db::{
    query_asset_trends_payload, query_category_pie_payload, query_category_statistics_payload,
    query_category_trends_payload, query_top_merchants_payload, query_transaction_amount_period,
    StatisticsBillFilters,
};
use serde_json::json;

use crate::state::HttpAppState;

use super::{
    query::{
        asset_date_range_from_query, bill_filters_for_timestamp_range, date_from_timestamp,
        non_empty_string, timestamp_range_from_query, year_month_range_from_query,
        BasicStatisticsQuery, CategoryStatisticsQuery, CategoryTrendsQuery,
    },
    response::{
        bad_request, db_error_response, internal_error, open_runtime, success_data, success_result,
        user_id_from_headers,
    },
};
pub(super) async fn category_statistics_handler(
    State(state): State<HttpAppState>,
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

pub(super) async fn category_trends_handler(
    State(state): State<HttpAppState>,
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

pub(super) async fn asset_trends_handler(
    State(state): State<HttpAppState>,
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

pub(super) async fn category_pie_handler(
    State(state): State<HttpAppState>,
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

pub(super) async fn top_merchants_handler(
    State(state): State<HttpAppState>,
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

pub(super) async fn transaction_amounts_handler(
    State(state): State<HttpAppState>,
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
