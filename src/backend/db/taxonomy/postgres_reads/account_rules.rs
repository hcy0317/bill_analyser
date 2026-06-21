/// 中文说明：按用户、账户和启用状态读取账户识别规则，保持优先级排序合同。
pub async fn list_postgres_account_rules(
    pool: &PostgresPool,
    user_id: i64,
    account_id: Option<i64>,
    enabled_only: bool,
) -> DbResult<Vec<AccountRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(account_rule_select_sql());
    builder.push(" WHERE ar.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND a.user_id = ");
    builder.push_bind(user_id);
    if let Some(account_id) = account_id {
        builder.push(" AND ar.account_id = ");
        builder.push_bind(account_id);
    }
    if enabled_only {
        builder.push(" AND ar.enabled = true");
    }
    builder.push(" ORDER BY ar.priority ASC, ar.id ASC");

    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(account_rule_from_postgres_row)
        .collect()
}

/// 中文说明：按规则 ID 与用户边界读取账户识别规则，并联表确认目标账户仍属于当前用户。
pub async fn get_postgres_account_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<Option<AccountRuleRecord>> {
    let mut builder = QueryBuilder::<Postgres>::new(account_rule_select_sql());
    builder.push(" WHERE ar.id = ");
    builder.push_bind(rule_id);
    builder.push(" AND ar.user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND a.user_id = ");
    builder.push_bind(user_id);
    let row = builder.build().fetch_optional(pool).await?;
    row.map(account_rule_from_postgres_row).transpose()
}

