fn empty_statistics_analyzer_report(generated_at: &str) -> Value {
    json!({
        "period": "",
        "start_date": "",
        "end_date": "",
        "total_records": 0,
        "summary": {"total_income_cents": 0, "total_expense_cents": 0, "net_income_cents": 0},
        "by_category": {},
        "by_type": {},
        "trend": [],
        "top_expenses": [],
        "top_income": [],
        "generated_at": generated_at,
    })
}

fn statistics_analyzer_summary(bills: &[StatisticsBillInput]) -> Value {
    let total_income_cents = bills
        .iter()
        .filter(|bill| is_income_type(&bill.bill_type))
        .map(|bill| bill.amount_cents)
        .sum::<i64>();
    let total_expense_cents = bills
        .iter()
        .filter(|bill| is_expense_type(&bill.bill_type))
        .map(|bill| bill.amount_cents)
        .sum::<i64>();
    json!({
        "total_income_cents": total_income_cents,
        "total_expense_cents": total_expense_cents,
        "net_income_cents": total_income_cents + total_expense_cents,
    })
}

type AnalyzerSubCategoryTotals = BTreeMap<String, (usize, i64)>;
type AnalyzerCategoryTotals = BTreeMap<String, (usize, i64, AnalyzerSubCategoryTotals)>;

fn statistics_analyzer_by_category(bills: &[StatisticsBillInput]) -> Value {
    let mut categories: AnalyzerCategoryTotals = BTreeMap::new();
    for bill in bills {
        let amount = bill.amount_cents;
        let entry = categories
            .entry(bill.main_category.clone())
            .or_insert((0, 0, BTreeMap::new()));
        entry.0 += 1;
        entry.1 += amount;
        let sub_entry = entry.2.entry(bill.sub_category.clone()).or_insert((0, 0));
        sub_entry.0 += 1;
        sub_entry.1 += amount;
    }

    let mut result = serde_json::Map::new();
    for (category, (count, total_cents, sub_categories)) in categories {
        let mut sub_result = serde_json::Map::new();
        for (sub_category, (sub_count, sub_total_cents)) in sub_categories {
            sub_result.insert(
                sub_category,
                json!({
                    "count": sub_count,
                    "total_cents": sub_total_cents,
                }),
            );
        }
        result.insert(
            category,
            json!({
                "count": count,
                "total_cents": total_cents,
                "average_cents": average_total_cents(total_cents, count),
                "sub_categories": sub_result,
            }),
        );
    }
    Value::Object(result)
}

fn statistics_analyzer_by_type(bills: &[StatisticsBillInput]) -> Value {
    let mut by_type: BTreeMap<String, (usize, i64)> = BTreeMap::new();
    for bill in bills {
        let entry = by_type.entry(bill.bill_type.clone()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += bill.amount_cents;
    }
    let mut result = serde_json::Map::new();
    for (bill_type, (count, total_cents)) in by_type {
        result.insert(
            bill_type,
            json!({
                "count": count,
                "total_cents": total_cents,
                "average_cents": average_total_cents(total_cents, count),
            }),
        );
    }
    Value::Object(result)
}

fn statistics_analyzer_report_trend(bills: &[StatisticsBillInput], period: &str) -> Vec<Value> {
    let mut buckets: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for bill in bills {
        let Some(date) = parse_bill_date_prefix(&bill.date) else {
            continue;
        };
        let bucket_date = match period {
            "quarter" => {
                let days_until_sunday = 6_i64 - i64::from(date.weekday().num_days_from_monday());
                date + Duration::days(days_until_sunday)
            }
            "year" => last_day_of_month(date.year(), date.month()).unwrap_or(date),
            _ => date,
        };
        let entry = buckets.entry(format_date(bucket_date)).or_insert((0, 0));
        if is_income_type(&bill.bill_type) {
            entry.0 += bill.amount_cents;
        } else if is_expense_type(&bill.bill_type) {
            entry.1 += bill.amount_cents;
        }
    }
    buckets
        .into_iter()
        .map(|(date, (income_cents, expense_cents))| {
            json!({
                "date": date,
                "income_cents": income_cents,
                "expense_cents": expense_cents,
                "net_cents": income_cents + expense_cents,
            })
        })
        .collect()
}

fn statistics_analyzer_top_bills(
    bills: &[StatisticsBillInput],
    bill_type: &str,
    limit: usize,
    include_category: bool,
) -> Vec<Value> {
    let mut rows = bills
        .iter()
        .filter(|bill| bill.bill_type == bill_type)
        .collect::<Vec<_>>();
    rows.sort_by_key(|bill| std::cmp::Reverse(bill.amount_cents.abs()));
    rows.into_iter()
        .take(limit)
        .map(|bill| {
            let mut item = serde_json::Map::from_iter([
                (
                    "date".to_string(),
                    json!(bill.date.chars().take(10).collect::<String>()),
                ),
                ("amount_cents".to_string(), json!(bill.amount_cents)),
                ("counterparty".to_string(), json!(bill.counterparty)),
                ("description".to_string(), json!(bill.description)),
            ]);
            if include_category {
                item.insert("category".to_string(), json!(bill.main_category));
            }
            Value::Object(item)
        })
        .collect()
}
