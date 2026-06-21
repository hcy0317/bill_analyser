/// 中文说明：读取当前用户交易模板或周期模板列表，按类型和 display_order 稳定排序。
pub async fn list_postgres_templates(
    pool: &PostgresPool,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<Vec<TemplateRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units::BIGINT AS source_amount_minor_units,
            destination_amount_minor_units::BIGINT AS destination_amount_minor_units,
            hide_amount,
            tag_ids, comment, scheduled_frequency_type, scheduled_frequency,
            scheduled_start_date, scheduled_end_date, scheduled_next_date,
            enabled, auto_create, display_order, hidden, utc_offset, created_at, updated_at
        FROM transaction_templates
        WHERE user_id = $1 AND ($2::BIGINT IS NULL OR template_type = $2)
        ORDER BY template_type ASC, display_order ASC, name ASC
        "#,
    )
    .bind(user_id)
    .bind(template_type)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(template_from_postgres_row).collect()
}

/// 中文说明：按模板 ID、用户和可选模板类型读取单条模板，防止跨类型误更新。
pub async fn get_postgres_template_by_id(
    pool: &PostgresPool,
    template_id: i64,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<Option<TemplateRecord>> {
    let sql = template_select_sql(
        "WHERE id = $1 AND user_id = $2 AND ($3::BIGINT IS NULL OR template_type = $3)",
    );
    let row = sqlx::query(&sql)
        .bind(template_id)
        .bind(user_id)
        .bind(template_type)
        .fetch_optional(pool)
        .await?;
    row.map(template_from_postgres_row).transpose()
}

/// 中文说明：创建交易模板，并把金额字段统一写入 explicit minor units 列。
pub async fn create_postgres_template(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<i64> {
    let template_type = int_value(payload.get("templateType")).unwrap_or(1);
    let display_order = next_template_display_order(pool, user_id, template_type).await?;
    let values = template_values_from_payload(payload, template_type, display_order, None)?;
    let row = sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units, destination_amount_minor_units,
            hide_amount, tag_ids, comment, scheduled_frequency_type,
            scheduled_frequency, scheduled_start_date, scheduled_end_date,
            scheduled_next_date, enabled, auto_create, display_order, hidden,
            utc_offset
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8,
            $9, $10,
            $11, $12, $13, $14,
            $15, $16, $17,
            $18, $19, $20, $21, $22,
            $23
        )
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(i64_to_i32(template_type))
    .bind(values.name)
    .bind(values.description)
    .bind(values.transaction_type)
    .bind(values.category_id)
    .bind(values.source_account_id)
    .bind(values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(values.tag_ids)
    .bind(values.comment)
    .bind(values.scheduled_frequency_type.map(i64_to_i32))
    .bind(values.scheduled_frequency)
    .bind(values.scheduled_start_date)
    .bind(values.scheduled_end_date)
    .bind(values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(i64_to_i32(values.utc_offset))
    .fetch_one(pool)
    .await?;
    Ok(row.try_get("id")?)
}

/// 中文说明：更新交易模板，复用现有模板类型并保持账户、分类、标签引用字段不漂移。
pub async fn update_postgres_template(
    pool: &PostgresPool,
    template_id: i64,
    payload: &Value,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<bool> {
    let Some(existing) =
        get_postgres_template_by_id(pool, template_id, user_id, template_type).await?
    else {
        return Ok(false);
    };
    let resolved_type = int_value(existing.get("templateType")).unwrap_or(1);
    let values = template_values_from_payload(payload, resolved_type, 0, Some(&existing))?;
    let changed = sqlx::query(
        r#"
        UPDATE transaction_templates
        SET name = $1,
            description = $2,
            transaction_type = $3,
            category_id = $4,
            source_account_id = $5,
            destination_account_id = $6,
            source_amount_minor_units = $7,
            destination_amount_minor_units = $8,
            hide_amount = $9,
            tag_ids = $10,
            comment = $11,
            scheduled_frequency_type = $12,
            scheduled_frequency = $13,
            scheduled_start_date = $14,
            scheduled_end_date = $15,
            scheduled_next_date = $16,
            enabled = $17,
            auto_create = $18,
            display_order = $19,
            hidden = $20,
            utc_offset = $21,
            updated_at = now(),
            version = version + 1
        WHERE id = $22 AND user_id = $23 AND template_type = $24
        "#,
    )
    .bind(values.name)
    .bind(values.description)
    .bind(values.transaction_type)
    .bind(values.category_id)
    .bind(values.source_account_id)
    .bind(values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(values.tag_ids)
    .bind(values.comment)
    .bind(values.scheduled_frequency_type.map(i64_to_i32))
    .bind(values.scheduled_frequency)
    .bind(values.scheduled_start_date)
    .bind(values.scheduled_end_date)
    .bind(values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(i64_to_i32(values.utc_offset))
    .bind(template_id)
    .bind(user_id)
    .bind(i64_to_i32(resolved_type))
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：按用户和可选模板类型删除模板，避免周期模板与普通模板互相影响。
pub async fn delete_postgres_template(
    pool: &PostgresPool,
    template_id: i64,
    user_id: i64,
    template_type: Option<i64>,
) -> DbResult<bool> {
    let changed = sqlx::query(
        r#"
        DELETE FROM transaction_templates
        WHERE id = $1 AND user_id = $2 AND ($3::BIGINT IS NULL OR template_type = $3)
        "#,
    )
    .bind(template_id)
    .bind(user_id)
    .bind(template_type)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：批量保存模板 display_order，并限定在同一模板类型下更新。
pub async fn update_postgres_template_display_orders(
    pool: &PostgresPool,
    orders: &[TemplateDisplayOrder],
    template_type: i64,
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for order in orders {
        sqlx::query(
            r#"
            UPDATE transaction_templates
            SET display_order = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3 AND template_type = $4
            "#,
        )
        .bind(i64_to_i32(order.display_order))
        .bind(order.template_id)
        .bind(user_id)
        .bind(i64_to_i32(template_type))
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}
