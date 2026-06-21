#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：同步父账户的子账户集合，更新已有子账户、创建新子账户并删除已移除项。
async fn update_postgres_sub_accounts(
    pool: &bill_analyser_db::PostgresPool,
    account_id: i64,
    user_id: i64,
    sub_accounts: &[Value],
) -> RouteResult<()> {
    let existing_sub_accounts = get_postgres_sub_accounts(pool, account_id, user_id)
        .await
        .map_err(|_| Box::new(db_error_response()))?;
    let existing_sub_ids = existing_sub_accounts
        .iter()
        .filter_map(|account| account.get("id").and_then(value_as_i64))
        .collect::<Vec<_>>();
    let mut updated_sub_ids = Vec::new();

    for sub_account in sub_accounts {
        let sub_account_id = sub_account.get("id").and_then(value_as_i64);
        let mut sub_payload =
            frontend_account_to_backend(sub_account).map_err(|message| Box::new(bad_request(message)))?;
        sub_payload.insert(
            "parent_id".to_string(),
            Value::Number(Number::from(account_id)),
        );
        if let Some(sub_account_id) =
            sub_account_id.filter(|candidate| existing_sub_ids.contains(candidate))
        {
            update_postgres_account(pool, sub_account_id, &Value::Object(sub_payload), user_id)
                .await
                .map_err(|_| Box::new(db_error_response()))?;
            updated_sub_ids.push(sub_account_id);
            continue;
        }

        create_postgres_account(pool, &Value::Object(sub_payload), user_id)
            .await
            .map_err(|_| Box::new(db_error_response()))?;
    }

    for old_sub_id in existing_sub_ids {
        if !updated_sub_ids.contains(&old_sub_id) {
            delete_postgres_account(pool, old_sub_id, user_id)
                .await
                .map_err(|_| Box::new(db_error_response()))?;
        }
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：读取账户详情并附带子账户数组，保证编辑页拿到完整父子账户结构。
async fn load_postgres_account_with_sub_accounts(
    pool: &bill_analyser_db::PostgresPool,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<AccountRecord>> {
    let Some(mut account) = get_postgres_account_by_id(pool, account_id, user_id).await? else {
        return Ok(None);
    };
    let sub_accounts = get_postgres_sub_accounts(pool, account_id, user_id).await?;
    if !sub_accounts.is_empty() {
        account.insert(
            "subAccounts".to_string(),
            Value::Array(sub_accounts.into_iter().map(Value::Object).collect()),
        );
    }
    Ok(Some(account))
}
