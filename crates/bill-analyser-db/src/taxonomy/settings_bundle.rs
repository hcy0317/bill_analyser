use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Value};

use crate::{DbError, DbResult};

const SETTINGS_BUNDLE_SCHEMA_VERSION: i64 = 1;
const LOCAL_REF_NAMESPACE: &str = "__local_settings_bundle_id__";
const OCR_CONFIG_SETTING_KEY: &str = "receipt_ocr_config";
const SECTION_KEYS: [&str; 8] = [
    "accounts",
    "transactionCategories",
    "transactionTags",
    "transactionTemplates",
    "scheduledTransactions",
    "categoryRecognitionRules",
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
        "aliases": dump_json_list(item.get("aliases")),
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

pub fn import_settings_bundle(
    connection: &mut Connection,
    bundle: &Value,
    user_id: i64,
    dry_run: bool,
) -> DbResult<Value> {
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
                 comment = ?, aliases = ?, updated_at = ?
             WHERE id = ? AND user_id = ?",
            params_from_iter(update_values),
        )?;
        section.updated += 1;
        return Ok(Some(account_id));
    }

    transaction.execute(
        "INSERT INTO accounts (
            user_id, name, type, category, currency, icon, color, balance,
            initial_balance, hidden, display_order, comment, aliases,
            parent_id, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        SqlValue::Text(safe_text(normalized.get("aliases"), "[]")),
    ])
}

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
    let mut existing = load_existing_llm_configs(transaction, user_id)?;
    let has_timestamps = table_has_column(transaction, "llm_configs", "updated_at")?;
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
        let advanced_settings =
            dump_json_object(get_any(item, &["advancedSettings", "advanced_settings"]));
        let now = utc_now_iso();

        if let Some(row) = existing.get(&name).cloned() {
            let api_key = if is_masked_secret(&incoming_secret) {
                row.api_key
            } else {
                incoming_secret
            };
            if has_timestamps {
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
        if has_timestamps {
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
    let mut statement =
        transaction.prepare("SELECT id, name, api_key FROM llm_configs WHERE user_id = ?1")?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok((
            row.get::<_, Option<String>>("name")?.unwrap_or_default(),
            ExistingLlmConfig {
                id: row.get::<_, i64>("id")?,
                api_key: row.get::<_, Option<String>>("api_key")?.unwrap_or_default(),
            },
        ))
    })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(DbError::from)
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

fn export_account(
    account: &Value,
    account_refs: &BTreeMap<i64, String>,
    account_names: &BTreeMap<i64, String>,
) -> Value {
    let account_id = safe_int(account.get("id"), 0);
    let parent_id = safe_int(account.get("parent_id"), 0);
    json!({
        "externalRef": account_refs.get(&account_id).cloned().unwrap_or_default(),
        "name": safe_text(account.get("name"), ""),
        "type": safe_int(account.get("type"), 1),
        "category": account.get("category").cloned().unwrap_or(Value::Null),
        "currency": safe_text(account.get("currency"), "CNY"),
        "icon": safe_text(account.get("icon"), ""),
        "color": safe_text(account.get("color"), ""),
        "balance": safe_float(account.get("balance"), 0.0),
        "initialBalance": safe_float(account.get("initial_balance"), 0.0),
        "hidden": safe_bool(account.get("hidden")),
        "displayOrder": safe_int(account.get("display_order"), 0),
        "comment": safe_text(account.get("comment"), ""),
        "aliases": load_json_list(account.get("aliases")),
        "parentRef": account_refs.get(&parent_id).cloned().unwrap_or_default(),
        "parentName": account_names.get(&parent_id).cloned().unwrap_or_default(),
    })
}

fn export_category(category: &Value, category_refs: &BTreeMap<i64, String>) -> Value {
    let category_id = safe_int(category.get("id"), 0);
    json!({
        "externalRef": category_refs.get(&category_id).cloned().unwrap_or_default(),
        "type": safe_int(category.get("type"), 1),
        "mainCategory": safe_text(category.get("main_category"), ""),
        "subCategory": safe_text(category.get("sub_category"), ""),
        "priority": safe_int(category.get("priority"), 0),
        "keywords": safe_text(category.get("keywords"), ""),
        "description": safe_text(category.get("description"), ""),
        "icon": safe_text(category.get("icon"), ""),
        "color": safe_text(category.get("color"), ""),
        "hidden": safe_bool(category.get("hidden")),
    })
}

fn export_tag(tag: &Value, tag_refs: &BTreeMap<i64, String>) -> Value {
    let tag_id = safe_int(tag.get("id"), 0);
    json!({
        "externalRef": tag_refs.get(&tag_id).cloned().unwrap_or_default(),
        "name": safe_text(tag.get("name"), ""),
        "color": safe_text(tag.get("color"), ""),
        "icon": safe_text(tag.get("icon"), ""),
        "displayOrder": safe_int(tag.get("display_order"), 0),
        "hidden": safe_bool(tag.get("hidden")),
    })
}

fn export_template(
    template: &Value,
    category_refs: &BTreeMap<i64, String>,
    category_names: &BTreeMap<i64, String>,
    account_refs: &BTreeMap<i64, String>,
    account_names: &BTreeMap<i64, String>,
    tag_refs: &BTreeMap<i64, String>,
    tag_names: &BTreeMap<i64, String>,
) -> Value {
    let mut exported = template.as_object().cloned().unwrap_or_default();
    exported.remove("id");
    exported.remove("tagIds");

    let source_account_id = safe_int(template.get("sourceAccountId"), 0);
    let destination_account_id = safe_int(template.get("destinationAccountId"), 0);
    let category_id = safe_int(template.get("categoryId"), 0);
    let tag_ids = template
        .get("tagIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| safe_int(Some(item), 0))
        .collect::<Vec<_>>();

    exported.insert(
        "externalRef".to_string(),
        Value::String(format!(
            "template:{}:{}",
            safe_text(template.get("templateType"), ""),
            safe_text(template.get("id"), "")
        )),
    );
    exported.insert(
        "categoryRef".to_string(),
        Value::String(category_refs.get(&category_id).cloned().unwrap_or_default()),
    );
    exported.insert(
        "categoryName".to_string(),
        Value::String(
            category_names
                .get(&category_id)
                .cloned()
                .unwrap_or_default(),
        ),
    );
    exported.insert(
        "sourceAccountRef".to_string(),
        Value::String(
            account_refs
                .get(&source_account_id)
                .cloned()
                .unwrap_or_default(),
        ),
    );
    exported.insert(
        "sourceAccountName".to_string(),
        Value::String(
            account_names
                .get(&source_account_id)
                .cloned()
                .unwrap_or_default(),
        ),
    );
    exported.insert(
        "destinationAccountRef".to_string(),
        Value::String(
            account_refs
                .get(&destination_account_id)
                .cloned()
                .unwrap_or_default(),
        ),
    );
    exported.insert(
        "destinationAccountName".to_string(),
        Value::String(
            account_names
                .get(&destination_account_id)
                .cloned()
                .unwrap_or_default(),
        ),
    );
    exported.insert(
        "tagRefs".to_string(),
        Value::Array(
            tag_ids
                .iter()
                .filter_map(|tag_id| tag_refs.get(tag_id).cloned().map(Value::String))
                .collect(),
        ),
    );
    exported.insert(
        "tagNames".to_string(),
        Value::Array(
            tag_ids
                .iter()
                .filter_map(|tag_id| tag_names.get(tag_id).cloned().map(Value::String))
                .collect(),
        ),
    );
    Value::Object(exported)
}

fn resolve_category_id(item: &Value, category_ref_map: &Value) -> Option<i64> {
    let category_ref = safe_text(get_any(item, &["categoryRef", "category_ref"]), "");
    if let Some(value) = map_get_int(category_ref_map, &category_ref) {
        return Some(value);
    }
    let category_id = safe_int(get_any(item, &["categoryId", "category_id"]), 0);
    let local_category_ref = local_id_ref("category", category_id);
    if let Some(value) = map_get_int(category_ref_map, &local_category_ref) {
        return Some(value);
    }
    let category_name = get_any(item, &["categoryName", "category_name"]);
    let mut main = safe_text(get_any(item, &["mainCategory", "main_category"]), "");
    let mut sub = safe_text(get_any(item, &["subCategory", "sub_category"]), "");
    if main.is_empty() && sub.is_empty() {
        (main, sub) = split_category_name(category_name);
    }
    let category_full_name = [main, sub]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    map_get_int(
        category_ref_map,
        &format!("categoryName:{category_full_name}"),
    )
}

fn resolve_account_id(item: &Value, account_ref_map: &Value, ref_key: &str, name_key: &str) -> i64 {
    let snake_ref_key = ref_key.replace("Ref", "_ref");
    let account_ref = safe_text(get_any(item, &[ref_key, snake_ref_key.as_str()]), "");
    if let Some(value) = map_get_int(account_ref_map, &account_ref) {
        return value;
    }
    let id_key = if ref_key.starts_with("source") {
        "sourceAccountId"
    } else {
        "destinationAccountId"
    };
    let snake_id_key = id_key.replace("Id", "_id");
    let raw_id = safe_int(get_any(item, &[id_key, snake_id_key.as_str()]), 0);
    let local_account_ref = local_id_ref("account", raw_id);
    if let Some(value) = map_get_int(account_ref_map, &local_account_ref) {
        return value;
    }
    let snake_name_key = name_key.replace("Name", "_name");
    let name = safe_text(get_any(item, &[name_key, snake_name_key.as_str()]), "");
    map_get_int(account_ref_map, &format!("accountName:{name}")).unwrap_or(0)
}

fn resolve_tag_ids(item: &Value, tag_ref_map: &Value, warnings: &mut Vec<String>) -> Vec<i64> {
    let mut tag_ids = BTreeSet::new();
    for tag_ref in item
        .get("tagRefs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let normalized_ref = safe_text(Some(tag_ref), "");
        if let Some(tag_id) = map_get_int(tag_ref_map, &normalized_ref) {
            tag_ids.insert(tag_id);
        } else if !normalized_ref.is_empty() {
            warnings.push(format!("Template tag ref not found: {normalized_ref}"));
        }
    }
    for tag_name in item
        .get("tagNames")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name_key = format!("tagName:{}", safe_text(Some(tag_name), ""));
        if let Some(tag_id) = map_get_int(tag_ref_map, &name_key) {
            tag_ids.insert(tag_id);
        }
    }
    for raw_id in item
        .get("tagIds")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let local_ref = local_id_ref("tag", safe_int(Some(raw_id), 0));
        if let Some(tag_id) = map_get_int(tag_ref_map, &local_ref) {
            tag_ids.insert(tag_id);
        }
    }
    tag_ids.into_iter().collect()
}

