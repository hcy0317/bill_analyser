// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额字段在核心和 API payload 中使用显式 cents/minor units；元单位只用于展示文本。

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER: [&str; 4] = ["boc_cn", "cmb_cn", "ecb", "rba"];
pub const TARGET_EXCHANGE_CURRENCIES: [&str; 16] = [
    "USD", "EUR", "GBP", "JPY", "HKD", "KRW", "AUD", "CAD", "SGD", "TWD", "MYR", "THB", "VND",
    "CHF", "NZD", "CNY",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatisticsContractError {
    pub error: String,
    pub message: String,
}

impl StatisticsContractError {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new(error: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StatisticsTimestampRange {
    All,
    Bounded { start_time: i64, end_time: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsYearMonthRange {
    pub start_year: i32,
    pub start_month: u32,
    pub end_year: i32,
    pub end_month: u32,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StatisticsYearMonthRangeMode {
    All,
    Bounded(StatisticsYearMonthRange),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatisticsBillInput {
    pub id: Option<i64>,
    pub date: String,
    pub bill_type: String,
    pub amount_cents: i64,
    pub channel: String,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub destination_account: String,
    pub destination_amount_cents: Option<i64>,
    pub main_category: String,
    pub sub_category: String,
    pub counterparty: String,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatisticsCategoryInput {
    pub id: i64,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatisticsAccountInput {
    pub id: i64,
    pub name: String,
    pub account_type: String,
    pub hidden: bool,
    pub balance_cents: i64,
    pub initial_balance_cents: i64,
    pub currency: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryStatisticItem {
    pub category_id: String,
    pub account_id: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryTrendBucket {
    pub year: i32,
    pub month: u32,
    pub items: Vec<CategoryStatisticItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetTrendAccountItem {
    pub account_id: String,
    pub account_opening_balance_cents: i64,
    pub account_closing_balance_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTrendDay {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub items: Vec<AssetTrendAccountItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTrendLegendItem {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthAccountEntry {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub icon: Option<String>,
    pub balance_cents: i64,
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthSnapshot {
    pub assets: Vec<NetWorthAccountEntry>,
    pub liabilities: Vec<NetWorthAccountEntry>,
    pub total_assets_cents: i64,
    pub total_liabilities_cents: i64,
    pub net_worth_cents: i64,
    pub account_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecurringRuleInput {
    pub id: Option<i64>,
    pub name: String,
    pub amount_cents: i64,
    pub bill_type: String,
    pub frequency: String,
    pub next_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarBillItem {
    pub id: Option<i64>,
    pub amount_cents: i64,
    #[serde(rename = "type")]
    pub bill_type: String,
    pub counterparty: String,
    pub description: String,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventDay {
    pub date: String,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub transfer_in_cents: i64,
    pub transfer_out_cents: i64,
    pub net_cents: i64,
    pub count: usize,
    pub bills: Vec<CalendarBillItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarRecurringProjection {
    pub date: String,
    #[serde(rename = "type")]
    pub projection_type: String,
    pub name: String,
    pub amount_cents: i64,
    pub bill_type: String,
    pub frequency: String,
    pub recurring_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventsData {
    pub events: Vec<CalendarEventDay>,
    pub recurring_projections: Vec<CalendarRecurringProjection>,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRateProviderOption {
    pub label: String,
    pub reference_url: String,
    pub region: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExchangeRateItem {
    pub currency: String,
    pub rate: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRatesResult {
    pub provider_key: String,
    pub requested_provider: String,
    pub fallback_used: bool,
    pub data_source: String,
    pub reference_url: String,
    pub update_time: i64,
    pub base_currency: String,
    pub exchange_rates: Vec<ExchangeRateItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserCustomExchangeRateInput {
    pub to_currency: String,
    pub rate: String,
    pub effective_date: Option<String>,
    pub effective_timestamp: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NameValueStatisticItem {
    pub name: String,
    pub value_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopMerchantStatisticItem {
    pub name: String,
    pub amount_cents: i64,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAmountBucket {
    pub currency: String,
    pub income_amount_cents: i64,
    pub expense_amount_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAmountPeriodResult {
    pub start_time: i64,
    pub end_time: i64,
    pub amounts: Vec<TransactionAmountBucket>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsTrendPoint {
    pub date: String,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub net_cents: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatisticsAnalyzerPeriodRange {
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsAnalyzerTrendBucket {
    pub period: String,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub net_cents: i64,
}

include!("statistics/ranges.rs");
include!("statistics/category.rs");
include!("statistics/asset.rs");
include!("statistics/anomalies.rs");
include!("statistics/calendar.rs");
include!("statistics/breakdown.rs");
include!("statistics/analyzer.rs");
include!("statistics/exchange.rs");
include!("statistics/misc_public.rs");
include!("statistics/analyzer_helpers.rs");
include!("statistics/date_helpers.rs");
include!("statistics/insight_helpers.rs");
include!("statistics/calendar_helpers.rs");
include!("statistics/exchange_helpers.rs");
include!("statistics/value_helpers.rs");
