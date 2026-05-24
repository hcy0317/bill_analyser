// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

pub fn build_transfer_pair_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
) -> Option<Value> {
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
    let anchor_amount = value_to_f64(anchor_bill.get("amount"));
    let candidate_amount = value_to_f64(candidate_bill.get("amount"));
    if (anchor_amount.abs() - candidate_amount.abs()).abs() > TRANSFER_AMOUNT_TOLERANCE
        || anchor_amount * candidate_amount >= 0.0
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
            "amount": candidate_amount,
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

pub fn build_transfer_pair_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
) -> Vec<Value> {
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

pub fn build_duplicate_bill_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
) -> Option<Value> {
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

pub fn build_duplicate_bill_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
) -> Vec<Value> {
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

    for field_name in ["amount", "destination_amount"] {
        if (value_to_f64(anchor_bill.get(field_name)) - value_to_f64(candidate_bill.get(field_name)))
            .abs()
            > TRANSFER_AMOUNT_TOLERANCE
        {
            return false;
        }
    }

    true
}

pub fn build_investment_pair_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> Option<Value> {
    score_investment_candidate(anchor_bill, true, keyword_config)?;
    score_investment_candidate(candidate_bill, true, keyword_config)?;

    let anchor_id = value_to_i64(anchor_bill.get("id")).filter(|value| *value > 0)?;
    let candidate_id = value_to_i64(candidate_bill.get("id")).filter(|value| *value > 0)?;
    if anchor_id == candidate_id {
        return None;
    }
    let anchor_amount = value_to_f64(anchor_bill.get("amount"));
    let candidate_amount = value_to_f64(candidate_bill.get("amount"));
    if (anchor_amount.abs() - candidate_amount.abs()).abs() > TRANSFER_AMOUNT_TOLERANCE
        || anchor_amount * candidate_amount >= 0.0
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

pub fn build_investment_pair_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
    keyword_config: Option<&Value>,
) -> Vec<Value> {
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

pub fn build_matching_session_candidates(session_id: &str, previews: &[Value]) -> Value {
    let mut candidates = Vec::new();
    let mut counts_by_kind: BTreeMap<&str, i64> =
        CANDIDATE_KIND_ORDER.iter().map(|kind| (*kind, 0)).collect();
    for preview in previews {
        let Some(preview_object) = preview.as_object() else {
            continue;
        };
        let matching = preview_object
            .get("matching")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let context = build_candidate_context(&matching);
        for kind in CANDIDATE_KIND_ORDER {
            let details = matching
                .get(*kind)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if !should_include_candidate(kind, &details) {
                continue;
            }
            candidates.push(build_session_candidate(
                session_id,
                preview_object,
                kind,
                &details,
                &context,
            ));
            *counts_by_kind.entry(kind).or_insert(0) += 1;
        }
    }
    let mut summary_counts = Map::new();
    for kind in LEGACY_SUMMARY_KIND_ORDER {
        summary_counts.insert((*kind).to_string(), json!(counts_by_kind[kind]));
    }
    if counts_by_kind["reconciliation"] > 0 {
        summary_counts.insert(
            "reconciliation".to_string(),
            json!(counts_by_kind["reconciliation"]),
        );
    }
    json!({
        "session_id": session_id,
        "summary": {
            "preview_count": previews.len(),
            "candidate_count": candidates.len(),
            "counts_by_kind": summary_counts,
        },
        "candidates": candidates,
    })
}

pub fn parse_reconciliation_candidates_query(query: &Map<String, Value>) -> Result<Value, String> {
    let candidate_type = optional_lower_query(query, "candidateType");
    if let Some(candidate_type) = candidate_type.as_deref() {
        if !matches!(candidate_type, "transfer" | "duplicate") {
            return Err("Invalid candidateType".to_string());
        }
    }
    let status = optional_lower_query(query, "status");
    if let Some(status) = status.as_deref() {
        if !matches!(
            status,
            "pending" | "accepted" | "rejected" | "merged" | "rolled_back" | "superseded"
        ) {
            return Err("Invalid status".to_string());
        }
    }
    let limit = parse_optional_positive_query_int(query, "limit")?
        .unwrap_or(200)
        .min(500);
    Ok(json!({
        "session_id": optional_string_query(query, "sessionId"),
        "preview_id": parse_optional_positive_query_int(query, "previewId")?,
        "existing_bill_id": parse_optional_positive_query_int(query, "billId")?,
        "candidate_type": candidate_type,
        "status": status,
        "limit": limit,
    }))
}

pub fn build_matching_candidate_action_payload(
    candidate_id: &str,
    result: &Map<String, Value>,
) -> Value {
    let mut payload = json!({
        "candidateId": value_to_string(result.get("candidate_id")).if_empty(candidate_id),
        "action": value_to_string(result.get("action")),
    });
    let object = payload.as_object_mut().expect("json object");
    insert_i64_if_present(object, "previewId", result.get("preview_id"));
    insert_string_if_present(object, "sessionId", result.get("session_id"));
    insert_i64_if_present(object, "recurringId", result.get("recurring_id"));
    insert_string_if_present(object, "reviewStatus", result.get("review_status"));
    if let Some(value) = result.get("suppressed") {
        object.insert(
            "suppressed".to_string(),
            json!(value.as_bool().unwrap_or(false)),
        );
    }
    for (source_key, target_key) in [
        ("preview_item", "previewItem"),
        ("projection", "projection"),
    ] {
        if let Some(value) = result.get(source_key).filter(|value| value.is_object()) {
            object.insert(target_key.to_string(), value.clone());
        }
    }
    if let Some(bill) = result.get("bill").and_then(Value::as_object) {
        object.insert("bill".to_string(), serialize_bill_snapshot(bill));
    }
    if let Some(value) = result.get("preview").filter(|value| value.is_array()) {
        object.insert("preview".to_string(), value.clone());
    }
    if let Some(pair) = result.get("pair").and_then(Value::as_object) {
        object.insert("pair".to_string(), serialize_bill_pair(pair));
    }
    payload
}

pub fn build_learning_candidate_for_bill(
    bill: &Map<String, Value>,
    rule: &Map<String, Value>,
    suppression_created_at: Option<&str>,
    categories: &[Value],
    accounts: &[Value],
) -> Option<Value> {
    let bill_id = value_to_i64(bill.get("id")).filter(|value| *value > 0)?;
    let rule_id = value_to_i64(rule.get("id")).filter(|value| *value > 0)?;
    if value_to_string(rule.get("match_type")) != "composite"
        || value_to_string(rule.get("match_features_json"))
            .trim()
            .is_empty()
    {
        return None;
    }
    let rule_revision = build_learning_rule_revision(rule);
    if suppression_created_at
        .map(str::trim)
        .is_some_and(|value| value == rule_revision)
    {
        return None;
    }
    let bill_features = build_composite_match_features(
        "",
        &value_to_string(bill.get("counterparty")),
        &value_to_string(bill.get("description")),
        &value_to_string(bill.get("payment_method")),
    )?;
    let rule_features = deserialize_learning_match_features(rule)?;
    let score_payload = score_learning_rule_similarity(&bill_features, &rule_features)?;
    let score = value_to_f64(score_payload.get("score"));
    if score < 0.72 {
        return None;
    }
    let level = if score >= 0.9 {
        "high"
    } else if score >= 0.82 {
        "medium"
    } else {
        "low"
    };
    Some(json!({
        "candidate_id": build_formal_learning_candidate_id(bill_id, rule_id, &rule_revision),
        "kind": "learning",
        "rule_id": rule_id,
        "score": score,
        "level": level,
        "reason": score_payload
            .get("reason_parts")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(value_to_string_value).collect::<Vec<_>>().join(", "))
            .unwrap_or_default(),
        "recommended_type": value_to_string(rule.get("learned_type")),
        "summary": build_learning_rule_result_summary(rule, categories, accounts),
        "suppressed": false,
    }))
}

