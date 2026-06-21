/// 中文说明：按当前用户读取标签列表，并保持 display_order 优先的展示顺序。
pub async fn list_postgres_tags(pool: &PostgresPool, user_id: i64) -> DbResult<Vec<TagRecord>> {
    let rows = sqlx::query(
        r#"
        SELECT id, user_id, name, color, display_order, metadata, created_at, updated_at
        FROM tags
        WHERE user_id = $1
        ORDER BY display_order ASC, created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(tag_from_postgres_row).collect()
}

/// 中文说明：按标签 ID 与用户边界读取单个标签，供编辑和删除前校验复用。
pub async fn get_postgres_tag(
    pool: &PostgresPool,
    tag_id: i64,
    user_id: i64,
) -> DbResult<Option<TagRecord>> {
    let row = sqlx::query(
        r#"
        SELECT id, user_id, name, color, display_order, metadata, created_at, updated_at
        FROM tags
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(tag_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(tag_from_postgres_row).transpose()
}

/// 中文说明：创建标签主数据，统一处理名称必填、颜色、排序和 metadata 字段。
pub async fn create_postgres_tag(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<i64> {
    let name = value_text(payload.get("name"))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DbError::InvalidOperation("name is required".to_string()))?;
    let color = value_text(payload.get("color")).unwrap_or_else(|| "#000000".to_string());
    let display_order = i64_to_i32(int_value(payload.get("display_order")).unwrap_or_default());
    let metadata = tag_metadata_from_payload(None, payload);

    let row = sqlx::query(
        r#"
        INSERT INTO tags (user_id, name, color, display_order, metadata)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(color)
    .bind(display_order)
    .bind(metadata)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get("id")?)
}

/// 中文说明：更新标签主数据，未传字段沿用已有值并保留 hidden/icon metadata。
pub async fn update_postgres_tag(
    pool: &PostgresPool,
    tag_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let existing = sqlx::query(
        r#"
        SELECT name, color, display_order, metadata
        FROM tags
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(tag_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let existing_metadata: Value = existing.try_get("metadata")?;
    let name = value_text(payload.get("name"))
        .unwrap_or_else(|| existing.try_get::<String, _>("name").unwrap_or_default());
    let color = if payload.get("color").is_some() {
        value_text(payload.get("color"))
    } else {
        existing.try_get("color")?
    };
    let display_order = payload
        .get("display_order")
        .and_then(|value| int_value(Some(value)))
        .map(i64_to_i32)
        .unwrap_or(existing.try_get("display_order")?);
    let metadata = tag_metadata_from_payload(Some(&existing_metadata), payload);

    let changed = sqlx::query(
        r#"
        UPDATE tags
        SET name = $1,
            color = $2,
            display_order = $3,
            metadata = $4,
            updated_at = now(),
            version = version + 1
        WHERE id = $5 AND user_id = $6
        "#,
    )
    .bind(name.trim())
    .bind(color)
    .bind(display_order)
    .bind(metadata)
    .bind(tag_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：按用户边界删除标签记录，避免跨用户标签被误删。
pub async fn delete_postgres_tag(pool: &PostgresPool, tag_id: i64, user_id: i64) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(tag_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：批量保存标签 display_order，服务于桌面标签页拖拽排序。
pub async fn update_postgres_tag_display_orders(
    pool: &PostgresPool,
    orders: &[TagDisplayOrder],
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for order in orders {
        sqlx::query(
            r#"
            UPDATE tags
            SET display_order = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(order.display_order))
        .bind(order.tag_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}
