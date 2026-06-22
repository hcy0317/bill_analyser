#[tracing::instrument(level = "debug", skip_all)]
fn find_split_bills(bills: &mut [DedupBill]) -> Vec<SplitGroup> {
    let mut groups = Vec::new();
    if !has_multiple_active_sources(bills, |bill| normalized_source(&bill.source_account_id)) {
        return groups;
    }

    let source_account_ids: Vec<String> = bills
        .iter()
        .map(|bill| normalized_source(&bill.source_account_id))
        .collect();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let mut matched_indices = HashSet::new();
    let active_indices: Vec<usize> = bills
        .iter()
        .enumerate()
        .filter_map(|(index, bill)| (!bill.removed && bill.amount != Money::ZERO).then_some(index))
        .collect();
    let split_groups_by_source = build_split_source_groups(
        active_indices.iter().copied(),
        &timestamps,
        &amount_cents,
        &source_account_ids,
    );
    let split_group_ranges = build_split_group_ranges(&split_groups_by_source, &amount_cents);

    for total_index in &active_indices {
        if matched_indices.contains(total_index) || abs_cents(bills[*total_index].amount) < 1_000 {
            continue;
        }
        let Some(total_ts) = timestamps[*total_index] else {
            continue;
        };
        let Some(total_dt) = datetimes[*total_index] else {
            continue;
        };
        let total_source = &source_account_ids[*total_index];
        if total_source.is_empty() {
            continue;
        }

        let total = SplitTotalCandidate {
            index: *total_index,
            timestamp: total_ts,
            datetime: total_dt,
            source: total_source,
            amount_cents: amount_cents[*total_index],
        };
        let Some((candidates, source_splits)) = find_split_candidates_for_total(
            &split_groups_by_source,
            &split_group_ranges,
            &amount_cents,
            &datetimes,
            &matched_indices,
            total,
        ) else {
            continue;
        };

        bills[*total_index].removed = true;
        matched_indices.insert(*total_index);
        let total_template_id = bills[*total_index].template_id.clone();
        for candidate_index in &candidates {
            matched_indices.insert(*candidate_index);
            bills[*candidate_index].dedup_type = Some("split".to_string());
            merge_template_id(&mut bills[*candidate_index], &total_template_id);
        }
        groups.push(SplitGroup {
            total_index: *total_index,
            split_indices: candidates,
            total_amount: bills[*total_index].amount,
            source_total: total_source.clone(),
            source_splits,
            reason: format!("总账单({total_source})拆分为多笔分账单"),
        });
    }

    groups
}

fn has_multiple_active_sources<F>(bills: &[DedupBill], mut source_for: F) -> bool
where
    F: FnMut(&DedupBill) -> String,
{
    let mut first_source: Option<String> = None;
    for bill in bills {
        if bill.removed {
            continue;
        }
        let source = source_for(bill);
        if source.is_empty() {
            continue;
        }
        match &first_source {
            Some(existing) if existing != &source => return true,
            Some(_) => {}
            None => first_source = Some(source),
        }
    }
    false
}

