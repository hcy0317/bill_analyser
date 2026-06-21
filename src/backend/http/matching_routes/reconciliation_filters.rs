fn reconciliation_filters_from_value(value: &Value) -> ReconciliationCandidateFilters {
    let object = value.as_object().cloned().unwrap_or_default();
    ReconciliationCandidateFilters {
        session_id: object.get("session_id").and_then(non_empty_value_string),
        preview_id: object.get("preview_id").and_then(value_to_i64),
        existing_bill_id: object.get("existing_bill_id").and_then(value_to_i64),
        candidate_type: object
            .get("candidate_type")
            .and_then(non_empty_value_string),
        status: object.get("status").and_then(non_empty_value_string),
        limit: object.get("limit").and_then(value_to_i64).unwrap_or(200),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_reconcile_history_bill_ids(object: &Map<String, Value>) -> Result<Vec<i64>, &'static str> {
    let Some(raw_bill_ids) = object.get("billIds") else {
        return Err("billIds is required");
    };
    let Some(raw_bill_ids) = raw_bill_ids.as_array() else {
        return Err("billIds must be a non-empty list");
    };
    if raw_bill_ids.is_empty() {
        return Err("billIds must be a non-empty list");
    }
    let mut bill_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for raw_bill_id in raw_bill_ids {
        let Some(bill_id) = value_to_i64(raw_bill_id).filter(|value| *value > 0) else {
            return Err("Invalid billIds");
        };
        if seen.insert(bill_id) {
            bill_ids.push(bill_id);
        }
    }
    Ok(bill_ids)
}

fn pair_type_in_allowed_families(value: &Value, allowed_families: &[&str]) -> bool {
    let pair_type = value
        .as_object()
        .and_then(|object| first_value(object, &["pairType", "pair_type"]))
        .and_then(value_to_text)
        .unwrap_or_else(|| "transfer".to_string())
        .trim()
        .to_ascii_lowercase();
    allowed_families.contains(&pair_type.as_str())
}

fn candidate_kind_in_allowed_families(value: &Value, allowed_families: &[&str]) -> bool {
    let kind = value
        .as_object()
        .and_then(|object| object.get("kind"))
        .and_then(value_to_text)
        .unwrap_or_else(|| "transfer".to_string())
        .trim()
        .to_ascii_lowercase();
    allowed_families.contains(&kind.as_str())
}
