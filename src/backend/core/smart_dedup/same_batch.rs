// 中文导读：同批导入重复账单折叠规则。
// 维护重点：按秒级时间窗口、同向金额与来源优先级合并同批重复账单。
// 不变式：只修改当前批次 DedupBill runtime 标记，不触碰数据库状态。

#[tracing::instrument(level = "debug", skip_all)]
fn find_same_batch_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let datetimes: Vec<Option<NaiveDateTime>> = bills.iter().map(bill_datetime).collect();
    let timestamps = timestamps_from_datetimes(&datetimes);
    let amount_cents = amount_cents_for_bills(bills);
    let mut by_amount: HashMap<i128, Vec<usize>> = HashMap::new();

    for (index, bill) in bills.iter().enumerate() {
        if bill.removed || amount_cents[index] == 0 || timestamps[index].is_none() {
            continue;
        }
        by_amount
            .entry(amount_cents[index])
            .or_default()
            .push(index);
    }

    let mut matched_indices = HashSet::new();
    for indices in by_amount.values_mut() {
        indices.sort_by_key(|index| timestamps[*index].unwrap_or_default());
        let mut cluster = Vec::new();
        let mut cluster_start_ts = None;

        for index in indices.iter().copied() {
            if matched_indices.contains(&index) || bills[index].removed {
                continue;
            }
            let Some(timestamp) = timestamps[index] else {
                continue;
            };
            match cluster_start_ts {
                Some(start_ts) if timestamp - start_ts <= TIME_TOLERANCE_SECONDS => {
                    cluster.push(index);
                }
                _ => {
                    append_same_batch_duplicate_group(
                        bills,
                        &datetimes,
                        &mut matched_indices,
                        &mut groups,
                        &cluster,
                    );
                    cluster.clear();
                    cluster.push(index);
                    cluster_start_ts = Some(timestamp);
                }
            }
        }
        append_same_batch_duplicate_group(
            bills,
            &datetimes,
            &mut matched_indices,
            &mut groups,
            &cluster,
        );
    }

    groups
}

fn append_same_batch_duplicate_group(
    bills: &mut [DedupBill],
    datetimes: &[Option<NaiveDateTime>],
    matched_indices: &mut HashSet<usize>,
    groups: &mut Vec<DuplicateGroup>,
    cluster: &[usize],
) {
    if cluster.len() <= 1 {
        return;
    }
    let indices = cluster
        .iter()
        .copied()
        .filter(|index| !matched_indices.contains(index) && !bills[*index].removed)
        .collect::<Vec<_>>();
    if indices.len() <= 1 {
        return;
    }
    let components = same_batch_duplicate_components(&*bills, indices);
    for component in components {
        append_same_batch_duplicate_component(bills, datetimes, matched_indices, groups, component);
    }
}

// 中文说明：识别同批同金额同时间窗口内的重复组件，按来源优先级保留一笔并合并其余来源信息。
fn append_same_batch_duplicate_component(
    bills: &mut [DedupBill],
    datetimes: &[Option<NaiveDateTime>],
    matched_indices: &mut HashSet<usize>,
    groups: &mut Vec<DuplicateGroup>,
    mut indices: Vec<usize>,
) {
    if indices.len() <= 1 {
        return;
    }
    indices.sort_by(|left, right| {
        same_batch_source_priority(&bills[*left])
            .cmp(&same_batch_source_priority(&bills[*right]))
            .then_with(|| datetimes[*left].cmp(&datetimes[*right]))
            .then_with(|| left.cmp(right))
    });
    let keep_index = indices[0];
    let remove_indices = indices[1..].to_vec();
    for index in &indices {
        matched_indices.insert(*index);
    }
    for index in &remove_indices {
        bills[*index].removed = true;
        let remove_bill = bills[*index].clone();
        merge_bill_fields(&mut bills[keep_index], &remove_bill, true);
    }
    bills[keep_index].dedup_type = Some(DeduplicationType::SameBatch.as_str().to_string());
    groups.push(DuplicateGroup {
        dedup_type: DeduplicationType::SameBatch,
        bill_indices: indices,
        keep_index,
        remove_indices,
        reason: format!(
            "同批导入重复，按秒级时间窗口与同向金额合并，保留 {} 来源",
            bills[keep_index].source_identifier()
        ),
    });
}

fn same_batch_duplicate_components(bills: &[DedupBill], indices: Vec<usize>) -> Vec<Vec<usize>> {
    // 中文说明：把同时间金额簇继续拆成有文本证据连通的重复组件，避免无关同金额账单误合并。
    let mut remaining = indices;
    let mut components = Vec::new();
    while let Some(seed) = remaining.pop() {
        let mut component = vec![seed];
        let mut changed = true;
        while changed {
            changed = false;
            let mut next_remaining = Vec::new();
            for candidate in remaining {
                if component.iter().any(|member| {
                    same_batch_pair_has_duplicate_evidence(&bills[*member], &bills[candidate])
                }) {
                    component.push(candidate);
                    changed = true;
                } else {
                    next_remaining.push(candidate);
                }
            }
            remaining = next_remaining;
        }
        components.push(component);
    }
    components
}

// 中文说明：复用 reconciliation 文本证据判断同批重复，避免同金额但描述无关的账单被合并。
fn same_batch_pair_has_duplicate_evidence(left: &DedupBill, right: &DedupBill) -> bool {
    has_reconciliation_duplicate_evidence(left, right)
}

fn same_batch_source_priority(bill: &DedupBill) -> i32 {
    let source = bill.source_type();
    if is_bank_source(&source) {
        0
    } else if is_platform_source(&source) {
        10
    } else {
        20 + source_priority(&source)
    }
}
