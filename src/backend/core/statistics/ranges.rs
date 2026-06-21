#[tracing::instrument(level = "debug", skip_all)]

pub fn parse_statistics_timestamp_range(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
) -> Result<StatisticsTimestampRange, StatisticsContractError> {
    let start_text = start_raw.unwrap_or_default().trim();
    let end_text = end_raw.unwrap_or_default().trim();
    if start_text == "0" && end_text == "0" {
        return Ok(StatisticsTimestampRange::All);
    }

    let start_time = parse_i64_text(start_text, "Invalid timestamp format")?;
    let end_time = parse_i64_text(end_text, "Invalid timestamp format")?;
    validate_statistics_time_range(start_time, end_time)?;
    Ok(StatisticsTimestampRange::Bounded {
        start_time,
        end_time,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_statistics_time_range(
    start_time: i64,
    end_time: i64,
) -> Result<(), StatisticsContractError> {
    if start_time > end_time {
        return Err(StatisticsContractError::new(
            "Invalid time range",
            "startTime must be less than or equal to endTime",
        ));
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_asset_trends_span(
    start_time: i64,
    end_time: i64,
    is_all_mode: bool,
) -> Result<(), StatisticsContractError> {
    validate_statistics_time_range(start_time, end_time)?;
    let day_count = (end_time - start_time) / 86_400;
    if !is_all_mode && day_count > 365 {
        let message = "资产趋势查询最多支持365天范围，请缩小时间范围";
        return Err(StatisticsContractError::new(message, message));
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_statistics_year_month_range(
    start_raw: Option<&str>,
    end_raw: Option<&str>,
) -> Result<StatisticsYearMonthRangeMode, StatisticsContractError> {
    let start_clean = clean_year_month_text(start_raw.unwrap_or_default());
    let end_clean = clean_year_month_text(end_raw.unwrap_or_default());
    if matches!(start_clean.as_str(), "0" | "197001")
        && matches!(end_clean.as_str(), "0" | "197001")
    {
        return Ok(StatisticsYearMonthRangeMode::All);
    }

    let (start_year, start_month) = parse_year_month_clean(&start_clean)?;
    let (end_year, end_month) = parse_year_month_clean(&end_clean)?;
    if (start_year, start_month) > (end_year, end_month) {
        return Err(StatisticsContractError::new(
            "Invalid year-month range",
            "startYearMonth must be less than or equal to endYearMonth",
        ));
    }

    let end_date = last_day_of_month(end_year, end_month)?;
    Ok(StatisticsYearMonthRangeMode::Bounded(
        StatisticsYearMonthRange {
            start_year,
            start_month,
            end_year,
            end_month,
            start_date: format!("{start_year:04}-{start_month:02}-01"),
            end_date: format_date(end_date),
        },
    ))
}
