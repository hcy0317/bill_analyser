#[tracing::instrument(level = "debug", skip_all)]

pub async fn find_postgres_statistics_all_date_range(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<StatisticsAllDateRange>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let row = sqlx::query(
        "
        SELECT
            MIN(occurred_at::date)::TEXT AS start_date,
            MAX(occurred_at::date)::TEXT AS end_date
        FROM bills
        WHERE user_id = $1 AND is_deleted = false
        ",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let start_date: Option<String> = row.try_get("start_date")?;
    let end_date: Option<String> = row.try_get("end_date")?;
    let (Some(start_date), Some(end_date)) = (start_date, end_date) else {
        return Ok(None);
    };
    if start_date.trim().is_empty() || end_date.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(StatisticsAllDateRange {
        start_date,
        end_date,
    }))
}

fn analyzer_trend_period_window(
    period: &str,
    today: NaiveDate,
    offset: i64,
) -> Option<(String, NaiveDate, NaiveDate)> {
    match period {
        "month" => {
            let target = today - Duration::days(30 * offset);
            let start = first_day(target.year(), target.month())?;
            let end = add_months(start, 1)? - Duration::days(1);
            Some((format!("{}-{:02}", start.year(), start.month()), start, end))
        }
        "year" => {
            let year = today.year() - offset as i32;
            let start = first_day(year, 1)?;
            let end = first_day(year, 12)?.with_day(31)?;
            Some((format!("{year:04}"), start, end))
        }
        _ => None,
    }
}

fn first_day(year: i32, month: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, 1)
}

fn add_months(date: NaiveDate, months: u32) -> Option<NaiveDate> {
    let zero_based = date.month0() + months;
    let year = date.year() + (zero_based / 12) as i32;
    let month = (zero_based % 12) + 1;
    first_day(year, month)
}