fn template_unresolved_warning(item: &Value, payload: &Value) -> Option<String> {
    let name = {
        let text = safe_text(item.get("name"), "");
        if text.is_empty() {
            "<unnamed>".to_string()
        } else {
            text
        }
    };
    if has_template_ref_value(
        item,
        &[
            "categoryRef",
            "category_ref",
            "categoryName",
            "category_name",
            "mainCategory",
            "main_category",
            "subCategory",
            "sub_category",
            "categoryId",
            "category_id",
        ],
    ) && safe_text(payload.get("category"), "").is_empty()
    {
        return Some(format!(
            "Skipped template {name} with unresolved category reference"
        ));
    }
    if has_template_ref_value(
        item,
        &[
            "sourceAccountRef",
            "source_account_ref",
            "sourceAccountName",
            "source_account_name",
            "sourceAccountId",
            "source_account_id",
        ],
    ) && safe_int(payload.get("account"), 0) == 0
    {
        return Some(format!(
            "Skipped template {name} with unresolved source account reference"
        ));
    }
    if has_template_ref_value(
        item,
        &[
            "destinationAccountRef",
            "destination_account_ref",
            "destinationAccountName",
            "destination_account_name",
            "destinationAccountId",
            "destination_account_id",
        ],
    ) && safe_int(payload.get("counterparty"), 0) == 0
    {
        return Some(format!(
            "Skipped template {name} with unresolved destination account reference"
        ));
    }
    None
}

