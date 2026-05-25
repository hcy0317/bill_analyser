// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[allow(clippy::too_many_arguments)]
fn import_settings_templates(
    transaction: &Transaction<'_>,
    templates: &[Value],
    user_id: i64,
    template_type: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    account_ref_map: &BTreeMap<String, i64>,
    category_ref_map: &BTreeMap<String, i64>,
    tag_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    let section_key = if template_type == 2 {
        "scheduledTransactions"
    } else {
        "transactionTemplates"
    };
    let mut existing = load_existing_template_names(transaction, user_id, template_type)?;

    for item in templates {
        upsert_settings_template(
            transaction,
            item,
            user_id,
            template_type,
            result.get_mut(section_key),
            &mut existing,
            warnings,
            account_ref_map,
            category_ref_map,
            tag_ref_map,
        )?;
    }

    Ok(())
}

fn load_existing_template_names(
    transaction: &Transaction<'_>,
    user_id: i64,
    template_type: i64,
) -> DbResult<BTreeMap<String, i64>> {
    let table_name = template_table_name(template_type);
    let mut statement = transaction.prepare(&format!(
        "SELECT id, name FROM {table_name} WHERE user_id = ?1"
    ))?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            row.get::<_, i64>("id")?,
        ))
    })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(DbError::from)
}

#[allow(clippy::too_many_arguments)]
fn upsert_settings_template(
    transaction: &Transaction<'_>,
    item: &Value,
    user_id: i64,
    template_type: i64,
    section: &mut SectionCounts,
    existing: &mut BTreeMap<String, i64>,
    warnings: &mut Vec<String>,
    account_ref_map: &BTreeMap<String, i64>,
    category_ref_map: &BTreeMap<String, i64>,
    tag_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    let name = safe_text(item.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped template without name".to_string());
        return Ok(());
    }

    let resolved = resolve_template_payload(&json!({
        "item": item,
        "account_ref_map": account_ref_map,
        "category_ref_map": category_ref_map,
        "tag_ref_map": tag_ref_map,
    }));
    if let Some(resolved_warnings) = resolved.get("warnings").and_then(Value::as_array) {
        warnings.extend(
            resolved_warnings
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string),
        );
    }
    if resolved
        .get("unresolved")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        section.skipped += 1;
        return Ok(());
    }
    let payload = resolved.get("payload").unwrap_or(&Value::Null);
    let table_name = template_table_name(template_type);
    let now = utc_now_iso();

    if let Some(template_id) = existing.get(&name).copied() {
        let mut update_values = template_update_values(payload, template_type);
        update_values.push(SqlValue::Text(now));
        update_values.push(SqlValue::Integer(template_id));
        update_values.push(SqlValue::Integer(user_id));
        let assignments = if template_type == 2 {
            "description = ?, type = ?, category = ?, amount = ?, account = ?,
             counterparty = ?, destination_amount = ?, hide_amount = ?, tag = ?,
             comment = ?, display_order = ?, hidden = ?, utc_offset = ?,
             frequency = ?, scheduled_frequency_type = ?, start_date = ?, end_date = ?,
             next_date = ?, enabled = ?, auto_create = ?, updated_at = ?"
        } else {
            "description = ?, type = ?, category = ?, amount = ?, account = ?,
             counterparty = ?, destination_amount = ?, hide_amount = ?, tag = ?,
             comment = ?, display_order = ?, hidden = ?, utc_offset = ?, updated_at = ?"
        };
        transaction.execute(
            &format!("UPDATE {table_name} SET {assignments} WHERE id = ? AND user_id = ?"),
            params_from_iter(update_values),
        )?;
        section.updated += 1;
        return Ok(());
    }

    if template_type == 2 {
        let mut insert_values = vec![
            SqlValue::Integer(user_id),
            SqlValue::Null,
            SqlValue::Text(name.clone()),
        ];
        insert_values.extend(template_update_values(payload, template_type));
        insert_values.push(SqlValue::Text(now.clone()));
        insert_values.push(SqlValue::Text(now));
        transaction.execute(
            "INSERT INTO recurring_bills (
                user_id, template_id, name, description, type, category,
                amount, account, counterparty, destination_amount, hide_amount,
                tag, comment, display_order, hidden, utc_offset, frequency,
                scheduled_frequency_type, start_date, end_date, next_date,
                enabled, auto_create, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params_from_iter(insert_values),
        )?;
    } else {
        let mut insert_values = vec![SqlValue::Integer(user_id), SqlValue::Text(name.clone())];
        insert_values.extend(template_update_values(payload, template_type));
        insert_values.push(SqlValue::Integer(0));
        insert_values.push(SqlValue::Text(now.clone()));
        insert_values.push(SqlValue::Text(now));
        transaction.execute(
            "INSERT INTO bill_templates (
                user_id, name, description, type, category, amount, account,
                counterparty, destination_amount, hide_amount, tag, comment,
                display_order, hidden, utc_offset, is_favorite, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params_from_iter(insert_values),
        )?;
    }
    let template_id = transaction.last_insert_rowid();
    existing.insert(name, template_id);
    section.created += 1;
    Ok(())
}

