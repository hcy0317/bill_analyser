pub fn normalize_reconcile_history_families(raw_families: Option<&Value>) -> Vec<String> {
    let allowed = ["transfer", "investment", "learning"];
    let families = match raw_families {
        Some(Value::Array(values)) => values.as_slice(),
        _ => return vec!["transfer".to_string()],
    };
    let mut normalized = Vec::new();
    for family in families {
        let family = value_to_string_value(family).trim().to_lowercase();
        if allowed.contains(&family.as_str()) && !normalized.contains(&family) {
            normalized.push(family);
        }
    }
    if normalized.is_empty() {
        vec!["transfer".to_string()]
    } else {
        normalized
    }
}

pub fn normalize_keyword_list(raw_value: Option<&Value>, fallback: &[&str]) -> Vec<String> {
    let values: Vec<String> = match raw_value {
        None | Some(Value::Null) => fallback.iter().map(|value| (*value).to_string()).collect(),
        Some(Value::String(text)) if text.trim().is_empty() => {
            fallback.iter().map(|value| (*value).to_string()).collect()
        }
        Some(Value::Array(values)) => values.iter().map(value_to_string_value).collect(),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.starts_with('[') {
                if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(trimmed) {
                    return dedupe_keywords(values.iter().map(value_to_string_value));
                }
            }
            let normalized = ["\n", ",", "，", "|", "、", ";", "；"]
                .iter()
                .fold(trimmed.to_string(), |text, separator| {
                    text.replace(separator, "\n")
                });
            normalized.lines().map(ToOwned::to_owned).collect()
        }
        Some(_) => fallback.iter().map(|value| (*value).to_string()).collect(),
    };
    dedupe_keywords(values)
}

pub fn serialize_keyword_list(raw_value: Option<&Value>) -> String {
    serde_json::to_string(&normalize_keyword_list(raw_value, &[]))
        .unwrap_or_else(|_| "[]".to_string())
}

pub fn build_user_investment_keyword_settings(user: Option<&Map<String, Value>>) -> Value {
    let user = user.cloned().unwrap_or_default();
    json!({
        "platform_keywords": normalize_keyword_list(user.get("investment_platform_keywords"), DEFAULT_INVESTMENT_PLATFORM_KEYWORDS),
        "product_keywords": normalize_keyword_list(user.get("investment_product_keywords"), DEFAULT_INVESTMENT_PRODUCT_KEYWORDS),
        "exclude_keywords": normalize_keyword_list(user.get("investment_exclude_keywords"), DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS),
    })
}

pub fn extract_investment_profile(text: &str, keyword_config: Option<&Value>) -> InvestmentProfile {
    let raw_text = text.trim();
    if raw_text.is_empty() {
        return InvestmentProfile {
            platform: String::new(),
            product: String::new(),
        };
    }
    let text_lower = raw_text.to_lowercase();
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    let platform_keywords = value_array_strings(config.get("platform_keywords"));
    let product_keywords = value_array_strings(config.get("product_keywords"));

    let mut platform = String::new();
    let mut best_platform_score = (-1, 0usize);
    for (canonical, aliases) in PLATFORM_ALIASES {
        let mut matched_aliases: Vec<&str> = vec![canonical];
        matched_aliases.extend_from_slice(aliases);
        let Some(best_alias) = matched_aliases
            .into_iter()
            .filter(|alias| text_lower.contains(&alias.to_lowercase()))
            .max_by_key(|alias| alias.chars().count())
        else {
            continue;
        };
        let score = (
            if GENERIC_INVESTMENT_PLATFORMS.contains(canonical) {
                0
            } else {
                1
            },
            best_alias.chars().count(),
        );
        if score > best_platform_score {
            best_platform_score = score;
            platform = (*canonical).to_string();
        }
    }
    if platform.is_empty() {
        for keyword in sort_by_len_desc(&platform_keywords) {
            if text_lower.contains(&keyword.to_lowercase()) {
                platform = keyword;
                break;
            }
        }
    }

    let mut product = sort_by_len_desc(&product_keywords)
        .into_iter()
        .find(|keyword| text_lower.contains(&keyword.to_lowercase()))
        .unwrap_or_default();
    if let Some(named_product) = extract_named_product(raw_text, &platform) {
        if product.is_empty() || named_product.chars().count() > product.chars().count() {
            product = named_product;
        }
    }
    let mut best_alias_product = String::new();
    let mut best_alias_product_score = 0usize;
    for (canonical, aliases) in PRODUCT_ALIASES {
        let mut alias_candidates: Vec<&str> = vec![canonical];
        alias_candidates.extend_from_slice(aliases);
        let Some(best_alias) = alias_candidates
            .into_iter()
            .filter(|alias| text_lower.contains(&alias.to_lowercase()))
            .max_by_key(|alias| alias.chars().count())
        else {
            continue;
        };
        if best_alias.chars().count() > best_alias_product_score {
            best_alias_product_score = best_alias.chars().count();
            best_alias_product = best_alias.to_string();
        }
    }
    if product.is_empty()
        || best_alias_product_score > product.chars().count()
        || matches!(
            product.as_str(),
            "基金" | "理财" | "债券" | "股票" | "黄金" | "组合" | "计划"
        )
    {
        product = best_alias_product;
    }
    product = clean_investment_product_name(&product, &platform);
    if GENERIC_INVESTMENT_PLATFORMS.contains(&platform.as_str())
        && matches!(
            product.as_str(),
            "基金" | "理财" | "债券" | "股票" | "黄金" | "组合" | "计划"
        )
        && !contains_any(&text_lower, INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS).is_empty()
    {
        product.clear();
    }
    InvestmentProfile { platform, product }
}

