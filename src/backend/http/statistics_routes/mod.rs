use axum::{
    routing::{delete, get, put},
    Router,
};

use crate::state::HttpAppState;

mod analyzer_handlers;
mod exchange_handlers;
mod exchange_providers;
mod query;
mod read_handlers;
mod response;

use analyzer_handlers::{
    analyzer_category_handler, analyzer_comparison_handler, analyzer_overview_handler,
    analyzer_trend_handler, analyzer_trends_handler, insights_anomalies_handler,
};
use exchange_handlers::{
    delete_user_custom_exchange_rate_handler, exchange_rates_handler,
    update_user_custom_exchange_rate_handler,
};
use read_handlers::{
    asset_trends_handler, category_pie_handler, category_statistics_handler,
    category_trends_handler, top_merchants_handler, transaction_amounts_handler,
};

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
    ("GET", "/api/insights/anomalies"),
    ("GET", "/api/statistics/exchange-rates"),
    ("PUT", "/api/statistics/exchange-rates/custom"),
    ("DELETE", "/api/statistics/exchange-rates/custom/{currency}"),
];

pub fn statistics_runtime_router() -> Router<HttpAppState> {
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
        .route("/api/insights/anomalies", get(insights_anomalies_handler))
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
