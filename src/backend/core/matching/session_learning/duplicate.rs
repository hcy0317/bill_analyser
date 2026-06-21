/// 构造单个重复账单候选，用于导入预览和正式账单 matching 展示。
pub fn build_duplicate_bill_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
) -> Option<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_duplicate_bill_candidate", "business operation entered");
    let anchor_id = value_to_i64(anchor_bill.get("id")).filter(|value| *value > 0)?;
    let candidate_id = value_to_i64(candidate_bill.get("id")).filter(|value| *value > 0)?;
    if anchor_id == candidate_id || !duplicate_bill_fields_match(anchor_bill, candidate_bill) {
        return None;
    }
    let candidate_source_account_id =
        value_to_i64(candidate_bill.get("source_account_id")).unwrap_or(0);

    Some(json!({
        "candidate_id": build_formal_duplicate_candidate_id(anchor_id, candidate_id),
        "kind": DUPLICATE_CANDIDATE_KIND,
        "bill_id": candidate_id,
        "score": 1.0,
        "level": "high",
        "reason": "same_bill_fields",
        "bill": build_historical_candidate_bill_snapshot(candidate_bill, candidate_source_account_id),
        "suppressed": false,
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 从候选账单集合中批量构造重复账单候选列表。
pub fn build_duplicate_bill_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
) -> Vec<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_duplicate_bill_candidates", "business operation entered");
    let mut candidates: Vec<Value> = candidate_bills
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|candidate_bill| build_duplicate_bill_candidate(anchor_bill, candidate_bill))
        .collect();
    candidates.sort_by(|left, right| {
        value_to_i64(left.get("bill_id"))
            .unwrap_or(0)
            .cmp(&value_to_i64(right.get("bill_id")).unwrap_or(0))
    });
    candidates
}

fn duplicate_bill_fields_match(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
) -> bool {
    for field_name in [
        "date",
        "type",
        "counterparty",
        "description",
        "payment_method",
        "main_category",
        "sub_category",
    ] {
        if value_to_string(anchor_bill.get(field_name)).trim()
            != value_to_string(candidate_bill.get(field_name)).trim()
        {
            return false;
        }
    }

    for field_name in ["source_account_id", "destination_account_id"] {
        if value_to_i64(anchor_bill.get(field_name)).unwrap_or(0)
            != value_to_i64(candidate_bill.get(field_name)).unwrap_or(0)
        {
            return false;
        }
    }

    for field_name in ["amount_cents", "destination_amount_cents"] {
        let anchor_amount_cents = value_to_i64(anchor_bill.get(field_name)).unwrap_or_default();
        let candidate_amount_cents =
            value_to_i64(candidate_bill.get(field_name)).unwrap_or_default();
        if (anchor_amount_cents - candidate_amount_cents).abs() > TRANSFER_AMOUNT_TOLERANCE_CENTS {
            return false;
        }
    }

    true
}
