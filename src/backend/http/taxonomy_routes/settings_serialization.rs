// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn format_category_rules_response(rules: Vec<CategoryRuleRecord>) -> Value {
    let total = rules.len();
    json!({
        "success": true,
        "data": Value::Array(rules.into_iter().map(Value::Object).collect()),
        "total": total,
    })
}

fn category_rule_data_response(status: StatusCode, rule: CategoryRuleRecord) -> Response {
    json_response(
        status,
        json!({
            "success": true,
            "data": Value::Object(rule),
        }),
    )
}

fn build_settings_bundle(
    connection: &mut Connection,
    user_id: i64,
    include_secrets: bool,
) -> Result<Value, String> {
    let accounts = {
        let mut repository = AccountsRepository::new(connection);
        repository
            .list_accounts(user_id)
            .map_err(|error| error.to_string())?
    };
    let categories = {
        let mut repository = CategoriesRepository::new(connection);
        repository
            .list_categories(user_id)
            .map_err(|error| error.to_string())?
    };
    let tags = {
        let mut repository = TagsRepository::new(connection);
        repository
            .list_tags(user_id)
            .map_err(|error| error.to_string())?
    };
    let templates = list_settings_templates(connection, user_id, 1)?;
    let scheduled = list_settings_templates(connection, user_id, 2)?;
    let category_refs = category_ref_map(&categories);
    let account_refs = account_ref_map(&accounts);

    let taxonomy_sections = export_taxonomy_sections(&json!({
        "accounts": records_to_array(&accounts),
        "categories": records_to_array(&categories),
        "tags": tags_to_array(&tags),
        "templates": records_to_array(&templates),
        "scheduled": records_to_array(&scheduled),
    }))
    .map_err(|error| error.to_string())?;

    let mut sections = taxonomy_sections
        .as_object()
        .cloned()
        .ok_or_else(|| "settings taxonomy sections must be an object".to_string())?;

    let category_rules = {
        let mut repository = CategoryRulesRepository::new(connection);
        repository
            .list_rules(user_id, None, false)
            .map_err(|error| error.to_string())?
    };
    sections.insert(
        "categoryRecognitionRules".to_string(),
        Value::Array(
            category_rules
                .iter()
                .map(|rule| export_settings_category_rule(rule, &category_refs))
                .collect(),
        ),
    );
    let account_rules = {
        let mut repository = AccountRulesRepository::new(connection);
        repository
            .list_rules(user_id, None, false, None, None)
            .map_err(|error| error.to_string())?
    };
    sections.insert(
        "accountRecognitionRules".to_string(),
        Value::Array(
            account_rules
                .iter()
                .map(|rule| export_settings_account_rule(rule, &account_refs))
                .collect(),
        ),
    );
    sections.insert(
        "llmConfigs".to_string(),
        Value::Array(list_settings_llm_configs(connection, user_id, include_secrets)?),
    );
    sections.insert(
        "ocrConfig".to_string(),
        Value::Array(vec![export_settings_ocr_config(connection, include_secrets)?]),
    );

    let counts = SETTINGS_BUNDLE_SECTION_KEYS
        .iter()
        .map(|key| {
            let count = sections
                .get(*key)
                .and_then(Value::as_array)
                .map(|items| items.len() as i64)
                .unwrap_or(0);
            ((*key).to_string(), Value::Number(Number::from(count)))
        })
        .collect::<Map<_, _>>();

    Ok(json!({
        "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
        "exportedAt": Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
        "secretsPolicy": {
            "llmApiKeys": if include_secrets { "included" } else { "redacted" },
            "providerCredentials": if include_secrets { "included" } else { "redacted" },
        },
        "sections": Value::Object(sections),
        "counts": Value::Object(counts),
    }))
}

