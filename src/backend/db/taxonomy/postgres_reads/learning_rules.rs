/// 学习规则允许编辑的 PostgreSQL 字段；`learned_category_id` 的外层 Option 区分省略与显式清空。
#[derive(Debug, Clone, Default)]
pub struct PostgresLearningRuleUpdate {
    pub match_value: Option<String>,
    pub match_features: Option<Value>,
    pub learned_type: Option<String>,
    pub learned_category_id: Option<Option<i64>>,
    pub enabled: Option<bool>,
}

/// 统计当前用户 PostgreSQL lifecycle 学习规则，保持 enabled 过滤与列表一致。
pub async fn count_postgres_learning_rules(
    pool: &PostgresPool,
    user_id: i64,
    enabled_only: Option<bool>,
) -> DbResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM import_learning_lifecycle
        WHERE user_id = $1
          AND (
            $2::BOOLEAN IS NULL
            OR (status NOT IN ('suppressed', 'disabled')) = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(enabled_only)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

/// 分页读取当前用户 PostgreSQL lifecycle 学习规则，返回学习中心约定的 snake_case DTO。
pub async fn list_postgres_learning_rules(
    pool: &PostgresPool,
    user_id: i64,
    enabled_only: Option<bool>,
    limit: usize,
    offset: usize,
) -> DbResult<Vec<Value>> {
    let limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let offset = i64::try_from(offset).unwrap_or(i64::MAX);
    let rows = sqlx::query(
        r#"
        SELECT id, recommendation_type, recommendation_key, status,
               accepted_count, auto_applied_count, last_feedback_at,
               metadata, created_at, updated_at, version
        FROM import_learning_lifecycle
        WHERE user_id = $1
          AND (
            $2::BOOLEAN IS NULL
            OR (status NOT IN ('suppressed', 'disabled')) = $2
          )
        ORDER BY updated_at DESC, id DESC
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(user_id)
    .bind(enabled_only)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id")?;
            let recommendation_type: String = row.try_get("recommendation_type")?;
            let recommendation_key: String = row.try_get("recommendation_key")?;
            let status: String = row.try_get("status")?;
            let accepted_count: i32 = row.try_get("accepted_count")?;
            let auto_applied_count: i32 = row.try_get("auto_applied_count")?;
            let last_feedback_at: Option<DateTime<Utc>> = row.try_get("last_feedback_at")?;
            let metadata: Value = row.try_get("metadata")?;
            let created_at: DateTime<Utc> = row.try_get("created_at")?;
            let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
            let version: i64 = row.try_get("version")?;
            let match_features_json = metadata
                .get("match_features")
                .map(Value::to_string)
                .unwrap_or_else(|| "{}".to_string());

            Ok(json!({
                "id": id,
                "match_type": metadata
                    .get("match_type")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_type.as_str()),
                "match_value": metadata
                    .get("match_value")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_key.as_str()),
                "learned_type": metadata
                    .get("learned_type")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_type.as_str()),
                "learned_category_id": metadata
                    .get("learned_category_id")
                    .cloned()
                    .unwrap_or(Value::Null),
                "learned_source_account_id": metadata
                    .get("learned_source_account_id")
                    .cloned()
                    .unwrap_or(Value::Null),
                "learned_destination_account_id": metadata
                    .get("learned_destination_account_id")
                    .cloned()
                    .unwrap_or(Value::Null),
                "enabled": status != "suppressed" && status != "disabled",
                "applied_count": i64::from(accepted_count) + i64::from(auto_applied_count),
                "last_applied_at": last_feedback_at.map(|value| value.to_rfc3339()),
                "match_features_json": match_features_json,
                "created_at": created_at.to_rfc3339(),
                "updated_at": updated_at.to_rfc3339(),
                "version": version,
            }))
        })
        .collect()
}

