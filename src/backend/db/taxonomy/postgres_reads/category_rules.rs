/// 中文说明：按用户、分类和启用状态读取分类识别规则，并过滤无可展示表达式的旧记录。
pub async fn list_postgres_category_rules(
    pool: &PostgresPool,
    user_id: i64,
    category_id: Option<i64>,
    enabled_only: bool,
) -> DbResult<Vec<CategoryRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(category_rule_select_sql());
    builder.push(" WHERE cr.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND c.user_id = ");
    builder.push_bind(user_id);
    if let Some(category_id) = category_id {
        builder.push(" AND cr.category_id = ");
        builder.push_bind(category_id);
    }
    if enabled_only {
        builder.push(" AND cr.enabled = true");
    }
    builder.push(" ORDER BY cr.priority ASC, cr.id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(category_rule_from_postgres_row)
        .filter(category_rule_result_has_displayable_expression)
        .collect()
}

/// 中文说明：按规则 ID 与用户边界读取分类识别规则，并联表确认目标分类仍属于当前用户。
pub async fn get_postgres_category_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<Option<CategoryRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(category_rule_select_sql());
    builder.push(" WHERE cr.id = ");
    builder.push_bind(rule_id);
    builder.push(" AND cr.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND c.user_id = ");
    builder.push_bind(user_id);
    let row = builder.build().fetch_optional(pool).await?;
    row.map(category_rule_from_postgres_row).transpose()
}

