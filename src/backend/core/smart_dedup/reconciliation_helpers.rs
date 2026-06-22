#[tracing::instrument(level = "debug", skip_all)]
fn build_reconciliation_import_key(bill: &DedupBill) -> String {
    if let Some(preview_id) = &bill.preview_id {
        if !preview_id.trim().is_empty() {
            return format!("preview:{}", preview_id.trim());
        }
    }

    if let (Some(session_id), Some(template_id)) = (&bill.session_id, &bill.template_id) {
        if !session_id.trim().is_empty() && !template_id.trim().is_empty() {
            return format!(
                "session:{}:template:{}",
                session_id.trim(),
                template_id.trim()
            );
        }
    }

    let key = format!(
        "{}|{}|{}|{}|{}|{}",
        bill.date,
        bill.amount.to_yuan_string(),
        bill.counterparty,
        bill.description,
        bill.parser_id,
        bill.source_account_id
    );
    format!("hash:{}", stable_hash_hex(&key))
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_reconciliation_candidate_id(
    candidate_type: ReconciliationCandidateType,
    existing_bill_id: &str,
    import_bill_key: &str,
) -> String {
    format!(
        "reconcile:import:{}:bill:{}:{}",
        candidate_type.as_str(),
        existing_bill_id,
        stable_hash_hex(import_bill_key)
    )
}

fn reconciliation_candidate_time_tolerance_seconds(
    candidate_type: ReconciliationCandidateType,
) -> i64 {
    match candidate_type {
        ReconciliationCandidateType::Duplicate => TIME_TOLERANCE_SECONDS,
        ReconciliationCandidateType::Transfer => DATABASE_TIME_TOLERANCE_SECONDS,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_reconciliation_candidate_type(
    imported_bill: &DedupBill,
    existing_bill: &DedupBill,
) -> Option<(ReconciliationCandidateType, String)> {
    if (abs_cents(imported_bill.amount) - abs_cents(existing_bill.amount)).abs()
        > AMOUNT_TOLERANCE_CENTS
    {
        return None;
    }

    let duplicate_evidence = has_reconciliation_duplicate_evidence(imported_bill, existing_bill);

    if amount_opposite(imported_bill.amount, existing_bill.amount) {
        if !has_distinct_reconciliation_transfer_sources(imported_bill, existing_bill) {
            return None;
        }
        return Some((
            ReconciliationCandidateType::Transfer,
            "opposite_amount|same_day|time_close".to_string(),
        ));
    }

    if amount_equal_same_direction(imported_bill.amount, existing_bill.amount) && duplicate_evidence
    {
        return Some((
            ReconciliationCandidateType::Duplicate,
            "same_amount|same_direction|same_day|time_close|duplicate_text_evidence".to_string(),
        ));
    }

    None
}

fn has_reconciliation_duplicate_evidence(
    imported_bill: &DedupBill,
    existing_bill: &DedupBill,
) -> bool {
    for (left, right) in [
        (&imported_bill.counterparty, &existing_bill.counterparty),
        (&imported_bill.description, &existing_bill.description),
        (&imported_bill.payment_method, &existing_bill.payment_method),
    ] {
        if text_evidence_matches(left, right, SIMILARITY_THRESHOLD) {
            return true;
        }
    }

    let imported_text = bill_text_for_intent(imported_bill);
    let existing_text = bill_text_for_intent(existing_bill);
    if imported_text.is_empty() && existing_text.is_empty() {
        return false;
    }
    text_evidence_matches(&imported_text, &existing_text, 0.62)
}

fn has_distinct_reconciliation_transfer_sources(left: &DedupBill, right: &DedupBill) -> bool {
    let left_account = normalized_non_zero_source(&left.source_account_id);
    let right_account = normalized_non_zero_source(&right.source_account_id);
    if !left_account.is_empty() && !right_account.is_empty() {
        return left_account != right_account;
    }

    let left_real_source = reconciliation_real_source_token(left);
    let right_real_source = reconciliation_real_source_token(right);
    if !left_real_source.is_empty() && !right_real_source.is_empty() {
        return left_real_source != right_real_source;
    }

    let left_parser = reconciliation_parser_source_token(left);
    let right_parser = reconciliation_parser_source_token(right);
    !left_parser.is_empty() && !right_parser.is_empty() && left_parser != right_parser
}

fn reconciliation_real_source_token(bill: &DedupBill) -> String {
    for value in [
        bill.source_account_id.as_str(),
        bill.payment_method.as_str(),
        bill.source.as_str(),
    ] {
        let value = normalized_non_zero_source(value);
        if !value.is_empty() {
            return value;
        }
    }
    String::new()
}

fn reconciliation_parser_source_token(bill: &DedupBill) -> String {
    let parser_id = normalized_non_zero_source(&bill.parser_id);
    if parser_id == "history_db" {
        String::new()
    } else {
        parser_id
    }
}

fn normalized_non_zero_source(value: &str) -> String {
    let value = normalized_source(value);
    if value == "0" {
        return String::new();
    }
    value
}

fn stable_hash_hex(input: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn longest_common_subsequence_len(left: &[char], right: &[char]) -> usize {
    let mut previous = vec![0; right.len() + 1];
    let mut current = vec![0; right.len() + 1];

    for left_char in left {
        for (right_index, right_char) in right.iter().enumerate() {
            current[right_index + 1] = if left_char == right_char {
                previous[right_index] + 1
            } else {
                previous[right_index + 1].max(current[right_index])
            };
        }
        std::mem::swap(&mut previous, &mut current);
        current.fill(0);
    }

    previous[right.len()]
}

fn source_priority(source_id: &str) -> i32 {
    match normalized_source(source_id).as_str() {
        "wechat" => 1,
        "alipay" => 2,
        "icbc" | "cmbc" | "abc" | "ccb" => 10,
        _ => 100,
    }
}

fn normalized_source(source_id: &str) -> String {
    source_id.trim().to_lowercase()
}

fn is_platform_source(source: &str) -> bool {
    PLATFORM_SOURCES.contains(&source)
}

fn is_bank_source(source: &str) -> bool {
    BANK_SOURCES.contains(&source)
}
