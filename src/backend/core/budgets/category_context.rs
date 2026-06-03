// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。


#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_budget_category_type(raw: Option<i32>) -> Option<TransactionType> {
    match raw {
        Some(CURRENT_EXPENSE_CATEGORY_TYPE) => Some(TransactionType::Expense),
        Some(value) => TransactionType::from_frontend_code(value).ok(),
        None => None,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_budget_category_context(categories: &[Value]) -> BudgetCategoryContext {
    #[cfg(not(coverage))]
    tracing::info!(domain = "budget", operation = "build_budget_category_context", "business operation entered");
    let mut context = BudgetCategoryContext::default();
    for category in categories {
        let Value::Object(object) = category else {
            continue;
        };

        let raw_name = string_field(object, "name").trim().to_string();
        let raw_parent_name = string_field(object, "parent_name").trim().to_string();
        let main_category = string_field(object, "main_category").trim().to_string();
        let sub_category = string_field(object, "sub_category").trim().to_string();
        let (main_category, sub_category) = if !main_category.is_empty() {
            (main_category, sub_category)
        } else if !raw_parent_name.is_empty() {
            (raw_parent_name, raw_name)
        } else {
            (raw_name, String::new())
        };

        if main_category.is_empty() {
            continue;
        }
        let category_type = object
            .get("type")
            .and_then(value_to_i32)
            .and_then(|value| normalize_budget_category_type(Some(value)))
            .map(TransactionType::code);
        let Some(category_type) = category_type else {
            continue;
        };
        let info = BudgetCategoryInfo {
            id: value_to_i64(object.get("id")),
            name: main_category.clone(),
            parent_name: String::new(),
            main_category: main_category.clone(),
            sub_category: sub_category.clone(),
            category_type: Some(category_type),
            icon: string_field(object, "icon"),
            color: string_field(object, "color"),
        };

        context
            .types_by_name
            .entry(main_category.clone())
            .or_default()
            .insert(category_type);

        if sub_category.is_empty() {
            context
                .primary_by_key
                .insert((category_type, main_category.clone()), info.clone());
            context.primary_by_name.entry(main_category).or_insert(info);
        } else {
            context.sub_by_key.insert(
                (category_type, main_category.clone(), sub_category.clone()),
                info.clone(),
            );
            context
                .sub_by_parent_name
                .insert((main_category.clone(), sub_category), info.clone());
            let fallback_key = (category_type, main_category);
            let should_replace = context
                .fallback_by_key
                .get(&fallback_key)
                .is_none_or(|existing| existing.icon.is_empty() && !info.icon.is_empty());
            if should_replace {
                context.fallback_by_key.insert(fallback_key, info);
            }
        }
    }
    context
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_budget_category_type(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    preferred_type: Option<i32>,
) -> Option<TransactionType> {
    let normalized_preferred = preferred_type
        .and_then(|value| normalize_budget_category_type(Some(value)))
        .map(TransactionType::code);
    let category = category.trim();
    let sub_category = sub_category
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let Some(types) = context.types_by_name.get(category) else {
        return normalized_preferred.and_then(|value| normalize_budget_category_type(Some(value)));
    };
    if sub_category.is_none() && types.len() > 1 {
        if let Some(preferred) = normalized_preferred {
            if context
                .primary_by_key
                .contains_key(&(preferred, category.to_string()))
            {
                return normalize_budget_category_type(Some(preferred));
            }
        }
        for candidate_type in types {
            if context
                .primary_by_key
                .contains_key(&(*candidate_type, category.to_string()))
            {
                return normalize_budget_category_type(Some(*candidate_type));
            }
        }
        return None;
    }

    if let Some(preferred) = normalized_preferred {
        if types.contains(&preferred)
            && budget_category_type_matches(context, category, sub_category, preferred)
        {
            return normalize_budget_category_type(Some(preferred));
        }
    }

    for candidate_type in types {
        if budget_category_type_matches(context, category, sub_category, *candidate_type) {
            return normalize_budget_category_type(Some(*candidate_type));
        }
    }

    None
}

fn budget_category_type_matches(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    candidate_type: i32,
) -> bool {
    if let Some(sub_category) = sub_category {
        return context.sub_by_key.contains_key(&(
            candidate_type,
            category.to_string(),
            sub_category.to_string(),
        ));
    }
    context
        .primary_by_key
        .contains_key(&(candidate_type, category.to_string()))
        || context
            .fallback_by_key
            .contains_key(&(candidate_type, category.to_string()))
}

fn budget_category_info_to_json(info: BudgetCategoryInfo) -> Value {
    json!({
        "id": info.id,
        "main_category": info.main_category,
        "sub_category": info.sub_category,
        "type": info.category_type,
        "icon": info.icon,
        "color": info.color
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn merge_budget_primary_with_fallback(
    mut primary: BudgetCategoryInfo,
    fallback: Option<&BudgetCategoryInfo>,
) -> BudgetCategoryInfo {
    let Some(fallback) = fallback else {
        return primary;
    };
    if primary.icon.is_empty() {
        primary.icon = fallback.icon.clone();
    }
    if primary.color.is_empty() {
        primary.color = fallback.color.clone();
    }
    primary
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_budget_category_info(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    preferred_type: Option<i32>,
) -> Value {
    let category = category.trim();
    if category.is_empty() {
        return Value::Null;
    }

    let budget_type = preferred_type
        .and_then(|value| normalize_budget_category_type(Some(value)))
        .map(TransactionType::code)
        .unwrap_or(BUDGET_TYPE_EXPENSE);
    let sub_category = sub_category
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(sub_category) = sub_category {
        return context
            .sub_by_key
            .get(&(budget_type, category.to_string(), sub_category.to_string()))
            .cloned()
            .map(budget_category_info_to_json)
            .unwrap_or(Value::Null);
    }

    let primary = context
        .primary_by_key
        .get(&(budget_type, category.to_string()))
        .cloned();
    let fallback = context
        .fallback_by_key
        .get(&(budget_type, category.to_string()));

    match primary {
        Some(primary) if !primary.icon.is_empty() || fallback.is_none() => {
            budget_category_info_to_json(primary)
        }
        Some(primary) => {
            budget_category_info_to_json(merge_budget_primary_with_fallback(primary, fallback))
        }
        None => fallback
            .cloned()
            .map(budget_category_info_to_json)
            .unwrap_or(Value::Null),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_budget_type_name(budget_type: i32) -> &'static str {
    if budget_type == BUDGET_TYPE_EXPENSE {
        "支出"
    } else {
        "投资"
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn budget_type_matches_category(
    context: &BudgetCategoryContext,
    category: &str,
    sub_category: Option<&str>,
    budget_type: i32,
) -> bool {
    resolve_budget_category_type(context, category, sub_category, Some(budget_type))
        .is_some_and(|category_type| category_type.code() == budget_type)
}
