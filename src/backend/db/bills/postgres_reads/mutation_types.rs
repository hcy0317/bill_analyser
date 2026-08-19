#[derive(Debug, Clone)]
struct PostgresBillMutation {
    occurred_at: DateTime<Utc>,
    amount_cents: i64,
    direction: String,
    transaction_type: String,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
    category_id: Option<i64>,
    merchant: Option<String>,
    description: Option<String>,
    payment_method: Option<String>,
    source_hash: Option<String>,
    standard_payload: Value,
}

#[derive(Debug, Clone)]
struct PreparedPostgresBillMutation {
    mutation: PostgresBillMutation,
    tag_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
struct LockedPostgresBill {
    bill_id: i64,
    record: BillRecord,
    version: i64,
}

#[derive(Debug, Clone)]
struct PreparedPostgresBillUpdate {
    bill_id: i64,
    expected_version: i64,
    old_mutation: PostgresBillMutation,
    new_mutation: PostgresBillMutation,
}

impl PostgresBillMutation {
    fn balance_deltas(&self) -> DbResult<Vec<(i64, i64)>> {
        project_postgres_bill_balance_deltas(
            &self.transaction_type,
            self.amount_cents,
            self.source_account_id,
            self.destination_account_id,
            destination_amount_cents(&self.standard_payload),
        )
    }
}