pub fn build_learning_candidates_for_bill(
    bill: &Map<String, Value>,
    rules: &[Value],
    suppression_revisions: &BTreeMap<i64, String>,
    categories: &[Value],
    accounts: &[Value],
) -> Vec<Value> {
    let mut candidates: Vec<Value> = rules
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|rule| {
            let rule_id = value_to_i64(rule.get("id")).unwrap_or(0);
            build_learning_candidate_for_bill(
                bill,
                rule,
                suppression_revisions.get(&rule_id).map(String::as_str),
                categories,
                accounts,
            )
        })
        .collect();
    candidates.sort_by(|left, right| {
        let left_score = value_to_f64(left.get("score"));
        let right_score = value_to_f64(right.get("score"));
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                value_to_i64(right.get("rule_id"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(left.get("rule_id")).unwrap_or(0))
            })
    });
    candidates
}

pub fn deserialize_learning_match_features(
    rule: &Map<String, Value>,
) -> Option<BTreeMap<String, String>> {
    let raw_payload = value_to_string(rule.get("match_features_json"));
    if raw_payload.trim().is_empty() {
        return None;
    }
    let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(&raw_payload) else {
        return None;
    };
    let mut normalized = BTreeMap::new();
    for (key, value) in payload {
        let normalized_value =
            normalize_import_learning_text(Some(&Value::String(value_to_string_value(&value))));
        if !normalized_value.is_empty() {
            normalized.insert(key, normalized_value);
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn score_learning_rule_similarity(
    bill_features: &BTreeMap<String, String>,
    rule_features: &BTreeMap<String, String>,
) -> Option<Value> {
    let feature_weights = BTreeMap::from([
        ("parser_id", 0.35),
        ("counterparty", 0.30),
        ("description", 0.20),
        ("payment_method", 0.15),
    ]);
    let bill_feature_keys: Vec<&str> = feature_weights
        .keys()
        .copied()
        .filter(|field| {
            bill_features
                .get(*field)
                .is_some_and(|value| !value.is_empty())
        })
        .collect();
    if bill_feature_keys.len() < 2 {
        return None;
    }
    let total_possible_weight: f64 = bill_feature_keys
        .iter()
        .filter_map(|field| feature_weights.get(field).copied())
        .sum();
    let mut weighted_score = 0.0;
    let mut matched_fields = Vec::new();
    let mut reason_parts = Vec::new();
    for field in bill_feature_keys {
        let bill_value = bill_features.get(field).map(String::as_str).unwrap_or("");
        let rule_value = rule_features.get(field).map(String::as_str).unwrap_or("");
        if rule_value.is_empty() {
            continue;
        }
        let similarity = calculate_learning_feature_similarity(field, bill_value, rule_value);
        if similarity <= 0.0 {
            continue;
        }
        weighted_score += feature_weights[field] * similarity;
        if similarity >= 0.8 {
            matched_fields.push(field.to_string());
            let match_label = if similarity >= 0.999 {
                "exact".to_string()
            } else {
                format!("similar({similarity:.2})")
            };
            reason_parts.push(format!("{field}:{match_label}"));
        }
    }
    if matched_fields.len() < 2 || total_possible_weight <= 0.0 {
        return None;
    }
    Some(json!({
        "score": round2(weighted_score / total_possible_weight),
        "matched_fields": matched_fields,
        "reason_parts": reason_parts,
    }))
}

pub fn build_learning_rule_result_summary(
    rule: &Map<String, Value>,
    categories: &[Value],
    accounts: &[Value],
) -> String {
    let mut parts = Vec::new();
    let learned_type = value_to_string(rule.get("learned_type")).trim().to_string();
    if !learned_type.is_empty() {
        parts.push(learned_type.clone());
    }
    if let Some(category_id) = value_to_i64(rule.get("learned_category_id")) {
        if let Some(category) = find_object_by_id(categories, category_id) {
            let main_category = value_to_string(category.get("main_category"));
            let sub_category = value_to_string(category.get("sub_category"));
            if !main_category.is_empty() && !sub_category.is_empty() {
                parts.push(format!("{main_category}/{sub_category}"));
            } else if !main_category.is_empty() {
                parts.push(main_category);
            }
        }
    }
    let source_account = value_to_i64(rule.get("learned_source_account_id"))
        .and_then(|account_id| find_object_by_id(accounts, account_id));
    let destination_account = value_to_i64(rule.get("learned_destination_account_id"))
        .and_then(|account_id| find_object_by_id(accounts, account_id));
    let source_account_name =
        source_account.map_or_else(String::new, |account| value_to_string(account.get("name")));
    let destination_account_name = destination_account
        .map_or_else(String::new, |account| value_to_string(account.get("name")));
    if !source_account_name.is_empty() || !destination_account_name.is_empty() {
        if matches!(
            learned_type.to_lowercase().as_str(),
            "转账" | "投资" | "transfer" | "investment"
        ) && !source_account_name.is_empty()
            && !destination_account_name.is_empty()
        {
            parts.push(format!(
                "{source_account_name} → {destination_account_name}"
            ));
        } else if !source_account_name.is_empty() {
            parts.push(source_account_name);
        } else {
            parts.push(destination_account_name);
        }
    }
    parts.join(" | ")
}
