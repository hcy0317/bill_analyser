/// 汇总导入 session 的配对候选响应，供前端候选面板消费。
pub fn build_matching_session_candidates(session_id: &str, previews: &[Value]) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(domain = "matching", operation = "build_matching_session_candidates", "business operation entered");
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
    for kind in SUMMARY_KIND_ORDER {
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

#[tracing::instrument(level = "debug", skip_all)]
/// 解析对账候选查询参数，并返回可透传给 repository 的筛选 JSON。
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
