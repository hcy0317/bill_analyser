// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[tracing::instrument(level = "debug", skip_all)]
async fn update_postgres_sub_accounts(
    pool: &bill_analyser_db::PostgresPool,
    account_id: i64,
    user_id: i64,
    sub_accounts: &[Value],
) -> RouteResult<()> {
    let existing_sub_accounts = get_postgres_sub_accounts(pool, account_id, user_id)
        .await
        .map_err(|_| Box::new(db_error_response()))?;
    let existing_sub_ids = existing_sub_accounts
        .iter()
        .filter_map(|account| account.get("id").and_then(value_as_i64))
        .collect::<Vec<_>>();
    let mut updated_sub_ids = Vec::new();

    for sub_account in sub_accounts {
        let sub_account_id = sub_account.get("id").and_then(value_as_i64);
        let mut sub_payload =
            frontend_account_to_backend(sub_account).map_err(|message| Box::new(bad_request(message)))?;
        sub_payload.insert(
            "parent_id".to_string(),
            Value::Number(Number::from(account_id)),
        );
        if let Some(sub_account_id) =
            sub_account_id.filter(|candidate| existing_sub_ids.contains(candidate))
        {
            update_postgres_account(pool, sub_account_id, &Value::Object(sub_payload), user_id)
                .await
                .map_err(|_| Box::new(db_error_response()))?;
            updated_sub_ids.push(sub_account_id);
            continue;
        }

        create_postgres_account(pool, &Value::Object(sub_payload), user_id)
            .await
            .map_err(|_| Box::new(db_error_response()))?;
    }

    for old_sub_id in existing_sub_ids {
        if !updated_sub_ids.contains(&old_sub_id) {
            delete_postgres_account(pool, old_sub_id, user_id)
                .await
                .map_err(|_| Box::new(db_error_response()))?;
        }
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_account_with_sub_accounts(
    pool: &bill_analyser_db::PostgresPool,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<AccountRecord>> {
    let Some(mut account) = get_postgres_account_by_id(pool, account_id, user_id).await? else {
        return Ok(None);
    };
    let sub_accounts = get_postgres_sub_accounts(pool, account_id, user_id).await?;
    if !sub_accounts.is_empty() {
        account.insert(
            "subAccounts".to_string(),
            Value::Array(sub_accounts.into_iter().map(Value::Object).collect()),
        );
    }
    Ok(Some(account))
}

fn format_account_list_response(accounts: Vec<AccountRecord>) -> Value {
    let formatted = accounts
        .into_iter()
        .map(backend_account_to_frontend)
        .collect::<Vec<_>>();
    let mut children_by_parent: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut top_level = Vec::new();

    for account in formatted {
        let parent_id = account
            .get("parentId")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        if parent_id.is_empty() || parent_id == "0" {
            top_level.push(account);
        } else {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(Value::Object(account));
        }
    }

    for account in &mut top_level {
        if let Some(account_id) = account
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            if let Some(children) = children_by_parent.remove(&account_id) {
                if !children.is_empty() {
                    account.insert("subAccounts".to_string(), Value::Array(children));
                }
            }
        }
    }

    Value::Array(top_level.into_iter().map(Value::Object).collect())
}

fn format_sync_account_balances_response(result: SyncAllAccountBalancesResult) -> Value {
    json!({
        "total_accounts": result.total_accounts,
        "synced_accounts": result.synced_accounts,
        "discrepancies": result
            .discrepancies
            .into_iter()
            .map(format_account_balance_discrepancy)
            .collect::<Vec<_>>(),
        "errors": result.errors,
    })
}

fn format_account_balance_discrepancy(discrepancy: AccountBalanceDiscrepancy) -> Value {
    json!({
        "account_id": discrepancy.account_id,
        "name": discrepancy.name,
        "oldBalanceCents": discrepancy.old_balance_cents,
        "newBalanceCents": discrepancy.new_balance_cents,
        "diffCents": discrepancy.diff_cents,
    })
}

fn frontend_account_to_backend(payload: &Value) -> Result<Map<String, Value>, String> {
    let Some(object) = payload.as_object() else {
        return Err("Account payload must be an object".to_string());
    };
    let balance_cents = account_balance_cents_value(object)?;
    let hidden = object
        .get("hidden")
        .map(value_truthy)
        .unwrap_or_else(|| !object.get("visible").map(value_truthy).unwrap_or(true));

    let mut result = Map::new();
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(object.get("name"), "")),
    );
    result.insert(
        "parent_id".to_string(),
        Value::Number(Number::from(
            object
                .get("parentId")
                .or_else(|| object.get("parent_id"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert(
        "category".to_string(),
        object.get("category").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "type".to_string(),
        object
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(object.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(object.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(object.get("currency"), "CNY")),
    );
    result.insert(
        "balance_cents".to_string(),
        Value::Number(Number::from(balance_cents)),
    );
    result.insert(
        "initial_balance_cents".to_string(),
        Value::Number(Number::from(balance_cents)),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(object.get("comment"), "")),
    );
    result.insert(
        "display_order".to_string(),
        Value::Number(Number::from(
            object
                .get("displayOrder")
                .or_else(|| object.get("display_order"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    if let Some(Value::Array(sub_accounts)) = object.get("subAccounts") {
        let converted_sub_accounts = sub_accounts
            .iter()
            .map(frontend_account_to_backend)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(Value::Object)
            .collect();
        result.insert(
            "subAccounts".to_string(),
            Value::Array(converted_sub_accounts),
        );
    }
    if let Some(statement_date) = object
        .get("creditCardStatementDate")
        .or_else(|| object.get("credit_card_statement_date"))
    {
        result.insert(
            "credit_card_statement_date".to_string(),
            statement_date.clone(),
        );
    }
    Ok(result)
}

fn account_balance_cents_value(object: &Map<String, Value>) -> Result<i64, String> {
    for field in [
        "balanceCents",
        "balance_cents",
        "initialBalanceCents",
        "initial_balance_cents",
    ] {
        if let Some(value) = object.get(field) {
            return strict_account_cents_value(value, field);
        }
    }
    Ok(0)
}

fn strict_account_cents_value(value: &Value, field: &str) -> Result<i64, String> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .ok_or_else(|| format!("{field} must be integer cents")),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                Err(format!("{field} must be integer cents"))
            } else {
                trimmed
                    .parse::<i64>()
                    .map_err(|_| format!("{field} must be integer cents"))
            }
        }
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => {
            Err(format!("{field} must be integer cents"))
        }
    }
}

fn backend_account_to_frontend(mut account: AccountRecord) -> Map<String, Value> {
    let hidden = account.get("hidden").map(value_truthy).unwrap_or(false);
    let sub_accounts = account.remove("subAccounts");
    let account_type = normalize_frontend_account_type(account.get("type"));
    let account_category = normalize_frontend_account_category(
        account.get("category"),
        account.get("type"),
        account.get("name"),
        account.get("icon"),
    );
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(account.get("id"), "")),
    );
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(account.get("name"), "")),
    );
    result.insert(
        "parentId".to_string(),
        Value::String(value_string(account.get("parent_id"), "0")),
    );
    result.insert(
        "category".to_string(),
        Value::Number(Number::from(account_category)),
    );
    result.insert("type".to_string(), Value::Number(Number::from(account_type)));
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(account.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(account.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(account.get("currency"), "CNY")),
    );
    result.insert(
        "balanceCents".to_string(),
        Value::Number(Number::from(account_balance_cents(&account))),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(account.get("comment"), "")),
    );
    result.insert(
        "creditCardStatementDate".to_string(),
        account
            .get("credit_card_statement_date")
            .cloned()
            .unwrap_or(Value::Null),
    );
    result.insert(
        "displayOrder".to_string(),
        account
            .get("display_order")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));

    if let Some(Value::Array(sub_accounts)) = sub_accounts.filter(|value| {
        value
            .as_array()
            .is_some_and(|sub_accounts| !sub_accounts.is_empty())
    }) {
        result.insert(
            "subAccounts".to_string(),
            Value::Array(
                sub_accounts
                    .into_iter()
                    .filter_map(|sub_account| {
                        sub_account
                            .as_object()
                            .cloned()
                            .map(backend_account_to_frontend)
                            .map(Value::Object)
                    })
                    .collect(),
            ),
        );
    }

    result
}

