// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Value};

use crate::{
    account_rules::{
        match_account_rules, AccountRuleCandidate, AccountRuleMatchContext, ACCOUNT_ROLE_SOURCE,
        TRANSACTION_SCOPE_ALL,
    },
    category_rules::match_rule_expression,
};

use super::types::{
    OcrProviderTextResult, PaymentScreenshotParseContract, ReceiptDraftAccount,
    ReceiptDraftContext, ReceiptDraftField, ReceiptDraftTag, ReceiptTransactionDraft,
};

const AUTO_FILL_THRESHOLD: f64 = 0.80;
const CANDIDATE_THRESHOLD: f64 = 0.55;

/// 根据 OCR 解析结果、provider 行文本和本地规则上下文构建交易草稿。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_receipt_transaction_draft(
    parsed: &PaymentScreenshotParseContract,
    provider_result: &OcrProviderTextResult,
    context: &ReceiptDraftContext,
) -> ReceiptTransactionDraft {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_receipt_transaction_draft",
        "business operation entered"
    );
    let mut draft = ReceiptTransactionDraft::default();
    let text = receipt_evidence_text(provider_result);
    let primary_evidence = primary_evidence_lines(provider_result, parsed);

    if let Some(amount) = parsed.amount {
        insert_field(
            &mut draft,
            "amount",
            ReceiptDraftField {
                value: json!(amount),
                confidence: 0.94,
                reason: "ocr_amount".to_string(),
                evidence: primary_evidence.clone(),
                label: None,
                unit: Some("yuan".to_string()),
            },
        );
    }
    if let Some(trade_time) = parsed
        .trade_time
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        insert_field(
            &mut draft,
            "time",
            ReceiptDraftField {
                value: json!(trade_time),
                confidence: 0.88,
                reason: "ocr_time".to_string(),
                evidence: evidence_containing(&primary_evidence, trade_time),
                label: None,
                unit: None,
            },
        );
    }
    if let Some(description) = parsed
        .description
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        insert_field(
            &mut draft,
            "description",
            ReceiptDraftField {
                value: json!(description),
                confidence: 0.84,
                reason: "ocr_description".to_string(),
                evidence: evidence_containing(&primary_evidence, description),
                label: Some(description.to_string()),
                unit: None,
            },
        );
    }

    let inferred_type = infer_transaction_type(&text, parsed.payment_platform.as_deref());
    if let Some((type_value, type_code, confidence, reason, evidence)) = inferred_type.clone() {
        insert_field(
            &mut draft,
            "type",
            ReceiptDraftField {
                value: json!(type_value),
                confidence,
                reason,
                evidence,
                label: Some(type_value.to_string()),
                unit: None,
            },
        );
        apply_category_mapping(&mut draft, &text, type_code, true, context);
    } else {
        apply_category_mapping(&mut draft, &text, 0, false, context);
    }

    apply_account_mapping(
        &mut draft,
        &text,
        parsed,
        inferred_type.as_ref().map(|(value, _, _, _, _)| *value),
        &context.account_rules,
        &context.accounts,
    );
    apply_tag_mapping(&mut draft, &text, &context.tags);

    draft
}

/// 按置信度把草稿字段放入 auto_fill 或 candidates，保持阈值集中一致。
#[tracing::instrument(level = "debug", skip_all)]
fn insert_field(draft: &mut ReceiptTransactionDraft, key: &str, field: ReceiptDraftField) {
    let confidence = field.confidence.clamp(0.0, 1.0);
    let field = ReceiptDraftField {
        confidence,
        ..field
    };
    if confidence >= AUTO_FILL_THRESHOLD {
        draft.auto_fill.insert(key.to_string(), field);
    } else if confidence >= CANDIDATE_THRESHOLD {
        draft
            .candidates
            .entry(key.to_string())
            .or_default()
            .push(field);
    }
}

fn receipt_evidence_text(provider_result: &OcrProviderTextResult) -> String {
    if !provider_result.lines.is_empty() {
        return provider_result
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
    }
    provider_result.text.clone()
}

/// 选取 OCR 主要证据行，优先使用 provider 行级结果，缺失时回退原始文本。
fn primary_evidence_lines(
    provider_result: &OcrProviderTextResult,
    parsed: &PaymentScreenshotParseContract,
) -> Vec<String> {
    let mut lines = provider_result
        .lines
        .iter()
        .map(|line| line.text.trim().to_string())
        .filter(|line| !line.is_empty())
        .take(6)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines = provider_result
            .text
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .take(6)
            .collect();
    }
    if lines.is_empty() {
        if let Some(description) = parsed.description.as_deref() {
            lines.push(description.to_string());
        }
    }
    lines
}