/// 中文说明：创建账户识别规则，写入前校验目标账户归属并规范化表达式 JSON。
pub async fn create_postgres_account_rule(
    pool: &PostgresPool,
    payload: &Value,
    user_id: i64,
) -> DbResult<Option<i64>> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("account rule payload must be an object".to_string())
    })?;
    let account_id = object
        .get("account_id")
        .or_else(|| object.get("accountId"))
        .and_then(|value| int_value(Some(value)))
        .ok_or_else(|| DbError::InvalidOperation("account_id is required".to_string()))?;
    if !postgres_account_belongs_to_user(pool, account_id, user_id).await? {
        return Ok(None);
    }
    let rule_expression = required_postgres_rule_expression(
        object
            .get("rule_expression")
            .or_else(|| object.get("ruleExpression")),
    )?;
    let regex_enabled = object
        .get("regex_enabled")
        .or_else(|| object.get("regexEnabled"))
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
    let source = object
        .get("source")
        .map(payload_scalar_text)
        .transpose()?
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "manual".to_string());

    let row = sqlx::query(
        r#"
        INSERT INTO account_rules (
            user_id, account_id, name, rule_expression, regex_enabled, priority, enabled, source
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(name)
    .bind(rule_expression_json(&rule_expression, regex_enabled))
    .bind(regex_enabled)
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .bind(source)
    .fetch_one(pool)
    .await?;
    Ok(Some(row.try_get("id")?))
}

/// 中文说明：局部更新账户识别规则，忽略旧 scope 字段并保持表达式/正则开关同步。
pub async fn update_postgres_account_rule(
    pool: &PostgresPool,
    rule_id: i64,
    payload: &Value,
    user_id: i64,
) -> DbResult<bool> {
    let object = payload.as_object().ok_or_else(|| {
        DbError::InvalidOperation("account rule update payload must be an object".to_string())
    })?;
    if object.is_empty() {
        return Ok(false);
    }
    let existing = sqlx::query(
        r#"
        SELECT account_id, name, rule_expression, regex_enabled, priority, enabled, source
        FROM account_rules
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

    let mut account_id = existing.try_get::<Option<i64>, _>("account_id")?;
    let mut name: String = existing.try_get("name")?;
    let mut rule_expression_json_value: Value = existing.try_get("rule_expression")?;
    let mut regex_enabled: bool = existing.try_get("regex_enabled")?;
    let mut priority = i64::from(existing.try_get::<i32, _>("priority")?);
    let mut enabled: bool = existing.try_get("enabled")?;
    let mut source: String = existing.try_get("source")?;
    let mut changed = false;

    for (key, value) in object {
        match key.as_str() {
            "id"
            | "user_id"
            | "userId"
            | "created_at"
            | "createdAt"
            | "updated_at"
            | "updatedAt"
            | "applied_count"
            | "appliedCount"
            | "last_applied_at"
            | "lastAppliedAt"
            | "match_count"
            | "matchCount"
            | "last_matched_at"
            | "lastMatchedAt"
            | "source_key"
            | "sourceKey"
            | "account_name"
            | "accountName"
            | "account_type"
            | "accountType"
            | "account_hidden"
            | "accountHidden"
            | "account_role_scope"
            | "accountRoleScope"
            | "transaction_type_scope"
            | "transactionTypeScope"
            | "field_scope"
            | "fieldScope" => {}
            "account_id" | "accountId" => {
                let next_account_id = required_postgres_i64(value, "account_id")?;
                if !postgres_account_belongs_to_user(pool, next_account_id, user_id).await? {
                    return Ok(false);
                }
                account_id = Some(next_account_id);
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
            "rule_expression" | "ruleExpression" => {
                let rule_expression = required_postgres_rule_expression(Some(value))?;
                rule_expression_json_value = rule_expression_json(&rule_expression, regex_enabled);
                changed = true;
            }
            "regex_enabled" | "regexEnabled" => {
                regex_enabled = payload_bool_value(value)?;
                let rule_expression = rule_expression_string(&rule_expression_json_value);
                rule_expression_json_value = rule_expression_json(&rule_expression, regex_enabled);
                changed = true;
            }
            "enabled" => {
                enabled = payload_bool_value(value)?;
                changed = true;
            }
            "source" => {
                source = payload_scalar_text(value)?;
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
        UPDATE account_rules
        SET account_id = $1,
            name = $2,
            rule_expression = $3,
            regex_enabled = $4,
            priority = $5,
            enabled = $6,
            source = $7,
            updated_at = now(),
            version = version + 1
        WHERE id = $8 AND user_id = $9
        "#,
    )
    .bind(account_id)
    .bind(name)
    .bind(rule_expression_json_value)
    .bind(regex_enabled)
    .bind(i64_to_i32(priority))
    .bind(enabled)
    .bind(source)
    .bind(rule_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(affected > 0)
}

/// 中文说明：按用户边界删除账户识别规则，避免影响其他用户的账户匹配配置。
pub async fn delete_postgres_account_rule(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    let changed = sqlx::query("DELETE FROM account_rules WHERE id = $1 AND user_id = $2")
        .bind(rule_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(changed > 0)
}

/// 中文说明：批量重排账户识别规则优先级，先校验列表唯一性和用户归属再写入。
pub async fn reorder_postgres_account_rules(
    pool: &PostgresPool,
    rule_ids: &[i64],
    user_id: i64,
) -> DbResult<bool> {
    let unique_ids = rule_ids.iter().copied().collect::<BTreeSet<_>>();
    if unique_ids.len() != rule_ids.len() {
        return Ok(false);
    }
    if !rule_ids.is_empty()
        && count_postgres_account_rules(pool, &unique_ids, user_id).await?
            != i64::try_from(unique_ids.len()).unwrap_or(i64::MAX)
    {
        return Ok(false);
    }
    let mut transaction = pool.begin().await?;
    for (priority, rule_id) in rule_ids.iter().enumerate() {
        sqlx::query(
            r#"
            UPDATE account_rules
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

/// 中文说明：用当前账户规则核心匹配器试跑单条规则，供规则编辑页测试入口调用。
pub async fn test_postgres_account_rule_match(
    pool: &PostgresPool,
    rule_id: i64,
    user_id: i64,
    context: &AccountRuleMatchContext,
    requested_role_scope: &str,
    transaction_type_scope: &str,
) -> DbResult<Option<AccountRuleMatch>> {
    let Some(rule) = get_postgres_account_rule(pool, rule_id, user_id).await? else {
        return Ok(None);
    };
    let candidate = account_rule_candidate_from_record(rule)?;
    Ok(bill_analyser_core::account_rules::match_account_rules(
        &[candidate],
        context,
        requested_role_scope,
        transaction_type_scope,
    ))
}
