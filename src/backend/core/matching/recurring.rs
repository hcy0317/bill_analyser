// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_recurring_date(value: &Value) -> Option<NaiveDate> {
    match value {
        Value::String(text) => NaiveDate::parse_from_str(text.trim().get(..10)?, "%Y-%m-%d").ok(),
        _ => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn compute_recurring_pattern_hash(
    transaction_type: &str,
    amount_cents: i64,
    counterparty: &str,
    account_id: Option<i64>,
) -> String {
    let key = format!(
        "{}|{}|{}|{}",
        transaction_type,
        amount_cents,
        counterparty.trim().to_lowercase(),
        account_id.unwrap_or(0)
    );
    let digest = Sha256::digest(key.as_bytes());
    hex_prefix(&digest, 16)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn detect_recurring_frequency(intervals: &[f64]) -> FrequencyDetection {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "detect_recurring_frequency", "business operation entered");
    if intervals.is_empty() {
        return FrequencyDetection {
            frequency: "unknown".to_string(),
            average_interval_days: 0.0,
            confidence: 0.0,
        };
    }
    let avg_interval = mean(intervals);
    let interval_stdev = sample_stdev(intervals);
    let interval_variation = if avg_interval > 0.0 {
        interval_stdev / avg_interval
    } else {
        0.0
    };
    if interval_variation > MAX_RECURRING_INTERVAL_VARIATION {
        return FrequencyDetection {
            frequency: "irregular".to_string(),
            average_interval_days: avg_interval,
            confidence: 0.0,
        };
    }
    let mut best: Option<FrequencyDetection> = None;
    for (label, nominal_days, tolerance) in FREQUENCY_PATTERNS {
        let deviation = (avg_interval - nominal_days).abs() / nominal_days;
        if deviation <= *tolerance {
            let closeness = 1.0 - deviation;
            let consistency = 1.0 - (interval_stdev / nominal_days).min(1.0);
            let score = closeness * 0.6 + consistency * 0.4;
            if best
                .as_ref()
                .is_none_or(|current| score > current.confidence)
            {
                best = Some(FrequencyDetection {
                    frequency: (*label).to_string(),
                    average_interval_days: avg_interval,
                    confidence: score,
                });
            }
        }
    }
    if let Some(best) = best {
        return best;
    }
    if avg_interval > 0.0 && interval_stdev / avg_interval < 0.4 {
        return FrequencyDetection {
            frequency: format!("every_{}_days", avg_interval.round() as i64),
            average_interval_days: avg_interval,
            confidence: (1.0 - (interval_stdev / avg_interval).min(1.0)) * 0.5,
        };
    }
    FrequencyDetection {
        frequency: "irregular".to_string(),
        average_interval_days: avg_interval,
        confidence: 0.0,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn estimate_next_recurring_date(
    last_date: NaiveDate,
    frequency: &str,
    avg_interval: f64,
) -> NaiveDate {
    let days = match frequency {
        "weekly" => 7,
        "biweekly" => 14,
        "monthly" => 30,
        "bimonthly" => 60,
        "quarterly" => 90,
        "semiannual" => 180,
        "annual" => 365,
        _ => {
            if avg_interval > 0.0 {
                avg_interval.round() as i64
            } else {
                30
            }
        }
    };
    last_date + Duration::days(days)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn detect_recurring_patterns(
    bills: &[Value],
    min_occurrences: usize,
    existing_recurring_ids: &BTreeSet<i64>,
) -> Vec<RecurringPattern> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "detect_recurring_patterns", "business operation entered");
    detect_recurring_patterns_with_today(
        bills,
        min_occurrences,
        existing_recurring_ids,
        Local::now().date_naive(),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn detect_recurring_patterns_with_today(
    bills: &[Value],
    min_occurrences: usize,
    existing_recurring_ids: &BTreeSet<i64>,
    today: NaiveDate,
) -> Vec<RecurringPattern> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "detect_recurring_patterns_with_today", "business operation entered");
    let mut group_order = Vec::new();
    let mut groups: BTreeMap<String, Vec<RecurringGroupEntry>> = BTreeMap::new();
    for bill in bills {
        let Some(object) = bill.as_object() else {
            continue;
        };
        let bill_id = value_to_i64(object.get("id")).unwrap_or(0);
        if existing_recurring_ids.contains(&bill_id) {
            continue;
        }
        let Some(bill_date) = object.get("date").and_then(parse_recurring_date) else {
            continue;
        };
        let transaction_type = value_to_string(object.get("type")).trim().to_string();
        let amount_cents = (value_to_f64(object.get("amount")).abs() * 100.0).round() as i64;
        let counterparty = value_to_string(object.get("counterparty"))
            .trim()
            .to_string();
        if transaction_type.is_empty() || amount_cents == 0 || counterparty.is_empty() {
            continue;
        }
        let account_id = value_to_i64(object.get("source_account_id"));
        let pattern_hash = compute_recurring_pattern_hash(
            &transaction_type,
            amount_cents,
            &counterparty,
            account_id,
        );
        if !groups.contains_key(&pattern_hash) {
            group_order.push(pattern_hash.clone());
        }
        groups
            .entry(pattern_hash)
            .or_default()
            .push((object.clone(), bill_date, amount_cents));
    }
    let mut patterns = Vec::new();
    for pattern_hash in group_order {
        let Some(group_bills) = groups.get_mut(&pattern_hash) else {
            continue;
        };
        if group_bills.len() < min_occurrences {
            continue;
        }
        group_bills.sort_by_key(|(_, bill_date, _)| *bill_date);
        let dates: Vec<NaiveDate> = group_bills
            .iter()
            .map(|(_, bill_date, _)| *bill_date)
            .collect();
        let intervals: Vec<f64> = dates
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).num_days() as f64)
            .collect();
        if intervals.is_empty() {
            continue;
        }
        let avg_raw = mean(&intervals);
        if avg_raw <= 0.0 {
            continue;
        }
        let raw_variation = sample_stdev(&intervals) / avg_raw;
        if raw_variation > MAX_RECURRING_INTERVAL_VARIATION {
            continue;
        }
        let valid_intervals: Vec<f64> = intervals
            .into_iter()
            .filter(|interval| *interval <= avg_raw * MAX_RECURRING_GAP_RATIO && *interval > 0.0)
            .collect();
        if valid_intervals.len() < min_occurrences.saturating_sub(1) {
            continue;
        }
        let frequency = detect_recurring_frequency(&valid_intervals);
        if matches!(frequency.frequency.as_str(), "unknown" | "irregular")
            || frequency.confidence < 0.3
        {
            continue;
        }
        let recency_days = (today - *dates.last().expect("date")).num_days();
        let recency_score = (1.0 - (recency_days as f64 / 180.0)).max(0.0);
        let count_score = (group_bills.len() as f64 / 12.0).min(1.0);
        let confidence =
            round3((frequency.confidence * 0.5 + recency_score * 0.3 + count_score * 0.2).min(1.0));
        if confidence < MIN_RECURRING_PATTERN_CONFIDENCE {
            continue;
        }
        let sample = &group_bills[0].0;
        let counterparty = value_to_string(sample.get("counterparty"))
            .trim()
            .to_string();
        let description = [
            value_to_string(sample.get("description")),
            value_to_string(sample.get("main_category")),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
        let next_date = estimate_next_recurring_date(
            *dates.last().expect("date"),
            &frequency.frequency,
            frequency.average_interval_days,
        );
        patterns.push(RecurringPattern {
            pattern_hash,
            name: if counterparty.is_empty() {
                value_to_string(sample.get("description")).if_empty("Unknown")
            } else {
                counterparty.clone()
            },
            description,
            transaction_type: value_to_string(sample.get("type")),
            amount: value_to_f64(sample.get("amount")).abs(),
            source_account_id: value_to_i64(sample.get("source_account_id")),
            destination_account_id: value_to_string(sample.get("destination_account_id")),
            counterparty,
            frequency: frequency.frequency,
            detected_interval_days: round1(frequency.average_interval_days),
            confidence_score: confidence,
            sample_count: group_bills.len(),
            sample_bill_ids: group_bills
                .iter()
                .filter_map(|(bill, _, _)| value_to_i64(bill.get("id")))
                .collect(),
            first_occurrence: dates.first().expect("date").to_string(),
            last_occurrence: dates.last().expect("date").to_string(),
            suggested_next_date: next_date.to_string(),
        });
    }
    patterns.sort_by(|left, right| {
        right
            .confidence_score
            .partial_cmp(&left.confidence_score)
            .unwrap_or(Ordering::Equal)
    });
    patterns
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn serialize_recurring_suggestion(item: &Map<String, Value>) -> Value {
    json!({
        "id": value_to_i64(item.get("id")).unwrap_or(0),
        "patternHash": value_to_string(item.get("pattern_hash")),
        "name": value_to_string(item.get("name")),
        "description": value_to_string(item.get("description")),
        "type": value_to_string(item.get("type")),
        "amount": value_to_f64(item.get("amount")),
        "sourceAccountId": item.get("source_account_id").cloned().unwrap_or(Value::Null),
        "destinationAccountId": item.get("destination_account_id").cloned().unwrap_or(Value::Null),
        "counterparty": value_to_string(item.get("counterparty")),
        "frequency": value_to_string(item.get("frequency")),
        "detectedIntervalDays": value_to_f64(item.get("detected_interval_days")),
        "confidenceScore": value_to_f64(item.get("confidence_score")),
        "sampleCount": value_to_i64(item.get("sample_count")).unwrap_or(0),
        "sampleBillIds": recurring_sample_bill_ids(item),
        "firstOccurrence": value_to_string(item.get("first_occurrence")),
        "lastOccurrence": value_to_string(item.get("last_occurrence")),
        "suggestedNextDate": value_to_string(item.get("suggested_next_date")),
        "status": value_to_string(item.get("status")),
        "createdAt": value_to_string(item.get("created_at")),
        "updatedAt": value_to_string(item.get("updated_at")),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn serialize_recurring_suggestions(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter_map(Value::as_object)
        .map(serialize_recurring_suggestion)
        .collect()
}
