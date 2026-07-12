fn transfer_source_label(sources: &[TransferSourceSnapshot]) -> String {
    let outgoing = sources
        .iter()
        .find(|source| source.role == "outgoing")
        .or_else(|| sources.first());
    let incoming = sources
        .iter()
        .find(|source| source.role == "incoming")
        .or_else(|| sources.get(1));
    let outgoing_label = outgoing
        .map(|source| parser_source_label(&source.parser_id).into_owned())
        .unwrap_or_default();
    let incoming_label = incoming
        .map(|source| parser_source_label(&source.parser_id).into_owned())
        .unwrap_or_default();
    format!("匹配 | {outgoing_label} | {incoming_label}")
}

fn merge_preview_text(left: &str, right: &str) -> String {
    let left = left.trim();
    let right = right.trim();
    if left.is_empty() {
        return right.to_string();
    }
    if right.is_empty() || left == right || left.contains(right) {
        return left.to_string();
    }
    if right.contains(left) {
        return right.to_string();
    }
    let mut values = Vec::new();
    for value in [left, right] {
        for part in value
            .split('|')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if !values.iter().any(|existing: &String| {
                existing == part || existing.contains(part) || part.contains(existing)
            }) {
                values.push(part.to_string());
            }
        }
    }
    values.join(" | ")
}

fn history_summary(row: &ImportHistoryBillRow) -> Value {
    let snapshot_text = |keys: &[&str]| -> String {
        keys.iter()
            .find_map(|key| row.snapshot.get(*key).and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_default()
            .to_string()
    };
    let category_name = snapshot_text(&["categoryName", "category_name", "category"]);
    let source_account_name = snapshot_text(&[
        "sourceAccountName",
        "source_account_name",
        "accountName",
        "account_name",
    ]);
    let destination_account_name = snapshot_text(&[
        "destinationAccountName",
        "destination_account_name",
    ]);
    let snapshot_bool = |keys: &[&str]| -> bool {
        keys.iter()
            .any(|key| row.snapshot.get(*key).and_then(Value::as_bool) == Some(true))
    };
    let category_deleted = snapshot_bool(&["categoryDeleted", "category_deleted"]);
    let source_account_deleted = snapshot_bool(&[
        "sourceAccountDeleted",
        "source_account_deleted",
        "accountDeleted",
        "account_deleted",
    ]);
    let destination_account_deleted = snapshot_bool(&[
        "destinationAccountDeleted",
        "destination_account_deleted",
    ]);
    let fallback_category = [row.bill.main_category.trim(), row.bill.sub_category.trim()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
    let currency = snapshot_text(&["currency"]);
    let currency = if currency.is_empty() {
        "CNY".to_string()
    } else {
        currency
    };
    let category_name = if category_name.is_empty() {
        fallback_category
    } else {
        category_name
    };
    let source_account_name = if source_account_name.is_empty() {
        let fallback = row.bill.account_name.trim();
        fallback.to_string()
    } else {
        source_account_name
    };
    let destination_account_name = if destination_account_name.is_empty() {
        Value::Null
    } else {
        json!(destination_account_name)
    };
    let category_status = if category_deleted {
        "deleted"
    } else if category_name.is_empty() {
        "unknown"
    } else {
        "known"
    };
    let source_account_status = if source_account_deleted {
        "deleted"
    } else if source_account_name.is_empty() {
        "unknown"
    } else {
        "known"
    };
    let destination_account_status = if destination_account_deleted {
        "deleted"
    } else if destination_account_name.is_null() {
        "unknown"
    } else {
        "known"
    };
    let identity_source = snapshot_text(&["identitySource", "identity_source"]);
    let identity_source = if identity_source.is_empty() {
        "snapshot_fallback".to_string()
    } else {
        identity_source
    };
    json!({
        "bill_id": row.history_bill_id,
        "date_time": row.bill.date,
        "amount_cents": row.bill.amount.to_cents(),
        "currency": currency,
        "category_name": category_name,
        "category_status": category_status,
        "source_account_name": source_account_name,
        "source_account_status": source_account_status,
        "destination_account_name": destination_account_name,
        "destination_account_status": destination_account_status,
        "identity_source": identity_source,
        "counterparty": row.bill.counterparty,
        "description": row.bill.description,
    })
}
