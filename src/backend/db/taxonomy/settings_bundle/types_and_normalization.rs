// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

const SETTINGS_BUNDLE_SCHEMA_VERSION: i64 = 1;
const LOCAL_REF_NAMESPACE: &str = "__local_settings_bundle_id__";
const OCR_CONFIG_SETTING_KEY: &str = "receipt_ocr_config";
const SECTION_KEYS: [&str; 9] = [
    "accounts",
    "transactionCategories",
    "transactionTags",
    "transactionTemplates",
    "scheduledTransactions",
    "categoryRecognitionRules",
    "accountRecognitionRules",
    "llmConfigs",
    "ocrConfig",
];

#[derive(Debug, Clone, Copy, Default)]
struct SectionCounts {
    created: i64,
    updated: i64,
    skipped: i64,
}

#[derive(Debug, Default)]
struct ImportSections {
    sections: BTreeMap<String, SectionCounts>,
}

impl ImportSections {
    fn new() -> Self {
        Self {
            sections: SECTION_KEYS
                .into_iter()
                .map(|section| (section.to_string(), SectionCounts::default()))
                .collect(),
        }
    }

    fn get_mut(&mut self, section: &str) -> &mut SectionCounts {
        self.sections.entry(section.to_string()).or_default()
    }

    fn into_value(self) -> Value {
        Value::Object(
            self.sections
                .into_iter()
                .map(|(section, counts)| {
                    (
                        section,
                        json!({
                            "created": counts.created,
                            "updated": counts.updated,
                            "skipped": counts.skipped,
                        }),
                    )
                })
                .collect(),
        )
    }
}

#[derive(Debug, Default)]
struct ExistingAccounts {
    by_id: BTreeMap<i64, (String, i64)>,
    by_key: BTreeMap<(String, i64), i64>,
}

#[derive(Debug, Default)]
struct ExistingCategories {
    by_id: BTreeMap<i64, (String, String)>,
    by_key: BTreeMap<(String, String), i64>,
}

#[derive(Debug, Default)]
struct ExistingTags {
    by_id: BTreeMap<i64, String>,
    by_name: BTreeMap<String, i64>,
}

#[derive(Debug, Clone)]
struct ExistingLlmConfig {
    id: i64,
    api_key: String,
    credential_config: String,
}

pub fn normalize_settings_bundle_sections(bundle: &Value) -> DbResult<Value> {
    let bundle_object = bundle.as_object().ok_or_else(|| {
        DbError::InvalidOperation("Settings bundle must be a JSON object".to_string())
    })?;
    let schema_version = bundle_object
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .filter(|value| *value == SETTINGS_BUNDLE_SCHEMA_VERSION)
        .ok_or_else(|| {
            let raw_value = bundle_object
                .get("schemaVersion")
                .map(json_to_python_like_string)
                .unwrap_or_else(|| "None".to_string());
            DbError::InvalidOperation(format!(
                "Unsupported settings bundle schemaVersion: {raw_value}"
            ))
        })?;
    if schema_version != SETTINGS_BUNDLE_SCHEMA_VERSION {
        return Err(DbError::InvalidOperation(format!(
            "Unsupported settings bundle schemaVersion: {schema_version}"
        )));
    }

    let sections = bundle_object
        .get("sections")
        .and_then(Value::as_object)
        .unwrap_or(bundle_object);
    let mut normalized = Map::new();
    for key in SECTION_KEYS {
        let raw_items = sections.get(key).unwrap_or(&Value::Null);
        let items = match raw_items {
            Value::Null => Vec::new(),
            Value::Array(values) => values
                .iter()
                .filter_map(|item| item.as_object().map(|object| Value::Object(object.clone())))
                .collect(),
            _ => {
                return Err(DbError::InvalidOperation(format!(
                    "Settings bundle section {key} must be a list"
                )))
            }
        };
        normalized.insert(key.to_string(), Value::Array(items));
    }
    Ok(Value::Object(normalized))
}

pub fn export_taxonomy_sections(payload: &Value) -> DbResult<Value> {
    let accounts = required_array(payload, "accounts")?;
    let categories = required_array(payload, "categories")?;
    let tags = required_array(payload, "tags")?;
    let templates = required_array(payload, "templates")?;
    let scheduled = required_array(payload, "scheduled")?;

    let account_refs = id_ref_map(accounts, "account");
    let account_names = id_name_map(accounts, "name");
    let category_refs = id_ref_map(categories, "category");
    let category_names = category_name_map(categories);
    let tag_refs = id_ref_map(tags, "tag");
    let tag_names = id_name_map(tags, "name");

    Ok(json!({
        "accounts": accounts
            .iter()
            .map(|item| export_account(item, &account_refs, &account_names))
            .collect::<Vec<_>>(),
        "transactionCategories": categories
            .iter()
            .map(|item| export_category(item, &category_refs))
            .collect::<Vec<_>>(),
        "transactionTags": tags
            .iter()
            .map(|item| export_tag(item, &tag_refs))
            .collect::<Vec<_>>(),
        "transactionTemplates": templates
            .iter()
            .map(|item| export_template(
                item,
                &category_refs,
                &category_names,
                &account_refs,
                &account_names,
                &tag_refs,
                &tag_names,
            ))
            .collect::<Vec<_>>(),
        "scheduledTransactions": scheduled
            .iter()
            .map(|item| export_template(
                item,
                &category_refs,
                &category_names,
                &account_refs,
                &account_names,
                &tag_refs,
                &tag_names,
            ))
            .collect::<Vec<_>>(),
    }))
}