async fn build_postgres_settings_bundle(
    pool: &PostgresPool,
    user_id: i64,
    include_secrets: bool,
) -> Result<Value, String> {
    let accounts = list_postgres_accounts(pool, user_id)
        .await
        .map_err(|error| error.to_string())?;
    let categories = list_postgres_categories(pool, user_id)
        .await
        .map_err(|error| error.to_string())?;
    let tags = list_postgres_tags(pool, user_id)
        .await
        .map_err(|error| error.to_string())?;
    let templates = list_postgres_templates(pool, user_id, Some(1))
        .await
        .map_err(|error| error.to_string())?;
    let scheduled = list_postgres_templates(pool, user_id, Some(2))
        .await
        .map_err(|error| error.to_string())?;
    let category_refs = category_ref_map(&categories);
    let account_refs = account_ref_map(&accounts);

    let taxonomy_sections = export_taxonomy_sections(&json!({
        "accounts": records_to_array(&accounts),
        "categories": records_to_array(&categories),
        "tags": tags_to_array(&tags),
        "templates": records_to_array(&templates),
        "scheduled": records_to_array(&scheduled),
    }))
    .map_err(|error| error.to_string())?;

    let mut sections = taxonomy_sections
        .as_object()
        .cloned()
        .ok_or_else(|| "settings taxonomy sections must be an object".to_string())?;

    let category_rules = list_postgres_category_rules(pool, user_id, None, false)
        .await
        .map_err(|error| error.to_string())?;
    sections.insert(
        "categoryRecognitionRules".to_string(),
        Value::Array(
            category_rules
                .iter()
                .map(|rule| export_settings_category_rule(rule, &category_refs))
                .collect(),
        ),
    );
    let account_rules = list_postgres_account_rules(pool, user_id, None, false, None, None)
        .await
        .map_err(|error| error.to_string())?;
    sections.insert(
        "accountRecognitionRules".to_string(),
        Value::Array(
            account_rules
                .iter()
                .map(|rule| export_settings_account_rule(rule, &account_refs))
                .collect(),
        ),
    );
    sections.insert("llmConfigs".to_string(), Value::Array(Vec::new()));
    sections.insert("ocrConfig".to_string(), Value::Array(Vec::new()));

    let counts = SETTINGS_BUNDLE_SECTION_KEYS
        .iter()
        .map(|key| {
            let count = sections
                .get(*key)
                .and_then(Value::as_array)
                .map(|items| items.len() as i64)
                .unwrap_or(0);
            ((*key).to_string(), Value::Number(Number::from(count)))
        })
        .collect::<Map<_, _>>();

    Ok(json!({
        "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
        "exportedAt": Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
        "secretsPolicy": {
            "llmApiKeys": if include_secrets { "included" } else { "redacted" },
            "providerCredentials": if include_secrets { "included" } else { "redacted" },
        },
        "sections": Value::Object(sections),
        "counts": Value::Object(counts),
    }))
}

