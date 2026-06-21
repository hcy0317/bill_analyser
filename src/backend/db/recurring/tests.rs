mod tests {
    use super::*;

    fn record(entries: impl IntoIterator<Item = (&'static str, Value)>) -> BillRecord {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    #[test]
    fn recurring_candidates_match_and_serialize_explicit_cents() {
        let bill = record([
            ("id", json!(100)),
            ("date", json!("2026-06-12T09:00:00Z")),
            ("type", json!("expense")),
            ("amount_cents", json!(-12345)),
            ("source_account_id", json!("11")),
            ("destination_account_id", json!("0")),
        ]);
        let recurring = record([
            ("id", json!(7)),
            ("name", json!("月租")),
            ("type", json!("expense")),
            ("category", json!("9")),
            ("account", json!("11")),
            ("counterparty", json!("0")),
            ("source_amount_cents", json!(12345)),
            ("destination_amount_cents", json!(0)),
            ("frequency", json!("monthly")),
            ("start_date", json!("2026-01-12")),
            ("next_date", json!("2026-06-12")),
            ("utc_offset", json!(480)),
            ("hide_amount", json!(0)),
            ("display_order", json!(1)),
            ("hidden", json!(0)),
            ("scheduled_frequency_type", json!(2)),
            ("tag", json!("1,2")),
            ("comment", json!("备注")),
        ]);

        let candidates = build_postgres_recurring_candidates_for_bill_data(
            &bill,
            std::slice::from_ref(&recurring),
            None,
            3,
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0]["sourceAmountCents"], json!(12345));
        assert_eq!(candidates[0]["destinationAmountCents"], json!(0));
        assert!(candidates[0]["matchReasons"]
            .as_array()
            .expect("reasons")
            .contains(&json!("amount")));

        let serialized = serialize_postgres_recurring_template_row(&recurring);
        assert_eq!(serialized.get("sourceAmountCents"), Some(&json!(12345)));
        assert_eq!(serialized.get("destinationAmountCents"), Some(&json!(0)));
        assert!(serialized.get("sourceAmount").is_none());
    }
}
