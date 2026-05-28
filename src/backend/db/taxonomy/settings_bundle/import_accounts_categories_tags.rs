// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
pub fn import_settings_bundle(
    connection: &mut Connection,
    bundle: &Value,
    user_id: i64,
    dry_run: bool,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "import_settings_bundle", "business operation entered");
    let sections = normalize_settings_bundle_sections(bundle)?;
    let transaction = connection.transaction()?;
    let mut result_sections = ImportSections::new();
    let mut warnings = Vec::new();

    let account_ref_map = import_settings_accounts(
        &transaction,
        section_items(&sections, "accounts"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )?;
    let category_ref_map = import_settings_categories(
        &transaction,
        section_items(&sections, "transactionCategories"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )?;
    let tag_ref_map = import_settings_tags(
        &transaction,
        section_items(&sections, "transactionTags"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )?;
    import_settings_templates(
        &transaction,
        section_items(&sections, "transactionTemplates"),
        user_id,
        1,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
        &category_ref_map,
        &tag_ref_map,
    )?;
    import_settings_templates(
        &transaction,
        section_items(&sections, "scheduledTransactions"),
        user_id,
        2,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
        &category_ref_map,
        &tag_ref_map,
    )?;
    import_settings_category_rules(
        &transaction,
        section_items(&sections, "categoryRecognitionRules"),
        user_id,
        &mut result_sections,
        &mut warnings,
        &category_ref_map,
    )?;
    import_settings_account_rules(
        &transaction,
        section_items(&sections, "accountRecognitionRules"),
        user_id,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
    )?;
    import_settings_llm_configs(
        &transaction,
        section_items(&sections, "llmConfigs"),
        user_id,
        &mut result_sections,
    )?;
    import_settings_ocr_config(
        &transaction,
        section_items(&sections, "ocrConfig"),
        &mut result_sections,
    )?;

    if dry_run {
        transaction.rollback()?;
    } else {
        transaction.commit()?;
    }

    Ok(json!({
        "dryRun": dry_run,
        "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
        "sections": result_sections.into_value(),
        "warnings": warnings,
    }))
}

fn section_items<'payload>(sections: &'payload Value, section: &str) -> &'payload [Value] {
    sections
        .get(section)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_settings_accounts(
    transaction: &Transaction<'_>,
    accounts: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_accounts(transaction, user_id)?;
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
            if let Some(account_id) = upsert_settings_account(
                transaction,
                item,
                user_id,
                result.get_mut("accounts"),
                &mut existing,
                &ref_map,
                warnings,
            )? {
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
        if let Some(account_id) = upsert_settings_account(
            transaction,
            &Value::Object(root_item),
            user_id,
            result.get_mut("accounts"),
            &mut existing,
            &ref_map,
            warnings,
        )? {
            add_account_refs(&mut ref_map, item, account_id);
        }
    }

    Ok(ref_map)
}

fn add_account_refs(ref_map: &mut BTreeMap<String, i64>, item: &Value, account_id: i64) {
    let account_ref = external_ref(item, "account");
    if !account_ref.is_empty() {
        ref_map.insert(account_ref, account_id);
    }
    let name = safe_text(item.get("name"), "");
    if !name.is_empty() {
        ref_map.insert(format!("accountName:{name}"), account_id);
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_existing_accounts(
    transaction: &Transaction<'_>,
    user_id: i64,
) -> DbResult<ExistingAccounts> {
    let mut statement =
        transaction.prepare("SELECT id, name, parent_id FROM accounts WHERE user_id = ?1")?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            row.get::<_, Option<i64>>("parent_id")?.unwrap_or(0),
        ))
    })?;
    let mut existing = ExistingAccounts::default();
    for row in rows {
        let (account_id, name, parent_id) = row?;
        existing.by_id.insert(account_id, (name.clone(), parent_id));
        existing.by_key.insert((name, parent_id), account_id);
    }
    Ok(existing)
}