pub fn normalize_account_import(payload: &Value) -> Value {
    let item = payload.get("item").unwrap_or(&Value::Null);
    let ref_map = payload.get("ref_map").unwrap_or(&Value::Null);
    let parent_ref = safe_text(get_any(item, &["parentRef", "parent_ref"]), "");
    let parent_id = map_int(ref_map, &parent_ref);
    json!({
        "name": safe_text(item.get("name"), ""),
        "type": safe_int(item.get("type"), 1),
        "category": get_any(item, &["category"]).cloned().unwrap_or(Value::Null),
        "currency": safe_text(item.get("currency"), "CNY"),
        "icon": safe_text(item.get("icon"), ""),
        "color": safe_text(item.get("color"), ""),
        "balance": safe_float(item.get("balance"), 0.0),
        "initial_balance": safe_float(get_any(item, &["initialBalance", "initial_balance"]), 0.0),
        "hidden": i64::from(safe_bool(item.get("hidden"))),
        "display_order": safe_int(get_any(item, &["displayOrder", "display_order"]), 0),
        "comment": safe_text(item.get("comment"), ""),
        "parent_id": parent_id,
    })
}

pub fn normalize_category_import(payload: &Value) -> Value {
    let item = payload.get("item").unwrap_or(&Value::Null);
    json!({
        "type": safe_int(item.get("type"), 3),
        "main_category": safe_text(get_any(item, &["mainCategory", "main_category"]), ""),
        "sub_category": safe_text(get_any(item, &["subCategory", "sub_category"]), ""),
        "description": safe_text(item.get("description"), ""),
        "priority": safe_int(item.get("priority"), 0),
        "keywords": safe_text(item.get("keywords"), ""),
        "hidden": i64::from(safe_bool(item.get("hidden"))),
        "icon": safe_text(item.get("icon"), ""),
        "color": safe_text(item.get("color"), ""),
    })
}

pub fn normalize_tag_import(payload: &Value) -> Value {
    let item = payload.get("item").unwrap_or(&Value::Null);
    json!({
        "name": safe_text(item.get("name"), ""),
        "color": safe_text(item.get("color"), ""),
        "icon": safe_text(item.get("icon"), ""),
        "display_order": safe_int(get_any(item, &["displayOrder", "display_order"]), 0),
        "hidden": i64::from(safe_bool(item.get("hidden"))),
    })
}

pub fn resolve_template_payload(payload: &Value) -> Value {
    let item = payload.get("item").unwrap_or(&Value::Null);
    let account_ref_map = payload.get("account_ref_map").unwrap_or(&Value::Null);
    let category_ref_map = payload.get("category_ref_map").unwrap_or(&Value::Null);
    let tag_ref_map = payload.get("tag_ref_map").unwrap_or(&Value::Null);
    let mut warnings = Vec::new();

    let category_id = resolve_category_id(item, category_ref_map);
    let source_account_id = resolve_account_id(
        item,
        account_ref_map,
        "sourceAccountRef",
        "sourceAccountName",
    );
    let destination_account_id = resolve_account_id(
        item,
        account_ref_map,
        "destinationAccountRef",
        "destinationAccountName",
    );
    let tag_ids = resolve_tag_ids(item, tag_ref_map, &mut warnings);
    let scheduled_start = safe_text(get_any(item, &["scheduledStartDate", "startDate"]), "");

    let resolved_payload = json!({
        "description": safe_text(item.get("description"), ""),
        "type": safe_int(item.get("type"), 3),
        "category": category_id.map_or_else(String::new, |value| value.to_string()),
        "amount": safe_float(get_any(item, &["sourceAmount", "amount"]), 0.0),
        "account": source_account_id.to_string(),
        "counterparty": destination_account_id.to_string(),
        "destination_amount": safe_float(get_any(item, &["destinationAmount", "destination_amount"]), 0.0),
        "hide_amount": i64::from(safe_bool(get_any(item, &["hideAmount", "hide_amount"]))),
        "tag": tag_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(","),
        "comment": safe_text(item.get("comment"), ""),
        "display_order": safe_int(get_any(item, &["displayOrder", "display_order"]), 0),
        "hidden": i64::from(safe_bool(item.get("hidden"))),
        "utc_offset": safe_int(get_any(item, &["utcOffset", "utc_offset"]), 0),
        "frequency": safe_text(get_any(item, &["scheduledFrequency", "frequency"]), ""),
        "scheduled_frequency_type": safe_int(get_any(item, &["scheduledFrequencyType", "scheduled_frequency_type"]), 0),
        "start_date": scheduled_start,
        "end_date": safe_text(get_any(item, &["scheduledEndDate", "endDate"]), ""),
        "next_date": safe_text(get_any_with_default(item, &["nextDate"], scheduled_start_value(item)), ""),
        "enabled": i64::from(safe_bool_with_default(get_any(item, &["enabled"]), true)),
        "auto_create": i64::from(safe_bool(get_any(item, &["autoCreate", "auto_create"]))),
    });

    let unresolved_warning = template_unresolved_warning(item, &resolved_payload);
    let unresolved = if let Some(warning) = unresolved_warning {
        warnings.push(warning);
        true
    } else {
        false
    };

    json!({
        "payload": resolved_payload,
        "warnings": warnings,
        "unresolved": unresolved,
    })
}
