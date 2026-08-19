/// 中文说明：按当前用户读取分类主数据列表，保持前端树形分类的稳定排序输入。
pub async fn list_postgres_categories(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<CategoryRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at
        FROM categories
        WHERE user_id = $1
        ORDER BY display_order ASC, path ASC NULLS LAST, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(category_from_postgres_row).collect()
}

/// 中文说明：按分类 ID 与用户边界读取分类，作为编辑、删除和引用校验的统一入口。
pub async fn get_postgres_category_by_id(
    pool: &PostgresPool,
    category_id: i64,
    user_id: i64,
) -> DbResult<Option<CategoryRecord>> {
    let row = sqlx::query(&category_select_sql("WHERE id = $1 AND user_id = $2"))
        .bind(category_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    row.map(category_from_postgres_row).transpose()
}

/// 中文说明：按主/子分类名称解析 canonical 分类记录，用于导入、默认包和规则创建前校验。
pub async fn get_postgres_category_by_name(
    pool: &PostgresPool,
    main_category: &str,
    sub_category: &str,
    user_id: i64,
) -> DbResult<Option<CategoryRecord>> {
    let path = category_path(main_category, sub_category);
    let row = sqlx::query(&category_select_sql("WHERE user_id = $1 AND path = $2"))
        .bind(user_id)
        .bind(path)
        .fetch_optional(pool)
        .await?;
    row.map(category_from_postgres_row).transpose()
}

/// 中文说明：创建分类主数据，保持主分类去重、父分类解析和隐藏状态投影一致。
pub async fn create_postgres_category(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let values = category_values_from_payload(payload, None);
    if values.name.trim().is_empty() {
        return Err(DbError::InvalidOperation(
            "category name is required".to_string(),
        ));
    }
    if get_postgres_category_by_name(pool, &values.main_category, &values.sub_category, user_id)
        .await?
        .is_some()
    {
        return Ok(None);
    }
    let parent_id = parent_category_id_for_values(pool, user_id, &values).await?;
    let row = sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(values.name)
    .bind(values.category_type)
    .bind(values.path)
    .bind(values.icon)
    .bind(values.color)
    .bind(i64_to_i32(values.display_order))
    .bind(!values.hidden)
    .bind(values.metadata)
    .fetch_one(pool)
    .await?;
    Ok(Some(row.try_get("id")?))
}

/// 中文说明：更新分类主数据，同时重算父分类、path、样式字段和 metadata 兼容字段。
pub async fn update_postgres_category(
    pool: &PostgresPool,
    category_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let row = sqlx::query(&category_select_sql("WHERE id = $1 AND user_id = $2"))
        .bind(category_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let existing = category_from_postgres_row(row)?;
    let values = category_values_from_payload(payload, Some(&existing));
    let parent_id = parent_category_id_for_values(pool, user_id, &values).await?;
    let changed = sqlx::query(
        r#"
        UPDATE categories
        SET parent_id = $1,
            name = $2,
            category_type = $3,
            path = $4,
            icon = $5,
            color = $6,
            display_order = $7,
            is_active = $8,
            metadata = $9,
            updated_at = now(),
            version = version + 1
        WHERE id = $10 AND user_id = $11
        "#,
    )
    .bind(parent_id)
    .bind(values.name)
    .bind(values.category_type)
    .bind(values.path)
    .bind(values.icon)
    .bind(values.color)
    .bind(i64_to_i32(values.display_order))
    .bind(!values.hidden)
    .bind(values.metadata)
    .bind(category_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：删除分类；删除主分类时按当前合同删除该主分类及其子分类。
pub async fn delete_postgres_category(
    pool: &PostgresPool,
    category_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let Some(category) = get_postgres_category_by_id(pool, category_id, user_id).await? else {
        return Ok(false);
    };
    let main_category = value_text(category.get("main_category")).unwrap_or_default();
    let sub_category = value_text(category.get("sub_category")).unwrap_or_default();
    if sub_category.trim().is_empty() {
        return delete_postgres_categories_by_main_category(pool, &main_category, user_id).await;
    }
    let changed = sqlx::query("DELETE FROM categories WHERE id = $1 AND user_id = $2")
        .bind(category_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：按主分类名称批量删除分类树，服务于前端删除主分类的兼容行为。
pub async fn delete_postgres_categories_by_main_category(
    pool: &PostgresPool,
    main_category: &str,
    user_id: i64,
) -> DbResult<bool> {
    let like_pattern = format!("{}/%", main_category.trim());
    let changed = sqlx::query(
        r#"
        DELETE FROM categories
        WHERE user_id = $1 AND (path = $2 OR path LIKE $3)
        "#,
    )
    .bind(user_id)
    .bind(main_category.trim())
    .bind(like_pattern)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：保存单个分类 display_order，供前端拖拽排序逐项调用。
pub async fn update_postgres_category_display_order(
    pool: &PostgresPool,
    category_id: i64,
    display_order: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query(
        r#"
        UPDATE categories
        SET display_order = $1, updated_at = now(), version = version + 1
        WHERE id = $2 AND user_id = $3
        "#,
    )
    .bind(i64_to_i32(display_order))
    .bind(category_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：查询账单分类统计，统一按 cents 聚合金额并支持前端日期范围筛选。
pub async fn query_postgres_category_statistics(
    pool: &PostgresPool,
    start_date: Option<&str>,
    end_date: Option<&str>,
    user_id: i64,
) -> DbResult<Vec<CategoryStatistic>> {
    let mut builder = QueryBuilder::<Postgres>::new("SELECT ");
    push_postgres_bill_main_category_expr(&mut builder);
    builder.push(" AS main_category, ");
    push_postgres_bill_sub_category_expr(&mut builder);
    builder.push(
        r#" AS sub_category,
            COUNT(*)::BIGINT AS count,
            COALESCE(SUM(ABS(b.amount_cents)), 0)::BIGINT AS total_amount_cents
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = "#,
    );
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false AND ");
    push_postgres_bill_main_category_expr(&mut builder);
    builder.push(" <> ''");

    if let Some(value) = start_date.filter(|value| !value.trim().is_empty()) {
        builder.push(" AND b.occurred_at >= ");
        builder.push_bind(value.trim().to_string());
        builder.push("::date");
    }
    if let Some(value) = end_date.filter(|value| !value.trim().is_empty()) {
        builder.push(" AND b.occurred_at < (");
        builder.push_bind(value.trim().to_string());
        builder.push("::date + interval '1 day')");
    }

    builder.push(" GROUP BY ");
    push_postgres_bill_main_category_expr(&mut builder);
    builder.push(", ");
    push_postgres_bill_sub_category_expr(&mut builder);
    builder.push(" ORDER BY total_amount_cents DESC, main_category ASC, sub_category ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let total_amount_cents: i64 = row.try_get("total_amount_cents")?;
            Ok(CategoryStatistic {
                main_category: row
                    .try_get::<Option<String>, _>("main_category")?
                    .unwrap_or_default(),
                sub_category: row
                    .try_get::<Option<String>, _>("sub_category")?
                    .unwrap_or_default(),
                count: row.try_get("count")?,
                total_amount_cents,
            })
        })
        .collect()
}

/// 中文说明：重命名主分类并同步其子分类 path 前缀，保持 canonical path 不断裂。
pub async fn update_postgres_main_category_name(
    pool: &PostgresPool,
    old_name: &str,
    new_name: &str,
    user_id: i64,
) -> DbResult<bool> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    if old_name.is_empty() || new_name.is_empty() {
        return Ok(false);
    }
    if get_postgres_category_by_name(pool, new_name, "", user_id)
        .await?
        .is_some()
    {
        return Ok(false);
    }
    let mut transaction = pool.begin().await?;
    let parent_changed = sqlx::query(
        r#"
        UPDATE categories
        SET name = $1,
            path = $1,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $2 AND path = $3
        "#,
    )
    .bind(new_name)
    .bind(user_id)
    .bind(old_name)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    let child_pattern = format!("{old_name}/%");
    let child_changed = sqlx::query(
        r#"
        UPDATE categories
        SET path = $1 || substring(path FROM char_length($2) + 1),
            updated_at = now(),
            version = version + 1
        WHERE user_id = $3 AND path LIKE $4
        "#,
    )
    .bind(new_name)
    .bind(old_name)
    .bind(user_id)
    .bind(child_pattern)
    .execute(&mut *transaction)
    .await?
    .rows_affected();
    transaction.commit().await?;
    Ok(parent_changed > 0 || child_changed > 0)
}
