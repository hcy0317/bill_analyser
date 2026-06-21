/// 中文说明：按当前用户读取账户主数据列表，并保持 display_order 与名称排序合同。
pub async fn list_postgres_accounts(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<AccountRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, account_type, payment_method, currency,
            balance_cents, is_active, display_order, metadata, created_at, updated_at
        FROM accounts
        WHERE user_id = $1
        ORDER BY display_order ASC, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(account_from_postgres_row).collect()
}

/// 中文说明：按账户 ID 与用户边界读取单个账户，避免跨用户访问恢复或编辑数据。
pub async fn get_postgres_account_by_id(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<Option<AccountRecord>> {
    let row = sqlx::query(&account_select_sql("WHERE id = $1 AND user_id = $2"))
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(account_from_postgres_row).transpose()
}

/// 中文说明：读取指定父账户下的子账户，用于前端父子账户编辑和删除级联编排。
pub async fn get_postgres_sub_accounts(
    pool: &PostgresPool,
    parent_id: i64,
    user_id: i64,
) -> DbResult<Vec<AccountRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, account_type, payment_method, currency,
            balance_cents, is_active, display_order, metadata, created_at, updated_at
        FROM accounts
        WHERE user_id = $1 AND metadata->>'parent_id' = $2
        ORDER BY display_order ASC, name ASC
        "#,
    )
    .bind(user_id)
    .bind(parent_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(account_from_postgres_row).collect()
}

/// 中文说明：创建账户主数据，并复用账户插入 helper 统一处理金额分、metadata 与排序字段。
pub async fn create_postgres_account(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<i64> {
    let mut transaction = pool.begin().await?;
    let account_id = insert_postgres_account(&mut transaction, payload, user_id, None).await?;
    transaction.commit().await?;
    Ok(account_id)
}

/// 中文说明：更新账户主数据，保留当前 metadata 兼容字段并强制余额使用整数分。
pub async fn update_postgres_account(
    pool: &PostgresPool,
    account_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let existing = sqlx::query(
        r#"
        SELECT metadata
        FROM accounts
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let metadata = account_metadata_from_payload(Some(&existing.try_get("metadata")?), payload)?;
    let balance_cents = optional_strict_minor_units(
        payload
            .get("balanceCents")
            .or_else(|| payload.get("balance_cents")),
        "balanceCents",
    )?
    .unwrap_or_default();
    let changed = sqlx::query(
        r#"
        UPDATE accounts
        SET name = $1,
            account_type = $2,
            currency = $3,
            balance_cents = $4,
            is_active = $5,
            display_order = $6,
            metadata = $7,
            updated_at = now(),
            version = version + 1
        WHERE id = $8 AND user_id = $9
        "#,
    )
    .bind(value_text(payload.get("name")).unwrap_or_default())
    .bind(value_text(payload.get("type")))
    .bind(value_text(payload.get("currency")).unwrap_or_else(|| "CNY".to_string()))
    .bind(balance_cents)
    .bind(!payload.get("hidden").is_some_and(value_truthy))
    .bind(i64_to_i32(
        int_value(payload.get("display_order")).unwrap_or_default(),
    ))
    .bind(metadata)
    .bind(account_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：按用户边界删除账户记录；父子账户级联由上层 handler 显式编排。
pub async fn delete_postgres_account(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：批量保存账户展示顺序，事务内逐项校验 user-scope 防止排序串号。
pub async fn update_postgres_account_display_orders(
    pool: &PostgresPool,
    orders: &[AccountDisplayOrder],
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for order in orders {
        sqlx::query(
            r#"
            UPDATE accounts
            SET display_order = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(order.display_order))
        .bind(order.account_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}