/// 中文说明：创建分类识别规则，写入前校验目标分类归属并规范化表达式 JSON。
pub async fn create_postgres_category_rule(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("category rule payload must be an object".to_string())
    })?;
    let category_id = object
        .get("category_id")
        .and_then(|value| int_value(Some(value)))
        .ok_or_else(|| DbError::InvalidOperation("category_id is required".to_string()))?;
    if !postgres_category_belongs_to_user(pool, category_id, user_id).await? {
        return Ok(None);
    }
    let rule_expression = required_postgres_rule_expression(object.get("rule_expression"))?;
    let regex_enabled = object
        .get("regex_enabled")
        .map(payload_bool_value)
        .transpose()?
        .unwrap_or(false);
    let enabled = object
        .get("enabled")
        .map(payload_bool_value)
        .transpose()?
        .unwrap_or(true);
    let name = object
        .get("name")
        .map(payload_scalar_text)
        .transpose()?
        .unwrap_or_default();
    let priority = object
        .get("priority")
        .and_then(|value| int_value(Some(value)))
        .unwrap_or(100);

    let row = sqlx::query(
        r#"
        INSERT INTO category_rules (
            user_id, category_id, name, transaction_type_scope, field_scope,
            rule_expression, priority, enabled
        )
        VALUES ($1, $2, $3, 'all', $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(name)
    .bind(json_array(DEFAULT_FIELD_SCOPES))
    .bind(rule_expression_json(&rule_expression, regex_enabled))
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .fetch_one(pool)
    .await?;
    Ok(Some(row.try_get("id")?))
}

/// 中文说明：局部更新分类识别规则，保持表达式文本和 regex_enabled JSON 合同一致。
pub async fn update_postgres_category_rule(
    pool: &PostgresPool,
    rule_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("category rule update payload must be an object".to_string())
    })?;
    if object.is_empty() {
        return Ok(false);
    }
    let existing = sqlx::query(
        r#"
        SELECT category_id, name, rule_expression, priority, enabled
        FROM category_rules
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(rule_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(existing) = existing else {
        return Ok(false);
    };

    let mut category_id = existing.try_get::<Option<i64>, _>("category_id")?;
    let mut name: String = existing.try_get("name")?;
    let mut rule_expression_json_value: Value = existing.try_get("rule_expression")?;
    let mut priority = i64::from(existing.try_get::<i32, _>("priority")?);
    let mut enabled: bool = existing.try_get("enabled")?;
    let mut changed = false;

    for (key, value) in object {
        match key.as_str() {
            "id" | "user_id" | "created_at" | "updated_at" | "applied_count"
            | "last_applied_at" | "main_category" | "sub_category" | "category_type" => {}
            "category_id" => {
                let next_category_id = required_postgres_i64(value, "category_id")?;
                if !postgres_category_belongs_to_user(pool, next_category_id, user_id).await? {
                    return Ok(false);
                }
                category_id = Some(next_category_id);
                changed = true;
            }
            "name" => {
                name = payload_scalar_text(value)?;
                changed = true;
            }
            "priority" => {
                priority = required_postgres_i64(value, "priority")?;
                changed = true;
            }
            "rule_expression" => {
                let rule_expression = required_postgres_rule_expression(Some(value))?;
                let regex_enabled = rule_expression_regex_enabled(&rule_expression_json_value);
                rule_expression_json_value = rule_expression_json(&rule_expression, regex_enabled);
                changed = true;
            }
            "regex_enabled" => {
                let rule_expression = rule_expression_string(&rule_expression_json_value);
                rule_expression_json_value =
                    rule_expression_json(&rule_expression, payload_bool_value(value)?);
                changed = true;
            }
            "enabled" => {
                enabled = payload_bool_value(value)?;
                changed = true;
            }
            _ => {}
        }
    }
    if !changed {
        return Ok(false);
    }

    let affected = sqlx::query(
        r#"
        UPDATE category_rules
        SET category_id = $1,
            name = $2,
            rule_expression = $3,
            priority = $4,
            enabled = $5,
            updated_at = now(),
            version = version + 1
        WHERE id = $6 AND user_id = $7
        "#,
    )
    .bind(category_id)
    .bind(name)
    .bind(rule_expression_json_value)
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .bind(rule_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

/// 中文说明：按用户边界删除分类识别规则，避免影响其他用户的自动分类配置。
pub async fn delete_postgres_category_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM category_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：批量重排分类识别规则优先级，服务于规则中心排序操作。
pub async fn reorder_postgres_category_rules(
    pool: &PostgresPool,
    rule_ids: &[i64],
    user_id: i64,
) -> DbResult<bool> {
    let mut transaction = pool.begin().await?;
    for (priority, rule_id) in rule_ids.iter().enumerate() {
        sqlx::query(
            r#"
            UPDATE category_rules
            SET priority = $1, updated_at = now(), version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64_to_i32(i64::try_from(priority + 1).unwrap_or(i64::MAX)))
        .bind(rule_id)
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

/// 中文说明：写入标准日常默认分类与规则包，注册默认包和手动补种共用此入口。
pub async fn ensure_postgres_category_rule_defaults(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<RegisterDefaultSeedSummary> {
    let mut summary = RegisterDefaultSeedSummary::empty(
        crate::auth_registration::RegisterDefaultSeedPackage::None,
    );

    for category in DEFAULT_DAILY_CATEGORIES {
        let parent_payload = json!({
            "main_category": category.name,
            "sub_category": "",
            "type": category.type_code,
            "priority": category.priority,
            "icon": category.icon,
            "color": category.color,
            "hidden": false,
        });
        match create_postgres_category(pool, &parent_payload, user_id).await? {
            Some(_) => summary.categories_created += 1,
            None => summary.categories_skipped += 1,
        }

        for (offset, sub_category) in category.sub_categories.iter().enumerate() {
            let child_payload = json!({
                "main_category": category.name,
                "sub_category": sub_category.name,
                "type": category.type_code,
                "priority": category.priority + i64::try_from(offset + 1).unwrap_or(i64::MAX),
                "icon": sub_category.icon,
                "color": sub_category.color,
                "hidden": false,
            });
            match create_postgres_category(pool, &child_payload, user_id).await? {
                Some(_) => summary.categories_created += 1,
                None => summary.categories_skipped += 1,
            }
        }
    }

    for rule in DEFAULT_DAILY_CATEGORY_RULES {
        let Some(category) =
            get_postgres_category_by_name(pool, rule.main_category, rule.sub_category, user_id)
                .await?
        else {
            summary.note_missing_category();
            continue;
        };
        if postgres_category_rule_name_exists(pool, user_id, rule.name).await? {
            summary.rules_skipped += 1;
            continue;
        }
        let Some(category_id) = int_value(category.get("id")) else {
            summary.note_missing_category();
            continue;
        };
        let payload = json!({
            "category_id": category_id,
            "name": rule.name,
            "priority": rule.priority,
            "rule_expression": rule.rule_expression,
            "regex_enabled": false,
            "enabled": true,
        });
        match create_postgres_category_rule(pool, &payload, user_id).await? {
            Some(_) => summary.rules_created += 1,
            None => summary.rules_skipped += 1,
        }
    }

    Ok(summary)
}

/// 中文说明：聚合规则中心概览数据，把 learning、分类规则和周期模板合成前端概览 payload。
pub async fn query_postgres_rules_overview_payload(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Value> {
    let learning_rules = list_postgres_rules_overview_learning_rules(pool, user_id).await?;
    let learning_count = i64::try_from(learning_rules.len()).unwrap_or(i64::MAX);
    let category_rule_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM category_rules WHERE user_id = $1 AND enabled = true",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    let recurring_rules = list_postgres_rules_overview_recurring_rules(pool, user_id).await?;
    let recurring_rule_count = i64::try_from(recurring_rules.len()).unwrap_or(i64::MAX);

    Ok(json!({
        "learningRules": learning_rules,
        "learningRuleCount": learning_count,
        "categoryRuleCount": category_rule_count,
        "recurringRules": recurring_rules,
        "recurringRuleCount": recurring_rule_count,
        "totalRuleCount": learning_count + category_rule_count + recurring_rule_count,
    }))
}

async fn postgres_category_rule_name_exists(
    pool: &PostgresPool,
    user_id: i64,
    name: &str,
) -> DbResult<bool> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM category_rules WHERE user_id = $1 AND name = $2 LIMIT 1",
    )
    .bind(user_id)
    .bind(name)
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

async fn list_postgres_rules_overview_learning_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, recommendation_type, recommendation_key, status,
               accepted_count, auto_applied_count, metadata
        FROM import_learning_lifecycle
        WHERE user_id = $1
        ORDER BY updated_at DESC, id DESC
        LIMIT 500
        "#,
    )
    .bind(user_id)
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
            let metadata: Value = row.try_get("metadata")?;
            Ok(json!({
                "id": id,
                "matchType": metadata
                    .get("match_type")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_type.as_str()),
                "matchValue": metadata
                    .get("match_value")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_key.as_str()),
                "learnedType": metadata
                    .get("learned_type")
                    .and_then(Value::as_str)
                    .unwrap_or(recommendation_type.as_str()),
                "learnedCategoryId": metadata.get("learned_category_id").cloned().unwrap_or(Value::Null),
                "enabled": status != "suppressed" && status != "disabled",
                "appliedCount": i64::from(accepted_count) + i64::from(auto_applied_count),
                "source": "learning",
            }))
        })
        .collect()
}

