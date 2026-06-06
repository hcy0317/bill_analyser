// 中文导读：PostgreSQL authority 设置包导入路径，复用设置包 schema/ref 解析并用 Postgres 事务承载 preview rollback。
// 维护重点：保持 user-scope、幂等 upsert 和引用重映射；不要回退到 non-Postgres runtime。
// 不变式：dry_run 必须 rollback；真实导入必须按用户事务提交；账户余额从设置包元单位写入分单位，模板金额保持当前交易 DTO 分单位。

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
    import_postgres_settings_templates(
        &mut transaction,
        "transactionTemplates",
        section_items(&sections, "transactionTemplates"),
        user_id,
        1,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
        &category_ref_map,
        &tag_ref_map,
    )
    .await?;
    import_postgres_settings_templates(
        &mut transaction,
        "scheduledTransactions",
        section_items(&sections, "scheduledTransactions"),
        user_id,
        2,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
        &category_ref_map,
        &tag_ref_map,
    )
    .await?;
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
#[allow(clippy::too_many_arguments)]
async fn import_postgres_settings_templates(
    transaction: &mut PgTransaction<'_, Postgres>,
    section_key: &str,
    templates: &[Value],
    user_id: i64,
    template_type: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    account_ref_map: &BTreeMap<String, i64>,
    category_ref_map: &BTreeMap<String, i64>,
    tag_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    if templates.is_empty() {
        return Ok(());
    }

    let mut existing = load_existing_postgres_templates(transaction, user_id).await?;
    for item in templates {
        let name = safe_text(item.get("name"), "");
        let fallback_display_order = existing
            .by_key
            .get(&(template_type, name.clone()))
            .map_or_else(
                || existing.next_display_order(template_type),
                |record| record.display_order,
            );
        let values = {
            let section = result.get_mut(section_key);
            settings_template_values_from_item(
                item,
                template_type,
                fallback_display_order,
                account_ref_map,
                category_ref_map,
                tag_ref_map,
                section,
                warnings,
            )
        };
        let Some(values) = values else {
            continue;
        };
        let key = (values.template_type, values.name.clone());

        if let Some(record) = existing.by_key.get(&key).cloned() {
            update_postgres_settings_template(transaction, user_id, record.id, &values).await?;
            existing.by_key.insert(
                key,
                ExistingTemplate {
                    id: record.id,
                    display_order: values.display_order,
                },
            );
            result.get_mut(section_key).updated += 1;
            continue;
        }

        let template_id = insert_postgres_settings_template(transaction, user_id, &values).await?;
        existing.by_key.insert(
            key,
            ExistingTemplate {
                id: template_id,
                display_order: values.display_order,
            },
        );
        result.get_mut(section_key).created += 1;
    }

    Ok(())
}

