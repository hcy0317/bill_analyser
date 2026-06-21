/// 汇总日历范围内的账单事件和周期账单投影。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_calendar_events_data(
    bills: &[StatisticsBillInput],
    recurring_rules: &[RecurringRuleInput],
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> CalendarEventsData {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "statistics",
        operation = "build_calendar_events_data",
        "business operation entered"
    );
    let mut daily: BTreeMap<String, CalendarEventDay> = BTreeMap::new();
    for bill in bills {
        let Some(date) = parse_bill_date_prefix(&bill.date) else {
            continue;
        };
        let date_text = format_date(date);
        let day = daily
            .entry(date_text.clone())
            .or_insert_with(|| empty_calendar_day(date_text.clone()));
        day.count += 1;

        let amount_cents = bill.amount_cents.abs();
        if is_expense_type(&bill.bill_type) {
            day.expense_cents += amount_cents;
        } else if is_income_type(&bill.bill_type) {
            day.income_cents += amount_cents;
        } else if is_transfer_type(&bill.bill_type) {
            day.transfer_out_cents += amount_cents;
        }
        day.net_cents = day.income_cents - day.expense_cents;
        day.bills.push(CalendarBillItem {
            id: bill.id,
            amount_cents: bill.amount_cents,
            bill_type: bill.bill_type.clone(),
            counterparty: bill.counterparty.clone(),
            description: bill.description.clone(),
            main_category: bill.main_category.clone(),
            sub_category: bill.sub_category.clone(),
        });
    }

    let recurring_projections =
        build_calendar_recurring_projections(recurring_rules, start_date, end_date);

    CalendarEventsData {
        events: daily.into_values().collect(),
        recurring_projections,
        start_date: format_date(start_date),
        end_date: format_date(end_date),
    }
}

/// 将日历事件数据包装为 success/data 接口响应。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_calendar_events_response(data: &CalendarEventsData) -> Value {
    json!({"success": true, "data": data})
}