fn list_settings_templates(
    connection: &Connection,
    user_id: i64,
    template_type: i64,
) -> Result<Vec<Map<String, Value>>, String> {
    let table_name = if template_type == 2 {
        "recurring_bills"
    } else {
        "bill_templates"
    };
    let mut statement = connection
        .prepare(&format!(
            "SELECT * FROM {table_name} WHERE user_id = ?1 ORDER BY COALESCE(display_order, 0), name"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![user_id], |row| {
            let mut item = Map::new();
            let row_ref = row.as_ref();
            for index in 0..row_ref.column_count() {
                let name = row_ref.column_name(index)?.to_string();
                item.insert(name, sqlite_ref_to_json(row.get_ref(index)?));
            }
            Ok(item)
        })
        .map_err(|error| error.to_string())?;

    rows.map(|row| {
        row.map(|raw| serialize_settings_template_row(&raw, template_type))
            .map_err(|error| error.to_string())
    })
    .collect()
}

fn serialize_settings_template_row(
    row: &Map<String, Value>,
    template_type: i64,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(row.get("id"), "")),
    );
    result.insert("timeSequenceId".to_string(), Value::String(String::new()));
    result.insert(
        "templateType".to_string(),
        Value::Number(Number::from(template_type)),
    );
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(row.get("name"), "")),
    );
    result.insert(
        "description".to_string(),
        Value::String(string_or_default(row.get("description"), "")),
    );
    result.insert(
        "type".to_string(),
        Value::Number(Number::from(normalize_template_transaction_type(
            row.get("type"),
        ))),
    );
    result.insert(
        "categoryId".to_string(),
        Value::String(value_string(row.get("category"), "")),
    );
    result.insert("time".to_string(), Value::Number(Number::from(0)));
    result.insert(
        "utcOffset".to_string(),
        Value::Number(Number::from(value_as_i64_or(row.get("utc_offset"), 0))),
    );
    result.insert(
        "sourceAccountId".to_string(),
        Value::String(value_string(row.get("account"), "0")),
    );
    result.insert(
        "destinationAccountId".to_string(),
        Value::String(value_string(row.get("counterparty"), "0")),
    );
    result.insert(
        "sourceAmount".to_string(),
        json_number(row.get("amount").and_then(value_as_f64).unwrap_or_default()),
    );
    result.insert(
        "destinationAmount".to_string(),
        json_number(
            row.get("destination_amount")
                .and_then(value_as_f64)
                .unwrap_or_default(),
        ),
    );
    result.insert(
        "hideAmount".to_string(),
        Value::Bool(row.get("hide_amount").is_some_and(value_truthy)),
    );
    result.insert(
        "tagIds".to_string(),
        Value::Array(template_tag_ids(row.get("tag"))),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(row.get("comment"), "")),
    );
    result.insert("editable".to_string(), Value::Bool(true));
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(value_as_i64_or(row.get("display_order"), 0))),
    );
    result.insert(
        "hidden".to_string(),
        Value::Bool(row.get("hidden").is_some_and(value_truthy)),
    );
    result.insert(
        "scheduledFrequencyType".to_string(),
        if template_type == 2 {
            Value::Number(Number::from(value_as_i64_or(
                row.get("scheduled_frequency_type"),
                0,
            )))
        } else {
            Value::Null
        },
    );
    result.insert(
        "scheduledFrequency".to_string(),
        recurring_string_or_null(row, template_type, "frequency"),
    );
    result.insert(
        "scheduledStartDate".to_string(),
        recurring_string_or_null(row, template_type, "start_date"),
    );
    result.insert(
        "scheduledEndDate".to_string(),
        recurring_string_or_null(row, template_type, "end_date"),
    );
    result.insert("scheduledAt".to_string(), Value::Null);
    if template_type == 2 {
        result.insert(
            "enabled".to_string(),
            Value::Bool(row.get("enabled").is_some_and(value_truthy)),
        );
        result.insert(
            "autoCreate".to_string(),
            Value::Bool(row.get("auto_create").is_some_and(value_truthy)),
        );
        result.insert(
            "nextDate".to_string(),
            row.get("next_date").cloned().unwrap_or(Value::Null),
        );
    }
    result
}