fn evidence_containing(evidence: &[String], needle: &str) -> Vec<String> {
    let needle = normalize_match_text(needle);
    let matched = evidence
        .iter()
        .filter(|line| normalize_match_text(line).contains(&needle))
        .cloned()
        .collect::<Vec<_>>();
    if matched.is_empty() {
        evidence.iter().take(2).cloned().collect()
    } else {
        matched
    }
}

/// 根据 OCR 文本关键词推断交易类型，并附带类型代码、置信度和命中证据。
fn infer_transaction_type(
    text: &str,
    payment_platform: Option<&str>,
) -> Option<(&'static str, i64, f64, String, Vec<String>)> {
    let normalized = normalize_match_text(text);
    if contains_any(&normalized, &["退款", "已退款", "退回", "refund"]) {
        return Some((
            "income",
            2,
            0.68,
            "refund_keyword".to_string(),
            matching_keywords(text, &["退款", "refund"]),
        ));
    }
    if contains_any(&normalized, &["收款", "已收款", "入账", "到账", "received"]) {
        return Some((
            "income",
            2,
            0.86,
            "income_keyword".to_string(),
            matching_keywords(text, &["收款", "入账", "到账", "received"]),
        ));
    }
    if contains_any(&normalized, &["转账", "转出", "转入", "transfer"]) {
        return Some((
            "transfer",
            4,
            0.72,
            "transfer_keyword".to_string(),
            matching_keywords(text, &["转账", "transfer"]),
        ));
    }
    if payment_platform.is_some()
        || contains_any(
            &normalized,
            &[
                "付款",
                "支付",
                "消费",
                "订单金额",
                "实付",
                "purchase",
                "paid",
            ],
        )
    {
        return Some((
            "expense",
            3,
            0.88,
            "payment_keyword".to_string(),
            matching_keywords(text, &["付款", "支付", "消费", "paid"]),
        ));
    }
    None
}

/// 使用分类规则和分类名称对 OCR 草稿做分类映射，严格尊重已知交易类型边界。
#[tracing::instrument(level = "debug", skip_all)]
fn apply_category_mapping(
    draft: &mut ReceiptTransactionDraft,
    text: &str,
    type_code: i64,
    type_is_known: bool,
    context: &ReceiptDraftContext,
) {
    let mut rule_matches = context
        .category_rules
        .iter()
        .filter(|rule| !rule.rule_expression.trim().is_empty())
        .filter(|rule| !type_is_known || rule.category_type == type_code)
        .filter(|rule| match_rule_expression(text, &rule.rule_expression, rule.regex_enabled))
        .collect::<Vec<_>>();
    rule_matches.sort_by_key(|rule| (rule.priority, rule.id.clone()));
    if rule_matches.len() == 1 && type_is_known {
        let rule = rule_matches[0];
        insert_field(
            draft,
            "category_id",
            ReceiptDraftField {
                value: json!(rule.category_id),
                confidence: 0.92,
                reason: "category_rule_exact".to_string(),
                evidence: vec![rule.rule_expression.clone()],
                label: Some(rule.label.clone()),
                unit: None,
            },
        );
        return;
    }
    for rule in rule_matches.into_iter().take(5) {
        push_candidate(
            draft,
            "category_id",
            ReceiptDraftField {
                value: json!(rule.category_id),
                confidence: 0.68,
                reason: "category_rule_ambiguous".to_string(),
                evidence: vec![rule.rule_expression.clone()],
                label: Some(rule.label.clone()),
                unit: None,
            },
        );
    }

    if draft.auto_fill.contains_key("category_id")
        || draft.candidates.contains_key("category_id")
        || text.trim().is_empty()
    {
        return;
    }
    let category_matches = context
        .categories
        .iter()
        .filter(|category| !type_is_known || category.type_code == type_code)
        .filter(|category| category_label_matches_text(&category.label, text))
        .take(5)
        .map(|category| ReceiptDraftField {
            value: json!(category.id),
            confidence: 0.62,
            reason: "category_name_match".to_string(),
            evidence: vec![category.label.clone()],
            label: Some(category.label.clone()),
            unit: None,
        })
        .collect::<Vec<_>>();
    for candidate in category_matches {
        push_candidate(draft, "category_id", candidate);
    }
}

