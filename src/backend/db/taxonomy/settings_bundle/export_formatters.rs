// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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
