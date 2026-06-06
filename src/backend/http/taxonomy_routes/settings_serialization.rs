// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
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
    let account_rules = list_postgres_account_rules(pool, user_id, None, false)
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
    sections.insert(
        "llmConfigs".to_string(),
        Value::Array(
            export_postgres_settings_llm_configs(pool, user_id, include_secrets).await?,
        ),
    );
    sections.insert(
        "ocrConfig".to_string(),
        Value::Array(vec![
            export_postgres_settings_ocr_config(pool, user_id, include_secrets).await?,
        ]),
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

async fn export_postgres_settings_llm_configs(
    pool: &PostgresPool,
    user_id: i64,
    include_secrets: bool,
) -> Result<Vec<Value>, String> {
    list_postgres_llm_configs(pool, user_id)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|config| {
            let api_key = string_or_default(config.get("api_key"), "");
            let credential_config = config
                .get("credential_config")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let exported_credential_config = if include_secrets {
                credential_config.clone()
            } else {
                bill_analyser_core::redact_provider_auth_config(&credential_config)
            };
            Ok(json!({
                "externalRef": format!("llmConfig:{}", value_string(config.get("id"), "")),
                "name": string_or_default(config.get("name"), ""),
                "provider": string_or_default(config.get("provider"), "openai"),
                "model": string_or_default(config.get("model"), ""),
                "apiKey": if include_secrets { api_key.clone() } else { String::new() },
                "hasApiKey": !api_key.is_empty(),
                "baseUrl": string_or_default(config.get("base_url"), ""),
                "credentialConfig": exported_credential_config.clone(),
                "authProfile": exported_credential_config,
                "advancedSettings": config
                    .get("advanced_settings")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
                "activeInSource": config
                    .get("is_active")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }))
        })
        .collect()
}

async fn export_postgres_settings_ocr_config(
    pool: &PostgresPool,
    user_id: i64,
    include_secrets: bool,
) -> Result<Value, String> {
    let config = load_postgres_ocr_config_setting(pool, user_id)
        .await
        .map_err(|error| error.to_string())?;
    let exported_credential_config = if include_secrets {
        config.credential_config.clone()
    } else {
        bill_analyser_core::redact_provider_auth_config(&config.credential_config)
    };
    Ok(json!({
        "externalRef": "ocrConfig:receipt-recognition",
        "provider": config.provider,
        "lang": config.lang,
        "model": config.model,
        "baseUrl": config.base_url,
        "parameters": config.parameters,
        "credentialConfig": exported_credential_config.clone(),
        "authProfile": exported_credential_config,
    }))
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
