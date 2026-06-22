#[tracing::instrument(level = "debug", skip_all)]
fn find_transfer_pairs(bills: &mut [DedupBill]) -> Vec<TransferPair> {
    let mut pairs = Vec::new();
    if !has_multiple_active_sources(bills, DedupBill::source_identifier) {
        return pairs;
    }

    let source_identifiers: Vec<String> = bills.iter().map(DedupBill::source_identifier).collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let time_amount_buckets =
        build_time_amount_buckets(0..bills.len(), &timestamps, |index| amount_cents[index]);
    let mut matched_indices = HashSet::new();

    for left_index in 0..bills.len() {
        if bills[left_index].removed
            || matched_indices.contains(&left_index)
            || bills[left_index].amount == Money::ZERO
        {
            continue;
        }
        let left_source = &source_identifiers[left_index];
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
        for target_amount in amount_tolerance_values(-amount_cents[left_index]) {
            for right_index in
                nearby_time_amount_indices(&time_amount_buckets, left_ts, target_amount)
            {
                if right_index <= left_index {
                    continue;
                }
                if bills[right_index].removed
                    || matched_indices.contains(&right_index)
                    || bills[right_index].amount == Money::ZERO
                {
                    continue;
                }
                let right_source = &source_identifiers[right_index];
                if right_source.is_empty() || right_source == left_source {
                    continue;
                }
                if !amount_opposite(bills[left_index].amount, bills[right_index].amount) {
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
        let (outgoing_index, incoming_index) = if bills[left_index].amount.is_negative() {
            (left_index, right_index)
        } else {
            (right_index, left_index)
        };
        matched_indices.insert(outgoing_index);
        matched_indices.insert(incoming_index);
        bills[outgoing_index].transaction_type = "转账".to_string();
        bills[outgoing_index].dedup_type = Some(DeduplicationType::Transfer.as_str().to_string());
        bills[incoming_index].transaction_type = "转账".to_string();
        bills[incoming_index].removed = true;
        bills[incoming_index].merged_into = bills[outgoing_index].template_id.clone();
        let incoming_bill = bills[incoming_index].clone();
        bills[outgoing_index].transfer_pair_order = Some("outgoing_first".to_string());
        bills[outgoing_index].transfer_pair_sources = vec![
            build_transfer_source_snapshot(&bills[outgoing_index], "outgoing"),
            build_transfer_source_snapshot(&incoming_bill, "incoming"),
        ];
        bills[outgoing_index].destination_parser_id = incoming_bill.parser_id.clone();
        bills[outgoing_index].destination_payment_method = incoming_bill.payment_method.clone();
        bills[outgoing_index].destination_counterparty = incoming_bill.counterparty.clone();
        bills[outgoing_index].destination_account_id =
            Some(incoming_bill.source_account_id.clone()).filter(|value| !value.is_empty());
        bills[outgoing_index].destination_account_name = first_non_empty([
            incoming_bill.account_name.as_str(),
            incoming_bill.payment_method.as_str(),
        ]);
        merge_template_ids_from_bill(&mut bills[outgoing_index], &incoming_bill);
        bills[outgoing_index].counterparty = merge_field_values(
            &bills[outgoing_index].counterparty,
            &incoming_bill.counterparty,
        );
        bills[outgoing_index].payment_method = merge_field_values(
            &bills[outgoing_index].payment_method,
            &incoming_bill.payment_method,
        );
        bills[outgoing_index].description = merge_field_values(
            &bills[outgoing_index].description,
            &incoming_bill.description,
        );
        pairs.push(TransferPair {
            outgoing_index,
            incoming_index,
            cross_batch_db_id: None,
        });
    }

    pairs
}