fn timestamps_from_datetimes(datetimes: &[Option<NaiveDateTime>]) -> Vec<Option<i64>> {
    datetimes
        .iter()
        .map(|datetime| datetime.map(|value| value.and_utc().timestamp()))
        .collect()
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
struct TimeAmountKey {
    bucket: i64,
    amount_cents: i128,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct SplitSourceKey {
    bucket: i64,
    direction: i8,
    source: String,
}

#[derive(Debug, Clone, Copy)]
struct SplitTotalCandidate<'a> {
    index: usize,
    timestamp: i64,
    datetime: NaiveDateTime,
    source: &'a str,
    amount_cents: i128,
}

#[derive(Debug, Clone, Copy)]
struct SplitAmountRange {
    min_possible_sum: i128,
    max_possible_sum: i128,
}

fn amount_cents_for_bills(bills: &[DedupBill]) -> Vec<i128> {
    bills
        .iter()
        .map(|bill| i128::from(bill.amount.to_cents()))
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_time_amount_buckets<I, F>(
    indices: I,
    timestamps: &[Option<i64>],
    mut amount_for: F,
) -> HashMap<TimeAmountKey, Vec<usize>>
where
    I: IntoIterator<Item = usize>,
    F: FnMut(usize) -> i128,
{
    let mut buckets: HashMap<TimeAmountKey, Vec<usize>> = HashMap::new();
    for index in indices {
        if let Some(timestamp) = timestamps.get(index).and_then(|value| *value) {
            buckets
                .entry(TimeAmountKey {
                    bucket: time_bucket_key(timestamp),
                    amount_cents: amount_for(index),
                })
                .or_default()
                .push(index);
        }
    }
    buckets
}

fn nearby_time_amount_indices(
    buckets: &HashMap<TimeAmountKey, Vec<usize>>,
    timestamp: i64,
    amount_cents: i128,
) -> impl Iterator<Item = usize> + '_ {
    let bucket = time_bucket_key(timestamp);
    (bucket - 1..=bucket + 1).flat_map(move |nearby_bucket| {
        buckets
            .get(&TimeAmountKey {
                bucket: nearby_bucket,
                amount_cents,
            })
            .into_iter()
            .flat_map(|values| values.iter().copied())
    })
}

fn amount_tolerance_values(amount_cents: i128) -> impl Iterator<Item = i128> {
    (amount_cents - AMOUNT_TOLERANCE_CENTS)..=(amount_cents + AMOUNT_TOLERANCE_CENTS)
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_split_source_groups<I>(
    indices: I,
    timestamps: &[Option<i64>],
    amount_cents: &[i128],
    source_account_ids: &[String],
) -> HashMap<SplitSourceKey, Vec<usize>>
where
    I: IntoIterator<Item = usize>,
{
    let mut groups: HashMap<SplitSourceKey, Vec<usize>> = HashMap::new();
    for index in indices {
        let Some(timestamp) = timestamps.get(index).and_then(|value| *value) else {
            continue;
        };
        let source = source_account_ids[index].clone();
        if source.is_empty() {
            continue;
        }
        let direction = amount_direction(amount_cents[index]);
        if direction == 0 {
            continue;
        }
        groups
            .entry(SplitSourceKey {
                bucket: time_bucket_key(timestamp),
                direction,
                source,
            })
            .or_default()
            .push(index);
    }
    groups
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_split_group_ranges(
    groups: &HashMap<SplitSourceKey, Vec<usize>>,
    amount_cents: &[i128],
) -> HashMap<SplitSourceKey, SplitAmountRange> {
    groups
        .iter()
        .filter_map(|(key, indices)| {
            split_group_possible_sum_range(indices, amount_cents).map(|range| (key.clone(), range))
        })
        .collect()
}

fn split_group_possible_sum_range(
    indices: &[usize],
    amount_cents: &[i128],
) -> Option<SplitAmountRange> {
    if indices.len() < 2 {
        return None;
    }

    let mut values = indices
        .iter()
        .map(|index| amount_cents[*index])
        .collect::<Vec<_>>();
    values.sort_unstable();

    let full_sum = values.iter().sum::<i128>();
    let first = *values.first()?;
    let second = values.get(1).copied()?;
    let last = *values.last()?;
    let before_last = values.get(values.len().saturating_sub(2)).copied()?;

    if last < 0 {
        Some(SplitAmountRange {
            min_possible_sum: full_sum,
            max_possible_sum: before_last + last,
        })
    } else if first > 0 {
        Some(SplitAmountRange {
            min_possible_sum: first + second,
            max_possible_sum: full_sum,
        })
    } else {
        None
    }
}

fn split_group_range_can_match(
    ranges: &HashMap<SplitSourceKey, SplitAmountRange>,
    key: &SplitSourceKey,
    target_amount_cents: i128,
) -> bool {
    ranges
        .get(key)
        .map(|range| {
            target_amount_cents >= range.min_possible_sum - AMOUNT_TOLERANCE_CENTS
                && target_amount_cents <= range.max_possible_sum + AMOUNT_TOLERANCE_CENTS
        })
        .unwrap_or(false)
}

#[tracing::instrument(level = "debug", skip_all)]
fn find_split_candidates_for_total(
    groups: &HashMap<SplitSourceKey, Vec<usize>>,
    group_ranges: &HashMap<SplitSourceKey, SplitAmountRange>,
    amount_cents: &[i128],
    datetimes: &[Option<NaiveDateTime>],
    matched_indices: &HashSet<usize>,
    total: SplitTotalCandidate<'_>,
) -> Option<(Vec<usize>, String)> {
    let direction = amount_direction(total.amount_cents);
    if direction == 0 {
        return None;
    }
    let total_bucket = time_bucket_key(total.timestamp);
    let mut source_keys = groups
        .keys()
        .filter(|key| {
            key.direction == direction
                && key.source != total.source
                && (total_bucket - 1..=total_bucket + 1).contains(&key.bucket)
        })
        .collect::<Vec<_>>();
    source_keys.sort_by(|left, right| {
        let left_first = groups
            .get(*left)
            .and_then(|indices| indices.iter().min())
            .copied()
            .unwrap_or(usize::MAX);
        let right_first = groups
            .get(*right)
            .and_then(|indices| indices.iter().min())
            .copied()
            .unwrap_or(usize::MAX);
        left_first
            .cmp(&right_first)
            .then_with(|| left.bucket.cmp(&right.bucket))
            .then_with(|| left.source.cmp(&right.source))
    });

    for key in source_keys {
        if !split_group_range_can_match(group_ranges, key, total.amount_cents) {
            continue;
        }
        let candidates = groups
            .get(key)?
            .iter()
            .copied()
            .filter(|candidate_index| {
                *candidate_index != total.index && !matched_indices.contains(candidate_index)
            })
            .filter(|candidate_index| {
                datetimes[*candidate_index]
                    .map(|candidate_dt| {
                        time_close(total.datetime, candidate_dt, TIME_TOLERANCE_SECONDS)
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if candidates.len() < 2 {
            continue;
        }
        let split_sum = candidates
            .iter()
            .map(|index| amount_cents[*index])
            .sum::<i128>();
        if (split_sum - total.amount_cents).abs() <= AMOUNT_TOLERANCE_CENTS {
            return Some((candidates, key.source.clone()));
        }
    }
    None
}

fn amount_direction(amount_cents: i128) -> i8 {
    match amount_cents.cmp(&0) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}
