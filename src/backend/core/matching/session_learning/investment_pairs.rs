pub fn build_investment_pair_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> Option<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_investment_pair_candidate", "business operation entered");
    score_investment_candidate(anchor_bill, true, keyword_config)?;
    score_investment_candidate(candidate_bill, true, keyword_config)?;

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
    let time_diff_seconds =
        pair_time_diff_seconds(anchor_bill, candidate_bill, INVESTMENT_PAIR_LOOKBACK_DAYS)?;
    let max_window_seconds = (INVESTMENT_PAIR_LOOKBACK_DAYS * 24 * 60 * 60) as f64;
    let time_score = (1.0 - (time_diff_seconds as f64 / max_window_seconds)).max(0.0);
    let score = round2((0.86 + (0.12 * time_score)).min(0.98));
    let mut reason_parts = vec![
        "investment_keyword",
        "opposite_amount",
        "different_source_account",
    ];
    if time_diff_seconds <= 3600 {
        reason_parts.push("time_close");
    } else {
        reason_parts.push("date_window");
    }
    Some(json!({
        "candidate_id": build_formal_investment_candidate_id(anchor_id, candidate_id),
        "kind": "investment",
        "bill_id": candidate_id,
        "score": score,
        "level": derive_level(score),
        "reason": reason_parts.join("|"),
        "bill": build_historical_candidate_bill_snapshot(candidate_bill, candidate_source_account_id),
        "time_diff_seconds": time_diff_seconds,
        "suppressed": false,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_investment_pair_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
    keyword_config: Option<&Value>,
) -> Vec<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_investment_pair_candidates", "business operation entered");
    let mut candidates: Vec<Value> = candidate_bills
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|candidate_bill| {
            build_investment_pair_candidate(anchor_bill, candidate_bill, keyword_config)
        })
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