fn list_settings_llm_configs(
    connection: &Connection,
    user_id: i64,
    include_secrets: bool,
) -> Result<Vec<Value>, String> {
    let has_credential_config =
        connection_table_has_column(connection, "llm_configs", "credential_config")?;
    let sql = if has_credential_config {
        "SELECT id, name, provider, model, api_key, base_url, credential_config, advanced_settings, is_active
         FROM llm_configs
         WHERE user_id = ?1
         ORDER BY id"
    } else {
        "SELECT id, name, provider, model, api_key, base_url, '{}' AS credential_config, advanced_settings, is_active
         FROM llm_configs
         WHERE user_id = ?1
         ORDER BY id"
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![user_id], |row| {
            let api_key = row.get::<_, Option<String>>("api_key")?.unwrap_or_default();
            let advanced_settings = row
                .get::<_, Option<String>>("advanced_settings")?
                .unwrap_or_default();
            let credential_config = row
                .get::<_, Option<String>>("credential_config")?
                .unwrap_or_default();
            let credential_config = serde_json::from_str::<Value>(&credential_config)
                .unwrap_or_else(|_| json!({}));
            let mut credential_config =
                bill_analyser_core::normalize_provider_auth_config(Some(&credential_config));
            if credential_config
                .get("access_token")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .is_empty()
                && !api_key.trim().is_empty()
            {
                if let Some(object) = credential_config.as_object_mut() {
                    object.insert("access_token".to_string(), json!(api_key.trim()));
                    object.insert("credential_mode".to_string(), json!("api_key"));
                }
            }
            let exported_credential_config = if include_secrets {
                credential_config.clone()
            } else {
                bill_analyser_core::redact_provider_auth_config(&credential_config)
            };
            Ok(json!({
                "externalRef": format!("llmConfig:{}", row.get::<_, i64>("id")?),
                "name": row.get::<_, Option<String>>("name")?.unwrap_or_default(),
                "provider": row.get::<_, Option<String>>("provider")?.unwrap_or_else(|| "openai".to_string()),
                "model": row.get::<_, Option<String>>("model")?.unwrap_or_default(),
                "apiKey": if include_secrets { api_key.clone() } else { String::new() },
                "hasApiKey": !api_key.is_empty(),
                "baseUrl": row.get::<_, Option<String>>("base_url")?.unwrap_or_default(),
                "credentialConfig": exported_credential_config.clone(),
                "authProfile": exported_credential_config,
                "advancedSettings": normalize_llm_advanced_settings(&advanced_settings),
                "activeInSource": row.get::<_, Option<i64>>("is_active")?.unwrap_or(0) != 0,
            }))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn export_settings_ocr_config(
    connection: &Connection,
    include_secrets: bool,
) -> Result<Value, String> {
    let raw_value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![OCR_CONFIG_SETTING_KEY],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .flatten()
        .unwrap_or_default();
    let loaded = serde_json::from_str::<Value>(&raw_value).unwrap_or_else(|_| json!({}));
    let normalized = normalize_ocr_config(&loaded);
    let credential_config = normalized
        .get("credential_config")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let exported_credential_config = if include_secrets {
        credential_config.clone()
    } else {
        bill_analyser_core::redact_provider_auth_config(&credential_config)
    };
    Ok(json!({
        "externalRef": "ocrConfig:receipt-recognition",
        "provider": normalized["provider"],
        "lang": normalized["lang"],
        "model": normalized["model"],
        "baseUrl": normalized["base_url"],
        "parameters": normalized["parameters"],
        "credentialConfig": exported_credential_config.clone(),
        "authProfile": exported_credential_config,
    }))
}

fn connection_table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> Result<bool, String> {
    let mut statement = connection
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?;
    for row in rows {
        if row.map_err(|error| error.to_string())? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn export_settings_category_rule(
    rule: &Map<String, Value>,
    category_refs: &BTreeMap<i64, String>,
) -> Value {
    let category_id = value_as_i64_or(rule.get("category_id"), 0);
    json!({
        "externalRef": format!("categoryRule:{}", value_string(rule.get("id"), "")),
        "categoryRef": category_refs.get(&category_id).cloned().unwrap_or_default(),
        "mainCategory": string_or_default(rule.get("main_category"), ""),
        "subCategory": string_or_default(rule.get("sub_category"), ""),
        "name": string_or_default(rule.get("name"), ""),
        "priority": value_as_i64_or(rule.get("priority"), 100),
        "ruleExpression": string_or_default(rule.get("rule_expression"), ""),
        "regexEnabled": rule.get("regex_enabled").is_some_and(value_truthy),
        "enabled": rule.get("enabled").map(value_truthy).unwrap_or(true),
    })
}

fn export_settings_account_rule(
    rule: &Map<String, Value>,
    account_refs: &BTreeMap<i64, String>,
) -> Value {
    let account_id = value_as_i64_or(rule.get("account_id"), 0);
    json!({
        "externalRef": format!("accountRule:{}", value_string(rule.get("id"), "")),
        "accountRef": account_refs.get(&account_id).cloned().unwrap_or_default(),
        "accountName": string_or_default(rule.get("account_name"), ""),
        "name": string_or_default(rule.get("name"), ""),
        "priority": value_as_i64_or(rule.get("priority"), 100),
        "ruleExpression": string_or_default(rule.get("rule_expression"), ""),
        "regexEnabled": rule.get("regex_enabled").is_some_and(value_truthy),
        "enabled": rule.get("enabled").map(value_truthy).unwrap_or(true),
        "accountRoleScope": string_or_default(rule.get("account_role_scope"), "any"),
        "transactionTypeScope": string_or_default(rule.get("transaction_type_scope"), "all"),
        "fieldScope": rule
            .get("field_scope")
            .cloned()
            .unwrap_or_else(|| json!(["counterparty", "payment_method", "description"])),
        "source": string_or_default(rule.get("source"), "manual"),
        "sourceKey": rule.get("source_key").cloned().unwrap_or(Value::Null),
    })
}

fn category_ref_map(categories: &[CategoryRecord]) -> BTreeMap<i64, String> {
    categories
        .iter()
        .filter_map(|category| {
            let id = category
                .get("id")
                .and_then(value_as_i64)
                .unwrap_or_default();
            (id > 0).then(|| (id, format!("category:{id}")))
        })
        .collect()
}

fn account_ref_map(accounts: &[AccountRecord]) -> BTreeMap<i64, String> {
    accounts
        .iter()
        .filter_map(|account| {
            let id = account.get("id").and_then(value_as_i64).unwrap_or_default();
            (id > 0).then(|| (id, format!("account:{id}")))
        })
        .collect()
}

fn records_to_array(records: &[Map<String, Value>]) -> Value {
    Value::Array(records.iter().cloned().map(Value::Object).collect())
}

fn tags_to_array(tags: &[TagRecord]) -> Value {
    Value::Array(
        tags.iter()
            .map(|tag| serde_json::to_value(tag).unwrap_or(Value::Null))
            .collect(),
    )
}

fn filter_settings_bundle_section(bundle: &Value, section_key: &str) -> Value {
    let section_items = bundle
        .get("sections")
        .and_then(Value::as_object)
        .and_then(|sections| sections.get(section_key))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let item_count = section_items.len() as i64;
    let mut sections = Map::new();
    sections.insert(section_key.to_string(), Value::Array(section_items));
    let mut counts = Map::new();
    counts.insert(
        section_key.to_string(),
        Value::Number(Number::from(item_count)),
    );

    json!({
        "schemaVersion": bundle
            .get("schemaVersion")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(SETTINGS_BUNDLE_SCHEMA_VERSION))),
        "exportedAt": bundle.get("exportedAt").cloned().unwrap_or(Value::Null),
        "secretsPolicy": bundle
            .get("secretsPolicy")
            .cloned()
            .unwrap_or_else(|| json!({"llmApiKeys": "redacted", "providerCredentials": "redacted"})),
        "sections": Value::Object(sections),
        "counts": Value::Object(counts),
    })
}

fn settings_bundle_download_response(bundle: Value, filename: &str) -> Response {
    let body = serde_json::to_string(&bundle).unwrap_or_else(|_| "{}".to_string());
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "application/json".to_string()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response()
}

fn is_valid_settings_bundle_section(section_key: &str) -> bool {
    SETTINGS_BUNDLE_SECTION_KEYS.contains(&section_key)
}

fn is_sensitive_settings_export_section(section_key: &str) -> bool {
    SENSITIVE_EXPORT_SECTIONS.contains(&section_key)
}

fn settings_bundle_section_not_found(section_key: &str) -> Response {
    json_response(
        StatusCode::NOT_FOUND,
        json!({
            "success": false,
            "error": format!("Unsupported settings bundle section: {section_key}"),
        }),
    )
}

fn settings_bundle_db_error_response() -> Response {
    json_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({"success": false, "error": "Failed to export settings bundle"}),
    )
}

