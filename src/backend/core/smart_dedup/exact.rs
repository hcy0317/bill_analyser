// 中文导读：同源完全重复账单折叠规则。
// 维护重点：在 smart dedup 早期按稳定 dedup key 合并完全重复项并保留来源证据。
// 不变式：完全重复只折叠当前 runtime 批次，不执行跨批或历史账单写入。

#[tracing::instrument(level = "debug", skip_all)]
fn find_exact_duplicates(bills: &mut [DedupBill]) -> Vec<DuplicateGroup> {
    let mut groups = Vec::new();
    let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();

    for (index, bill) in bills.iter().enumerate() {
        if !bill.removed {
            by_key.entry(dedup_key(bill)).or_default().push(index);
        }
    }

    for mut indices in by_key.into_values() {
        if indices.len() <= 1 {
            continue;
        }
        indices.sort_by_key(|index| source_priority(&bills[*index].source_type()));
        let keep_index = indices[0];
        let remove_indices = indices[1..].to_vec();
        for index in &remove_indices {
            bills[*index].removed = true;
            let remove_bill = bills[*index].clone();
            merge_bill_fields(&mut bills[keep_index], &remove_bill, true);
        }
        bills[keep_index].dedup_type = Some(DeduplicationType::Exact.as_str().to_string());
        groups.push(DuplicateGroup {
            dedup_type: DeduplicationType::Exact,
            bill_indices: indices,
            keep_index,
            remove_indices,
            reason: format!(
                "完全重复，保留 {} 来源",
                bills[keep_index].source_identifier()
            ),
        });
    }

    groups
}
