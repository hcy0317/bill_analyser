async fn investment_settings_gone_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "investment_settings_gone_handler",
        "business operation entered"
    );
    match user_id_from_headers(&headers, &state.config) {
        Ok(_) => error_response(
            StatusCode::GONE,
            "Investment recognition settings are managed by category rules",
        ),
        Err(response) => *response,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
async fn reconcile_history_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Option<Json<Value>>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "matching",
        operation = "reconcile_history_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = payload
        .map(|payload| payload.0)
        .unwrap_or_else(|| json!({}));
    let Some(object) = payload.as_object() else {
        return error_response(StatusCode::BAD_REQUEST, "Invalid request");
    };
    let bill_ids = match parse_reconcile_history_bill_ids(object) {
        Ok(value) => value,
        Err(message) => return error_response(StatusCode::BAD_REQUEST, message),
    };
    let families = normalize_reconcile_history_families(object.get("families"));
    let allowed_families = families.iter().map(String::as_str).collect::<Vec<_>>();

    return reconcile_history_postgres_response(&state, user_id, &bill_ids, &allowed_families)
        .await;
}

async fn reconcile_history_postgres_response(
    state: &HttpAppState,
    user_id: UserId,
    bill_ids: &[i64],
    allowed_families: &[&str],
) -> Response {
    let runtime = match open_postgres_runtime(state, "matching") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut results = Vec::new();
    let mut candidate_count = 0usize;
    let mut linked_pair_keys = BTreeSet::new();
    for bill_id in bill_ids {
        let payload = match query_postgres_matching_bill_candidates_payload(
            runtime.pool(),
            user_id,
            *bill_id,
        )
        .await
        {
            Ok(Some(value)) => value,
            Ok(None) => return error_response(StatusCode::NOT_FOUND, "Bill not found"),
            Err(error) => return matching_error_response(error),
        };
        let linked_pair = payload
            .get("linkedPair")
            .filter(|value| value.is_object())
            .filter(|value| pair_type_in_allowed_families(value, allowed_families))
            .cloned()
            .unwrap_or(Value::Null);
        let candidates = payload
            .get("candidates")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|candidate| {
                        candidate_kind_in_allowed_families(candidate, allowed_families)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        candidate_count += candidates.len();
        if linked_pair.is_object() {
            if let Some(pair_key) = matching_history_pair_key(&linked_pair) {
                linked_pair_keys.insert(pair_key);
            }
        }
        results.push(json!({
            "billId": payload.get("billId").and_then(value_to_i64).unwrap_or(*bill_id),
            "linkedPair": linked_pair,
            "candidates": candidates,
            "reconciliation": payload.get("reconciliation").cloned().unwrap_or(Value::Null),
        }));
    }
    success_data(
        StatusCode::OK,
        json!({
            "summary": {
                "billCount": results.len(),
                "candidateCount": candidate_count,
                "linkedPairCount": linked_pair_keys.len(),
            },
            "results": results,
        }),
    )
}
