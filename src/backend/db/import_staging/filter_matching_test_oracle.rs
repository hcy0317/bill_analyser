fn sort_preview_rows(rows: &mut [ImportPreviewRow], sort_by: &str, sort_direction: &str) {
    let descending = sort_direction.eq_ignore_ascii_case("desc");
    rows.sort_by(|left, right| {
        let order = match sort_by {
            "amount_cents"
            | "preview_amount_cents"
            | "previewAmountCents"
            | "sourceAmountCents" => left
                .preview_amount_cents
                .cmp(&right.preview_amount_cents)
                .then(left.id.cmp(&right.id)),
            "counterparty" => left
                .preview_counterparty
                .cmp(&right.preview_counterparty)
                .then(left.id.cmp(&right.id)),
            "type" => preview_type_sort_rank(&left.preview_type)
                .cmp(&preview_type_sort_rank(&right.preview_type))
                .then(left.preview_type.cmp(&right.preview_type))
                .then(left.id.cmp(&right.id)),
            "paymentMethod" => left
                .preview_payment_method
                .cmp(&right.preview_payment_method)
                .then(left.id.cmp(&right.id)),
            "comment" => left
                .preview_description
                .cmp(&right.preview_description)
                .then(left.id.cmp(&right.id)),
            "time" => left
                .preview_date
                .cmp(&right.preview_date)
                .then(left.id.cmp(&right.id)),
            _ => left
                .preview_date
                .cmp(&right.preview_date)
                .then(left.id.cmp(&right.id)),
        };
        if descending {
            order.reverse()
        } else {
            order
        }
    });
}

fn preview_type_sort_rank(value: &str) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => 0,
        "支出" | "expense" | "3" => 1,
        "转账" | "transfer" | "4" => 2,
        "投资" | "investment" | "5" => 3,
        _ => 4,
    }
}