#[tracing::instrument(level = "debug", skip_all)]
fn upsert_settings_account(
    transaction: &Transaction<'_>,
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
    let now = utc_now_iso();
    let values = account_import_sql_values(&normalized, &now, user_id)?;

    if let Some(account_id) = existing.by_key.get(&(name.clone(), parent_id)).copied() {
        let mut update_values = account_update_sql_values(&normalized)?;
        update_values.push(SqlValue::Text(now));
        update_values.push(SqlValue::Integer(account_id));
        update_values.push(SqlValue::Integer(user_id));
        transaction.execute(
            "UPDATE accounts
             SET type = ?, category = ?, currency = ?, icon = ?, color = ?,
                 balance = ?, initial_balance = ?, hidden = ?, display_order = ?,
                 comment = ?, updated_at = ?
             WHERE id = ? AND user_id = ?",
            params_from_iter(update_values),
        )?;
        section.updated += 1;
        return Ok(Some(account_id));
    }

    transaction.execute(
        "INSERT INTO accounts (
            user_id, name, type, category, currency, icon, color, balance,
            initial_balance, hidden, display_order, comment,
            parent_id, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params_from_iter(values),
    )?;
    let account_id = transaction.last_insert_rowid();
    existing.by_id.insert(account_id, (name.clone(), parent_id));
    existing.by_key.insert((name, parent_id), account_id);
    section.created += 1;
    Ok(Some(account_id))
}

fn account_import_sql_values(
    normalized: &Value,
    now: &str,
    user_id: i64,
) -> DbResult<Vec<SqlValue>> {
    let mut values = vec![SqlValue::Integer(user_id)];
    values.push(sql_value_or_null(normalized.get("name"))?);
    values.extend(account_update_sql_values(normalized)?);
    values.push(SqlValue::Integer(safe_int(normalized.get("parent_id"), 0)));
    values.push(SqlValue::Text(now.to_string()));
    values.push(SqlValue::Text(now.to_string()));
    Ok(values)
}

fn account_update_sql_values(normalized: &Value) -> DbResult<Vec<SqlValue>> {
    Ok(vec![
        SqlValue::Integer(safe_int(normalized.get("type"), 1)),
        sql_value_or_null(normalized.get("category"))?,
        SqlValue::Text(safe_text(normalized.get("currency"), "CNY")),
        SqlValue::Text(safe_text(normalized.get("icon"), "")),
        SqlValue::Text(safe_text(normalized.get("color"), "")),
        SqlValue::Real(safe_float(normalized.get("balance"), 0.0)),
        SqlValue::Real(safe_float(normalized.get("initial_balance"), 0.0)),
        SqlValue::Integer(safe_int(normalized.get("hidden"), 0)),
        SqlValue::Integer(safe_int(normalized.get("display_order"), 0)),
        SqlValue::Text(safe_text(normalized.get("comment"), "")),
    ])
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_settings_categories(
    transaction: &Transaction<'_>,
    categories: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_categories(transaction, user_id)?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|category_id| (local_id_ref("category", *category_id), *category_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(
        existing
            .by_id
            .iter()
            .filter_map(|(category_id, (main, sub))| {
                let name = category_name(main, sub);
                (!name.is_empty()).then(|| (format!("categoryName:{name}"), *category_id))
            }),
    );

    for item in categories {
        if let Some(category_id) = upsert_settings_category(
            transaction,
            item,
            user_id,
            result.get_mut("transactionCategories"),
            &mut existing,
            warnings,
        )? {
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

#[tracing::instrument(level = "debug", skip_all)]
fn load_existing_categories(
    transaction: &Transaction<'_>,
    user_id: i64,
) -> DbResult<ExistingCategories> {
    let mut statement = transaction
        .prepare("SELECT id, main_category, sub_category FROM categories WHERE user_id = ?1")?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, Option<String>>("main_category")?
                .unwrap_or_default(),
            row.get::<_, Option<String>>("sub_category")?
                .unwrap_or_default(),
        ))
    })?;
    let mut existing = ExistingCategories::default();
    for row in rows {
        let (category_id, main, sub) = row?;
        existing
            .by_id
            .insert(category_id, (main.clone(), sub.clone()));
        existing.by_key.insert((main, sub), category_id);
    }
    Ok(existing)
}

#[tracing::instrument(level = "debug", skip_all)]
fn upsert_settings_category(
    transaction: &Transaction<'_>,
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
    let values = category_update_sql_values(&normalized);
    if let Some(category_id) = existing.by_key.get(&(main.clone(), sub.clone())).copied() {
        let mut update_values = values;
        update_values.push(SqlValue::Integer(category_id));
        update_values.push(SqlValue::Integer(user_id));
        transaction.execute(
            "UPDATE categories
             SET type = ?, description = ?, priority = ?, keywords = ?,
                 hidden = ?, icon = ?, color = ?
             WHERE id = ? AND user_id = ?",
            params_from_iter(update_values),
        )?;
        section.updated += 1;
        return Ok(Some(category_id));
    }

    let now = utc_now_iso();
    let mut insert_values = vec![
        SqlValue::Integer(user_id),
        SqlValue::Integer(safe_int(normalized.get("type"), 3)),
        SqlValue::Text(main.clone()),
        SqlValue::Text(sub.clone()),
        SqlValue::Text(safe_text(normalized.get("description"), "")),
        SqlValue::Integer(safe_int(normalized.get("priority"), 0)),
        SqlValue::Text(safe_text(normalized.get("keywords"), "")),
        SqlValue::Integer(safe_int(normalized.get("hidden"), 0)),
        SqlValue::Text(safe_text(normalized.get("icon"), "")),
        SqlValue::Text(safe_text(normalized.get("color"), "")),
        SqlValue::Text(now.clone()),
    ];
    transaction.execute(
        "INSERT INTO categories (
            user_id, type, main_category, sub_category, description,
            priority, keywords, hidden, icon, color, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params_from_iter(insert_values.drain(..)),
    )?;
    let category_id = transaction.last_insert_rowid();
    existing
        .by_id
        .insert(category_id, (main.clone(), sub.clone()));
    existing.by_key.insert((main, sub), category_id);
    section.created += 1;
    Ok(Some(category_id))
}

fn category_update_sql_values(normalized: &Value) -> Vec<SqlValue> {
    vec![
        SqlValue::Integer(safe_int(normalized.get("type"), 3)),
        SqlValue::Text(safe_text(normalized.get("description"), "")),
        SqlValue::Integer(safe_int(normalized.get("priority"), 0)),
        SqlValue::Text(safe_text(normalized.get("keywords"), "")),
        SqlValue::Integer(safe_int(normalized.get("hidden"), 0)),
        SqlValue::Text(safe_text(normalized.get("icon"), "")),
        SqlValue::Text(safe_text(normalized.get("color"), "")),
    ]
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_settings_tags(
    transaction: &Transaction<'_>,
    tags: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_tags(transaction, user_id)?;
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
        if let Some(tag_id) = upsert_settings_tag(
            transaction,
            item,
            user_id,
            result.get_mut("transactionTags"),
            &mut existing,
            warnings,
        )? {
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

#[tracing::instrument(level = "debug", skip_all)]
fn load_existing_tags(transaction: &Transaction<'_>, user_id: i64) -> DbResult<ExistingTags> {
    let mut statement =
        transaction.prepare("SELECT id, name FROM tags WHERE user_id = ?1 ORDER BY id")?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, i64>("id")?,
            row.get::<_, Option<String>>("name")?.unwrap_or_default(),
        ))
    })?;
    let mut existing = ExistingTags::default();
    for row in rows {
        let (tag_id, name) = row?;
        existing.by_id.insert(tag_id, name.clone());
        existing.by_name.insert(name, tag_id);
    }
    Ok(existing)
}

