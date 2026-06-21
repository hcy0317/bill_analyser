#[tracing::instrument(level = "debug", skip_all)]
async fn import_postgres_settings_category_rules(
    transaction: &mut PgTransaction<'_, Postgres>,
    rules: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    category_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    let mut existing = load_existing_postgres_category_rules(transaction, user_id).await?;
    for item in rules {
        let category_id = resolve_settings_category_id(item, category_ref_map).unwrap_or(0);
        let rule_expression = safe_text(get_any(item, &["ruleExpression", "rule_expression"]), "");
        let section = result.get_mut("categoryRecognitionRules");
        if category_id == 0 || rule_expression.is_empty() {
            section.skipped += 1;
            warnings
                .push("Skipped category rule with missing category or ruleExpression".to_string());
            continue;
        }
        let name = safe_text(item.get("name"), "");
        let priority = safe_int(item.get("priority"), 100);
        let regex_enabled = safe_bool(get_any(item, &["regexEnabled", "regex_enabled"]));
        let enabled = safe_bool_with_default(item.get("enabled"), true);
        let key = (category_id, rule_expression.clone(), name.clone());
        let rule_expression_json = postgres_rule_expression_json(&rule_expression, regex_enabled);

        if let Some(rule_id) = existing.get(&key).copied() {
            sqlx::query(
                r#"
                UPDATE category_rules
                SET category_id = $1,
                    name = $2,
                    priority = $3,
                    rule_expression = $4,
                    enabled = $5,
                    updated_at = now(),
                    version = version + 1
                WHERE id = $6 AND user_id = $7
                "#,
            )
            .bind(category_id)
            .bind(name)
            .bind(settings_i64_to_i32(priority))
            .bind(rule_expression_json)
            .bind(enabled)
            .bind(rule_id)
            .bind(user_id)
            .execute(&mut **transaction)
            .await?;
            section.updated += 1;
            continue;
        }

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
        .bind(name.clone())
        .bind(json!(["counterparty", "payment_method", "description"]))
        .bind(rule_expression_json)
        .bind(settings_i64_to_i32(priority))
        .bind(enabled)
        .fetch_one(&mut **transaction)
        .await?;
        existing.insert(key, row.try_get("id")?);
        section.created += 1;
    }
    Ok(())
}

async fn load_existing_postgres_category_rules(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<BTreeMap<(i64, String, String), i64>> {
    let rows = sqlx::query(
        "SELECT id, category_id, rule_expression, name FROM category_rules WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = BTreeMap::new();
    for row in rows {
        let category_id = row.try_get::<Option<i64>, _>("category_id")?.unwrap_or(0);
        let expression: Value = row.try_get("rule_expression")?;
        let name: String = row.try_get("name")?;
        existing.insert(
            (category_id, postgres_rule_expression_string(&expression), name),
            row.try_get("id")?,
        );
    }
    Ok(existing)
}
