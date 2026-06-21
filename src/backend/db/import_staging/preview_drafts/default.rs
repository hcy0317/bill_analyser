impl Default for ImportPreviewDraft {
    fn default() -> Self {
        Self {
            preview_date: String::new(),
            preview_type: String::new(),
            preview_amount_cents: 0,
            preview_destination_amount_cents: 0,
            category_id: None,
            preview_main_category: String::new(),
            preview_sub_category: String::new(),
            preview_source_account_id: None,
            preview_destination_account_id: None,
            preview_counterparty: String::new(),
            preview_payment_method: String::new(),
            preview_description: String::new(),
            preview_parser_id: String::new(),
            preview_parser_tags: None,
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: false,
            dedup_type: None,
            dedup_source_ids: Vec::new(),
            preview_matching_feedback: Value::Object(Default::default()),
        }
    }
}
