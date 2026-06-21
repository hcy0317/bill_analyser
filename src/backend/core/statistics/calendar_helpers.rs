fn empty_calendar_day(date: String) -> CalendarEventDay {
    CalendarEventDay {
        date,
        income_cents: 0,
        expense_cents: 0,
        transfer_in_cents: 0,
        transfer_out_cents: 0,
        net_cents: 0,
        count: 0,
        bills: Vec::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_calendar_recurring_projections(
    recurring_rules: &[RecurringRuleInput],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<CalendarRecurringProjection> {
    let mut projections = Vec::new();
    for rule in recurring_rules {
        let Some(mut current) = parse_bill_date_prefix(&rule.next_date) else {
            continue;
        };
        let interval = recurring_interval_days(&rule.frequency);
        for _ in 0..50 {
            if current > end_date {
                break;
            }
            if current >= start_date {
                projections.push(CalendarRecurringProjection {
                    date: format_date(current),
                    projection_type: "recurring_projection".to_string(),
                    name: rule.name.clone(),
                    amount_cents: rule.amount_cents,
                    bill_type: rule.bill_type.clone(),
                    frequency: rule.frequency.clone(),
                    recurring_id: rule.id,
                });
            }
            current += Duration::days(interval);
        }
    }
    projections
}

fn recurring_interval_days(frequency: &str) -> i64 {
    match frequency {
        "weekly" => 7,
        "biweekly" => 14,
        "monthly" => 30,
        "bimonthly" => 60,
        "quarterly" => 90,
        "semiannual" => 180,
        "annual" => 365,
        _ => 30,
    }
}
