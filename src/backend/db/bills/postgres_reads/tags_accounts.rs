/// 读取正式账单绑定的标签名称列表。
pub async fn get_postgres_bill_tags(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT t.id, t.name, t.color, t.metadata
        FROM bill_tags bt
        JOIN tags t ON t.user_id = bt.user_id AND t.id = bt.tag_id
        WHERE bt.user_id = $1 AND bt.bill_id = $2
        ORDER BY t.display_order ASC, t.name ASC
        "#,
    )
    .bind(user_id)
    .bind(bill_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(tag_value_from_postgres_row).collect()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 读取当前用户第一个可用账户，作为手工录入缺省账户兜底。
pub async fn get_first_postgres_account_id(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Option<i64>> {
    sqlx::query(
        r#"
        SELECT id
        FROM accounts
        WHERE user_id = $1 AND is_active = true
        ORDER BY display_order ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .map(|row| row.try_get("id").map_err(DbError::from))
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 在当前用户范围内按分类 id 解析主/子分类名称。
pub async fn resolve_postgres_category_by_id(
    pool: &PostgresPool,
    user_id: i64,
    category_id: i64,
) -> DbResult<Option<(String, String)>> {
    let row = sqlx::query(
        r#"
        SELECT path, name
        FROM categories
        WHERE user_id = $1 AND id = $2
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let path: Option<String> = row.try_get("path")?;
        let name: String = row.try_get("name")?;
        Ok(category_names_from_path(path.as_deref(), &name))
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 读取对账接口指定的账户基础信息和期初余额。
pub async fn get_postgres_reconciliation_account(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
) -> DbResult<Option<PostgresReconciliationAccount>> {
    let row = sqlx::query(
        r#"
        SELECT name, balance_cents, metadata
        FROM accounts
        WHERE user_id = $1 AND id = $2
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let metadata: Value = row.try_get("metadata")?;
        let balance_cents: i64 = row.try_get("balance_cents")?;
        Ok(PostgresReconciliationAccount {
            name: row.try_get("name")?,
            initial_balance: postgres_initial_balance_money(&metadata, balance_cents)?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 读取对账筛选所需的当前用户分类清单。
pub async fn list_postgres_reconciliation_categories(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<ReconciliationCategoryRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, path, name
        FROM categories
        WHERE user_id = $1
        ORDER BY display_order ASC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main_category, sub_category) = category_names_from_path(path.as_deref(), &name);
            Ok(ReconciliationCategoryRecord {
                id: row.try_get("id")?,
                main_category,
                sub_category,
            })
        })
        .collect()
}
