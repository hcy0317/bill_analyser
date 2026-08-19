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
        let transaction_type = TransactionType::from_backend_name(&self.transaction_type)
            .unwrap_or(TransactionType::Expense);
        let effects = derive_ledger_balance_effects(LedgerBalanceInput {
            transaction_type,
            amount: Money::from_cents(self.amount_cents),
            destination_amount: destination_amount_cents(&self.standard_payload)
                .map(Money::from_cents),
            source_account_id: self.source_account_id,
            destination_account_id: self.destination_account_id,
        })
        .map_err(|error| DbError::InvalidOperation(error.to_string()))?;
        Ok(effects
            .legs()
            .map(|leg| (leg.account_id, leg.delta.to_cents()))
            .collect())
    }
}