fn has_template_ref_value(item: &Value, keys: &[&str]) -> bool {
    keys.iter()
        .any(|key| !safe_text(get_any(item, &[*key]), "").is_empty())
}

fn id_ref_map(items: &[Value], prefix: &str) -> BTreeMap<i64, String> {
    items
        .iter()
        .filter_map(|item| {
            let id = safe_int(item.get("id"), 0);
            (id > 0).then(|| (id, format!("{prefix}:{id}")))
        })
        .collect()
}

fn id_name_map(items: &[Value], name_key: &str) -> BTreeMap<i64, String> {
    items
        .iter()
        .filter_map(|item| {
            let id = safe_int(item.get("id"), 0);
            (id > 0).then(|| (id, safe_text(item.get(name_key), "")))
        })
        .collect()
}

fn category_name_map(items: &[Value]) -> BTreeMap<i64, String> {
    items
        .iter()
        .filter_map(|item| {
            let id = safe_int(item.get("id"), 0);
            let name = settings_category_name(item);
            (id > 0).then_some((id, name))
        })
        .collect()
}

fn settings_category_name(category: &Value) -> String {
    [
        safe_text(category.get("main_category"), ""),
        safe_text(category.get("sub_category"), ""),
    ]
    .into_iter()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join("/")
}

fn get_any<'payload>(data: &'payload Value, keys: &[&str]) -> Option<&'payload Value> {
    let object = data.as_object()?;
    keys.iter().find_map(|key| object.get(*key))
}

