#[cfg(test)]
mod tests {
    use super::*;

    fn budget_record(entries: impl IntoIterator<Item = (&'static str, Value)>) -> BudgetRecord {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    #[test]
    fn budget_create_update_and_import_helpers_require_explicit_cents() {
        let payload = budget_record([
            ("category", json!("餐饮")),
            ("sub_category", json!(" 午餐 ")),
            ("period_type", json!("monthly")),
            ("amount_cents", json!(12345)),
            ("start_date", json!("2026-06-01")),
        ]);

        let normalized =
            normalize_create_payload(&payload, "2026-06-12 12:00:00").expect("create payload");
        assert_eq!(normalized.get("amount_cents"), Some(&json!(12345)));
        assert_eq!(normalized.get("sub_category"), Some(&json!("午餐")));
        assert_eq!(normalized.get("enabled"), Some(&json!(true)));
        assert!(budget_import_has_required_name_and_amount(&budget_record(
            [("name", json!("餐饮预算")), ("amount_cents", json!(12345)),]
        )));
        assert!(!budget_import_has_required_name_and_amount(&budget_record(
            [("name", json!("餐饮预算")), ("amount", json!(123.45)),]
        )));

        let existing = budget_record([("sub_category", json!("旧午餐"))]);
        let update =
            normalize_update_payload(&existing, &budget_record([("amount_cents", json!(23456))]))
                .expect("update payload");
        assert_eq!(update.get("amount_cents"), Some(&json!(23456)));
        assert_eq!(update.get("sub_category"), Some(&json!("旧午餐")));

        assert!(normalize_create_payload(
            &budget_record([
                ("category", json!("餐饮")),
                ("period_type", json!("monthly")),
                ("amount", json!(123.45)),
                ("start_date", json!("2026-06-01")),
            ]),
            "2026-06-12 12:00:00",
        )
        .is_err());
        assert!(normalize_create_payload(
            &budget_record([
                ("category", json!("餐饮")),
                ("period_type", json!("monthly")),
                ("amount_cents", json!(true)),
                ("start_date", json!("2026-06-01")),
            ]),
            "2026-06-12 12:00:00",
        )
        .is_err());
        assert_eq!(
            record_i64(
                &budget_record([("amount_cents", json!(true))]),
                "amount_cents"
            ),
            None
        );
        assert!(normalize_update_payload(
            &BudgetRecord::new(),
            &budget_record([("amount_cents", json!(12.34))])
        )
        .is_err());
    }

    #[test]
    fn budget_execution_helpers_use_cents_for_selection_and_output() {
        let lower = budget_record([
            ("id", json!(1)),
            ("category", json!("餐饮")),
            ("sub_category", json!("午餐")),
            ("period_type", json!("monthly")),
            ("amount_cents", json!(1000)),
            ("start_date", json!("2026-06-01")),
        ]);
        let higher = budget_record([
            ("id", json!(2)),
            ("category", json!("餐饮")),
            ("sub_category", json!("午餐")),
            ("period_type", json!("monthly")),
            ("amount_cents", json!(2000)),
            ("start_date", json!("2026-06-01")),
            ("_resolved_budget_type", json!(3)),
        ]);

        let deduped = dedupe_budget_execution_candidates(vec![lower, higher.clone()]);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].get("id"), Some(&json!(2)));

        let categories = vec![json!({
            "id": 9,
            "main_category": "餐饮",
            "sub_category": "午餐",
            "type": 3,
            "icon": "meal",
            "color": "#f80"
        })];
        let context = bill_analyser_core::budgets::build_budget_category_context(&categories);
        let item = build_budget_execution_item(&higher, 1500, &context, 3);

        assert_eq!(item["budget_amount_cents"], json!(2000));
        assert_eq!(item["spent_amount_cents"], json!(1500));
        assert_eq!(item["remaining_amount_cents"], json!(500));
        assert_eq!(item["execution_rate"], json!(75.0));
        assert_eq!(item["category_id"], json!("9"));
    }
}