pub fn is_ordinary_bank_interest_income(
    bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> bool {
    let current_type = value_to_string(bill.get("type")).trim().to_lowercase();
    if matches!(current_type.as_str(), "转账" | "transfer" | "4") {
        return false;
    }
    let evidence_text = join_nonempty([
        value_to_string(bill.get("counterparty")),
        value_to_string(bill.get("payment_method")),
        value_to_string(bill.get("description")),
        value_to_string(bill.get("original_category")),
    ]);
    if evidence_text.is_empty() {
        return false;
    }
    let evidence_lower = evidence_text.to_lowercase();
    let has_settlement_interest = !contains_any(&evidence_lower, ORDINARY_BANK_INTEREST_KEYWORDS)
        .is_empty()
        || evidence_lower.contains("结息");
    let has_generic_interest = evidence_lower.contains("利息");
    if !has_settlement_interest && !has_generic_interest {
        return false;
    }
    if has_generic_interest && !has_settlement_interest {
        let amount = value_to_f64(bill.get("amount"));
        if matches!(current_type.as_str(), "支出" | "expense" | "3") || amount < 0.0 {
            return false;
        }
    }
    if !ORDINARY_BANK_CONTEXT_KEYWORDS
        .iter()
        .any(|keyword| evidence_lower.contains(&keyword.to_lowercase()))
    {
        return false;
    }
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    let profile = extract_investment_profile(&evidence_text, Some(&config));
    if !profile.platform.is_empty() || !profile.product.is_empty() {
        return false;
    }
    let mut investment_keywords = value_array_strings(config.get("platform_keywords"));
    investment_keywords.extend(value_array_strings(config.get("product_keywords")));
    investment_keywords.extend(
        INVESTMENT_CONTEXT_KEYWORDS
            .iter()
            .map(|item| (*item).to_string()),
    );
    !investment_keywords
        .iter()
        .any(|keyword| evidence_lower.contains(&keyword.to_lowercase()))
}

pub fn score_investment_candidate(
    bill: &Map<String, Value>,
    allow_existing_investment: bool,
    keyword_config: Option<&Value>,
) -> Option<InvestmentSignal> {
    let current_type = value_to_string(bill.get("type")).trim().to_lowercase();
    if matches!(current_type.as_str(), "转账" | "transfer" | "4") {
        return None;
    }
    let is_explicit_investment_type = EXPLICIT_INVESTMENT_TYPES.contains(&current_type.as_str());
    if !allow_existing_investment && is_explicit_investment_type {
        return None;
    }
    let evidence_text = join_nonempty([
        value_to_string(bill.get("counterparty")),
        value_to_string(bill.get("payment_method")),
        value_to_string(bill.get("description")),
        value_to_string(bill.get("original_category")),
    ]);
    let assigned_category_text = join_nonempty([
        value_to_string(bill.get("main_category")),
        value_to_string(bill.get("sub_category")),
    ]);
    let all_text = join_nonempty([evidence_text.clone(), assigned_category_text]);
    if evidence_text.is_empty() {
        return None;
    }
    let evidence_lower = evidence_text.to_lowercase();
    let all_text_lower = all_text.to_lowercase();
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    if is_ordinary_bank_interest_income(bill, Some(&config))
        || classify_investment_pnl_change(bill, Some(&config)).is_some()
    {
        return None;
    }
    let profile = extract_investment_profile(&evidence_text, Some(&config));
    let platform_keywords = value_array_strings(config.get("platform_keywords"));
    let product_keywords: Vec<String> = value_array_strings(config.get("product_keywords"))
        .into_iter()
        .filter(|keyword| !INVESTMENT_ACTION_KEYWORDS.contains(&keyword.as_str()))
        .collect();
    let exclude_keywords = value_array_strings(config.get("exclude_keywords"));
    let action_matches = contains_any(&evidence_lower, INVESTMENT_ACTION_KEYWORDS);
    let intrinsic_negative_matches =
        contains_any(&all_text_lower, INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS);
    let mut matched_platforms = Vec::new();
    append_unique(&mut matched_platforms, &profile.platform);
    for keyword in &platform_keywords {
        if evidence_lower.contains(&keyword.to_lowercase()) {
            append_unique(&mut matched_platforms, keyword);
        }
    }
    let mut matched_products = Vec::new();
    append_unique(&mut matched_products, &profile.product);
    for keyword in &product_keywords {
        if evidence_lower.contains(&keyword.to_lowercase()) {
            append_unique(&mut matched_products, keyword);
        }
    }
    let matched_excludes: Vec<String> = exclude_keywords
        .into_iter()
        .filter(|keyword| all_text_lower.contains(&keyword.to_lowercase()))
        .collect();
    let has_specific_platform = matched_platforms
        .iter()
        .any(|platform| !GENERIC_INVESTMENT_PLATFORMS.contains(&platform.as_str()));
    let has_named_or_specific_product = matched_products.iter().any(|product| {
        product.chars().count() > 2
            && !matches!(
                product.as_str(),
                "基金" | "理财" | "债券" | "股票" | "黄金" | "组合" | "计划"
            )
    });
    let mut score = 0.0;
    if has_specific_platform {
        score += (0.38 * matched_platforms.iter().take(2).count() as f64).min(0.65);
    } else if !matched_platforms.is_empty() {
        score += 0.28;
    }
    if !matched_products.is_empty() {
        score += (0.18 * matched_products.iter().take(3).count() as f64).min(0.42);
    }
    if !action_matches.is_empty() {
        score += 0.12;
    }
    let explicit_boost = if allow_existing_investment
        && is_explicit_investment_type
        && has_specific_platform
        && !action_matches.is_empty()
    {
        0.08
    } else {
        0.0
    };
    score += explicit_boost;
    if !matched_excludes.is_empty() {
        score -= (0.28 * matched_excludes.iter().take(2).count() as f64).min(0.48);
    }
    if !intrinsic_negative_matches.is_empty() {
        score -= (0.35 * intrinsic_negative_matches.iter().take(2).count() as f64).min(0.55);
    }
    if matched_platforms.is_empty() && matched_products.len() < 2 {
        return None;
    }
    if !matched_platforms.is_empty()
        && !has_specific_platform
        && !has_named_or_specific_product
        && action_matches.is_empty()
    {
        return None;
    }
    if score < 0.55 {
        return None;
    }
    let mut reason_parts = Vec::new();
    if !matched_platforms.is_empty() {
        reason_parts.push(format!(
            "platform:{}",
            matched_platforms
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !matched_products.is_empty() {
        reason_parts.push(format!(
            "product:{}",
            matched_products
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !matched_excludes.is_empty() {
        reason_parts.push(format!(
            "exclude:{}",
            matched_excludes
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !intrinsic_negative_matches.is_empty() {
        reason_parts.push(format!(
            "negative:{}",
            intrinsic_negative_matches
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if explicit_boost > 0.0 {
        reason_parts.push("type:investment".to_string());
    }
    let mut hint_tokens = matched_platforms
        .iter()
        .take(1)
        .cloned()
        .collect::<Vec<_>>();
    hint_tokens.extend(matched_products.iter().take(2).cloned());
    Some(InvestmentSignal {
        score: round2(score.min(1.0)),
        reason: reason_parts.join(", "),
        hint_text: hint_tokens.join(" "),
        platform: profile.platform,
        product: profile.product,
        signal_type: None,
        label: None,
        direction: None,
        direction_label: None,
    })
}

pub fn classify_investment_pnl_change(
    bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> Option<InvestmentSignal> {
    let current_type = value_to_string(bill.get("type")).trim().to_lowercase();
    if matches!(current_type.as_str(), "转账" | "transfer" | "4") {
        return None;
    }
    let evidence_text = join_nonempty([
        value_to_string(bill.get("counterparty")),
        value_to_string(bill.get("payment_method")),
        value_to_string(bill.get("description")),
        value_to_string(bill.get("original_category")),
    ]);
    let assigned_category_text = join_nonempty([
        value_to_string(bill.get("main_category")),
        value_to_string(bill.get("sub_category")),
    ]);
    let all_text = join_nonempty([evidence_text.clone(), assigned_category_text]);
    if evidence_text.is_empty() {
        return None;
    }
    let all_text_lower = all_text.to_lowercase();
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    if is_ordinary_bank_interest_income(bill, Some(&config)) {
        return None;
    }
    let loss_matches = contains_any(&all_text_lower, INVESTMENT_PNL_LOSS_KEYWORDS);
    let gain_matches = contains_any(&all_text_lower, INVESTMENT_PNL_GAIN_KEYWORDS);
    if loss_matches.is_empty() && gain_matches.is_empty() {
        return None;
    }
    let profile = extract_investment_profile(&evidence_text, Some(&config));
    let has_config_platform = value_array_strings(config.get("platform_keywords"))
        .iter()
        .any(|keyword| all_text_lower.contains(&keyword.to_lowercase()));
    let has_config_product = value_array_strings(config.get("product_keywords"))
        .iter()
        .any(|keyword| all_text_lower.contains(&keyword.to_lowercase()));
    let has_explicit_context = EXPLICIT_INVESTMENT_TYPES.contains(&current_type.as_str())
        || INVESTMENT_CONTEXT_KEYWORDS
            .iter()
            .any(|keyword| all_text_lower.contains(keyword));
    if !(!profile.platform.is_empty()
        || !profile.product.is_empty()
        || has_config_platform
        || has_config_product
        || has_explicit_context)
    {
        return None;
    }
    let (direction, direction_label, matched_keywords) = if loss_matches.is_empty() {
        ("gain", "收益", gain_matches)
    } else {
        ("loss", "亏损", loss_matches)
    };
    let mut score: f64 = 0.72;
    if !profile.platform.is_empty() || has_config_platform {
        score += 0.08;
    }
    if !profile.product.is_empty() || has_config_product {
        score += 0.06;
    }
    if EXPLICIT_INVESTMENT_TYPES.contains(&current_type.as_str()) {
        score += 0.04;
    }
    let mut reason_parts = vec![format!("盈亏变化:{direction_label}")];
    if !profile.platform.is_empty() {
        reason_parts.push(format!("platform:{}", profile.platform));
    }
    if !profile.product.is_empty() {
        reason_parts.push(format!("product:{}", profile.product));
    }
    reason_parts.push(format!(
        "keyword:{}",
        matched_keywords
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("/")
    ));
    let hint_text = [profile.platform.clone(), profile.product.clone()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some(InvestmentSignal {
        score: round2(score.min(1.0)),
        reason: reason_parts.join(", "),
        hint_text,
        platform: profile.platform,
        product: profile.product,
        signal_type: Some("pnl_change".to_string()),
        label: Some("盈亏变化".to_string()),
        direction: Some(direction.to_string()),
        direction_label: Some(direction_label.to_string()),
    })
}
