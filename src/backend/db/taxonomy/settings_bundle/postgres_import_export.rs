// 中文导读：PostgreSQL authority 设置包导入路径，复用设置包 schema/ref 解析并用 Postgres 事务承载 preview rollback。
// 维护重点：保持 user-scope、幂等 upsert 和引用重映射；不要回退到 non-Postgres runtime。
// 不变式：dry_run 必须 rollback；真实导入必须按用户事务提交，金额从设置包元单位写入 Postgres 分单位。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_postgres_settings_bundle(
    pool: &PostgresPool,
    bundle: &Value,
    user_id: i64,
    dry_run: bool,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "import_postgres_settings_bundle", "business operation entered");
    let sections = normalize_settings_bundle_sections(bundle)?;
    let mut transaction = pool.begin().await?;
    let mut result_sections = ImportSections::new();
    let mut warnings = Vec::new();

    let account_ref_map = import_postgres_settings_accounts(
        &mut transaction,
        section_items(&sections, "accounts"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )
    .await?;
    let category_ref_map = import_postgres_settings_categories(
        &mut transaction,
        section_items(&sections, "transactionCategories"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )
    .await?;
    let tag_ref_map = import_postgres_settings_tags(
        &mut transaction,
        section_items(&sections, "transactionTags"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )
    .await?;
    skip_postgres_settings_section(
        "transactionTemplates",
        section_items(&sections, "transactionTemplates"),
        &mut result_sections,
        &mut warnings,
        "PostgreSQL settings bundle template import is not supported for this section",
    );
    skip_postgres_settings_section(
        "scheduledTransactions",
        section_items(&sections, "scheduledTransactions"),
        &mut result_sections,
        &mut warnings,
        "PostgreSQL settings bundle scheduled template import is not supported for this section",
    );
    import_postgres_settings_category_rules(
        &mut transaction,
        section_items(&sections, "categoryRecognitionRules"),
        user_id,
        &mut result_sections,
        &mut warnings,
        &category_ref_map,
    )
    .await?;
    import_postgres_settings_account_rules(
        &mut transaction,
        section_items(&sections, "accountRecognitionRules"),
        user_id,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
    )
    .await?;
    skip_postgres_settings_section(
        "llmConfigs",
        section_items(&sections, "llmConfigs"),
        &mut result_sections,
        &mut warnings,
        "PostgreSQL settings bundle LLM config import is not supported for this section",
    );
    skip_postgres_settings_section(
        "ocrConfig",
        section_items(&sections, "ocrConfig"),
        &mut result_sections,
        &mut warnings,
        "PostgreSQL settings bundle OCR config import is not supported for this section",
    );

    if dry_run {
        transaction.rollback().await?;
    } else {
        transaction.commit().await?;
    }

    Ok(json!({
        "dryRun": dry_run,
        "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
        "sections": result_sections.into_value(),
        "warnings": warnings,
        "resolvedRefs": {
            "accounts": account_ref_map.len(),
            "transactionCategories": category_ref_map.len(),
            "transactionTags": tag_ref_map.len(),
        },
    }))
}

fn skip_postgres_settings_section(
    section_key: &str,
    items: &[Value],
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    message: &str,
) {
    if items.is_empty() {
        return;
    }
    result.get_mut(section_key).skipped += i64::try_from(items.len()).unwrap_or(i64::MAX);
    warnings.push(message.to_string());
}

#[tracing::instrument(level = "debug", skip_all)]
async fn import_postgres_settings_accounts(
    transaction: &mut PgTransaction<'_, Postgres>,
    accounts: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_postgres_accounts(transaction, user_id).await?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|account_id| (local_id_ref("account", *account_id), *account_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(
        existing
            .by_id
            .iter()
            .filter(|(_, (name, _))| !name.is_empty())
            .map(|(account_id, (name, _))| (format!("accountName:{name}"), *account_id)),
    );

    let mut pending = accounts.iter().collect::<Vec<_>>();
    for _ in 0..=pending.len() {
        let mut next_pending = Vec::new();
        let mut progressed = false;
        for item in pending {
            let parent_ref = safe_text(get_any(item, &["parentRef", "parent_ref"]), "");
            if !parent_ref.is_empty() && !ref_map.contains_key(&parent_ref) {
                next_pending.push(item);
                continue;
            }
            if let Some(account_id) = upsert_postgres_settings_account(
                transaction,
                item,
                user_id,
                result.get_mut("accounts"),
                &mut existing,
                &ref_map,
                warnings,
            )
            .await?
            {
                add_account_refs(&mut ref_map, item, account_id);
                progressed = true;
            }
        }
        pending = next_pending;
        if pending.is_empty() || !progressed {
            break;
        }
    }

    for item in pending {
        warnings.push(format!(
            "Account parent not found; imported as root: {}",
            safe_text(item.get("name"), "")
        ));
        let mut root_item = item.as_object().cloned().unwrap_or_default();
        root_item.insert("parentRef".to_string(), Value::String(String::new()));
        if let Some(account_id) = upsert_postgres_settings_account(
            transaction,
            &Value::Object(root_item),
            user_id,
            result.get_mut("accounts"),
            &mut existing,
            &ref_map,
            warnings,
        )
        .await?
        {
            add_account_refs(&mut ref_map, item, account_id);
        }
    }

    Ok(ref_map)
}

async fn load_existing_postgres_accounts(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingAccounts> {
    let rows = sqlx::query(
        "SELECT id, name, metadata FROM accounts WHERE user_id = $1 ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = ExistingAccounts::default();
    for row in rows {
        let account_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let metadata: Value = row.try_get("metadata")?;
        let parent_id = postgres_metadata_parent_id(&metadata);
        existing
            .by_id
            .insert(account_id, (name.clone(), parent_id));
        existing.by_key.insert((name.clone(), parent_id), account_id);
        existing.by_key.insert((name, 0), account_id);
    }
    Ok(existing)
}

#[allow(clippy::too_many_arguments)]
async fn upsert_postgres_settings_account(
    transaction: &mut PgTransaction<'_, Postgres>,
    item: &Value,
    user_id: i64,
    section: &mut SectionCounts,
    existing: &mut ExistingAccounts,
    ref_map: &BTreeMap<String, i64>,
    warnings: &mut Vec<String>,
) -> DbResult<Option<i64>> {
    let name = safe_text(item.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped account without name".to_string());
        return Ok(None);
    }

    let normalized = normalize_account_import(&json!({
        "item": item,
        "ref_map": ref_map,
    }));
    let parent_id = safe_int(normalized.get("parent_id"), 0);
    let metadata = postgres_account_metadata(&normalized, parent_id);
    let balance_cents = settings_yuan_to_cents(safe_float(normalized.get("balance"), 0.0));

    if let Some(account_id) = existing
        .by_key
        .get(&(name.clone(), parent_id))
        .or_else(|| existing.by_key.get(&(name.clone(), 0)))
        .copied()
    {
        sqlx::query(
            r#"
            UPDATE accounts
            SET account_type = $1,
                currency = $2,
                balance_cents = $3,
                is_active = $4,
                display_order = $5,
                metadata = $6,
                updated_at = now(),
                version = version + 1
            WHERE id = $7 AND user_id = $8
            "#,
        )
        .bind(safe_text(normalized.get("type"), "1"))
        .bind(safe_text(normalized.get("currency"), "CNY"))
        .bind(balance_cents)
        .bind(safe_int(normalized.get("hidden"), 0) == 0)
        .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
        .bind(metadata)
        .bind(account_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        existing
            .by_id
            .insert(account_id, (name.clone(), parent_id));
        existing.by_key.insert((name, parent_id), account_id);
        section.updated += 1;
        return Ok(Some(account_id));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents, is_active,
            display_order, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(safe_text(normalized.get("type"), "1"))
    .bind(safe_text(normalized.get("currency"), "CNY"))
    .bind(balance_cents)
    .bind(safe_int(normalized.get("hidden"), 0) == 0)
    .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
    .bind(metadata)
    .fetch_one(&mut **transaction)
    .await?;
    let account_id: i64 = row.try_get("id")?;
    existing
        .by_id
        .insert(account_id, (name.clone(), parent_id));
    existing.by_key.insert((name.clone(), parent_id), account_id);
    existing.by_key.insert((name, 0), account_id);
    section.created += 1;
    Ok(Some(account_id))
}

fn postgres_account_metadata(normalized: &Value, parent_id: i64) -> Value {
    let mut metadata = Map::new();
    for key in ["category", "icon", "color", "comment"] {
        metadata.insert(
            key.to_string(),
            normalized.get(key).cloned().unwrap_or(Value::Null),
        );
    }
    metadata.insert(
        "initial_balance".to_string(),
        json!(safe_float(normalized.get("initial_balance"), 0.0)),
    );
    if parent_id > 0 {
        metadata.insert("parent_id".to_string(), Value::Number(parent_id.into()));
    }
    Value::Object(metadata)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn import_postgres_settings_categories(
    transaction: &mut PgTransaction<'_, Postgres>,
    categories: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_postgres_categories(transaction, user_id).await?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|category_id| (local_id_ref("category", *category_id), *category_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(existing.by_id.iter().filter_map(|(category_id, (main, sub))| {
        let name = category_name(main, sub);
        (!name.is_empty()).then(|| (format!("categoryName:{name}"), *category_id))
    }));

    for item in categories {
        if let Some(category_id) = upsert_postgres_settings_category(
            transaction,
            item,
            user_id,
            result.get_mut("transactionCategories"),
            &mut existing,
            warnings,
        )
        .await?
        {
            let category_ref = external_ref(item, "category");
            if !category_ref.is_empty() {
                ref_map.insert(category_ref, category_id);
            }
            let main = safe_text(get_any(item, &["mainCategory", "main_category"]), "");
            let sub = safe_text(get_any(item, &["subCategory", "sub_category"]), "");
            let name = category_name(&main, &sub);
            if !name.is_empty() {
                ref_map.insert(format!("categoryName:{name}"), category_id);
            }
        }
    }

    Ok(ref_map)
}

async fn load_existing_postgres_categories(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingCategories> {
    let rows = sqlx::query("SELECT id, name, path FROM categories WHERE user_id = $1 ORDER BY id")
        .bind(user_id)
        .fetch_all(&mut **transaction)
        .await?;
    let mut existing = ExistingCategories::default();
    for row in rows {
        let category_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let path: Option<String> = row.try_get("path")?;
        let (main, sub) = postgres_category_parts(path.as_deref(), &name);
        existing
            .by_id
            .insert(category_id, (main.clone(), sub.clone()));
        existing.by_key.insert((main, sub), category_id);
    }
    Ok(existing)
}

async fn upsert_postgres_settings_category(
    transaction: &mut PgTransaction<'_, Postgres>,
    item: &Value,
    user_id: i64,
    section: &mut SectionCounts,
    existing: &mut ExistingCategories,
    warnings: &mut Vec<String>,
) -> DbResult<Option<i64>> {
    let normalized = normalize_category_import(&json!({ "item": item }));
    let main = safe_text(normalized.get("main_category"), "");
    let sub = safe_text(normalized.get("sub_category"), "");
    if main.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped category without mainCategory".to_string());
        return Ok(None);
    }
    let parent_id = if sub.is_empty() {
        None
    } else {
        Some(
            ensure_postgres_parent_category(transaction, user_id, &main, existing)
                .await?,
        )
    };
    let name = if sub.is_empty() { main.clone() } else { sub.clone() };
    let path = category_name(&main, &sub);
    let metadata = json!({
        "description": safe_text(normalized.get("description"), ""),
        "keywords": safe_text(normalized.get("keywords"), ""),
    });

    if let Some(category_id) = existing.by_key.get(&(main.clone(), sub.clone())).copied() {
        sqlx::query(
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
        .bind(name)
        .bind(safe_text(normalized.get("type"), "3"))
        .bind(path)
        .bind(safe_text(normalized.get("icon"), ""))
        .bind(safe_text(normalized.get("color"), ""))
        .bind(settings_i64_to_i32(safe_int(normalized.get("priority"), 0)))
        .bind(safe_int(normalized.get("hidden"), 0) == 0)
        .bind(metadata)
        .bind(category_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        section.updated += 1;
        return Ok(Some(category_id));
    }

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
    .bind(name)
    .bind(safe_text(normalized.get("type"), "3"))
    .bind(path)
    .bind(safe_text(normalized.get("icon"), ""))
    .bind(safe_text(normalized.get("color"), ""))
    .bind(settings_i64_to_i32(safe_int(normalized.get("priority"), 0)))
    .bind(safe_int(normalized.get("hidden"), 0) == 0)
    .bind(metadata)
    .fetch_one(&mut **transaction)
    .await?;
    let category_id: i64 = row.try_get("id")?;
    existing
        .by_id
        .insert(category_id, (main.clone(), sub.clone()));
    existing.by_key.insert((main, sub), category_id);
    section.created += 1;
    Ok(Some(category_id))
}

async fn ensure_postgres_parent_category(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
    main: &str,
    existing: &mut ExistingCategories,
) -> DbResult<i64> {
    if let Some(parent_id) = existing
        .by_key
        .get(&(main.to_string(), String::new()))
        .copied()
    {
        return Ok(parent_id);
    }
    let row = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, display_order, is_active)
        VALUES ($1, $2, '3', $2, 0, true)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(main)
    .fetch_one(&mut **transaction)
    .await?;
    let parent_id: i64 = row.try_get("id")?;
    existing
        .by_id
        .insert(parent_id, (main.to_string(), String::new()));
    existing
        .by_key
        .insert((main.to_string(), String::new()), parent_id);
    Ok(parent_id)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn import_postgres_settings_tags(
    transaction: &mut PgTransaction<'_, Postgres>,
    tags: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_postgres_tags(transaction, user_id).await?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|tag_id| (local_id_ref("tag", *tag_id), *tag_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(
        existing
            .by_id
            .iter()
            .filter(|(_, name)| !name.is_empty())
            .map(|(tag_id, name)| (format!("tagName:{name}"), *tag_id)),
    );

    for item in tags {
        if let Some(tag_id) = upsert_postgres_settings_tag(
            transaction,
            item,
            user_id,
            result.get_mut("transactionTags"),
            &mut existing,
            warnings,
        )
        .await?
        {
            let tag_ref = external_ref(item, "tag");
            if !tag_ref.is_empty() {
                ref_map.insert(tag_ref, tag_id);
            }
            let name = safe_text(item.get("name"), "");
            if !name.is_empty() {
                ref_map.insert(format!("tagName:{name}"), tag_id);
            }
        }
    }
    Ok(ref_map)
}

async fn load_existing_postgres_tags(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingTags> {
    let rows = sqlx::query("SELECT id, name FROM tags WHERE user_id = $1 ORDER BY id")
        .bind(user_id)
        .fetch_all(&mut **transaction)
        .await?;
    let mut existing = ExistingTags::default();
    for row in rows {
        let tag_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        existing.by_id.insert(tag_id, name.clone());
        existing.by_name.insert(name, tag_id);
    }
    Ok(existing)
}

async fn upsert_postgres_settings_tag(
    transaction: &mut PgTransaction<'_, Postgres>,
    item: &Value,
    user_id: i64,
    section: &mut SectionCounts,
    existing: &mut ExistingTags,
    warnings: &mut Vec<String>,
) -> DbResult<Option<i64>> {
    let normalized = normalize_tag_import(&json!({ "item": item }));
    let name = safe_text(normalized.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped tag without name".to_string());
        return Ok(None);
    }
    let metadata = json!({
        "icon": safe_text(normalized.get("icon"), ""),
        "hidden": safe_bool(normalized.get("hidden")),
    });
    if let Some(tag_id) = existing.by_name.get(&name).copied() {
        sqlx::query(
            r#"
            UPDATE tags
            SET color = $1,
                display_order = $2,
                metadata = $3,
                updated_at = now(),
                version = version + 1
            WHERE id = $4 AND user_id = $5
            "#,
        )
        .bind(safe_text(normalized.get("color"), ""))
        .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
        .bind(metadata)
        .bind(tag_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        section.updated += 1;
        return Ok(Some(tag_id));
    }
    let row = sqlx::query(
        r#"
        INSERT INTO tags (user_id, name, color, display_order, metadata)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(safe_text(normalized.get("color"), ""))
    .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
    .bind(metadata)
    .fetch_one(&mut **transaction)
    .await?;
    let tag_id: i64 = row.try_get("id")?;
    existing.by_id.insert(tag_id, name.clone());
    existing.by_name.insert(name, tag_id);
    section.created += 1;
    Ok(Some(tag_id))
}

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

#[tracing::instrument(level = "debug", skip_all)]
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
        let account_role_scope = match bill_analyser_core::account_rules::normalize_account_role_scope(
            get_any(item, &["accountRoleScope", "account_role_scope"]).and_then(Value::as_str),
        ) {
            Ok(value) => value,
            Err(message) => {
                section.skipped += 1;
                warnings.push(format!("Skipped account rule with invalid role scope: {message}"));
                continue;
            }
        };
        let transaction_type_scope =
            match bill_analyser_core::account_rules::normalize_transaction_type_scope(
                get_any(item, &["transactionTypeScope", "transaction_type_scope"])
                    .and_then(Value::as_str),
            ) {
                Ok(value) => value,
                Err(message) => {
                    section.skipped += 1;
                    warnings.push(format!(
                        "Skipped account rule with invalid transaction type scope: {message}"
                    ));
                    continue;
                }
            };
        let field_scope = match bill_analyser_core::account_rules::normalize_account_rule_field_scope(
            get_any(item, &["fieldScope", "field_scope"]),
        ) {
            Ok(value) => value,
            Err(message) => {
                section.skipped += 1;
                warnings.push(format!("Skipped account rule with invalid field scope: {message}"));
                continue;
            }
        };
        let field_scope = Value::Array(field_scope.into_iter().map(Value::String).collect());
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
            account_role_scope.clone(),
            transaction_type_scope.clone(),
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
                    account_role_scope = $7,
                    transaction_type_scope = $8,
                    field_scope = $9,
                    source = $10,
                    source_key = $11,
                    updated_at = now(),
                    version = version + 1
                WHERE id = $12 AND user_id = $13
                "#,
            )
            .bind(account_id)
            .bind(name)
            .bind(settings_i64_to_i32(priority))
            .bind(rule_expression_json)
            .bind(regex_enabled)
            .bind(enabled)
            .bind(account_role_scope)
            .bind(transaction_type_scope)
            .bind(field_scope)
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
                regex_enabled, enabled, account_role_scope, transaction_type_scope,
                field_scope, source, source_key
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
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
        .bind(account_role_scope.clone())
        .bind(transaction_type_scope.clone())
        .bind(field_scope)
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
        SELECT id, account_id, rule_expression, name, account_role_scope, transaction_type_scope
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
        let account_role_scope: String = row.try_get("account_role_scope")?;
        let transaction_type_scope: String = row.try_get("transaction_type_scope")?;
        existing.insert(
            (
                account_id,
                postgres_rule_expression_string(&expression),
                name,
                account_role_scope,
                transaction_type_scope,
            ),
            row.try_get("id")?,
        );
    }
    Ok(existing)
}

fn postgres_metadata_parent_id(metadata: &Value) -> i64 {
    match metadata.get("parent_id") {
        Some(Value::Number(number)) => number.as_i64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or_default(),
        Some(Value::Bool(flag)) => i64::from(*flag),
        Some(Value::Array(_) | Value::Object(_) | Value::Null) | None => 0,
    }
}

fn postgres_category_parts(path: Option<&str>, name: &str) -> (String, String) {
    let path = path.unwrap_or_default().trim();
    if let Some((main, sub)) = path.split_once('/') {
        return (main.trim().to_string(), sub.trim().to_string());
    }
    if !path.is_empty() {
        return (path.to_string(), String::new());
    }
    (name.trim().to_string(), String::new())
}

fn postgres_rule_expression_json(expression: &str, regex_enabled: bool) -> Value {
    json!({
        "expression": expression,
        "regex_enabled": regex_enabled,
    })
}

fn postgres_rule_expression_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(object) => object
            .get("expression")
            .or_else(|| object.get("rule_expression"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_default(),
        Value::Null => String::new(),
        Value::Number(_) | Value::Bool(_) | Value::Array(_) => value.to_string(),
    }
}

fn settings_yuan_to_cents(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

fn settings_i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}
