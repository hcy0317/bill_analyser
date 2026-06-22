fn time_bucket_key(timestamp: i64) -> i64 {
    timestamp.div_euclid(TIME_TOLERANCE_SECONDS.max(1) + 1)
}

fn dedup_key(bill: &DedupBill) -> String {
    format!(
        "{}|{}|{}|{}",
        bill.date,
        bill.amount.to_cents(),
        bill.counterparty,
        bill.description
    )
}

fn clean_runtime_markers(mut bill: DedupBill) -> DedupBill {
    bill.removed = false;
    bill.duplicate_of_db_id = None;
    bill
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_template_id(target: &mut DedupBill, template_id: &Option<String>) {
    if let Some(template_id) = template_id {
        append_id(&mut target.merged_template_ids, template_id);
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_template_ids_from_bill(target: &mut DedupBill, secondary: &DedupBill) {
    append_optional_id(&mut target.merged_template_ids, &secondary.template_id);
    append_ids(
        &mut target.merged_template_ids,
        &secondary.merged_template_ids,
    );
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_bill_fields(target: &mut DedupBill, secondary: &DedupBill, merge_parser_tags: bool) {
    target.counterparty = merge_field_values(&target.counterparty, &secondary.counterparty);
    target.payment_method = merge_field_values(&target.payment_method, &secondary.payment_method);
    target.description = merge_field_values(&target.description, &secondary.description);
    target.merged_from.push(MergedBillSource {
        source: secondary.source_identifier(),
        date: secondary.date.clone(),
        amount: Some(secondary.amount.to_yuan_string()),
        template_id: secondary.template_id.clone(),
    });
    merge_template_ids_from_bill(target, secondary);
    if merge_parser_tags {
        merge_parser_tags_from_bill(target, secondary);
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_field_values(left: &str, right: &str) -> String {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() && right.is_empty() {
        return String::new();
    }
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() || left == right || left.contains(right) {
        return left.to_string();
    }
    if right.contains(left) {
        return right.to_string();
    }

    let mut parts = Vec::new();
    let mut seen = HashSet::new();
    for value in [left, right] {
        for part in value
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if let Some(existing_index) = parts
                .iter()
                .position(|existing: &String| part.contains(existing) || existing.contains(part))
            {
                if part.len() > parts[existing_index].len() {
                    seen.remove(&parts[existing_index]);
                    parts[existing_index] = part.to_string();
                    seen.insert(part.to_string());
                }
            } else if seen.insert(part.to_string()) {
                parts.push(part.to_string());
            }
        }
    }
    parts.join(" | ")
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_parser_tags_from_bill(target: &mut DedupBill, secondary: &DedupBill) {
    let mut tags = normalized_parser_tags(target);
    for tag in normalized_parser_tags(secondary) {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    target.parser_tags = tags;
}

fn normalized_parser_tags(bill: &DedupBill) -> Vec<String> {
    let mut tags = Vec::new();
    for tag in &bill.parser_tags {
        let tag = tag.trim().to_lowercase();
        if !tag.is_empty() && !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    if tags.is_empty() {
        let source_type = bill.source_type();
        if !source_type.is_empty() {
            tags.push(format!("parser:{source_type}"));
        }
    }
    tags
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_transfer_source_snapshot(bill: &DedupBill, role: &str) -> TransferSourceSnapshot {
    TransferSourceSnapshot {
        role: role.to_string(),
        original_type: bill.original_type.clone(),
        parser_id: bill.parser_id.clone(),
        source: bill.source_identifier(),
        payment_method: bill.payment_method.clone(),
        counterparty: bill.counterparty.clone(),
        description: bill.description.clone(),
        source_account_id: Some(bill.source_account_id.clone()).filter(|value| !value.is_empty()),
        account_name: first_non_empty([bill.account_name.as_str(), bill.payment_method.as_str()]),
        bill_id: bill.id.clone(),
        template_id: bill.template_id.clone(),
        tags: normalized_parser_tags(bill),
    }
}

fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .find_map(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then_some(trimmed.to_string())
        })
        .unwrap_or_default()
}

fn append_optional_id(target: &mut Vec<String>, value: &Option<String>) {
    if let Some(value) = value {
        append_id(target, value);
    }
}

fn append_ids(target: &mut Vec<String>, values: &[String]) {
    for value in values {
        append_id(target, value);
    }
}

fn append_id(target: &mut Vec<String>, value: &str) {
    let value = value.trim();
    if !value.is_empty() && !target.iter().any(|existing| existing == value) {
        target.push(value.to_string());
    }
}
