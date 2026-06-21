/// 按用户、筛选、分页和排序查询正式账单列表，并返回总数与记录。
pub async fn query_postgres_bills(
    pool: &PostgresPool,
    user_id: i64,
    page: usize,
    page_size: usize,
    filters: &BillFilters,
) -> DbResult<BillPage> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 500);
    let offset = (page - 1).saturating_mul(page_size);

    let mut count_builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT AS total FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ",
    );
    count_builder.push_bind(user_id);
    count_builder.push(" AND b.is_deleted = false");
    push_bill_filters(&mut count_builder, user_id, filters);
    let total = count_builder
        .build()
        .fetch_one(pool)
        .await?
        .try_get::<i64, _>("total")?;

    let mut list_builder = QueryBuilder::<Postgres>::new(
        "SELECT b.id, b.user_id, b.occurred_at, b.transaction_type, b.amount_cents, b.merchant, b.description, b.payment_method, ",
    );
    list_builder.push(MAIN_CATEGORY_EXPR);
    list_builder.push(" AS main_category, ");
    list_builder.push(SUB_CATEGORY_EXPR);
    list_builder.push(" AS sub_category, b.standard_payload, b.source_hash, b.created_at, b.updated_at, b.account_id, b.source_account_id, b.target_account_id, b.transfer_target_account_id, b.category_id FROM bills b LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id WHERE b.user_id = ");
    list_builder.push_bind(user_id);
    list_builder.push(" AND b.is_deleted = false");
    push_bill_filters(&mut list_builder, user_id, filters);
    list_builder.push(" ORDER BY b.occurred_at DESC, b.id DESC LIMIT ");
    list_builder.push_bind(i64::try_from(page_size).unwrap_or(i64::MAX));
    list_builder.push(" OFFSET ");
    list_builder.push_bind(i64::try_from(offset).unwrap_or(i64::MAX));

    let rows = list_builder.build().fetch_all(pool).await?;
    let bills = rows
        .into_iter()
        .map(bill_record_from_postgres_row)
        .collect::<DbResult<Vec<_>>>()?;
    Ok(BillPage { bills, total })
}

#[tracing::instrument(level = "debug", skip_all)]
/// 在当前用户范围内读取单笔正式账单详情。
pub async fn get_postgres_bill_by_id(
    pool: &PostgresPool,
    user_id: i64,
    bill_id: i64,
) -> DbResult<Option<BillRecord>> {
    let row = sqlx::query(&format!(
        r#"
        SELECT b.id, b.user_id, b.occurred_at, b.transaction_type, b.amount_cents,
            b.merchant, b.description, b.payment_method,
            {MAIN_CATEGORY_EXPR} AS main_category,
            {SUB_CATEGORY_EXPR} AS sub_category,
            b.standard_payload, b.source_hash, b.created_at, b.updated_at,
            b.account_id, b.source_account_id, b.target_account_id,
            b.transfer_target_account_id, b.category_id
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = $1 AND b.id = $2 AND b.is_deleted = false
        "#
    ))
    .bind(user_id)
    .bind(bill_id)
    .fetch_optional(pool)
    .await?;
    row.map(bill_record_from_postgres_row).transpose()
}