fn template_table_name(template_type: i64) -> &'static str {
    if template_type == 2 {
        "recurring_bills"
    } else {
        "bill_templates"
    }
}

fn template_update_values(payload: &Value, template_type: i64) -> Vec<SqlValue> {
    let mut values = vec![
        SqlValue::Text(safe_text(payload.get("description"), "")),
        SqlValue::Integer(safe_int(payload.get("type"), 3)),
        SqlValue::Text(safe_text(payload.get("category"), "")),
        SqlValue::Real(safe_float(payload.get("amount"), 0.0)),
        SqlValue::Text(safe_text(payload.get("account"), "0")),
        SqlValue::Text(safe_text(payload.get("counterparty"), "0")),
        SqlValue::Real(safe_float(payload.get("destination_amount"), 0.0)),
        SqlValue::Integer(safe_int(payload.get("hide_amount"), 0)),
        SqlValue::Text(safe_text(payload.get("tag"), "")),
        SqlValue::Text(safe_text(payload.get("comment"), "")),
        SqlValue::Integer(safe_int(payload.get("display_order"), 0)),
        SqlValue::Integer(safe_int(payload.get("hidden"), 0)),
        SqlValue::Integer(safe_int(payload.get("utc_offset"), 0)),
    ];
    if template_type == 2 {
        values.extend([
            SqlValue::Text(safe_text(payload.get("frequency"), "")),
            SqlValue::Integer(safe_int(payload.get("scheduled_frequency_type"), 0)),
            SqlValue::Text(safe_text(payload.get("start_date"), "")),
            SqlValue::Text(safe_text(payload.get("end_date"), "")),
            SqlValue::Text(safe_text(payload.get("next_date"), "")),
            SqlValue::Integer(safe_int(payload.get("enabled"), 1)),
            SqlValue::Integer(safe_int(payload.get("auto_create"), 0)),
        ]);
    }
    values
}

fn import_settings_category_rules(
    transaction: &Transaction<'_>,
    rules: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    category_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    let mut existing = load_existing_category_rules(transaction, user_id)?;

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
        let regex_enabled = i64::from(safe_bool(get_any(item, &["regexEnabled", "regex_enabled"])));
        let enabled = i64::from(safe_bool_with_default(item.get("enabled"), true));
        let now = utc_now_iso();
        let key = (category_id, rule_expression.clone(), name.clone());

        if let Some(rule_id) = existing.get(&key).copied() {
            transaction.execute(
                "UPDATE category_rules
                 SET category_id = ?, name = ?, priority = ?, rule_expression = ?,
                     regex_enabled = ?, enabled = ?, updated_at = ?
                 WHERE id = ? AND user_id = ?",
                params![
                    category_id,
                    name,
                    priority,
                    rule_expression,
                    regex_enabled,
                    enabled,
                    now,
                    rule_id,
                    user_id
                ],
            )?;
            section.updated += 1;
            continue;
        }

        transaction.execute(
            "INSERT INTO category_rules (
                user_id, category_id, name, priority, rule_expression,
                regex_enabled, enabled, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                user_id,
                category_id,
                name,
                priority,
                rule_expression,
                regex_enabled,
                enabled,
                now,
                now
            ],
        )?;
        existing.insert(key, transaction.last_insert_rowid());
        section.created += 1;
    }

    Ok(())
}

