use std::{fmt, str::FromStr};

use chrono::{Datelike, NaiveDate, NaiveDateTime};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ErrorCode, RuntimeError};

const NORMALIZED_BILL_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";
const BILL_DATE_TIME_FORMATS: &[&str] = &[
    "%Y-%m-%d %H:%M:%S",
    "%Y-%m-%d %H:%M",
    "%Y-%m-%dT%H:%M:%S",
    "%Y/%m/%d %H:%M:%S",
    "%Y/%m/%d %H:%M",
    "%Y年%m月%d日 %H:%M:%S",
    "%Y年%m月%d日 %H:%M",
];
const BILL_DATE_FORMATS: &[&str] = &["%Y-%m-%d", "%Y/%m/%d", "%Y年%m月%d日"];

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

    None
}

pub fn normalize_bill_date_text(raw_value: &str) -> String {
    parse_bill_datetime(raw_value)
        .map(BillDateTime::normalize)
        .unwrap_or_else(|| raw_value.trim().to_string())
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