async fn list_postgres_rules_overview_recurring_rules(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, source_amount_minor_units, scheduled_frequency,
               scheduled_frequency_type, enabled, scheduled_next_date
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = 2
        ORDER BY display_order, name
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("id")?;
            let name: String = row.try_get("name")?;
            let amount_minor: i64 = row.try_get("source_amount_minor_units")?;
            let scheduled_frequency: Option<String> = row.try_get("scheduled_frequency")?;
            let scheduled_frequency_type: Option<i32> = row.try_get("scheduled_frequency_type")?;
            let enabled: bool = row.try_get("enabled")?;
            let next_date: Option<String> = row.try_get("scheduled_next_date")?;
            Ok(rules_overview_recurring_rule_payload(
                id,
                name,
                amount_minor,
                scheduled_frequency,
                scheduled_frequency_type,
                enabled,
                next_date,
            ))
        })
        .collect()
}

fn rules_overview_recurring_rule_payload(
    id: i64,
    name: String,
    amount_minor: i64,
    scheduled_frequency: Option<String>,
    scheduled_frequency_type: Option<i32>,
    enabled: bool,
    next_date: Option<String>,
) -> Value {
    json!({
        "id": id,
        "name": name,
        "amountCents": amount_minor,
        "frequency": scheduled_frequency
            .or_else(|| scheduled_frequency_type.map(|value| value.to_string())),
        "enabled": enabled,
        "nextDate": next_date,
        "source": "recurring",
    })
}
