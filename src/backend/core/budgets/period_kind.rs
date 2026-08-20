// 中文导读：预算周期值对象，统一周期身份、包含范围、位移与 bucket key 语义。
// 维护重点：字符串只在 adapter 边界解析一次；下游 read model 只传 typed period。
// 不变式：weekly 使用周一为首日、首个周一前属于 00 周的 POSIX `%W` 合同。

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BudgetPeriodKind {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl BudgetPeriodKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "yearly" => Ok(Self::Yearly),
            _ => Err(format!("Invalid period_type: {value}")),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Yearly => "yearly",
        }
    }

    pub const fn rollup_parent_kinds(self) -> &'static [Self] {
        match self {
            Self::Monthly => &[Self::Quarterly, Self::Yearly],
            Self::Quarterly => &[Self::Yearly],
            Self::Daily | Self::Weekly | Self::Yearly => &[],
        }
    }

    pub fn containing(self, anchor: NaiveDate) -> Result<BudgetPeriodRange, String> {
        let (start, end) = match self {
            Self::Daily => (anchor, anchor),
            Self::Weekly => {
                let start =
                    anchor - Duration::days(i64::from(anchor.weekday().num_days_from_monday()));
                (start, start + Duration::days(6))
            }
            Self::Monthly => {
                let start = make_date(anchor.year(), anchor.month(), 1)?;
                let end = last_day_of_month(anchor.year(), anchor.month())?;
                (start, end)
            }
            Self::Quarterly => {
                let start_month = (((anchor.month() - 1) / 3) * 3) + 1;
                let end_month = start_month + 2;
                let start = make_date(anchor.year(), start_month, 1)?;
                let end = last_day_of_month(anchor.year(), end_month)?;
                (start, end)
            }
            Self::Yearly => (
                make_date(anchor.year(), 1, 1)?,
                make_date(anchor.year(), 12, 31)?,
            ),
        };
        Ok(BudgetPeriodRange {
            start_date: format_date(start),
            end_date: format_date(end),
        })
    }

    pub fn bucket_key(self, date: NaiveDate) -> String {
        match self {
            Self::Daily => format_date(date),
            Self::Weekly => date.format("%Y-%W").to_string(),
            Self::Monthly => date.format("%Y-%m").to_string(),
            Self::Quarterly => {
                format!("{}-Q{}", date.year(), ((date.month() - 1) / 3) + 1)
            }
            Self::Yearly => date.year().to_string(),
        }
    }

    fn shift_start(self, start: NaiveDate, periods: i64) -> Result<NaiveDate, String> {
        match self {
            Self::Daily | Self::Weekly => {
                let days = if self == Self::Weekly {
                    periods
                        .checked_mul(7)
                        .ok_or_else(|| "Invalid shifted date range".to_string())?
                } else {
                    periods
                };
                let duration = Duration::try_days(days)
                    .ok_or_else(|| "Invalid shifted date range".to_string())?;
                start
                    .checked_add_signed(duration)
                    .ok_or_else(|| "Invalid shifted date range".to_string())
            }
            Self::Monthly => shift_month_start(start, periods),
            Self::Quarterly => shift_month_start(
                start,
                periods
                    .checked_mul(3)
                    .ok_or_else(|| "Invalid shifted date range".to_string())?,
            ),
            Self::Yearly => {
                let delta =
                    i32::try_from(periods).map_err(|_| "Invalid shifted date year".to_string())?;
                let year = start
                    .year()
                    .checked_add(delta)
                    .ok_or_else(|| "Invalid shifted date year".to_string())?;
                make_date(year, start.month(), start.day())
            }
        }
    }
}