async fn load_existing_postgres_templates(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingTemplates> {
    let rows = sqlx::query(
        r#"
        SELECT id, template_type, name, display_order
        FROM transaction_templates
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = ExistingTemplates::default();
    for row in rows {
        let template_type = i64::from(row.try_get::<i32, _>("template_type")?);
        let name: String = row.try_get("name")?;
        existing.by_key.insert(
            (template_type, name),
            ExistingTemplate {
                id: row.try_get("id")?,
                display_order: i64::from(row.try_get::<i32, _>("display_order")?),
            },
        );
    }
    Ok(existing)
}

#[allow(clippy::too_many_arguments)]
fn settings_template_values_from_item(
    item: &Value,
    template_type: i64,
    fallback_display_order: i64,
    account_ref_map: &BTreeMap<String, i64>,
    category_ref_map: &BTreeMap<String, i64>,
    tag_ref_map: &BTreeMap<String, i64>,
    section: &mut SectionCounts,
    warnings: &mut Vec<String>,
) -> Option<SettingsTemplateImportValues> {
    let name = safe_text(item.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped template without name".to_string());
        return None;
    }

    let resolved = resolve_template_payload(&json!({
        "item": item,
        "account_ref_map": account_ref_map,
        "category_ref_map": category_ref_map,
        "tag_ref_map": tag_ref_map,
    }));
    warnings.extend(
        resolved
            .get("warnings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned),
    );
    if safe_bool(resolved.get("unresolved")) {
        section.skipped += 1;
        return None;
    }

    let payload = resolved.get("payload").unwrap_or(&Value::Null);
    let scheduled = template_type == 2;
    let display_order = get_any(item, &["displayOrder", "display_order"])
        .map(|value| safe_int(Some(value), fallback_display_order))
        .unwrap_or(fallback_display_order);
    let source_account_id = settings_zero_id_text(payload.get("account"));
    let destination_account_id = settings_zero_id_text(payload.get("counterparty"));

    Some(SettingsTemplateImportValues {
        name,
        template_type,
        description: settings_optional_text(payload.get("description")),
        transaction_type: Some(safe_text(payload.get("type"), "3")),
        category_id: settings_optional_text(payload.get("category")),
        source_account_id,
        destination_account_id,
        source_amount_minor_units: settings_round_minor_units(get_any(
            item,
            &["sourceAmount", "source_amount", "amount"],
        )),
        destination_amount_minor_units: settings_round_minor_units(get_any(
            item,
            &["destinationAmount", "destination_amount"],
        )),
        hide_amount: safe_int(payload.get("hide_amount"), 0) != 0,
        tag_ids: settings_template_tag_ids(payload),
        comment: settings_optional_text(payload.get("comment")),
        scheduled_frequency_type: scheduled
            .then(|| safe_int(payload.get("scheduled_frequency_type"), 0)),
        scheduled_frequency: scheduled
            .then(|| settings_optional_text(payload.get("frequency")))
            .flatten(),
        scheduled_start_date: scheduled
            .then(|| settings_optional_text(payload.get("start_date")))
            .flatten(),
        scheduled_end_date: scheduled
            .then(|| settings_optional_text(payload.get("end_date")))
            .flatten(),
        scheduled_next_date: scheduled
            .then(|| settings_optional_text(payload.get("next_date")))
            .flatten(),
        enabled: safe_int(payload.get("enabled"), 1) != 0,
        auto_create: safe_int(payload.get("auto_create"), 0) != 0,
        display_order,
        hidden: safe_int(payload.get("hidden"), 0) != 0,
        utc_offset: safe_int(payload.get("utc_offset"), 0),
    })
}

async fn update_postgres_settings_template(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
    template_id: i64,
    values: &SettingsTemplateImportValues,
) -> DbResult<()> {
    sqlx::query(
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
    .bind(&values.name)
    .bind(&values.description)
    .bind(&values.transaction_type)
    .bind(&values.category_id)
    .bind(&values.source_account_id)
    .bind(&values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(&values.tag_ids)
    .bind(&values.comment)
    .bind(values.scheduled_frequency_type.map(settings_i64_to_i32))
    .bind(&values.scheduled_frequency)
    .bind(&values.scheduled_start_date)
    .bind(&values.scheduled_end_date)
    .bind(&values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(settings_i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(settings_i64_to_i32(values.utc_offset))
    .bind(template_id)
    .bind(user_id)
    .bind(settings_i64_to_i32(values.template_type))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn insert_postgres_settings_template(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
    values: &SettingsTemplateImportValues,
) -> DbResult<i64> {
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
    .bind(settings_i64_to_i32(values.template_type))
    .bind(&values.name)
    .bind(&values.description)
    .bind(&values.transaction_type)
    .bind(&values.category_id)
    .bind(&values.source_account_id)
    .bind(&values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(&values.tag_ids)
    .bind(&values.comment)
    .bind(values.scheduled_frequency_type.map(settings_i64_to_i32))
    .bind(&values.scheduled_frequency)
    .bind(&values.scheduled_start_date)
    .bind(&values.scheduled_end_date)
    .bind(&values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(settings_i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(settings_i64_to_i32(values.utc_offset))
    .fetch_one(&mut **transaction)
    .await?;
    row.try_get("id").map_err(Into::into)
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
        if let Some(warning) = account_rule_scope_compat_warning(item) {
            warnings.push(warning);
        }
        let (account_role_scope, transaction_type_scope, field_scope) =
            default_account_rule_scope_columns();
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

#[derive(Debug, Clone, Default)]
struct ExistingTemplates {
    by_key: BTreeMap<(i64, String), ExistingTemplate>,
}

impl ExistingTemplates {
    fn next_display_order(&self, template_type: i64) -> i64 {
        self.by_key
            .iter()
            .filter(|((existing_type, _), _)| *existing_type == template_type)
            .map(|(_, record)| record.display_order)
            .max()
            .unwrap_or_default()
            + 1
    }
}

#[derive(Debug, Clone)]
struct ExistingTemplate {
    id: i64,
    display_order: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct SettingsTemplateImportValues {
    name: String,
    template_type: i64,
    description: Option<String>,
    transaction_type: Option<String>,
    category_id: Option<String>,
    source_account_id: String,
    destination_account_id: String,
    source_amount_minor_units: i64,
    destination_amount_minor_units: i64,
    hide_amount: bool,
    tag_ids: Value,
    comment: Option<String>,
    scheduled_frequency_type: Option<i64>,
    scheduled_frequency: Option<String>,
    scheduled_start_date: Option<String>,
    scheduled_end_date: Option<String>,
    scheduled_next_date: Option<String>,
    enabled: bool,
    auto_create: bool,
    display_order: i64,
    hidden: bool,
    utc_offset: i64,
}

fn settings_optional_text(value: Option<&Value>) -> Option<String> {
    let text = safe_text(value, "");
    (!text.is_empty()).then_some(text)
}

fn settings_zero_id_text(value: Option<&Value>) -> String {
    let text = safe_text(value, "0");
    if text.is_empty() {
        "0".to_string()
    } else {
        text
    }
}

fn settings_template_tag_ids(payload: &Value) -> Value {
    Value::Array(
        safe_text(payload.get("tag"), "")
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(|item| Value::String(item.to_string()))
            .collect(),
    )
}

fn settings_round_minor_units(value: Option<&Value>) -> i64 {
    let Some(value) = value else {
        return 0;
    };
    settings_round_decimal_text_to_i64(&json_to_display_string(value)).unwrap_or_default()
}

fn settings_round_decimal_text_to_i64(text: &str) -> Option<i64> {
    settings_decimal_text_to_scaled_i64(text, 0)
}

fn settings_decimal_text_to_scaled_i64(text: &str, scale: usize) -> Option<i64> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (negative, magnitude) = trimmed
        .strip_prefix('-')
        .map_or((false, trimmed), |rest| (true, rest));
    let magnitude = magnitude.strip_prefix('+').unwrap_or(magnitude);
    let mut parts = magnitude.splitn(2, '.');
    let integer = parts.next()?.parse::<i64>().ok()?;
    let multiplier = 10_i64.checked_pow(u32::try_from(scale).ok()?)?;
    let fraction = parts.next().unwrap_or_default();
    let mut scaled_fraction = 0_i64;
    let mut consumed = 0_usize;
    let mut round_digit = '0';
    for digit in fraction.chars().filter(char::is_ascii_digit) {
        if consumed < scale {
            scaled_fraction = scaled_fraction
                .checked_mul(10)?
                .checked_add(i64::from(digit.to_digit(10)?))?;
            consumed += 1;
        } else {
            round_digit = digit;
            break;
        }
    }
    for _ in consumed..scale {
        scaled_fraction = scaled_fraction.checked_mul(10)?;
    }
    let scaled = integer
        .checked_mul(multiplier)?
        .checked_add(scaled_fraction)?
        .checked_add(i64::from(round_digit >= '5'))?;
    Some(if negative { -scaled } else { scaled })
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

#[cfg(test)]
mod postgres_settings_template_import_tests {
    use super::*;

    fn ref_maps() -> (
        BTreeMap<String, i64>,
        BTreeMap<String, i64>,
        BTreeMap<String, i64>,
    ) {
        let account_ref_map = BTreeMap::from([
            ("account:1".to_string(), 11),
            ("accountName:现金".to_string(), 22),
        ]);
        let category_ref_map = BTreeMap::from([("category:3".to_string(), 33)]);
        let tag_ref_map = BTreeMap::from([("tag:7".to_string(), 77)]);
        (account_ref_map, category_ref_map, tag_ref_map)
    }

    #[test]
    fn settings_template_values_resolve_refs_and_preserve_minor_units() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "name": "午餐模板",
                "type": 3,
                "categoryRef": "category:3",
                "sourceAccountRef": "account:1",
                "destinationAccountName": "现金",
                "sourceAmount": "1234.5",
                "destinationAmount": "88.49",
                "hideAmount": true,
                "tagRefs": ["tag:7"],
                "comment": "常用午餐",
                "displayOrder": 9,
                "hidden": true,
                "utcOffset": 480
            }),
            1,
            5,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("valid template values");

        assert_eq!(values.template_type, 1);
        assert_eq!(values.category_id.as_deref(), Some("33"));
        assert_eq!(values.source_account_id, "11");
        assert_eq!(values.destination_account_id, "22");
        assert_eq!(values.source_amount_minor_units, 1235);
        assert_eq!(values.destination_amount_minor_units, 88);
        assert_eq!(values.tag_ids, json!(["77"]));
        assert_eq!(values.display_order, 9);
        assert!(values.hide_amount);
        assert!(values.hidden);
        assert!(warnings.is_empty());
        assert_eq!(section.skipped, 0);
    }

    #[test]
    fn settings_scheduled_template_values_force_section_type_and_next_date() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "templateType": 1,
                "name": "房租计划",
                "sourceAccountRef": "account:1",
                "sourceAmount": 250000,
                "scheduledFrequencyType": 3,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "enabled": false,
                "autoCreate": true
            }),
            2,
            12,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("valid scheduled template values");

        assert_eq!(values.template_type, 2);
        assert_eq!(values.source_amount_minor_units, 250000);
        assert_eq!(values.scheduled_frequency_type, Some(3));
        assert_eq!(values.scheduled_frequency.as_deref(), Some("1"));
        assert_eq!(values.scheduled_start_date.as_deref(), Some("2026-06-01"));
        assert_eq!(values.scheduled_next_date.as_deref(), Some("2026-06-01"));
        assert!(!values.enabled);
        assert!(values.auto_create);
        assert_eq!(values.display_order, 12);
        assert!(warnings.is_empty());
    }

    #[test]
    fn settings_template_values_skip_unresolved_refs_without_unsupported_warning() {
        let (account_ref_map, category_ref_map, tag_ref_map) = ref_maps();
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let values = settings_template_values_from_item(
            &json!({
                "name": "缺失分类",
                "categoryRef": "category:missing",
                "sourceAccountRef": "account:1",
                "sourceAmount": 1999
            }),
            1,
            1,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        );

        assert!(values.is_none());
        assert_eq!(section.skipped, 1);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("unresolved category reference")));
        assert!(warnings
            .iter()
            .all(|warning| !warning.contains("not supported")));
    }

    #[test]
    fn exported_template_sections_build_importable_roundtrip_values() {
        let sections = export_taxonomy_sections(&json!({
            "accounts": [{
                "id": 1,
                "name": "现金",
                "type": 1,
                "currency": "CNY"
            }],
            "categories": [{
                "id": 3,
                "type": 3,
                "main_category": "餐饮",
                "sub_category": "午餐"
            }],
            "tags": [{
                "id": 7,
                "name": "项目"
            }],
            "templates": [{
                "id": "8",
                "templateType": 1,
                "name": "午餐模板",
                "type": 3,
                "categoryId": "3",
                "sourceAccountId": "1",
                "destinationAccountId": "0",
                "sourceAmount": 1234,
                "destinationAmount": 0,
                "hideAmount": false,
                "tagIds": ["7"],
                "comment": "工作日午餐",
                "displayOrder": 4
            }],
            "scheduled": [{
                "id": "9",
                "templateType": 2,
                "name": "房租计划",
                "type": 3,
                "categoryId": "3",
                "sourceAccountId": "1",
                "destinationAccountId": "0",
                "sourceAmount": 250000,
                "destinationAmount": 0,
                "hideAmount": false,
                "tagIds": ["7"],
                "scheduledFrequencyType": 3,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "displayOrder": 5
            }]
        }))
        .expect("taxonomy sections export");
        let template = &sections["transactionTemplates"][0];
        let scheduled = &sections["scheduledTransactions"][0];
        assert_eq!(template["sourceAccountRef"], "account:1");
        assert_eq!(template["categoryRef"], "category:3");
        assert_eq!(template["tagRefs"], json!(["tag:7"]));
        assert!(template.get("id").is_none());
        assert!(template.get("tagIds").is_none());

        let account_ref_map = BTreeMap::from([("account:1".to_string(), 101)]);
        let category_ref_map = BTreeMap::from([("category:3".to_string(), 303)]);
        let tag_ref_map = BTreeMap::from([("tag:7".to_string(), 707)]);
        let mut section = SectionCounts::default();
        let mut warnings = Vec::new();

        let template_values = settings_template_values_from_item(
            template,
            1,
            1,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("exported transaction template values");
        let scheduled_values = settings_template_values_from_item(
            scheduled,
            2,
            2,
            &account_ref_map,
            &category_ref_map,
            &tag_ref_map,
            &mut section,
            &mut warnings,
        )
        .expect("exported scheduled template values");

        assert_eq!(template_values.source_account_id, "101");
        assert_eq!(template_values.category_id.as_deref(), Some("303"));
        assert_eq!(template_values.source_amount_minor_units, 1234);
        assert_eq!(template_values.tag_ids, json!(["707"]));
        assert_eq!(scheduled_values.template_type, 2);
        assert_eq!(scheduled_values.source_amount_minor_units, 250000);
        assert_eq!(
            scheduled_values.scheduled_start_date.as_deref(),
            Some("2026-06-01")
        );
        assert!(warnings.is_empty());
        assert_eq!(section.skipped, 0);
    }

    #[test]
    fn settings_template_minor_units_are_not_yuan_scaled() {
        assert_eq!(settings_round_minor_units(Some(&json!(1999))), 1999);
        assert_eq!(settings_round_minor_units(Some(&json!("18.5"))), 19);
        assert_eq!(settings_round_minor_units(Some(&json!("-18.5"))), -19);
    }
}
