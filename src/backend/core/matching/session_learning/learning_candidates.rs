pub fn build_learning_candidate_for_bill(
    bill: &Map<String, Value>,
    rule: &Map<String, Value>,
    suppression_created_at: Option<&str>,
    categories: &[Value],
    accounts: &[Value],
) -> Option<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_learning_candidate_for_bill", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_learning_candidates_for_bill(
    bill: &Map<String, Value>,
    rules: &[Value],
    suppression_revisions: &BTreeMap<i64, String>,
    categories: &[Value],
    accounts: &[Value],
) -> Vec<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_learning_candidates_for_bill", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_learning_rule_result_summary(
    rule: &Map<String, Value>,
    categories: &[Value],
    accounts: &[Value],
) -> String {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_learning_rule_result_summary", "business operation entered");
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
