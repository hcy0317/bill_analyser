#[tracing::instrument(level = "debug", skip_all)]
/// 构造单个转账配对候选，保持来源/目标行和金额方向语义。
pub fn build_transfer_pair_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
) -> Option<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_transfer_pair_candidate", "business operation entered");
    if is_explicit_transfer_type(anchor_bill.get("type"))
        || is_explicit_transfer_type(candidate_bill.get("type"))
    {
        return None;
    }
    let anchor_id = value_to_i64(anchor_bill.get("id")).filter(|value| *value > 0)?;
    let candidate_id = value_to_i64(candidate_bill.get("id")).filter(|value| *value > 0)?;
    if anchor_id == candidate_id {
        return None;
    }
    let anchor_amount_cents = bill_amount_cents(anchor_bill);
    let candidate_amount_cents = bill_amount_cents(candidate_bill);
    if (anchor_amount_cents.abs() - candidate_amount_cents.abs()).abs()
        > TRANSFER_AMOUNT_TOLERANCE_CENTS
        || anchor_amount_cents == 0
        || candidate_amount_cents == 0
        || anchor_amount_cents.signum() == candidate_amount_cents.signum()
    {
        return None;
    }
    let anchor_source_account_id =
        value_to_i64(anchor_bill.get("source_account_id")).filter(|value| *value > 0)?;
    let candidate_source_account_id =
        value_to_i64(candidate_bill.get("source_account_id")).filter(|value| *value > 0)?;
    if anchor_source_account_id == candidate_source_account_id {
        return None;
    }
    let time_diff_seconds = same_day_pair_time_diff_seconds(
        anchor_bill,
        candidate_bill,
        HISTORICAL_TRANSFER_PAIR_TIME_TOLERANCE_SECONDS,
    )?;
    let max_window_seconds = HISTORICAL_TRANSFER_PAIR_TIME_TOLERANCE_SECONDS as f64;
    let time_score = (1.0 - (time_diff_seconds as f64 / max_window_seconds)).max(0.0);
    let score = round2((0.85 + (0.14 * time_score)).min(0.99));
    let mut reason_parts = vec!["opposite_amount", "different_source_account"];
    reason_parts.push("time_close");
    Some(json!({
        "candidate_id": build_formal_transfer_candidate_id(anchor_id, candidate_id),
        "bill_id": candidate_id,
        "score": score,
        "level": derive_level(score),
        "reason": reason_parts.join("|"),
        "time_diff_seconds": time_diff_seconds,
        "bill": {
            "id": candidate_id,
            "date": value_to_string(candidate_bill.get("date")),
            "type": value_to_string(candidate_bill.get("type")),
            "amount_cents": candidate_amount_cents,
            "counterparty": value_to_string(candidate_bill.get("counterparty")),
            "description": value_to_string(candidate_bill.get("description")),
            "payment_method": value_to_string(candidate_bill.get("payment_method")),
            "main_category": value_to_string(candidate_bill.get("main_category")),
            "sub_category": value_to_string(candidate_bill.get("sub_category")),
            "source_account_id": candidate_source_account_id,
            "destination_account_id": value_to_i64(candidate_bill.get("destination_account_id")).unwrap_or(0),
        },
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 从预览行组合中筛选并构造可展示的转账配对候选列表。
pub fn build_transfer_pair_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
) -> Vec<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_transfer_pair_candidates", "business operation entered");
    let mut candidates: Vec<Value> = candidate_bills
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|candidate_bill| build_transfer_pair_candidate(anchor_bill, candidate_bill))
        .collect();
    candidates.sort_by(|left, right| {
        let left_score = value_to_f64(left.get("score"));
        let right_score = value_to_f64(right.get("score"));
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                value_to_i64(left.get("time_diff_seconds"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(right.get("time_diff_seconds")).unwrap_or(0))
            })
            .then_with(|| {
                value_to_i64(left.get("bill_id"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(right.get("bill_id")).unwrap_or(0))
            })
    });
    for candidate in &mut candidates {
        if let Some(object) = candidate.as_object_mut() {
            object.remove("time_diff_seconds");
        }
    }
    candidates
}
