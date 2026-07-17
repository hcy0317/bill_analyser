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
    fn balance_deltas(&self) -> Vec<(i64, i64)> {
        let mut deltas = Vec::new();
        let amount = self.amount_cents.abs();
        let destination_amount = destination_amount_cents(&self.standard_payload)
            .unwrap_or(amount)
            .abs();
        match self.transaction_type.as_str() {
            "income" => {
                if let Some(account_id) = self.source_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, amount));
                }
            }
            "transfer" | "investment" => {
                if let Some(account_id) = self.source_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, -amount));
                }
                if let Some(account_id) = self.destination_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, destination_amount));
                }
            }
            _ => {
                if let Some(account_id) = self.source_account_id.filter(|value| *value > 0) {
                    deltas.push((account_id, -amount));
                }
            }
        }
        deltas
    }
}
