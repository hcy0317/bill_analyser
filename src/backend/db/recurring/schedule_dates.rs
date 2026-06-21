async fn pg_recalculate_recurring_next_date_on_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    recurring_id: i64,
) -> DbResult<()> {
    let Some(recurring) = get_postgres_recurring_template_on_tx(tx, user_id, recurring_id).await?
    else {
        return Ok(());
    };
    let latest_linked_date = sqlx::query(
        r#"
        SELECT occurred_at
        FROM bills
        WHERE user_id = $1
          AND standard_payload->>'created_from_recurring' = $2
          AND is_deleted = false
        ORDER BY occurred_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(recurring_id.to_string())
    .fetch_optional(&mut **tx)
    .await?
    .map(|row| row.try_get::<chrono::DateTime<Utc>, _>("occurred_at"))
    .transpose()?
    .map(|value| value.date_naive());
    let fallback_next_date = pg_recurring_text(&recurring, "next_date");
    let next_occurrence = latest_linked_date
        .and_then(|date| pg_next_recurring_occurrence_after(&recurring, date, 370))
        .or_else(|| pg_first_recurring_occurrence(&recurring, 370))
        .map(|date| date.to_string())
        .or_else(|| non_empty_text(Some(fallback_next_date.as_str())).map(ToOwned::to_owned));
    sqlx::query(
        r#"
        UPDATE transaction_templates
        SET scheduled_next_date = $3,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1 AND id = $2 AND template_type = 2
        "#,
    )
    .bind(user_id)
    .bind(recurring_id)
    .bind(next_occurrence.as_deref())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn pg_parse_date_value(value: &str) -> Option<NaiveDate> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(text.get(..10)?, "%Y-%m-%d").ok()
}

fn pg_parse_schedule_frequency_values(value: &str) -> Vec<u32> {
    let mut values = value
        .split(',')
        .filter_map(|item| item.trim().parse::<u32>().ok())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}

fn pg_weekday_sunday_first(date: NaiveDate) -> u32 {
    date.weekday().num_days_from_sunday()
}

fn pg_recurring_active_on_date(recurring: &BillRecord, target_date: NaiveDate) -> bool {
    if pg_parse_date_value(&pg_recurring_text(recurring, "start_date"))
        .is_some_and(|start_date| target_date < start_date)
    {
        return false;
    }
    if pg_parse_date_value(&pg_recurring_text(recurring, "end_date"))
        .is_some_and(|end_date| target_date > end_date)
    {
        return false;
    }
    true
}

fn pg_recurring_due_on_date(recurring: &BillRecord, target_date: NaiveDate) -> bool {
    if !pg_recurring_active_on_date(recurring, target_date) {
        return false;
    }
    let frequency_type = pg_recurring_i64(recurring, "scheduled_frequency_type").unwrap_or(0);
    let frequency_values =
        pg_parse_schedule_frequency_values(&pg_recurring_text(recurring, "frequency"));
    let start_date = pg_parse_date_value(&pg_recurring_text(recurring, "start_date"));
    let next_date = pg_parse_date_value(&pg_recurring_text(recurring, "next_date"));

    if frequency_type == 1 {
        let valid_weekdays = if frequency_values.is_empty() {
            start_date
                .map(pg_weekday_sunday_first)
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            frequency_values
        };
        return valid_weekdays.contains(&pg_weekday_sunday_first(target_date));
    }
    if frequency_type == 2 {
        let valid_days = if frequency_values.is_empty() {
            start_date
                .map(|date| date.day())
                .into_iter()
                .collect::<Vec<_>>()
        } else {
            frequency_values
        };
        return valid_days.contains(&target_date.day());
    }
    next_date.is_some_and(|date| date == target_date)
        || start_date.is_some_and(|date| date == target_date)
}

fn pg_find_recurring_occurrence_near_date(
    recurring: &BillRecord,
    target_date: NaiveDate,
    tolerance_days: i64,
) -> Option<NaiveDate> {
    let mut nearest_date = None;
    let mut nearest_diff = None;
    for offset in -tolerance_days..=tolerance_days {
        let current_date = target_date + Duration::days(offset);
        if !pg_recurring_due_on_date(recurring, current_date) {
            continue;
        }
        let diff = offset.abs();
        if nearest_date.is_none() || nearest_diff.is_some_and(|value| diff < value) {
            nearest_date = Some(current_date);
            nearest_diff = Some(diff);
        }
    }
    nearest_date
}

fn pg_next_recurring_occurrence_after(
    recurring: &BillRecord,
    after_date: NaiveDate,
    max_search_days: i64,
) -> Option<NaiveDate> {
    (1..=max_search_days)
        .map(|offset| after_date + Duration::days(offset))
        .find(|candidate| pg_recurring_due_on_date(recurring, *candidate))
}

fn pg_first_recurring_occurrence(
    recurring: &BillRecord,
    max_search_days: i64,
) -> Option<NaiveDate> {
    let Some(start_date) = pg_parse_date_value(&pg_recurring_text(recurring, "start_date")) else {
        return pg_parse_date_value(&pg_recurring_text(recurring, "next_date"));
    };
    (0..=max_search_days)
        .map(|offset| start_date + Duration::days(offset))
        .find(|candidate| pg_recurring_due_on_date(recurring, *candidate))
        .or_else(|| pg_parse_date_value(&pg_recurring_text(recurring, "next_date")))
}