fn load_existing_category_rules(
    transaction: &Transaction<'_>,
    user_id: i64,
) -> DbResult<BTreeMap<(i64, String, String), i64>> {
    let mut statement = transaction.prepare(
        "SELECT id, category_id, rule_expression, name FROM category_rules WHERE user_id = ?1",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            (
                row.get::<_, Option<i64>>("category_id")?.unwrap_or(0),
                row.get::<_, Option<String>>("rule_expression")?
                    .unwrap_or_default(),
                row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            ),
            row.get::<_, i64>("id")?,
        ))
    })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(DbError::from)
}

fn resolve_settings_category_id(
    item: &Value,
    category_ref_map: &BTreeMap<String, i64>,
) -> Option<i64> {
    let category_ref = safe_text(get_any(item, &["categoryRef", "category_ref"]), "");
    if let Some(category_id) = category_ref_map.get(&category_ref).copied() {
        return Some(category_id);
    }
    let category_id = safe_int(get_any(item, &["categoryId", "category_id"]), 0);
    if let Some(category_id) = category_ref_map
        .get(&local_id_ref("category", category_id))
        .copied()
    {
        return Some(category_id);
    }
    let category_name_value = get_any(item, &["categoryName", "category_name"]);
    let mut main = safe_text(get_any(item, &["mainCategory", "main_category"]), "");
    let mut sub = safe_text(get_any(item, &["subCategory", "sub_category"]), "");
    if main.is_empty() && sub.is_empty() {
        (main, sub) = split_category_name(category_name_value);
    }
    let full_name = category_name(&main, &sub);
    category_ref_map
        .get(&format!("categoryName:{full_name}"))
        .copied()
}

