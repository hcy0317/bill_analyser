// 中文导读：PostgreSQL statistics 仓储层，负责统计、分析器、日历、净值与用户汇率读写。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑。

use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::statistics::{
    build_asset_trend_legend, build_asset_trends, build_calendar_events_data,
    build_category_pie_data, build_category_statistics_items, build_category_trend_statistics,
    build_insight_anomaly_summary, build_net_worth_snapshot,
    build_statistics_analyzer_category_result, build_statistics_analyzer_comparison_result,
    build_statistics_analyzer_report, build_statistics_analyzer_trend_bucket,
    build_statistics_analyzer_trends_result, build_top_merchants_data,
    statistics_analyzer_period_range, CalendarEventsData, NameValueStatisticItem,
    RecurringRuleInput, StatisticsAccountInput, StatisticsAnalyzerTrendBucket, StatisticsBillInput,
    StatisticsCategoryInput, StatisticsYearMonthRange, TopMerchantStatisticItem,
    TransactionAmountBucket, TransactionAmountPeriodResult, UserCustomExchangeRateInput,
};
use bill_analyser_core::UserId;
use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder, Row};

use crate::{DbError, DbResult, PostgresPool, UserScope};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatisticsBillFilters {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub transaction_type: Option<String>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatisticsAllDateRange {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserCustomExchangeRateUpsert {
    pub update_time: i64,
}

include!("statistics/category.rs");
include!("statistics/asset_analyzer.rs");
include!("statistics/exchange.rs");
include!("statistics/ranges.rs");
include!("statistics/loaders.rs");
include!("statistics/helpers.rs");
