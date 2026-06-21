#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：导入账户识别规则 section，使用账户引用映射恢复目标账户并忽略旧 scope 兼容字段。
async fn import_postgres_settings_account_rules(
    transaction: &mut PgTransaction<'_, Postgres>,
    rules: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    account_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    let mut existing = load_existing_postgres_account_rules(transaction, user_id).await?;
    for item in rules {
        let account_id = resolve_settings_account_rule_account_id(item, account_ref_map).unwrap_or(0);
        let rule_expression = safe_text(get_any(item, &["ruleExpression", "rule_expression"]), "");
        let section = result.get_mut("accountRecognitionRules");
        if account_id == 0 || rule_expression.is_empty() {
            section.skipped += 1;
            warnings.push("Skipped account rule with missing account or ruleExpression".to_string());
            continue;
        }
        if let Some(warning) = account_rule_scope_compat_warning(item) {
            warnings.push(warning);
        }
        let name = safe_text(item.get("name"), "");
        let priority = safe_int(item.get("priority"), 100);
        let regex_enabled = safe_bool(get_any(item, &["regexEnabled", "regex_enabled"]));
        let enabled = safe_bool_with_default(item.get("enabled"), true);
        let source = safe_text(item.get("source"), "manual");
        let source_key = safe_text(get_any(item, &["sourceKey", "source_key"]), "");
        let key = (
            account_id,
            rule_expression.clone(),
            name.clone(),
        );
        let rule_expression_json = postgres_rule_expression_json(&rule_expression, regex_enabled);
        let source_key = (!source_key.is_empty()).then_some(source_key);

        if let Some(rule_id) = existing.get(&key).copied() {
            sqlx::query(
                r#"
                UPDATE account_rules
                SET account_id = $1,
                    name = $2,
                    priority = $3,
                    rule_expression = $4,
                    regex_enabled = $5,
                    enabled = $6,
                    source = $7,
                    source_key = $8,
                    updated_at = now(),
                    version = version + 1
                WHERE id = $9 AND user_id = $10
                "#,
            )
            .bind(account_id)
            .bind(name)
            .bind(settings_i64_to_i32(priority))
            .bind(rule_expression_json)
            .bind(regex_enabled)
            .bind(enabled)
            .bind(source)
            .bind(source_key)
            .bind(rule_id)
            .bind(user_id)
            .execute(&mut **transaction)
            .await?;
            section.updated += 1;
            continue;
        }

        let row = sqlx::query(
            r#"
            INSERT INTO account_rules (
                user_id, account_id, name, priority, rule_expression,
                regex_enabled, enabled, source, source_key
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(account_id)
        .bind(name.clone())
        .bind(settings_i64_to_i32(priority))
        .bind(rule_expression_json)
        .bind(regex_enabled)
        .bind(enabled)
        .bind(source)
        .bind(source_key)
        .fetch_one(&mut **transaction)
        .await?;
        existing.insert(key, row.try_get("id")?);
        section.created += 1;
    }
    Ok(())
}

async fn load_existing_postgres_account_rules(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<AccountRuleSettingsIndex> {
    let rows = sqlx::query(
        r#"
        SELECT id, account_id, rule_expression, name
        FROM account_rules
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = BTreeMap::new();
    for row in rows {
        let account_id = row.try_get::<Option<i64>, _>("account_id")?.unwrap_or(0);
        let expression: Value = row.try_get("rule_expression")?;
        let name: String = row.try_get("name")?;
        existing.insert(
            (
                account_id,
                postgres_rule_expression_string(&expression),
                name,
            ),
            row.try_get("id")?,
        );
    }
    Ok(existing)
}