fn import_settings_llm_configs(
    transaction: &Transaction<'_>,
    configs: &[Value],
    user_id: i64,
    result: &mut ImportSections,
) -> DbResult<()> {
    ensure_settings_llm_credential_column(transaction)?;
    let mut existing = load_existing_llm_configs(transaction, user_id)?;
    let has_timestamps = table_has_column(transaction, "llm_configs", "updated_at")?;
    let has_credential_config = table_has_column(transaction, "llm_configs", "credential_config")?;
    for item in configs {
        let name = safe_text(item.get("name"), "");
        let section = result.get_mut("llmConfigs");
        if name.is_empty() {
            section.skipped += 1;
            continue;
        }
        let provider = safe_text(item.get("provider"), "openai");
        let model = safe_text(item.get("model"), "");
        let incoming_secret = safe_text(get_any(item, &["apiKey", "api_key"]), "");
        let base_url = safe_text(get_any(item, &["baseUrl", "base_url"]), "");
        let incoming_credential_config =
            settings_llm_credential_config(item, &incoming_secret);
        let advanced_settings =
            dump_json_object(get_any(item, &["advancedSettings", "advanced_settings"]));
        let now = utc_now_iso();

        if let Some(row) = existing.get(&name).cloned() {
            let api_key = if is_masked_secret(&incoming_secret) {
                row.api_key.clone()
            } else {
                incoming_secret
            };
            let credential_config = if settings_credential_config_is_masked(&incoming_credential_config) {
                row.credential_config.clone()
            } else {
                incoming_credential_config.to_string()
            };
            if has_timestamps {
                if has_credential_config {
                    transaction.execute(
                        "UPDATE llm_configs
                         SET provider = ?, model = ?, api_key = ?, base_url = ?,
                             credential_config = ?, advanced_settings = ?, updated_at = ?
                         WHERE id = ? AND user_id = ?",
                        params![
                            provider,
                            model,
                            api_key,
                            base_url,
                            credential_config,
                            advanced_settings,
                            now,
                            row.id,
                            user_id
                        ],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE llm_configs
                         SET provider = ?, model = ?, api_key = ?, base_url = ?,
                             advanced_settings = ?, updated_at = ?
                         WHERE id = ? AND user_id = ?",
                        params![
                            provider,
                            model,
                            api_key,
                            base_url,
                            advanced_settings,
                            now,
                            row.id,
                            user_id
                        ],
                    )?;
                }
            } else if has_credential_config {
                transaction.execute(
                    "UPDATE llm_configs
                     SET provider = ?, model = ?, api_key = ?, base_url = ?,
                         credential_config = ?, advanced_settings = ?
                     WHERE id = ? AND user_id = ?",
                    params![
                        provider,
                        model,
                        api_key,
                        base_url,
                        credential_config,
                        advanced_settings,
                        row.id,
                        user_id
                    ],
                )?;
            } else {
                transaction.execute(
                    "UPDATE llm_configs
                     SET provider = ?, model = ?, api_key = ?, base_url = ?,
                         advanced_settings = ?
                     WHERE id = ? AND user_id = ?",
                    params![
                        provider,
                        model,
                        api_key,
                        base_url,
                        advanced_settings,
                        row.id,
                        user_id
                    ],
                )?;
            }
            existing.insert(
                name,
                ExistingLlmConfig {
                    id: row.id,
                    api_key,
                    credential_config,
                },
            );
            section.updated += 1;
            continue;
        }

        let stored_secret = if is_masked_secret(&incoming_secret) {
            String::new()
        } else {
            incoming_secret
        };
        let credential_config = incoming_credential_config.to_string();
        if has_timestamps {
            if has_credential_config {
                transaction.execute(
                    "INSERT INTO llm_configs (
                        user_id, name, provider, model, api_key, base_url,
                        credential_config, advanced_settings, is_active, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
                    params![
                        user_id,
                        name,
                        provider,
                        model,
                        stored_secret,
                        base_url,
                        credential_config,
                        advanced_settings,
                        now,
                        now
                    ],
                )?;
            } else {
                transaction.execute(
                    "INSERT INTO llm_configs (
                        user_id, name, provider, model, api_key, base_url,
                        advanced_settings, is_active, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
                    params![
                        user_id,
                        name,
                        provider,
                        model,
                        stored_secret,
                        base_url,
                        advanced_settings,
                        now,
                        now
                    ],
                )?;
            }
        } else if has_credential_config {
            transaction.execute(
                "INSERT INTO llm_configs (
                    user_id, name, provider, model, api_key, base_url,
                    credential_config, advanced_settings, is_active
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0)",
                params![
                    user_id,
                    name,
                    provider,
                    model,
                    stored_secret,
                    base_url,
                    credential_config,
                    advanced_settings
                ],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO llm_configs (
                    user_id, name, provider, model, api_key, base_url,
                    advanced_settings, is_active
                ) VALUES (?, ?, ?, ?, ?, ?, ?, 0)",
                params![
                    user_id,
                    name,
                    provider,
                    model,
                    stored_secret,
                    base_url,
                    advanced_settings
                ],
            )?;
        }
        existing.insert(
            name,
            ExistingLlmConfig {
                id: transaction.last_insert_rowid(),
                api_key: stored_secret,
                credential_config,
            },
        );
        section.created += 1;
    }

    Ok(())
}