fn get_any_with_default<'payload>(
    data: &'payload Value,
    keys: &[&str],
    default: Option<&'payload Value>,
) -> Option<&'payload Value> {
    get_any(data, keys).or(default)
}

fn scheduled_start_value(item: &Value) -> Option<&Value> {
    get_any(item, &["scheduledStartDate", "startDate"])
}

fn safe_text(value: Option<&Value>, default: &str) -> String {
    match value {
        None | Some(Value::Null) => default.to_string(),
        Some(Value::String(text)) => text.trim().to_string(),
        Some(value) => json_to_python_like_string(value).trim().to_string(),
    }
}

fn safe_int(value: Option<&Value>, default: i64) -> i64 {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64))
            .unwrap_or(default),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or(default),
        Some(Value::Bool(flag)) => i64::from(*flag),
        _ => default,
    }
}

fn safe_float(value: Option<&Value>, default: f64) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(default),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or(default),
        Some(Value::Bool(flag)) => {
            if *flag {
                1.0
            } else {
                0.0
            }
        }
        _ => default,
    }
}

fn safe_bool(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(text)) => matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_i64().unwrap_or(0) != 0,
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
        _ => false,
    }
}

fn safe_bool_with_default(value: Option<&Value>, default: bool) -> bool {
    match value {
        None | Some(Value::Null) => default,
        Some(_) => safe_bool(value),
    }
}

