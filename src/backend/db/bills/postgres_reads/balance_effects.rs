fn project_postgres_bill_balance_deltas(
    transaction_type: &str,
    amount_cents: i64,
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
    destination_amount_cents: Option<i64>,
) -> DbResult<Vec<(i64, i64)>> {
    let transaction_type = TransactionType::from_backend_name(transaction_type)
        .unwrap_or(TransactionType::Expense);
    let effects = derive_ledger_balance_effects(LedgerBalanceInput {
        transaction_type,
        amount: Money::from_cents(amount_cents),
        destination_amount: destination_amount_cents.map(Money::from_cents),
        source_account_id,
        destination_account_id,
    })
    .map_err(|error| DbError::InvalidOperation(error.to_string()))?;
    Ok(effects
        .legs()
        .map(|leg| (leg.account_id, leg.delta.to_cents()))
        .collect())
}
