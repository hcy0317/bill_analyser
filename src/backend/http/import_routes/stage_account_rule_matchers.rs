// 中文导读：导入 stage2 的账户规则匹配 pass，负责把 account_rules 应用到转账、投资、收入和支出预览行。
// 维护重点：本文件只组合导入预览上下文与 core account_rules，不重新实现规则表达式语义。
// 不变式：导入账户识别不得回退到账户别名直接匹配；旧别名只允许恢复工具一次性转换为账户规则。

#[tracing::instrument(level = "debug", skip_all)]
fn apply_transfer_account_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[AccountRuleCandidate],
    accounts: &[ImportIntelligenceAccount],
) -> bool {
    if rules.is_empty() {
        return false;
    }

    let source_chain = transfer_source_chain(draft).unwrap_or_default();
    let outgoing = transfer_chain_entry_for_roles(
        &source_chain,
        &["outgoing", "source", "from", "debit", "out"],
        Some(0),
    )
    .cloned();
    let incoming = transfer_chain_entry_for_roles(
        &source_chain,
        &["incoming", "destination", "to", "credit", "in"],
        Some(1),
    )
    .cloned();

    let mut changed = false;
    if draft.preview_source_account_id.is_none() {
        let context = transfer_account_rule_context(draft, outgoing.as_ref(), true);
        if let Some(rule_match) = valid_account_rule_match(
            rules,
            accounts,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_TRANSFER,
        ) {
            draft.preview_source_account_id = Some(rule_match.account_id);
            annotate_account_rule_match(draft, accounts, "source", &rule_match, None);
            changed = true;
        }
    }

    if draft.preview_destination_account_id.is_none() {
        let context = transfer_account_rule_context(draft, incoming.as_ref(), false);
        if let Some(rule_match) = valid_account_rule_match(
            rules,
            accounts,
            &context,
            ACCOUNT_ROLE_DESTINATION,
            TRANSACTION_SCOPE_TRANSFER,
        )
        .filter(|rule_match| draft.preview_source_account_id != Some(rule_match.account_id))
        {
            draft.preview_destination_account_id = Some(rule_match.account_id);
            annotate_account_rule_match(draft, accounts, "destination", &rule_match, None);
            changed = true;
        }
    }

    if changed {
        annotate_transfer_account_rule_resolution(draft);
    }
    changed
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_investment_account_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[AccountRuleCandidate],
    accounts: &[ImportIntelligenceAccount],
) -> bool {
    if rules.is_empty() {
        return false;
    }

    let mut changed = false;
    if draft.preview_source_account_id.is_none() {
        let context = investment_source_account_rule_context(draft);
        if let Some(rule_match) = valid_account_rule_match(
            rules,
            accounts,
            &context,
            ACCOUNT_ROLE_SOURCE,
            TRANSACTION_SCOPE_INVESTMENT,
        ) {
            draft.preview_source_account_id = Some(rule_match.account_id);
            annotate_account_rule_match(draft, accounts, "source", &rule_match, None);
            changed = true;
        }
    }

    if draft.preview_destination_account_id.is_none() {
        let counterparty_context = investment_counterparty_account_rule_context(draft);
        let matched = valid_account_rule_match(
            rules,
            accounts,
            &counterparty_context,
            ACCOUNT_ROLE_INVESTMENT,
            TRANSACTION_SCOPE_INVESTMENT,
        )
        .map(|rule_match| (rule_match, None))
        .or_else(|| {
            let description_context = investment_description_account_rule_context(draft);
            valid_account_rule_match(
                rules,
                accounts,
                &description_context,
                ACCOUNT_ROLE_INVESTMENT,
                TRANSACTION_SCOPE_INVESTMENT,
            )
            .map(|rule_match| (rule_match, Some("description")))
        });

        if let Some((rule_match, fallback)) = matched {
            draft.preview_destination_account_id = Some(rule_match.account_id);
            annotate_account_rule_match(draft, accounts, "investment", &rule_match, fallback);
            changed = true;
        }
    }

    changed
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_standard_account_rule_match(
    draft: &mut ImportPreviewDraft,
    rules: &[AccountRuleCandidate],
    accounts: &[ImportIntelligenceAccount],
) -> bool {
    if rules.is_empty() || draft.preview_source_account_id.is_some() {
        return false;
    }

    let Some(transaction_scope) = (match preview_type_code(&draft.preview_type) {
        Some(2) => Some(TRANSACTION_SCOPE_INCOME),
        Some(3) => Some(TRANSACTION_SCOPE_EXPENSE),
        _ => None,
    }) else {
        return false;
    };
    let context = visible_account_rule_context(draft);
    let Some(rule_match) = valid_account_rule_match(
        rules,
        accounts,
        &context,
        ACCOUNT_ROLE_SOURCE,
        transaction_scope,
    ) else {
        return false;
    };

    draft.preview_source_account_id = Some(rule_match.account_id);
    annotate_account_rule_match(draft, accounts, "source", &rule_match, None);
    true
}

fn valid_account_rule_match(
    rules: &[AccountRuleCandidate],
    accounts: &[ImportIntelligenceAccount],
    context: &AccountRuleMatchContext,
    requested_role_scope: &str,
    transaction_type: &str,
) -> Option<AccountRuleMatch> {
    match_account_rules(rules, context, requested_role_scope, transaction_type)
        .filter(|rule_match| account_rule_account_exists(accounts, rule_match.account_id))
}

fn account_rule_account_exists(accounts: &[ImportIntelligenceAccount], account_id: i64) -> bool {
    accounts.is_empty() || accounts.iter().any(|account| account.id == account_id)
}

fn visible_account_rule_context(draft: &ImportPreviewDraft) -> AccountRuleMatchContext {
    AccountRuleMatchContext {
        parser_id: draft.preview_parser_id.clone(),
        parser_label: parser_source_label(&draft.preview_parser_id).to_string(),
        parser_tags: parser_tags_from_value(draft.preview_parser_tags.as_ref()),
        counterparty: draft.preview_counterparty.clone(),
        payment_method: draft.preview_payment_method.clone(),
        description: draft.preview_description.clone(),
        ..AccountRuleMatchContext::default()
    }
}

fn transfer_account_rule_context(
    draft: &ImportPreviewDraft,
    entry: Option<&Value>,
    source_side: bool,
) -> AccountRuleMatchContext {
    let mut context = visible_account_rule_context(draft);
    let counterparty;
    let payment_method;
    let description;
    if let Some(entry) = entry {
        let parser_id = transfer_entry_text(entry.get("parser_id"))
            .unwrap_or_else(|| draft.preview_parser_id.clone());
        context.parser_id = parser_id.clone();
        context.parser_label = parser_source_label(&parser_id).to_string();
        let parser_tags = transfer_entry_tags(entry);
        if !parser_tags.is_empty() {
            context.parser_tags = parser_tags;
        }
        counterparty = transfer_entry_text(entry.get("counterparty")).unwrap_or_default();
        payment_method = transfer_entry_text(entry.get("payment_method")).unwrap_or_default();
        description = transfer_entry_text(entry.get("description")).unwrap_or_default();
        context.counterparty = counterparty.clone();
        context.payment_method = payment_method.clone();
        context.description = description.clone();
    } else {
        counterparty = draft.preview_counterparty.clone();
        payment_method = draft.preview_payment_method.clone();
        description = draft.preview_description.clone();
    }

    if source_side {
        context.expense_counterparty = counterparty;
        context.expense_payment_method = payment_method;
        context.expense_description = description;
    } else {
        context.income_counterparty = counterparty;
        context.income_payment_method = payment_method;
        context.income_description = description;
    }
    context
}

fn investment_source_account_rule_context(draft: &ImportPreviewDraft) -> AccountRuleMatchContext {
    let mut context = visible_account_rule_context(draft);
    context.counterparty.clear();
    context.description.clear();
    context
}

fn investment_counterparty_account_rule_context(
    draft: &ImportPreviewDraft,
) -> AccountRuleMatchContext {
    let mut context = visible_account_rule_context(draft);
    context.payment_method.clear();
    context.description.clear();
    context.investment_counterparty = draft.preview_counterparty.clone();
    context
}

fn investment_description_account_rule_context(
    draft: &ImportPreviewDraft,
) -> AccountRuleMatchContext {
    let mut context = visible_account_rule_context(draft);
    context.counterparty.clear();
    context.payment_method.clear();
    context.investment_description = draft.preview_description.clone();
    context
}

fn transfer_source_chain(draft: &ImportPreviewDraft) -> Option<Vec<Value>> {
    draft
        .preview_matching_feedback
        .get("transfer")
        .and_then(|transfer| transfer.get("source_chain"))
        .and_then(Value::as_array)
        .cloned()
        .filter(|chain| !chain.is_empty())
}

fn transfer_entry_tags(entry: &Value) -> Vec<String> {
    ["tags", "parser_tags"]
        .into_iter()
        .filter_map(|field| entry.get(field).and_then(Value::as_array))
        .flat_map(|tags| tags.iter().filter_map(value_to_text))
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect()
}

fn parser_tags_from_value(value: Option<&Value>) -> Vec<String> {
    value.and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(value_to_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn import_preview_transfer_rule_text(draft: &ImportPreviewDraft) -> String {
    let mut parts = vec![import_preview_rule_text(draft)];
    if let Some(source_chain) = transfer_source_chain(draft) {
        for entry in source_chain {
            for field in [
                "counterparty",
                "payment_method",
                "description",
                "parser_id",
                "parser_label",
            ] {
                if let Some(text) = transfer_entry_text(entry.get(field)) {
                    parts.push(text);
                }
            }
            parts.extend(transfer_entry_tags(&entry));
        }
    }
    parts
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn annotate_account_rule_match(
    draft: &mut ImportPreviewDraft,
    accounts: &[ImportIntelligenceAccount],
    key: &str,
    rule_match: &AccountRuleMatch,
    fallback: Option<&str>,
) {
    let mut payload = json!({
        "account_id": rule_match.account_id,
        "rule_id": rule_match.rule_id,
        "matched_fields": rule_match.matched_fields.clone(),
        "priority": rule_match.priority,
        "fallback_used": rule_match.fallback_used,
        "account_role_scope": rule_match.account_role_scope.clone(),
        "transaction_type_scope": rule_match.transaction_type_scope.clone(),
        "reason": "account rule expression matched",
        "review_status": "auto_applied",
    });
    if let Some(account_name) = account_name_for_id(accounts, rule_match.account_id) {
        if let Some(object) = payload.as_object_mut() {
            object.insert("account_name".to_string(), json!(account_name));
        }
    }
    if let Some(fallback) = fallback {
        if let Some(object) = payload.as_object_mut() {
            object.insert("fallback".to_string(), json!(fallback));
        }
    }

    let feedback = matching_feedback_object_mut(draft);
    let account_rule = feedback
        .entry("account_rule".to_string())
        .or_insert_with(|| json!({}));
    if !account_rule.is_object() {
        *account_rule = json!({});
    }
    if let Some(object) = account_rule.as_object_mut() {
        object.insert(key.to_string(), payload);
    }
    annotate_account_rule_summary(draft, accounts);
}

fn annotate_account_rule_summary(
    draft: &mut ImportPreviewDraft,
    accounts: &[ImportIntelligenceAccount],
) {
    let source_account_id = draft.preview_source_account_id;
    let destination_account_id = draft.preview_destination_account_id;
    let source_account_name = draft
        .preview_source_account_id
        .and_then(|account_id| account_name_for_id(accounts, account_id));
    let destination_account_name = draft
        .preview_destination_account_id
        .and_then(|account_id| account_name_for_id(accounts, account_id));
    matching_feedback_object_mut(draft).insert(
        "account".to_string(),
        json!({
            "source": "account_rules",
            "source_account_id": source_account_id,
            "source_account_name": source_account_name,
            "destination_account_id": destination_account_id,
            "destination_account_name": destination_account_name,
            "reason": "account rule expression matched",
            "review_status": "auto_applied",
        }),
    );
}

fn annotate_transfer_account_rule_resolution(draft: &mut ImportPreviewDraft) {
    let source_account_id = draft.preview_source_account_id;
    let destination_account_id = draft.preview_destination_account_id;
    let feedback = matching_feedback_object_mut(draft);
    let transfer = feedback
        .entry("transfer".to_string())
        .or_insert_with(|| json!({}));
    if !transfer.is_object() {
        *transfer = json!({});
    }
    if let Some(transfer_object) = transfer.as_object_mut() {
        transfer_object.insert("account_resolution".to_string(), json!("account_rules"));
        transfer_object.insert(
            "resolved_source_account_id".to_string(),
            json!(source_account_id),
        );
        transfer_object.insert(
            "resolved_destination_account_id".to_string(),
            json!(destination_account_id),
        );
    }
}

fn account_name_for_id(accounts: &[ImportIntelligenceAccount], account_id: i64) -> Option<String> {
    accounts
        .iter()
        .find(|account| account.id == account_id)
        .map(|account| account.name.clone())
}

#[cfg(test)]
include!("stage_account_rule_matchers_tests.rs");
