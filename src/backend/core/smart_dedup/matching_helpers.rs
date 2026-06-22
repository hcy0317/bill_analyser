fn is_platform_bank_duplicate_candidate(platform_bill: &DedupBill, bank_bill: &DedupBill) -> bool {
    amount_same_direction(platform_bill.amount, bank_bill.amount)
}

// 中文说明：判断两笔账单是否有足够文本证据可合并，平台-银行组合比同源去重更严格。
fn duplicate_text_match(left: &DedupBill, right: &DedupBill, is_platform_bank_pair: bool) -> bool {
    let desc_similar =
        text_evidence_matches(&left.description, &right.description, SIMILARITY_THRESHOLD);
    let counterparty_similar = text_evidence_matches(
        &left.counterparty,
        &right.counterparty,
        SIMILARITY_THRESHOLD,
    );
    if is_platform_bank_pair {
        desc_similar || counterparty_similar
    } else {
        desc_similar
            || counterparty_similar
            || (left.description.is_empty() && right.description.is_empty())
    }
}

fn text_evidence_matches(left: &str, right: &str, threshold: f64) -> bool {
    let left = left.trim().to_lowercase();
    let right = right.trim().to_lowercase();
    if left.is_empty() || right.is_empty() {
        return false;
    }
    left.contains(&right)
        || right.contains(&left)
        || normalized_similarity(&left, &right) >= threshold
}

fn bill_text_for_intent(bill: &DedupBill) -> String {
    [
        &bill.counterparty,
        &bill.payment_method,
        &bill.description,
        &bill.original_category,
        &bill.main_category,
        &bill.sub_category,
    ]
    .iter()
    .filter_map(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then_some(trimmed)
    })
    .collect::<Vec<_>>()
    .join(" ")
}

fn bill_datetime(bill: &DedupBill) -> Option<NaiveDateTime> {
    parse_bill_datetime(&bill.date).map(|value| value.inner())
}

fn time_close(left: NaiveDateTime, right: NaiveDateTime, tolerance_seconds: i64) -> bool {
    (left - right).num_seconds().abs() <= tolerance_seconds
}

fn same_day(left: NaiveDateTime, right: NaiveDateTime) -> bool {
    left.date() == right.date()
}

fn abs_amount_close(left: Money, right: Money) -> bool {
    (abs_cents(left) - abs_cents(right)).abs() <= AMOUNT_TOLERANCE_CENTS
}

fn amount_equal_same_direction(left: Money, right: Money) -> bool {
    abs_amount_close(left, right) && amount_same_direction(left, right)
}

fn amount_opposite(left: Money, right: Money) -> bool {
    abs_amount_close(left, right) && i128::from(left.to_cents()) * i128::from(right.to_cents()) < 0
}

fn amount_same_direction(left: Money, right: Money) -> bool {
    (left.to_cents() >= 0 && right.to_cents() >= 0) || (left.to_cents() < 0 && right.to_cents() < 0)
}

fn abs_cents(amount: Money) -> i128 {
    let cents = i128::from(amount.to_cents());
    if cents < 0 {
        -cents
    } else {
        cents
    }
}

fn normalized_similarity(left: &str, right: &str) -> f64 {
    // 中文说明：用包含关系和 LCS 相似度判断账单文本接近程度，支撑重复、相似和 reconciliation 证据。
    let left = left.trim().to_lowercase();
    let right = right.trim().to_lowercase();
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left == right || left.contains(&right) || right.contains(&left) {
        return 1.0;
    }

    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    let lcs = longest_common_subsequence_len(&left_chars, &right_chars);
    (2.0 * lcs as f64) / (left_chars.len() + right_chars.len()) as f64
}
