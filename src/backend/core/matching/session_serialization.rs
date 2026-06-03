// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

#[tracing::instrument(level = "debug", skip_all)]
fn build_candidate_context(matching: &Map<String, Value>) -> Value {
    let dedup = matching.get("dedup").and_then(Value::as_object);
    let parser = matching.get("parser").and_then(Value::as_object);
    let annotation = matching.get("annotation").and_then(Value::as_object);
    json!({
        "dedup": {
            "type": dedup.map_or_else(String::new, |value| value_to_string(value.get("type"))),
            "source_ids": normalize_list(dedup.and_then(|value| value.get("source_ids"))),
        },
        "parser": {
            "id": parser.map_or_else(String::new, |value| value_to_string(value.get("id"))),
            "tags": normalize_list(parser.and_then(|value| value.get("tags"))).into_iter().filter(|value| !value_to_string_value(value).is_empty()).collect::<Vec<_>>(),
        },
        "annotation": {
            "is_manually_annotated": annotation.and_then(|value| value.get("is_manually_annotated")).and_then(Value::as_bool).unwrap_or(false),
        },
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_historical_candidate_bill_snapshot(
    candidate_bill: &Map<String, Value>,
    source_account_id: i64,
) -> Value {
    json!({
        "id": value_to_i64(candidate_bill.get("id")).unwrap_or(0),
        "date": value_to_string(candidate_bill.get("date")),
        "type": value_to_string(candidate_bill.get("type")),
        "amount": value_to_f64(candidate_bill.get("amount")),
        "counterparty": value_to_string(candidate_bill.get("counterparty")),
        "description": value_to_string(candidate_bill.get("description")),
        "payment_method": value_to_string(candidate_bill.get("payment_method")),
        "main_category": value_to_string(candidate_bill.get("main_category")),
        "sub_category": value_to_string(candidate_bill.get("sub_category")),
        "source_account_id": source_account_id,
        "destination_account_id": value_to_i64(candidate_bill.get("destination_account_id")).unwrap_or(0),
    })
}

fn should_include_candidate(kind: &str, details: &Map<String, Value>) -> bool {
    match kind {
        "reconciliation" => !value_to_string(details.get("candidate_id"))
            .trim()
            .is_empty(),
        "transfer" => {
            !value_to_string(details.get("candidate_type"))
                .trim()
                .is_empty()
                || matches!(
                    value_to_string(details.get("review_status"))
                        .trim()
                        .to_lowercase()
                        .as_str(),
                    "accepted" | "rejected"
                )
        }
        "learning" => {
            details.get("rule_id").is_some_and(|value| !value.is_null())
                || value_to_f64(details.get("score")) > 0.0
                || ["level", "reason", "recommended_type", "summary"]
                    .iter()
                    .any(|field| !value_to_string(details.get(*field)).trim().is_empty())
                || matches!(
                    value_to_string(details.get("review_status"))
                        .trim()
                        .to_lowercase()
                        .as_str(),
                    "accepted" | "rejected"
                )
        }
        "recurring" => {
            details.get("id").is_some_and(|value| !value.is_null())
                || value_to_i64(details.get("candidate_count")).unwrap_or(0) > 0
                || ["name", "match_reasons", "matched_date"]
                    .iter()
                    .any(|field| !value_to_string(details.get(*field)).trim().is_empty())
                || value_to_f64(details.get("match_score")) > 0.0
        }
        _ => false,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_session_candidate(
    session_id: &str,
    preview: &Map<String, Value>,
    kind: &str,
    details: &Map<String, Value>,
    context: &Value,
) -> Value {
    let preview_id = value_to_i64(preview.get("id")).unwrap_or(0);
    let normalized_details = build_candidate_details(kind, details);
    let (score, level, reason, status) = match kind {
        "reconciliation" => (
            value_to_f64(details.get("score")),
            value_to_string(details.get("level")),
            value_to_string(details.get("reason")),
            value_to_string(normalized_details.get("status")).if_empty("pending"),
        ),
        "recurring" => {
            let score = value_to_f64(normalized_details.get("match_score"));
            (
                score,
                derive_level(score).to_string(),
                value_to_string(normalized_details.get("match_reasons")),
                if normalized_details
                    .get("id")
                    .is_some_and(|value| !value.is_null())
                {
                    "confirmed".to_string()
                } else {
                    "pending".to_string()
                },
            )
        }
        "learning" => (
            value_to_f64(normalized_details.get("score")),
            value_to_string(normalized_details.get("level")),
            value_to_string(normalized_details.get("reason")),
            value_to_string(normalized_details.get("review_status")).if_empty("pending"),
        ),
        _ => (
            value_to_f64(normalized_details.get("score")),
            value_to_string(normalized_details.get("level")),
            value_to_string(normalized_details.get("reason")),
            value_to_string(normalized_details.get("review_status")).if_empty("pending"),
        ),
    };
    let candidate_id = if kind == "reconciliation" {
        value_to_string(normalized_details.get("candidate_id"))
            .if_empty(&format!("preview:{preview_id}:{kind}"))
    } else {
        format!("preview:{preview_id}:{kind}")
    };
    json!({
        "candidate_id": candidate_id,
        "kind": kind,
        "session_id": session_id,
        "preview_id": preview_id,
        "score": score,
        "level": level,
        "reason": reason,
        "status": status,
        "details": normalized_details,
        "preview": build_candidate_preview(preview),
        "context": context,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_candidate_details(kind: &str, details: &Map<String, Value>) -> Value {
    match kind {
        "reconciliation" => json!({
            "candidate_id": value_to_string(details.get("candidate_id")),
            "candidate_type": value_to_string(details.get("candidate_type")),
            "status": value_to_string(details.get("status")),
            "existing_bill_id": details.get("existing_bill_id").cloned().unwrap_or(Value::Null),
            "group_id": details.get("group_id").cloned().unwrap_or(Value::Null),
            "signal_label": value_to_string(details.get("signal_label")),
            "source_chain": normalize_list(details.get("source_chain")),
        }),
        "recurring" => json!({
            "id": details.get("id").cloned().unwrap_or(Value::Null),
            "name": value_to_string(details.get("name")),
            "candidate_count": value_to_i64(details.get("candidate_count")).unwrap_or(0),
            "match_score": value_to_f64(details.get("match_score")),
            "match_reasons": value_to_string(details.get("match_reasons")),
            "matched_date": value_to_string(details.get("matched_date")),
        }),
        "transfer" => json!({
            "candidate_type": value_to_string(details.get("candidate_type")),
            "score": value_to_f64(details.get("score")),
            "level": value_to_string(details.get("level")),
            "reason": value_to_string(details.get("reason")),
            "review_status": value_to_string(details.get("review_status")),
            "reviewed_type": value_to_string(details.get("reviewed_type")),
            "suppressed": details.get("suppressed").and_then(Value::as_bool).unwrap_or(false),
        }),
        _ => json!({
            "rule_id": details.get("rule_id").cloned().unwrap_or(Value::Null),
            "score": value_to_f64(details.get("score")),
            "level": value_to_string(details.get("level")),
            "reason": value_to_string(details.get("reason")),
            "recommended_type": value_to_string(details.get("recommended_type")),
            "summary": value_to_string(details.get("summary")),
            "review_status": value_to_string(details.get("review_status")),
            "suppressed": details.get("suppressed").and_then(Value::as_bool).unwrap_or(false),
        }),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_candidate_preview(preview: &Map<String, Value>) -> Value {
    json!({
        "preview_date": preview.get("preview_date").cloned().unwrap_or(json!("")),
        "preview_type": preview.get("preview_type").cloned().unwrap_or(json!("")),
        "preview_amount": preview.get("preview_amount").cloned().unwrap_or(json!(0)),
        "preview_destination_amount": preview.get("preview_destination_amount").cloned().unwrap_or(json!(0)),
        "preview_main_category": preview.get("preview_main_category").cloned().unwrap_or(json!("")),
        "preview_sub_category": preview.get("preview_sub_category").cloned().unwrap_or(json!("")),
        "preview_source_account_id": preview.get("preview_source_account_id").cloned().unwrap_or(Value::Null),
        "preview_destination_account_id": preview.get("preview_destination_account_id").cloned().unwrap_or(Value::Null),
        "preview_counterparty": preview.get("preview_counterparty").cloned().unwrap_or(json!("")),
        "preview_payment_method": preview.get("preview_payment_method").cloned().unwrap_or(json!("")),
        "preview_description": preview.get("preview_description").cloned().unwrap_or(json!("")),
        "preview_selected": preview.get("preview_selected").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn serialize_bill_pair(pair: &Map<String, Value>) -> Value {
    let mut serialized = json!({
        "id": value_to_i64(pair.get("id")).unwrap_or(0),
        "pairType": value_to_string(pair.get("pair_type")).if_empty(TRANSFER_PAIR_TYPE),
        "source": value_to_string(pair.get("source")).if_empty(MANUAL_PAIR_SOURCE),
        "leftBillId": value_to_i64(pair.get("left_bill_id")).unwrap_or(0),
        "rightBillId": value_to_i64(pair.get("right_bill_id")).unwrap_or(0),
    });
    insert_i64_if_present(
        serialized.as_object_mut().expect("pair object"),
        "otherBillId",
        pair.get("other_bill_id"),
    );
    serialized
}

fn serialize_bill_snapshot(snapshot: &Map<String, Value>) -> Value {
    json!({
        "id": value_to_i64(snapshot.get("id")).unwrap_or(0),
        "date": value_to_string(snapshot.get("date")),
        "type": value_to_string(snapshot.get("type")),
        "amount": value_to_f64(snapshot.get("amount")),
        "destinationAmount": value_to_f64(snapshot.get("destination_amount")),
        "counterparty": value_to_string(snapshot.get("counterparty")),
        "description": value_to_string(snapshot.get("description")),
        "paymentMethod": value_to_string(snapshot.get("payment_method")),
        "mainCategory": value_to_string(snapshot.get("main_category")),
        "subCategory": value_to_string(snapshot.get("sub_category")),
        "sourceAccountId": value_to_i64(snapshot.get("source_account_id")).unwrap_or(0),
        "destinationAccountId": value_to_i64(snapshot.get("destination_account_id")).unwrap_or(0),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_positive_request_int(
    object: &Map<String, Value>,
    field_name: &str,
) -> Result<i64, &'static str> {
    let Some(value) = object.get(field_name) else {
        return Err("billId and candidateBillId are required");
    };
    match value {
        Value::Bool(_) => Err("Invalid request"),
        Value::Number(number) => number
            .as_i64()
            .filter(|value| *value > 0)
            .ok_or("Invalid request"),
        Value::String(text)
            if text
                .trim()
                .chars()
                .all(|character| character.is_ascii_digit()) =>
        {
            text.trim()
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or("Invalid request")
        }
        _ => Err("Invalid request"),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_optional_positive_query_int(
    query: &Map<String, Value>,
    field_name: &str,
) -> Result<Option<i64>, String> {
    let Some(value) = query.get(field_name) else {
        return Ok(None);
    };
    if value.is_null() || value_to_string(Some(value)).is_empty() {
        return Ok(None);
    }
    let parsed = value_to_i64(Some(value)).ok_or_else(|| format!("Invalid {field_name}"))?;
    if parsed <= 0 {
        return Err(format!("Invalid {field_name}"));
    }
    Ok(Some(parsed))
}

fn pair_time_diff_seconds(
    left_bill: &Map<String, Value>,
    right_bill: &Map<String, Value>,
    lookback_days: i64,
) -> Option<i64> {
    let left_datetime = parse_bill_datetime(&value_to_string(left_bill.get("date")))?;
    let right_datetime = parse_bill_datetime(&value_to_string(right_bill.get("date")))?;
    let diff_seconds = (left_datetime.inner() - right_datetime.inner())
        .num_seconds()
        .abs();
    let max_window_seconds = lookback_days.max(0) * 24 * 60 * 60;
    (max_window_seconds > 0 && diff_seconds <= max_window_seconds).then_some(diff_seconds)
}

fn same_day_pair_time_diff_seconds(
    left_bill: &Map<String, Value>,
    right_bill: &Map<String, Value>,
    max_window_seconds: i64,
) -> Option<i64> {
    let left_datetime = parse_bill_datetime(&value_to_string(left_bill.get("date")))?;
    let right_datetime = parse_bill_datetime(&value_to_string(right_bill.get("date")))?;
    if left_datetime.inner().date() != right_datetime.inner().date() {
        return None;
    }
    let diff_seconds = (left_datetime.inner() - right_datetime.inner())
        .num_seconds()
        .abs();
    (max_window_seconds > 0 && diff_seconds <= max_window_seconds).then_some(diff_seconds)
}

fn is_explicit_transfer_type(raw_type: Option<&Value>) -> bool {
    matches!(
        value_to_string(raw_type).trim().to_lowercase().as_str(),
        "转账" | "transfer"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_positive_i64(text: &str) -> Option<i64> {
    text.parse::<i64>().ok().filter(|value| *value > 0)
}

fn value_to_string(value: Option<&Value>) -> String {
    value.map(value_to_string_value).unwrap_or_default()
}

fn value_to_string_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Some(Value::String(text)) if !text.trim().is_empty() => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(true)) => Some(1),
        Some(Value::Bool(false)) => Some(0),
        _ => None,
    }
}

fn value_to_f64(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn json_object_text(pairs: &[(&str, String)]) -> String {
    let body = pairs
        .iter()
        .map(|(key, value)| {
            format!(
                "{}: {}",
                serde_json::to_string(key).expect("json key"),
                serde_json::to_string(value).expect("json value")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{body}}}")
}
