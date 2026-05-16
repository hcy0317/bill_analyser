
fn select_budget_items(items: &[Value], detail_mode: bool) -> Vec<Value> {
    let mut grouped: Vec<((String, String, String), Vec<Value>)> = Vec::new();
    for item in items {
        let key = (
            value_string(item.get("category")),
            value_string(item.get("period_type")),
            value_string(item.get("start_date")),
        );
        if let Some((_, group)) = grouped
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &key)
        {
            group.push(item.clone());
        } else {
            grouped.push((key, vec![item.clone()]));
        }
    }

    let mut selected = Vec::new();
    for (_, group) in grouped {
        let (primary, secondary): (Vec<Value>, Vec<Value>) = group
            .iter()
            .cloned()
            .partition(|item| value_string(item.get("sub_category")).trim().is_empty());

        if primary.len() > 1 {
            selected.extend(group);
        } else if let Some(primary_item) = primary.first() {
            if detail_mode
                && !secondary.is_empty()
                && primary_is_synchronized_shadow(primary_item, &secondary)
            {
                selected.extend(secondary);
            } else {
                selected.push(primary_item.clone());
            }
        } else {
            selected.extend(secondary);
        }
    }
    selected
}

fn primary_is_synchronized_shadow(primary: &Value, secondary: &[Value]) -> bool {
    let secondary_budget = secondary
        .iter()
        .map(|item| budget_item_amount(item, "budget_amount"))
        .sum::<f64>();
    let secondary_spent = secondary
        .iter()
        .map(|item| budget_item_amount(item, "spent_amount"))
        .sum::<f64>();
    (budget_item_amount(primary, "budget_amount") - secondary_budget).abs()
        <= SYNCHRONIZED_PRIMARY_TOLERANCE
        && (budget_item_amount(primary, "spent_amount") - secondary_spent).abs()
            <= SYNCHRONIZED_PRIMARY_TOLERANCE
}

fn parse_budget_i64(cleaned: &str, prefix: &str) -> Result<i64, String> {
    cleaned
        .parse::<i64>()
        .map_err(|_| format!("{prefix}: {cleaned}"))
}

fn json_int_candidate_text(item: &Value) -> String {
    match item {
        Value::String(value) => value.trim().to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "None".to_string(),
        Value::Bool(value) => value.to_string(),
        other => other.to_string(),
    }
}

fn string_field(object: &Map<String, Value>, field: &str) -> String {
    value_string(object.get(field))
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn budget_item_amount(item: &Value, field: &str) -> f64 {
    item.get(field).and_then(value_to_f64).unwrap_or_default()
}

fn value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn value_to_i32(value: &Value) -> Option<i32> {
    match value {
        Value::Number(number) => number.as_i64().and_then(|value| i32::try_from(value).ok()),
        Value::String(text) => text.trim().parse::<i32>().ok(),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn field_or_null(row: &Value, field: &str) -> Value {
    row.get(field).cloned().unwrap_or(Value::Null)
}

fn non_empty_str(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn parse_budget_date_prefix(value: &str) -> Result<NaiveDate, String> {
    let date_text = value
        .trim()
        .get(..10)
        .ok_or_else(|| format!("Invalid date: {value}"))?;
    NaiveDate::parse_from_str(date_text, "%Y-%m-%d").map_err(|_| format!("Invalid date: {value}"))
}

fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

fn make_date(year: i32, month: u32, day: u32) -> Result<NaiveDate, String> {
    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| format!("Invalid date components: {year}-{month}-{day}"))
}

fn last_day_of_month(year: i32, month: u32) -> Result<NaiveDate, String> {
    let next_month = if month == 12 {
        make_date(year + 1, 1, 1)?
    } else {
        make_date(year, month + 1, 1)?
    };
    Ok(next_month - Duration::days(1))
}

fn shift_month_start(date: NaiveDate, delta_months: i64) -> Result<NaiveDate, String> {
    let absolute_month = i64::from(date.year()) * 12 + i64::from(date.month0()) + delta_months;
    let year = absolute_month.div_euclid(12);
    let month0 = absolute_month.rem_euclid(12);
    let year = i32::try_from(year).map_err(|_| "Invalid shifted date year".to_string())?;
    let month = u32::try_from(month0 + 1).map_err(|_| "Invalid shifted date month".to_string())?;
    make_date(year, month, 1)
}

fn round2(value: f64) -> f64 {
    let rounded = (value * 100.0).round() / 100.0;
    if rounded.abs() < 0.005 {
        0.0
    } else {
        rounded
    }
}

fn average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn sorted_ids(values: Option<&[i64]>) -> Vec<i64> {
    let mut values = values.map_or_else(Vec::new, ToOwned::to_owned);
    values.sort_unstable();
    values
}

fn python_json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn forecast_strategy_explanation(strategy: &str, period_count: usize) -> String {
    if strategy == "moving_average" {
        format!("基于最近{}个周期的移动平均", period_count.min(3))
    } else {
        format!("基于最近{period_count}个周期的历史均值")
    }
}

enum PythonJsonValue {
    Int(i64),
    OptionalInt(Option<i64>),
    String(String),
    IntArray(Vec<i64>),
}

impl PythonJsonValue {
    fn render(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::OptionalInt(Some(value)) => value.to_string(),
            Self::OptionalInt(None) => "null".to_string(),
            Self::String(value) => python_json_string(value),
            Self::IntArray(values) => {
                let body = values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{body}]")
            }
        }
    }
}