fn load_existing_llm_configs(
    transaction: &Transaction<'_>,
    user_id: i64,
) -> DbResult<BTreeMap<String, ExistingLlmConfig>> {
    let has_credential_config = table_has_column(transaction, "llm_configs", "credential_config")?;
    let sql = if has_credential_config {
        "SELECT id, name, api_key, credential_config FROM llm_configs WHERE user_id = ?1"
    } else {
        "SELECT id, name, api_key, '{}' AS credential_config FROM llm_configs WHERE user_id = ?1"
    };
    let mut statement = transaction.prepare(sql)?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            ExistingLlmConfig {
                id: row.get::<_, i64>("id")?,
                api_key: row.get::<_, Option<String>>("api_key")?.unwrap_or_default(),
                credential_config: row
                    .get::<_, Option<String>>("credential_config")?
                    .unwrap_or_else(|| "{}".to_string()),
            },
        ))
    })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(DbError::from)
}

fn ensure_settings_llm_credential_column(transaction: &Transaction<'_>) -> DbResult<()> {
    if !table_has_column(transaction, "llm_configs", "credential_config")? {
        transaction.execute(
            "ALTER TABLE llm_configs ADD COLUMN credential_config TEXT NOT NULL DEFAULT '{}'",
            [],
        )?;
    }
    Ok(())
}

fn settings_llm_credential_config(item: &Value, api_key: &str) -> Value {
    let source = get_any(
        item,
        &[
            "credentialConfig",
            "credential_config",
            "authProfile",
            "auth_profile",
        ],
    )
    .cloned()
    .unwrap_or_else(|| json!({}));
    let mut normalized = bill_analyser_core::normalize_provider_auth_config(Some(&source));
    if normalized
        .get("access_token")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .is_empty()
        && !is_masked_secret(api_key)
    {
        if let Some(object) = normalized.as_object_mut() {
            object.insert("access_token".to_string(), json!(api_key.trim()));
            object.insert("credential_mode".to_string(), json!("api_key"));
        }
    }
    normalized
}

fn settings_credential_config_is_masked(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object
        .get("access_token")
        .and_then(Value::as_str)
        .is_some_and(is_masked_secret)
        || object
            .get("refresh_token")
            .and_then(Value::as_str)
            .is_some_and(is_masked_secret)
}

fn import_settings_ocr_config(
    transaction: &Transaction<'_>,
    configs: &[Value],
    result: &mut ImportSections,
) -> DbResult<()> {
    if configs.is_empty() {
        return Ok(());
    }
    let normalized = normalize_ocr_config(&json!({
        "provider": safe_text(configs[0].get("provider"), "disabled"),
        "lang": safe_text(configs[0].get("lang"), "chi_sim+eng"),
        "model": safe_text(configs[0].get("model"), ""),
        "baseUrl": safe_text(get_any(&configs[0], &["baseUrl", "base_url"]), ""),
        "parameters": get_any(&configs[0], &["parameters", "params"])
            .cloned()
            .unwrap_or_else(|| json!({})),
        "credentialConfig": get_any(
            &configs[0],
            &["credentialConfig", "credential_config", "authProfile", "auth_profile"]
        )
        .cloned()
        .unwrap_or_else(|| json!({})),
    }));
    let now = utc_now_iso();
    let existing = transaction
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![OCR_CONFIG_SETTING_KEY],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    transaction.execute(
        "INSERT INTO app_settings (
            key, value, value_type, description, is_encrypted, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            value_type = excluded.value_type,
            description = excluded.description,
            is_encrypted = excluded.is_encrypted,
            updated_at = excluded.updated_at",
        params![
            OCR_CONFIG_SETTING_KEY,
            normalized.to_string(),
            "json",
            "Receipt OCR runtime configuration",
            0,
            now,
            now
        ],
    )?;
    let section = result.get_mut("ocrConfig");
    if existing.is_some() {
        section.updated += 1;
    } else {
        section.created += 1;
    }
    Ok(())
}

fn required_array<'payload>(payload: &'payload Value, key: &str) -> DbResult<&'payload Vec<Value>> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be an array")))
}
