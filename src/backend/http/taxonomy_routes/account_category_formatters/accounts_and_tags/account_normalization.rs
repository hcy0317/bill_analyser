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
