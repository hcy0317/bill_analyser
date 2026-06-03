// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

use bill_analyser_core::statistics::{
    parse_statistics_timestamp_range, parse_statistics_year_month_range,
    validate_asset_trends_span, StatisticsTimestampRange, StatisticsYearMonthRange,
    StatisticsYearMonthRangeMode,
};
use bill_analyser_db::{StatisticsAllDateRange, StatisticsBillFilters};
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use serde::Deserialize;
use serde_json::Value;

use super::response::{
    asset_trends_error_response, bad_request, statistics_error_response, RouteResult,
};
#[derive(Debug, Default, Deserialize)]
pub(super) struct CategoryStatisticsQuery {
    #[serde(rename = "startTime")]
    pub(super) start_time_camel: Option<String>,
    #[serde(rename = "endTime")]
    pub(super) end_time_camel: Option<String>,
    pub(super) start_time: Option<String>,
    pub(super) end_time: Option<String>,
    pub(super) keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct CategoryTrendsQuery {
    #[serde(rename = "startYearMonth")]
    pub(super) start_year_month_camel: Option<String>,
    #[serde(rename = "endYearMonth")]
    pub(super) end_year_month_camel: Option<String>,
    pub(super) start_year_month: Option<String>,
    pub(super) end_year_month: Option<String>,
    pub(super) keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct BasicStatisticsQuery {
    #[serde(rename = "type")]
    pub(super) transaction_type: Option<String>,
    pub(super) start_date: Option<String>,
    pub(super) end_date: Option<String>,
    pub(super) limit: Option<String>,
    pub(super) query: Option<String>,
    pub(super) periods: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct AnalyzerStatisticsQuery {
    pub(super) period: Option<String>,
    pub(super) category: Option<String>,
    #[serde(rename = "type")]
    pub(super) compare_type: Option<String>,
    pub(super) main_category: Option<String>,
    pub(super) granularity: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct InsightsAnomaliesQuery {
    pub(super) months: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct ExchangeRatesQuery {
    pub(super) base_currency: Option<String>,
    pub(super) base: Option<String>,
    pub(super) provider: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct UserCustomExchangeRateRequest {
    pub(super) currency: Option<String>,
    pub(super) rate: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ResolvedTimestampRange {
    pub(super) start_time: i64,
    pub(super) end_time: i64,
    pub(super) start_date: Option<String>,
    pub(super) end_date: Option<String>,
}

pub(super) fn timestamp_range_from_query(
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

pub(super) fn year_month_range_from_values(
    start_raw: &str,
    end_raw: &str,
    all_range: Option<StatisticsAllDateRange>,
) -> RouteResult<Option<StatisticsYearMonthRange>> {
    match parse_statistics_year_month_range(Some(start_raw), Some(end_raw)) {
        Ok(StatisticsYearMonthRangeMode::Bounded(range)) => Ok(Some(range)),
        Ok(StatisticsYearMonthRangeMode::All) => {
            let Some(range) = all_range else {
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

pub(super) fn default_year_month_query_values(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
) -> (String, String) {
    match (start_raw, end_raw) {
        (Some(start), Some(end)) => (start.to_string(), end.to_string()),
        _ => {
            let now = Local::now();
            (format!("{}01", now.year()), format!("{}12", now.year()))
        }
    }
}

pub(super) fn asset_date_range_from_all_range(
    all_range: Option<StatisticsAllDateRange>,
) -> RouteResult<Option<(NaiveDate, NaiveDate)>> {
    let Some(range) = all_range else {
        return Ok(None);
    };
    Ok(Some((
        parse_date_prefix(&range.start_date)?,
        parse_date_prefix(&range.end_date)?,
    )))
}

pub(super) fn asset_date_range_from_timestamp_range(
    range: ResolvedTimestampRange,
) -> RouteResult<Option<(NaiveDate, NaiveDate)>> {
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

pub(super) fn bill_filters_for_timestamp_range(
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

pub(super) fn parse_date_prefix(value: &str) -> RouteResult<NaiveDate> {
    NaiveDate::parse_from_str(
        value
            .get(..10)
            .ok_or_else(|| Box::new(bad_request("Invalid timestamp format: invalid date")))?,
        "%Y-%m-%d",
    )
    .map_err(|_| Box::new(bad_request("Invalid timestamp format: invalid date")))
}

pub(super) fn current_month_timestamp_range() -> (i64, i64) {
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

pub(super) fn date_from_timestamp(value: i64) -> Option<NaiveDate> {
    Local
        .timestamp_opt(value, 0)
        .single()
        .map(|datetime| datetime.date_naive())
}
pub(super) fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn analyzer_period(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("month")
        .to_string()
}

pub(super) fn insights_analyzed_months(value: Option<&str>) -> Result<u32, String> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(6);
    };
    raw.parse::<u32>()
        .map(|months| months.min(24))
        .map_err(|_| format!("invalid literal for int() with base 10: '{raw}'"))
}
