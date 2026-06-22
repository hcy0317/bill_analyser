// 中文说明：识别同批导入内支付平台与银行流水的重复记账，优先保留平台账单并合并银行来源信息。
#[tracing::instrument(level = "debug", skip_all)]
fn find_platform_bank_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let source_types: Vec<String> = bills.iter().map(DedupBill::source_type).collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let platform_indices: Vec<usize> = bills
        .iter()
        .enumerate()
        .filter_map(|(index, bill)| {
            (!bill.removed && is_platform_source(&source_types[index])).then_some(index)
        })
        .collect();
    let bank_indices: Vec<usize> = bills
        .iter()
        .enumerate()
        .filter_map(|(index, bill)| {
            (!bill.removed && is_bank_source(&source_types[index])).then_some(index)
        })
        .collect();

    if platform_indices.is_empty() || bank_indices.is_empty() {
        return groups;
    }

    let bank_time_amount_buckets =
        build_time_amount_buckets(bank_indices, &timestamps, |index| amount_cents[index].abs());
    let mut matched_bank_indices = HashSet::new();
    for platform_index in platform_indices {
        let Some(platform_ts) = timestamps[platform_index] else {
            continue;
        };
        let Some(platform_dt) = datetimes[platform_index] else {
            continue;
        };

        for target_amount in amount_tolerance_values(amount_cents[platform_index].abs()) {
            for bank_index in
                nearby_time_amount_indices(&bank_time_amount_buckets, platform_ts, target_amount)
            {
                if matched_bank_indices.contains(&bank_index) || bills[bank_index].removed {
                    continue;
                }
                let Some(bank_dt) = datetimes[bank_index] else {
                    continue;
                };
                if !time_close(platform_dt, bank_dt, TIME_TOLERANCE_SECONDS)
                    || !abs_amount_close(bills[platform_index].amount, bills[bank_index].amount)
                    || !is_platform_bank_duplicate_candidate(
                        &bills[platform_index],
                        &bills[bank_index],
                    )
                {
                    continue;
                }

                matched_bank_indices.insert(bank_index);
                bills[bank_index].removed = true;
                let bank_bill = bills[bank_index].clone();
                merge_bill_fields(&mut bills[platform_index], &bank_bill, true);
                bills[platform_index].dedup_type =
                    Some(DeduplicationType::PlatformBank.as_str().to_string());
                let bill_indices = vec![platform_index, bank_index];
                groups.push(DuplicateGroup {
                    dedup_type: DeduplicationType::PlatformBank,
                    bill_indices,
                    keep_index: platform_index,
                    remove_indices: vec![bank_index],
                    reason: format!(
                        "支付平台({})与银行({})重复，金额={}，保留平台账单",
                        bills[platform_index].source_type(),
                        bills[bank_index].source_type(),
                        bills[platform_index].amount.to_yuan_string()
                    ),
                });
            }
        }
    }

    groups
}