fn account_balance_cents(account: &AccountRecord) -> i64 {
    account
        .get("balanceCents")
        .or_else(|| account.get("balance_cents"))
        .and_then(value_as_i64)
        .unwrap_or_default()
}

fn normalize_frontend_account_type(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(number)) if number.as_i64() == Some(2) => 2,
        Some(Value::String(value)) => {
            let normalized = value.trim().to_ascii_lowercase();
            if normalized == "2"
                || normalized == "multi"
                || normalized == "multi_sub_accounts"
                || normalized == "multiple_sub_accounts"
                || normalized == "multiple sub-accounts"
            {
                2
            } else {
                1
            }
        }
        _ => 1,
    }
}

fn normalize_frontend_account_category(
    category: Option<&Value>,
    account_type: Option<&Value>,
    name: Option<&Value>,
    icon: Option<&Value>,
) -> i64 {
    if let Some(category) = category.and_then(value_as_i64) {
        if (1..=9).contains(&category) {
            return category;
        }
    }

    for hint in [
        normalized_account_hint(category),
        normalized_account_hint(name),
        normalized_account_hint(account_type),
    ] {
        if let Some(category) = category_from_account_hint(&hint) {
            return category;
        }
    }

    if let Some(category) = category_from_account_icon(&normalized_account_hint(icon)) {
        return category;
    }

    1
}

