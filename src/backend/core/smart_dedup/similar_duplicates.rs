// 中文说明：在不同来源之间识别金额、时间和文本相近的重复账单，按来源优先级决定保留项。
#[tracing::instrument(level = "debug", skip_all)]
fn find_similar_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    if !has_multiple_active_sources(bills, DedupBill::source_type) {
        return groups;
    }

    let source_types: Vec<String> = bills.iter().map(DedupBill::source_type).collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let time_amount_buckets =
        build_time_amount_buckets(0..bills.len(), &timestamps, |index| amount_cents[index]);
    let mut matched_indices = HashSet::new();

    for left_index in 0..bills.len() {
        if bills[left_index].removed || matched_indices.contains(&left_index) {
            continue;
        }
        let left_source = &source_types[left_index];
        if left_source.is_empty() {
            continue;
        }
        let Some(left_ts) = timestamps[left_index] else {
            continue;
        };
        let Some(left_dt) = datetimes[left_index] else {
            continue;
        };
        let mut matched_right_index = None;
        for candidate_amount in amount_tolerance_values(amount_cents[left_index]) {
            for right_index in
                nearby_time_amount_indices(&time_amount_buckets, left_ts, candidate_amount)
            {
                if right_index <= left_index {
                    continue;
                }
                if bills[right_index].removed || matched_indices.contains(&right_index) {
                    continue;
                }
                let right_source = &source_types[right_index];
                if left_source.is_empty() || right_source.is_empty() || left_source == right_source
                {
                    continue;
                }
                if !amount_equal_same_direction(bills[left_index].amount, bills[right_index].amount)
                {
                    continue;
                }
                let Some(right_dt) = datetimes[right_index] else {
                    continue;
                };
                if !time_close(left_dt, right_dt, TIME_TOLERANCE_SECONDS) {
                    continue;
                }
                matched_right_index = Some(right_index);
                break;
            }
            if matched_right_index.is_some() {
                break;
            }
        }

        let Some(right_index) = matched_right_index else {
            continue;
        };
        let counterparty_similarity = normalized_similarity(
            &bills[left_index].counterparty,
            &bills[right_index].counterparty,
        );
        let payment_similarity = normalized_similarity(
            &bills[left_index].payment_method,
            &bills[right_index].payment_method,
        );
        if counterparty_similarity < SIMILARITY_THRESHOLD
            && payment_similarity < SIMILARITY_THRESHOLD
        {
            continue;
        }

        let right_source = &source_types[right_index];
        let (keep_index, remove_index) =
            if source_priority(left_source) <= source_priority(right_source) {
                (left_index, right_index)
            } else {
                (right_index, left_index)
            };
        matched_indices.insert(keep_index);
        matched_indices.insert(remove_index);
        bills[remove_index].removed = true;
        let remove_bill = bills[remove_index].clone();
        merge_bill_fields(&mut bills[keep_index], &remove_bill, false);
        bills[keep_index].dedup_type = Some(DeduplicationType::Similar.as_str().to_string());
        groups.push(DuplicateGroup {
            dedup_type: DeduplicationType::Similar,
            bill_indices: vec![left_index, right_index],
            keep_index,
            remove_indices: vec![remove_index],
            reason: "类似账单去重".to_string(),
        });
    }

    groups
}