#[tracing::instrument(level = "debug", skip_all)]
fn upsert_settings_tag(
    transaction: &Transaction<'_>,
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
    let now = utc_now_iso();
    let values = tag_update_sql_values(&normalized, &now);
    if let Some(tag_id) = existing.by_name.get(&name).copied() {
        let mut update_values = values;
        update_values.push(SqlValue::Integer(tag_id));
        update_values.push(SqlValue::Integer(user_id));
        transaction.execute(
            "UPDATE tags
             SET color = ?, icon = ?, display_order = ?, hidden = ?, updated_at = ?
             WHERE id = ? AND user_id = ?",
            params_from_iter(update_values),
        )?;
        section.updated += 1;
        return Ok(Some(tag_id));
    }

    let mut insert_values = vec![SqlValue::Integer(user_id), SqlValue::Text(name.clone())];
    insert_values.extend(tag_update_sql_values(&normalized, &now));
    insert_values.push(SqlValue::Text(now));
    transaction.execute(
        "INSERT INTO tags (
            user_id, name, color, icon, display_order, hidden, updated_at, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params_from_iter(insert_values),
    )?;
    let tag_id = transaction.last_insert_rowid();
    existing.by_id.insert(tag_id, name.clone());
    existing.by_name.insert(name, tag_id);
    section.created += 1;
    Ok(Some(tag_id))
}

fn tag_update_sql_values(normalized: &Value, now: &str) -> Vec<SqlValue> {
    vec![
        SqlValue::Text(safe_text(normalized.get("color"), "")),
        SqlValue::Text(safe_text(normalized.get("icon"), "")),
        SqlValue::Integer(safe_int(normalized.get("display_order"), 0)),
        SqlValue::Integer(safe_int(normalized.get("hidden"), 0)),
        SqlValue::Text(now.to_string()),
    ]
}