fn normalized_account_hint(value: Option<&Value>) -> String {
    value_string(value, "").trim().to_ascii_lowercase()
}

fn hint_contains_any(hint: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| hint.contains(needle))
}

fn category_from_account_hint(hint: &str) -> Option<i64> {
    if hint.is_empty() {
        return None;
    }

    if hint_contains_any(
        hint,
        &[
            "信用卡",
            "credit_card",
            "credit-card",
            "credit card",
        ],
    ) {
        return Some(3);
    }
    if hint_contains_any(
        hint,
        &[
            "证券",
            "投资",
            "理财",
            "基金",
            "股票",
            "黄金",
            "期货",
            "债券",
            "资产",
            "investment",
            "securities",
            "brokerage",
        ],
    ) {
        return Some(7);
    }
    if hint_contains_any(
        hint,
        &["应收", "垫付", "垫款", "借账", "待收", "receivable", "receivables"],
    ) {
        return Some(6);
    }
    if hint_contains_any(
        hint,
        &["贷款", "负债", "房贷", "车贷", "借入", "欠款", "应付", "debt", "loan"],
    ) {
        return Some(5);
    }
    if hint_contains_any(hint, &["定期", "存单", "certificate", "deposit"]) {
        return Some(9);
    }
    if hint_contains_any(hint, &["储蓄", "savings", "saving"]) {
        return Some(8);
    }
    if hint_contains_any(
        hint,
        &[
            "支付宝",
            "微信",
            "零钱",
            "第三方",
            "三方",
            "paypal",
            "virtual_account",
            "virtual",
        ],
    ) {
        return Some(4);
    }
    if hint_contains_any(hint, &["现金", "cash"]) {
        return Some(1);
    }
    if hint_contains_any(
        hint,
        &["银行卡", "借记", "银行", "checking", "bank_card", "bank", "debit"],
    ) {
        return Some(2);
    }

    None
}

