// 中文导读：PostgreSQL settings bundle 仓储入口。
// 维护重点：仅保留 settings bundle 规范化、导出格式化与 Postgres 导入实现。

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction as PgTransaction};

use crate::{DbError, DbResult, PostgresPool};

type AccountRuleSettingsKey = (i64, String, String, String, String);
type AccountRuleSettingsIndex = BTreeMap<AccountRuleSettingsKey, i64>;

fn section_items<'payload>(sections: &'payload Value, section: &str) -> &'payload [Value] {
    sections
        .get(section)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
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

fn resolve_settings_account_rule_account_id(
    item: &Value,
    account_ref_map: &BTreeMap<String, i64>,
) -> Option<i64> {
    let account_ref = safe_text(get_any(item, &["accountRef", "account_ref"]), "");
    if let Some(account_id) = account_ref_map.get(&account_ref).copied() {
        return Some(account_id);
    }
    let account_id = safe_int(get_any(item, &["accountId", "account_id"]), 0);
    if let Some(account_id) = account_ref_map
        .get(&local_id_ref("account", account_id))
        .copied()
    {
        return Some(account_id);
    }
    let account_name = safe_text(get_any(item, &["accountName", "account_name"]), "");
    account_ref_map
        .get(&format!("accountName:{account_name}"))
        .copied()
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

fn required_array<'payload>(payload: &'payload Value, key: &str) -> DbResult<&'payload Vec<Value>> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| DbError::InvalidOperation(format!("missing settings section: {key}")))
}

include!("types_and_normalization.rs");
include!("export_formatters.rs");
include!("postgres_import_export.rs");
include!("value_helpers.rs");
