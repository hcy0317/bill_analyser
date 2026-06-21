fn format_account_list_response(accounts: Vec<AccountRecord>) -> Value {
    let formatted = accounts
        .into_iter()
        .map(backend_account_to_frontend)
        .collect::<Vec<_>>();
    let mut children_by_parent: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut top_level = Vec::new();

    for account in formatted {
        let parent_id = account
            .get("parentId")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        if parent_id.is_empty() || parent_id == "0" {
            top_level.push(account);
        } else {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(Value::Object(account));
        }
    }

    for account in &mut top_level {
        if let Some(account_id) = account
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            if let Some(children) = children_by_parent.remove(&account_id) {
                if !children.is_empty() {
                    account.insert("subAccounts".to_string(), Value::Array(children));
                }
            }
        }
    }

    Value::Array(top_level.into_iter().map(Value::Object).collect())
}

fn format_sync_account_balances_response(result: SyncAllAccountBalancesResult) -> Value {
    json!({
        "total_accounts": result.total_accounts,
        "synced_accounts": result.synced_accounts,
        "discrepancies": result
            .discrepancies
            .into_iter()
            .map(format_account_balance_discrepancy)
            .collect::<Vec<_>>(),
        "errors": result.errors,
    })
}

fn format_account_balance_discrepancy(discrepancy: AccountBalanceDiscrepancy) -> Value {
    json!({
        "account_id": discrepancy.account_id,
        "name": discrepancy.name,
        "oldBalanceCents": discrepancy.old_balance_cents,
        "newBalanceCents": discrepancy.new_balance_cents,
        "diffCents": discrepancy.diff_cents,
    })
}