fn category_from_account_icon(icon: &str) -> Option<i64> {
    if icon.is_empty() {
        return None;
    }

    match icon {
        "1" => Some(1),
        "100" => Some(2),
        value if value.starts_with("830") || value == "500" => Some(4),
        value if value.starts_with("700") => Some(6),
        value if value.starts_with("600") => Some(5),
        value if value.starts_with("80") => Some(7),
        _ => None,
    }
}

fn format_tag_list_response(tags: Vec<TagRecord>) -> Value {
    Value::Array(
        tags.into_iter()
            .map(backend_tag_to_frontend)
            .map(Value::Object)
            .collect(),
    )
}

fn format_template_list_response(templates: Vec<TemplateRecord>) -> Value {
    Value::Array(templates.into_iter().map(Value::Object).collect())
}

fn backend_tag_to_frontend(tag: TagRecord) -> Map<String, Value> {
    let hidden = tag.hidden != 0;
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(tag.id.to_string()));
    result.insert("name".to_string(), Value::String(tag.name));
    result.insert(
        "color".to_string(),
        tag.color.map_or(Value::Null, Value::String),
    );
    result.insert(
        "icon".to_string(),
        tag.icon.map_or(Value::Null, Value::String),
    );
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(tag.display_order)),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_account_to_frontend_normalizes_missing_legacy_category_and_type() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(17000000044_i64)));
        account.insert("name".to_string(), Value::String("工商银行".to_string()));
        account.insert("type".to_string(), Value::Number(Number::from(0)));
        account.insert("category".to_string(), Value::Number(Number::from(0)));
        account.insert("hidden".to_string(), Value::Bool(false));

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(1));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(2));
        assert_eq!(frontend.get("hidden").and_then(Value::as_bool), Some(false));
        assert_eq!(frontend.get("visible").and_then(Value::as_bool), Some(true));
    }

    #[test]
    fn backend_account_to_frontend_preserves_known_category_and_multi_type() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(93)));
        account.insert("name".to_string(), Value::String("支付宝（三方主账户）".to_string()));
        account.insert("type".to_string(), Value::Number(Number::from(2)));
        account.insert("category".to_string(), Value::Number(Number::from(4)));
        account.insert("balance_cents".to_string(), Value::Number(Number::from(1234)));
        account.insert("balance".to_string(), Value::Number(Number::from(99)));

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(2));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(4));
        assert_eq!(frontend.get("balanceCents").and_then(Value::as_i64), Some(1234));
        assert!(frontend.get("balance").is_none());
    }

    #[test]
    fn backend_account_to_frontend_maps_legacy_text_category_hints() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(38)));
        account.insert("name".to_string(), Value::String("农业银行信用卡".to_string()));
        account.insert("type".to_string(), Value::String("credit_card".to_string()));
        account.insert("category".to_string(), Value::Null);

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(1));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(3));
    }

    #[test]
    fn backend_account_to_frontend_maps_legacy_text_category_hint_variants() {
        let cases = [
            ("investment", 7),
            ("loan", 5),
            ("receivable", 6),
            ("bank", 2),
            ("saving", 8),
            ("virtual_account", 4),
        ];

        for (account_type, expected_category) in cases {
            let mut account = Map::new();
            account.insert("id".to_string(), Value::Number(Number::from(1)));
            account.insert("name".to_string(), Value::String(account_type.to_string()));
            account.insert("type".to_string(), Value::String(account_type.to_string()));

            let frontend = backend_account_to_frontend(account);

            assert_eq!(
                frontend.get("category").and_then(Value::as_i64),
                Some(expected_category)
            );
        }
    }

    #[test]
    fn backend_account_to_frontend_normalizes_legacy_multi_type_string() {
        let mut account = Map::new();
        account.insert("id".to_string(), Value::Number(Number::from(93)));
        account.insert("name".to_string(), Value::String("支付宝".to_string()));
        account.insert(
            "type".to_string(),
            Value::String("multiple_sub_accounts".to_string()),
        );

        let frontend = backend_account_to_frontend(account);

        assert_eq!(frontend.get("type").and_then(Value::as_i64), Some(2));
        assert_eq!(frontend.get("category").and_then(Value::as_i64), Some(4));
    }

    #[test]
    fn backend_account_to_frontend_infers_recovered_account_category_distribution() {
        let cases = [
            ("农业银行", 1, "", 2),
            ("农业银行信用卡", 1, "110", 3),
            ("邮储银行信用卡", 1, "110", 3),
            ("浦发银行信用卡", 1, "110", 3),
            ("民生银行", 1, "100", 2),
            ("建设银行", 1, "100", 2),
            ("工商银行", 0, "100", 2),
            ("微信", 1, "8302", 4),
            ("中信建投证券", 1, "801", 7),
            ("支付宝（三方主账户）", 2, "8300", 4),
            ("支付宝（投资主账户）", 2, "8300", 7),
            ("花呗", 1, "8300", 4),
            ("活期资产", 1, "8300", 7),
            ("稳健理财", 1, "8300", 7),
            ("进阶理财", 1, "8300", 7),
            ("垫付款", 1, "700", 6),
            ("借账单", 1, "700", 6),
            ("现金", 1, "1", 1),
        ];

        for (name, account_type, icon, expected_category) in cases {
            let mut account = Map::new();
            account.insert("id".to_string(), Value::Number(Number::from(1)));
            account.insert("name".to_string(), Value::String(name.to_string()));
            account.insert(
                "type".to_string(),
                Value::Number(Number::from(account_type)),
            );
            account.insert("category".to_string(), Value::Null);
            account.insert("icon".to_string(), Value::String(icon.to_string()));

            let frontend = backend_account_to_frontend(account);

            assert_eq!(
                frontend.get("category").and_then(Value::as_i64),
                Some(expected_category),
                "{name} should map to category {expected_category}"
            );
        }
    }

    #[test]
    fn frontend_account_to_backend_uses_explicit_cents_fields() {
        let backend = frontend_account_to_backend(&json!({
            "name": "招商银行",
            "balanceCents": 123456,
            "visible": false,
            "displayOrder": 3
        }))
        .expect("backend payload");

        assert_eq!(backend.get("balance_cents"), Some(&json!(123456)));
        assert_eq!(backend.get("initial_balance_cents"), Some(&json!(123456)));
        assert_eq!(backend.get("hidden"), Some(&json!(true)));
        assert!(backend.get("balance").is_none());

        let fallback = frontend_account_to_backend(&json!({
            "name": "期初资产",
            "initialBalanceCents": "654321"
        }))
        .expect("fallback payload");
        assert_eq!(fallback.get("balance_cents"), Some(&json!(654321)));
        assert_eq!(fallback.get("initial_balance_cents"), Some(&json!(654321)));

        for (field, value) in [
            ("balanceCents", json!("12.34")),
            ("balance_cents", json!(18.49)),
            ("initialBalanceCents", json!(true)),
            ("initial_balance_cents", json!({"cents": 1})),
        ] {
            let mut payload = Map::new();
            payload.insert("name".to_string(), json!("坏账户"));
            payload.insert(field.to_string(), value);
            let error = frontend_account_to_backend(&Value::Object(payload))
            .expect_err("invalid explicit cents should be rejected");
            assert!(error.contains(field), "{error}");
            assert!(error.contains("integer cents"), "{error}");
        }
    }

    #[test]
    fn balance_discrepancy_response_uses_explicit_cents_names() {
        let value = format_account_balance_discrepancy(AccountBalanceDiscrepancy {
            account_id: 7,
            name: "招商银行".to_string(),
            old_balance_cents: 1000,
            new_balance_cents: 1250,
            diff_cents: 250,
        });

        assert_eq!(value["oldBalanceCents"], json!(1000));
        assert_eq!(value["newBalanceCents"], json!(1250));
        assert_eq!(value["diffCents"], json!(250));
        assert!(value.get("oldBalance").is_none());
    }
}