/// 删除当前用户的 PostgreSQL lifecycle 学习规则；不存在或跨用户统一返回 false。
pub async fn delete_postgres_learning_rule(
    pool: &PostgresPool,
    user_id: i64,
    rule_id: i64,
) -> DbResult<bool> {
    let deleted = sqlx::query_scalar::<_, i64>(
        r#"
        DELETE FROM import_learning_lifecycle
        WHERE id = $1 AND user_id = $2
        RETURNING id
        "#,
    )
    .bind(rule_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(deleted.is_some())
}

/// 以当前用户为边界更新学习 lifecycle，并返回学习中心使用的当前投影。
pub async fn update_postgres_learning_rule(
    pool: &PostgresPool,
    user_id: i64,
    rule_id: i64,
    update: &PostgresLearningRuleUpdate,
) -> DbResult<Option<Value>> {
    let mut transaction = pool.begin().await?;
    let existing = sqlx::query(
        r#"
        SELECT recommendation_type, recommendation_key, status,
               accepted_count, auto_applied_count, auto_apply_enabled, metadata
        FROM import_learning_lifecycle
        WHERE id = $1 AND user_id = $2
        FOR UPDATE
        "#,
    )
    .bind(rule_id)
    .bind(user_id)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(existing) = existing else {
        return Ok(None);
    };

    if let Some(Some(category_id)) = update.learned_category_id {
        let category_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE id = $1 AND user_id = $2)",
        )
        .bind(category_id)
        .bind(user_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !category_exists {
            return Err(DbError::InvalidOperation(
                "invalid_learned_category_id".to_string(),
            ));
        }
    }

    let recommendation_key: String = existing.try_get("recommendation_key")?;
    let mut recommendation_type: String = existing.try_get("recommendation_type")?;
    let mut status: String = existing.try_get("status")?;
    let accepted_count: i32 = existing.try_get("accepted_count")?;
    let auto_applied_count: i32 = existing.try_get("auto_applied_count")?;
    let mut auto_apply_enabled: bool = existing.try_get("auto_apply_enabled")?;
    let existing_metadata: Value = existing.try_get("metadata")?;
    let mut metadata = existing_metadata.as_object().cloned().unwrap_or_default();

    if let Some(match_value) = update.match_value.as_ref() {
        if metadata.get("match_type").and_then(Value::as_str) == Some("composite")
            && update.match_features.is_none()
        {
            return Err(DbError::InvalidOperation("invalid_match_value".to_string()));
        }
        metadata.insert(
            "match_value".to_string(),
            Value::String(match_value.clone()),
        );
        if let Some(match_features) = update.match_features.as_ref() {
            metadata.insert("match_features".to_string(), match_features.clone());
        } else {
            metadata.remove("match_features");
        }
    }
    if let Some(learned_type) = update.learned_type.as_ref() {
        recommendation_type.clone_from(learned_type);
        metadata.insert(
            "learned_type".to_string(),
            Value::String(learned_type.clone()),
        );
    }
    if let Some(category_id) = update.learned_category_id {
        metadata.insert(
            "learned_category_id".to_string(),
            category_id.map_or(Value::Null, |value| json!(value)),
        );
    }
    if let Some(enabled) = update.enabled {
        if enabled {
            if status == "disabled" {
                status = metadata
                    .remove("disabled_from_status")
                    .and_then(|value| value.as_str().map(str::to_string))
                    .filter(|value| value != "disabled" && value != "suppressed")
                    .unwrap_or_else(|| "yellow".to_string());
                auto_apply_enabled = metadata
                    .remove("disabled_from_auto_apply_enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
            } else if status == "suppressed" {
                status = "yellow".to_string();
                auto_apply_enabled = false;
            }
        } else {
            if status != "disabled" {
                metadata.insert(
                    "disabled_from_status".to_string(),
                    Value::String(status.clone()),
                );
                metadata.insert(
                    "disabled_from_auto_apply_enabled".to_string(),
                    Value::Bool(auto_apply_enabled),
                );
            }
            status = "disabled".to_string();
            auto_apply_enabled = false;
        }
    }

    let row = sqlx::query(
        r#"
        UPDATE import_learning_lifecycle
        SET recommendation_type = $3,
            status = $4,
            auto_apply_enabled = $5,
            metadata = $6,
            updated_at = clock_timestamp(),
            version = version + 1
        WHERE id = $1 AND user_id = $2
        RETURNING id, created_at, updated_at, version
        "#,
    )
    .bind(rule_id)
    .bind(user_id)
    .bind(&recommendation_type)
    .bind(&status)
    .bind(auto_apply_enabled)
    .bind(Value::Object(metadata.clone()))
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;

    let created_at: DateTime<Utc> = row.try_get("created_at")?;
    let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
    let version: i64 = row.try_get("version")?;
    Ok(Some(json!({
        "id": rule_id,
        "match_type": metadata
            .get("match_type")
            .and_then(Value::as_str)
            .unwrap_or(recommendation_type.as_str()),
        "match_value": metadata
            .get("match_value")
            .and_then(Value::as_str)
            .unwrap_or(recommendation_key.as_str()),
        "learned_type": metadata
            .get("learned_type")
            .and_then(Value::as_str)
            .unwrap_or(recommendation_type.as_str()),
        "learned_category_id": metadata
            .get("learned_category_id")
            .cloned()
            .unwrap_or(Value::Null),
        "enabled": status != "suppressed" && status != "disabled",
        "applied_count": i64::from(accepted_count) + i64::from(auto_applied_count),
        "created_at": created_at.to_rfc3339(),
        "updated_at": updated_at.to_rfc3339(),
        "version": version,
    })))
}
