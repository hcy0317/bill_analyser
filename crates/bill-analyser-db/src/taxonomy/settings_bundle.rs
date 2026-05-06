use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};

use crate::{DbError, DbResult};

const SETTINGS_BUNDLE_SCHEMA_VERSION: i64 = 1;
const LOCAL_REF_NAMESPACE: &str = "__local_settings_bundle_id__";
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