/// 使用账户规则对 OCR 草稿做来源账户映射，复用正式账户规则匹配上下文。
#[tracing::instrument(level = "debug", skip_all)]
fn apply_account_mapping(
    draft: &mut ReceiptTransactionDraft,
    text: &str,
    parsed: &PaymentScreenshotParseContract,
    transaction_type: Option<&str>,
    account_rules: &[AccountRuleCandidate],
    accounts: &[ReceiptDraftAccount],
) {
    if account_rules.is_empty() {
        return;
    }
    let context = AccountRuleMatchContext {
        parser_id: "ocr_receipt".to_string(),
        parser_label: "Receipt OCR".to_string(),
        parser_tags: Vec::new(),
        counterparty: parsed
            .description
            .clone()
            .unwrap_or_else(|| text.to_string()),
        payment_method: parsed.payment_platform.clone().unwrap_or_default(),
        description: text.to_string(),
        expense_counterparty: parsed.description.clone().unwrap_or_default(),
        expense_payment_method: parsed.payment_platform.clone().unwrap_or_default(),
        expense_description: text.to_string(),
        income_counterparty: parsed.description.clone().unwrap_or_default(),
        income_payment_method: parsed.payment_platform.clone().unwrap_or_default(),
        income_description: text.to_string(),
        investment_counterparty: parsed.description.clone().unwrap_or_default(),
        investment_description: text.to_string(),
    };
    let Some(matched) = match_account_rules(
        account_rules,
        &context,
        ACCOUNT_ROLE_SOURCE,
        transaction_type.unwrap_or(TRANSACTION_SCOPE_ALL),
    ) else {
        return;
    };
    let account_id = matched.account_id.to_string();
    let account_name = accounts
        .iter()
        .find(|account| account.id == account_id)
        .map(|account| account.name.clone());
    insert_field(
        draft,
        "source_account_id",
        ReceiptDraftField {
            value: json!(account_id),
            confidence: 0.88,
            reason: "account_rule".to_string(),
            evidence: matched.matched_fields,
            label: account_name,
            unit: None,
        },
    );
}

/// 根据 tag 名称命中 OCR 文本，为草稿提供自动标签或候选标签。
#[tracing::instrument(level = "debug", skip_all)]
fn apply_tag_mapping(draft: &mut ReceiptTransactionDraft, text: &str, tags: &[ReceiptDraftTag]) {
    let matched = tags
        .iter()
        .filter(|tag| tag_name_matches_text(&tag.name, text))
        .take(5)
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return;
    }
    if matched.len() <= 3 {
        insert_field(
            draft,
            "tag_ids",
            ReceiptDraftField {
                value: Value::Array(matched.iter().map(|tag| json!(tag.id)).collect()),
                confidence: 0.82,
                reason: "tag_name_match".to_string(),
                evidence: matched.iter().map(|tag| tag.name.clone()).collect(),
                label: Some(
                    matched
                        .iter()
                        .map(|tag| tag.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                unit: None,
            },
        );
        return;
    }
    for tag in matched {
        push_candidate(
            draft,
            "tag_ids",
            ReceiptDraftField {
                value: json!([tag.id]),
                confidence: 0.61,
                reason: "tag_name_ambiguous".to_string(),
                evidence: vec![tag.name.clone()],
                label: Some(tag.name.clone()),
                unit: None,
            },
        );
    }
}

fn push_candidate(draft: &mut ReceiptTransactionDraft, key: &str, field: ReceiptDraftField) {
    if field.confidence >= CANDIDATE_THRESHOLD {
        draft
            .candidates
            .entry(key.to_string())
            .or_default()
            .push(field);
    }
}

fn category_label_matches_text(label: &str, text: &str) -> bool {
    let normalized_text = normalize_match_text(text);
    split_label_tokens(label)
        .into_iter()
        .any(|token| token.chars().count() >= 2 && normalized_text.contains(&token))
}

fn tag_name_matches_text(name: &str, text: &str) -> bool {
    let normalized_name = normalize_match_text(name);
    !normalized_name.is_empty()
        && normalized_name.chars().count() >= 2
        && normalize_match_text(text).contains(&normalized_name)
}

fn split_label_tokens(label: &str) -> Vec<String> {
    label
        .split(['/', '／', '>', '-', ' '])
        .map(normalize_match_text)
        .filter(|token| !token.is_empty())
        .collect()
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| text.contains(&normalize_match_text(needle)))
}

fn matching_keywords(text: &str, needles: &[&str]) -> Vec<String> {
    let normalized_text = normalize_match_text(text);
    let matched = needles
        .iter()
        .filter(|needle| normalized_text.contains(&normalize_match_text(needle)))
        .map(|needle| (*needle).to_string())
        .collect::<Vec<_>>();
    if matched.is_empty() {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(2)
            .map(str::to_string)
            .collect()
    } else {
        matched
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_match_text(value: &str) -> String {
    value.trim().to_lowercase()
}
