// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use std::{fmt, str::FromStr};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ErrorCode, RuntimeError};

const NORMALIZED_BILL_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const EXCEL_SERIAL_UNIX_EPOCH_DAYS: i64 = 25_569;
const EXCEL_SERIAL_DATE_MIN_DAYS: i64 = 20_000;
const EXCEL_SERIAL_DATE_MAX_DAYS: i64 = 80_000;
const SECONDS_PER_DAY: f64 = 86_400.0;
const BILL_DATE_TIME_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%dT%H:%M:%S",
    "%Y/%m/%d %H:%M:%S",
    "%Y/%m/%d %H:%M",
    "%Y.%m.%d %H:%M:%S",
    "%Y.%m.%d %H:%M",
    "%Y年%m月%d日 %H:%M:%S",
    "%Y年%m月%d日 %H:%M",
];
const BILL_DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%Y/%m/%d", "%Y.%m.%d", "%Y年%m月%d日"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BillDateTime {
    value: NaiveDateTime,
}

impl BillDateTime {
    pub const fn new(value: NaiveDateTime) -> Self {
        Self { value }
    }

    pub const fn inner(self) -> NaiveDateTime {
        self.value
    }

    pub fn normalize(self) -> String {
        self.value.format(NORMALIZED_BILL_DATE_FORMAT).to_string()
    }

    pub fn display_day_of_week(self) -> u32 {
        self.value.weekday().num_days_from_sunday() + 1
    }

    pub fn day_of_month(self) -> u32 {
        self.value.day()
    }
}

impl fmt::Display for BillDateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.normalize())
    }
}

impl FromStr for BillDateTime {
    type Err = RuntimeError;

    fn from_str(raw_value: &str) -> Result<Self, Self::Err> {
        parse_bill_datetime(raw_value)
            .ok_or_else(|| RuntimeError::new(ErrorCode::InvalidInput, "invalid bill date time"))
    }
}

impl Serialize for BillDateTime {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.normalize())
    }
}

impl<'de> Deserialize<'de> for BillDateTime {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

pub fn parse_bill_datetime(raw_value: &str) -> Option<BillDateTime> {
    let text = raw_value.trim();
    if text.is_empty() {
        return None;
    }

    let iso_prefix: String = text.chars().take(19).collect();
    for format in BILL_DATE_TIME_FORMATS {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(&iso_prefix, format) {
            return Some(BillDateTime::new(parsed));
        }
    }

    for format in BILL_DATE_TIME_FORMATS {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(text, format) {
            return Some(BillDateTime::new(parsed));
        }
    }

    for format in BILL_DATE_FORMATS {
        if let Ok(parsed_date) = NaiveDate::parse_from_str(text, format) {
            return parsed_date.and_hms_opt(0, 0, 0).map(BillDateTime::new);
        }
    }

    if let Some(parsed) = parse_excel_serial_bill_datetime(text) {
        return Some(parsed);
    }

    None
}

pub fn normalize_bill_date_text(raw_value: &str) -> String {
    parse_bill_datetime(raw_value)
        .map(BillDateTime::normalize)
        .unwrap_or_else(|| raw_value.trim().to_string())
}

fn parse_excel_serial_bill_datetime(text: &str) -> Option<BillDateTime> {
    if let Some((date_token, fraction_token)) = split_date_and_fractional_time(text) {
        let date = parse_textual_bill_date(date_token)
            .or_else(|| parse_integral_excel_serial_bill_date(date_token))?;
        let fraction = parse_fractional_day(fraction_token)?;
        return combine_date_and_fractional_day(date, fraction);
    }

    let serial_days = text.parse::<f64>().ok()?;
    if !serial_days.is_finite() {
        return None;
    }
    let date_days = serial_days.floor();
    let date = excel_serial_days_to_date(date_days)?;
    let fraction = serial_days - date_days;
    combine_date_and_fractional_day(date, fraction)
}

fn split_date_and_fractional_time(text: &str) -> Option<(&str, &str)> {
    let mut parts = text.split_whitespace();
    let date_token = parts.next()?;
    let fraction_token = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((date_token, fraction_token))
}

fn parse_textual_bill_date(text: &str) -> Option<NaiveDate> {
    BILL_DATE_FORMATS
        .iter()
        .find_map(|format| NaiveDate::parse_from_str(text, format).ok())
}

fn parse_integral_excel_serial_bill_date(text: &str) -> Option<NaiveDate> {
    let serial_days = text.parse::<f64>().ok()?;
    if !serial_days.is_finite() || serial_days.fract().abs() > f64::EPSILON {
        return None;
    }
    excel_serial_days_to_date(serial_days)
}

fn excel_serial_days_to_date(serial_days: f64) -> Option<NaiveDate> {
    if !serial_days.is_finite() {
        return None;
    }
    let date_days = serial_days.floor();
    if date_days < EXCEL_SERIAL_DATE_MIN_DAYS as f64
        || date_days > EXCEL_SERIAL_DATE_MAX_DAYS as f64
    {
        return None;
    }
    let offset_days = date_days as i64 - EXCEL_SERIAL_UNIX_EPOCH_DAYS;
    NaiveDate::from_ymd_opt(1970, 1, 1)?.checked_add_signed(Duration::days(offset_days))
}

fn parse_fractional_day(text: &str) -> Option<f64> {
    let fraction = text.parse::<f64>().ok()?;
    if fraction.is_finite() && (0.0..1.0).contains(&fraction) {
        Some(fraction)
    } else {
        None
    }
}

fn combine_date_and_fractional_day(date: NaiveDate, fraction: f64) -> Option<BillDateTime> {
    let seconds = (fraction * SECONDS_PER_DAY).round() as i64;
    let date_time = date
        .and_hms_opt(0, 0, 0)?
        .checked_add_signed(Duration::seconds(seconds))?;
    Some(BillDateTime::new(date_time))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixTimestampSeconds {
    seconds: i64,
}

impl UnixTimestampSeconds {
    pub const fn new(seconds: i64) -> Self {
        Self { seconds }
    }

    pub fn from_frontend_value(raw_value: impl ToString) -> Self {
        let text = raw_value.to_string();
        let text = text.trim();
        if text.is_empty() {
            return Self::new(0);
        }
        let Ok(mut value) = text.parse::<i128>() else {
            return Self::new(0);
        };
        if value >= 100_000_000_000 || value <= -100_000_000_000 {
            value /= 1000;
        }
        Self::new(i64::try_from(value).unwrap_or(0))
    }

    pub const fn as_i64(self) -> i64 {
        self.seconds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UtcOffsetMinutes {
    minutes: i32,
}

impl UtcOffsetMinutes {
    pub const fn new(minutes: i32) -> Self {
        Self { minutes }
    }

    pub const fn as_i32(self) -> i32 {
        self.minutes
    }
}

impl Default for UtcOffsetMinutes {
    fn default() -> Self {
        Self::new(480)
    }
}