fn load_json_list(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(values)) => values
            .iter()
            .map(|item| safe_text(Some(item), ""))
            .filter(|item| !item.is_empty())
            .map(Value::String)
            .collect(),
        Some(value) if !safe_text(Some(value), "").is_empty() => {
            serde_json::from_str::<Value>(&safe_text(Some(value), ""))
                .ok()
                .and_then(|loaded| match loaded {
                    Value::Array(values) => Some(
                        values
                            .iter()
                            .map(|item| safe_text(Some(item), ""))
                            .filter(|item| !item.is_empty())
                            .map(Value::String)
                            .collect(),
                    ),
                    _ => None,
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn dump_json_list(value: Option<&Value>) -> String {
    serde_json::to_string(&load_json_list(value)).unwrap_or_else(|_| "[]".to_string())
}

fn map_int(map_value: &Value, key: &str) -> i64 {
    map_get_int(map_value, key).unwrap_or(0)
}

fn map_get_int(map_value: &Value, key: &str) -> Option<i64> {
    if key.is_empty() {
        return None;
    }
    map_value
        .as_object()?
        .get(key)
        .map(|value| safe_int(Some(value), 0))
}

fn local_id_ref(prefix: &str, item_id: i64) -> String {
    format!("{LOCAL_REF_NAMESPACE}:{prefix}:{item_id}")
}

fn split_category_name(value: Option<&Value>) -> (String, String) {
    let text = safe_text(value, "");
    if text.is_empty() {
        return (String::new(), String::new());
    }
    if let Some((main, sub)) = text.split_once('/') {
        return (main.trim().to_string(), sub.trim().to_string());
    }
    (text, String::new())
}

fn json_to_python_like_string(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(flag) => {
            if *flag {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn external_ref(item: &Value, prefix: &str) -> String {
    let explicit = safe_text(get_any(item, &["externalRef", "external_ref"]), "");
    if !explicit.is_empty() {
        if explicit.starts_with(&format!("{LOCAL_REF_NAMESPACE}:")) {
            return String::new();
        }
        return explicit;
    }
    let item_id = safe_text(get_any(item, &["id", "sourceId", "source_id"]), "");
    if item_id.is_empty() {
        String::new()
    } else {
        format!("{prefix}:{item_id}")
    }
}

fn category_name(main: &str, sub: &str) -> String {
    [main, sub]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn sql_value_or_null(value: Option<&Value>) -> DbResult<SqlValue> {
    Ok(match value {
        Some(Value::Null) | None => SqlValue::Null,
        Some(Value::String(text)) => SqlValue::Text(text.clone()),
        Some(Value::Bool(flag)) => SqlValue::Integer(i64::from(*flag)),
        Some(Value::Number(number)) => {
            if let Some(value) = number.as_i64() {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_u64().and_then(|value| i64::try_from(value).ok())
            {
                SqlValue::Integer(value)
            } else if let Some(value) = number.as_f64() {
                SqlValue::Real(value)
            } else {
                SqlValue::Null
            }
        }
        Some(value @ (Value::Array(_) | Value::Object(_))) => SqlValue::Text(
            serde_json::to_string(value)
                .map_err(|error| DbError::InvalidOperation(error.to_string()))?,
        ),
    })
}

fn dump_json_object(value: Option<&Value>) -> String {
    let loaded = match value {
        Some(Value::String(text)) => {
            serde_json::from_str::<Value>(text).unwrap_or_else(|_| json!({}))
        }
        Some(Value::Object(object)) => Value::Object(object.clone()),
        _ => json!({}),
    };
    let object = loaded
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or_else(|| json!({}));
    serde_json::to_string(&object).unwrap_or_else(|_| "{}".to_string())
}

fn is_masked_secret(value: &str) -> bool {
    matches!(value.trim(), "" | "********" | "redacted" | "<redacted>")
}

fn normalize_ocr_config(raw_value: &Value) -> Value {
    let provider = raw_value
        .get("provider")
        .map(|value| safe_text(Some(value), "disabled").to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "disabled" | "tesseract" | "cloud_stub"))
        .unwrap_or_else(|| "disabled".to_string());
    let lang = raw_value
        .get("lang")
        .map(|value| safe_text(Some(value), "chi_sim+eng"))
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '.' | '-'))
        })
        .unwrap_or_else(|| "chi_sim+eng".to_string());
    json!({
        "provider": provider,
        "lang": lang,
    })
}

fn table_has_column(
    transaction: &Transaction<'_>,
    table_name: &str,
    column_name: &str,
) -> DbResult<bool> {
    let mut statement = transaction.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn utc_now_iso() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_bundle_normalizes_sections_and_rejects_bad_schema() {
        let normalized = normalize_settings_bundle_sections(&json!({
            "schemaVersion": 1,
            "sections": {
                "accounts": [{"name": "Cash"}, "ignored", null],
                "transactionTags": null
            }
        }))
        .unwrap();

        assert_eq!(normalized["accounts"].as_array().unwrap().len(), 1);
        assert_eq!(normalized["transactionTags"].as_array().unwrap().len(), 0);
        assert_eq!(normalized["ocrConfig"].as_array().unwrap().len(), 0);

        let error = normalize_settings_bundle_sections(&json!({
            "schemaVersion": true,
            "sections": {}
        }))
        .unwrap_err()
        .to_string();
        assert!(error.contains("Unsupported settings bundle schemaVersion"));
    }

    #[test]
    fn settings_bundle_exports_taxonomy_sections_with_refs() {
        let exported = export_taxonomy_sections(&json!({
            "accounts": [{
                "id": 10,
                "name": "Wallet",
                "type": 1,
                "currency": "CNY",
                "balance": 12.5,
                "initial_balance": 2.5,
                "aliases": "[\"cash\"]"
            }],
            "categories": [{
                "id": 20,
                "type": 3,
                "main_category": "Food",
                "sub_category": "Coffee"
            }],
            "tags": [{"id": 30, "name": "Work"}],
            "templates": [{
                "id": "40",
                "templateType": 1,
                "name": "Latte",
                "categoryId": "20",
                "sourceAccountId": "10",
                "tagIds": ["30"]
            }],
            "scheduled": []
        }))
        .unwrap();

        assert_eq!(exported["accounts"][0]["externalRef"], "account:10");
        assert_eq!(exported["accounts"][0]["aliases"], json!(["cash"]));
        assert_eq!(
            exported["transactionCategories"][0]["externalRef"],
            "category:20"
        );
        assert_eq!(
            exported["transactionTemplates"][0]["categoryName"],
            "Food/Coffee"
        );
        assert_eq!(
            exported["transactionTemplates"][0]["tagRefs"],
            json!(["tag:30"])
        );
    }

    #[test]
    fn settings_bundle_resolves_template_refs_without_external_id_collision() {
        let resolved = resolve_template_payload(&json!({
            "item": {
                "name": "T",
                "categoryRef": "category:123",
                "sourceAccountRef": "account:1",
                "tagRefs": ["tag:1"],
                "sourceAmount": 9.5
            },
            "category_ref_map": {
                "__local_settings_bundle_id__:category:20": 200
            },
            "account_ref_map": {
                "__local_settings_bundle_id__:account:10": 100
            },
            "tag_ref_map": {
                "__local_settings_bundle_id__:tag:30": 300
            }
        }));

        assert!(resolved["unresolved"].as_bool().unwrap());
        assert_eq!(resolved["warnings"][0], "Template tag ref not found: tag:1");

        let legacy_id_resolved = resolve_template_payload(&json!({
            "item": {
                "name": "T2",
                "categoryId": 20,
                "sourceAccountId": 10,
                "tagIds": [30],
                "sourceAmount": 9.5
            },
            "category_ref_map": {
                "__local_settings_bundle_id__:category:20": 200
            },
            "account_ref_map": {
                "__local_settings_bundle_id__:account:10": 100
            },
            "tag_ref_map": {
                "__local_settings_bundle_id__:tag:30": 300
            }
        }));
        assert!(!legacy_id_resolved["unresolved"].as_bool().unwrap());
        assert_eq!(legacy_id_resolved["payload"]["category"], "200");
        assert_eq!(legacy_id_resolved["payload"]["account"], "100");
        assert_eq!(legacy_id_resolved["payload"]["tag"], "300");
    }

    #[test]
    fn settings_bundle_normalizes_import_values() {
        let account = normalize_account_import(&json!({
            "item": {
                "name": "Card",
                "parentRef": "account:root",
                "aliases": ["visa", ""],
                "hidden": "yes"
            },
            "ref_map": {"account:root": 7}
        }));
        assert_eq!(account["parent_id"], 7);
        assert_eq!(account["hidden"], 1);
        assert_eq!(account["aliases"], "[\"visa\"]");

        let category = normalize_category_import(&json!({
            "item": {"mainCategory": "Food", "hidden": true}
        }));
        assert_eq!(category["main_category"], "Food");
        assert_eq!(category["type"], 3);

        let tag = normalize_tag_import(&json!({
            "item": {"name": "Work", "displayOrder": "4"}
        }));
        assert_eq!(tag["display_order"], 4);
    }
}
