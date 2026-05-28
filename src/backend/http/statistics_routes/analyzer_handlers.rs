// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use bill_analyser_core::statistics::{
    build_overview_result_from_report, build_statistics_trend_response,
};
use bill_analyser_db::{
    query_insight_anomaly_summary_payload, query_postgres_insight_anomaly_summary_payload,
    query_postgres_statistics_analyzer_category_payload,
    query_postgres_statistics_analyzer_comparison_payload,
    query_postgres_statistics_analyzer_report_payload,
    query_postgres_statistics_analyzer_trends_payload, query_statistics_analyzer_category_payload,
    query_statistics_analyzer_comparison_payload, query_statistics_analyzer_report_payload,
    query_statistics_analyzer_trends_payload,
};
use chrono::Local;

use crate::state::HttpAppState;

use super::{
    query::{
        analyzer_period, insights_analyzed_months, AnalyzerStatisticsQuery, InsightsAnomaliesQuery,
    },
    response::{
        db_error_response, insights_error, json_response, open_postgres_runtime, open_runtime,
        success_data, success_result, user_id_from_headers,
    },
};
#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn analyzer_overview_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "analyzer_overview_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let period = analyzer_period(query.period.as_deref());
        return match query_postgres_statistics_analyzer_report_payload(
            runtime.pool(),
            user_id,
            &period,
        )
        .await
        {
            Ok(report) => {
                success_result(StatusCode::OK, build_overview_result_from_report(&report))
            }
            Err(_) => db_error_response(),
        };
    }
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn analyzer_trends_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "analyzer_trends_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let period = analyzer_period(query.period.as_deref());
        return match query_postgres_statistics_analyzer_trends_payload(
            runtime.pool(),
            user_id,
            &period,
            query.category.as_deref(),
        )
        .await
        {
            Ok(result) => success_result(StatusCode::OK, result),
            Err(_) => db_error_response(),
        };
    }
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn analyzer_comparison_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "analyzer_comparison_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
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
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_statistics_analyzer_comparison_payload(
            runtime.pool(),
            user_id,
            &period,
            &compare_type,
        )
        .await
        {
            Ok(result) => success_result(StatusCode::OK, result),
            Err(_) => db_error_response(),
        };
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn analyzer_category_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "analyzer_category_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let period = analyzer_period(query.period.as_deref());
        return match query_postgres_statistics_analyzer_category_payload(
            runtime.pool(),
            user_id,
            &period,
            query.main_category.as_deref(),
        )
        .await
        {
            Ok(result) => success_data(StatusCode::OK, result),
            Err(_) => db_error_response(),
        };
    }
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn analyzer_trend_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<AnalyzerStatisticsQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "analyzer_trend_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let period = analyzer_period(query.granularity.as_deref());
        return match query_postgres_statistics_analyzer_trends_payload(
            runtime.pool(),
            user_id,
            &period,
            query.category.as_deref(),
        )
        .await
        {
            Ok(result) => json_response(StatusCode::OK, build_statistics_trend_response(&result)),
            Err(_) => db_error_response(),
        };
    }
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

#[tracing::instrument(level = "debug", skip_all)]
pub(super) async fn insights_anomalies_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<InsightsAnomaliesQuery>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "insights_anomalies_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let analyzed_months = match insights_analyzed_months(query.months.as_deref()) {
        Ok(value) => value,
        Err(message) => return insights_error(message),
    };
    if state.config.database_backend.uses_postgres() {
        let runtime = match open_postgres_runtime(&state) {
            Ok(value) => value,
            Err(response) => return *response,
        };
        return match query_postgres_insight_anomaly_summary_payload(
            runtime.pool(),
            user_id,
            analyzed_months,
            Local::now().date_naive(),
        )
        .await
        {
            Ok(payload) => json_response(StatusCode::OK, payload),
            Err(_) => db_error_response(),
        };
    }
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match query_insight_anomaly_summary_payload(
        runtime.connection(),
        user_id,
        analyzed_months,
        Local::now().date_naive(),
    ) {
        Ok(payload) => json_response(StatusCode::OK, payload),
        Err(_) => db_error_response(),
    }
}
