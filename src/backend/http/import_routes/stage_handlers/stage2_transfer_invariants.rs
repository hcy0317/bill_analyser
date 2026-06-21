#[tracing::instrument(level = "debug", skip_all)]
fn ensure_base_matching_feedback(draft: &mut ImportPreviewDraft) {
    let parser_id = draft.preview_parser_id.clone();
    let parser_tags = draft.preview_parser_tags.clone().unwrap_or_else(|| json!([]));
    let payment_method = draft.preview_payment_method.clone();
    let counterparty = draft.preview_counterparty.clone();
    let dedup_type = draft
        .dedup_type
        .clone()
        .unwrap_or_else(|| "remaining".to_string());
    let dedup_source_ids = draft.dedup_source_ids.clone();
    let dedup_source_count = dedup_source_ids.len();
    let feedback = matching_feedback_object_mut(draft);
    feedback.entry("parser".to_string()).or_insert_with(|| {
        json!({
            "parser_id": parser_id,
            "parser_tags": parser_tags,
            "payment_method": payment_method,
            "counterparty": counterparty,
        })
    });
    feedback.entry("dedup".to_string()).or_insert_with(|| {
        json!({
            "type": dedup_type,
            "source_ids": dedup_source_ids,
            "source_count": dedup_source_count,
        })
    });
}

fn matching_feedback_object_mut(draft: &mut ImportPreviewDraft) -> &mut Map<String, Value> {
    if !draft.preview_matching_feedback.is_object() {
        draft.preview_matching_feedback = json!({});
    }
    draft
        .preview_matching_feedback
        .as_object_mut()
        .expect("matching feedback object")
}

/// 清理未获得结构化转账授权的自动转账预览态，防止规则或模型自行创建转账。
fn demote_unauthorized_transfer_preview(
    draft: &mut ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
) -> bool {
    if preview_type_code(&draft.preview_type) != Some(4)
        || is_transfer_protected_preview(draft)
    {
        return false;
    }

    let manually_annotated = is_manually_annotated_preview(draft);
    if !manually_annotated {
        draft.preview_type = unauthorized_transfer_demoted_type(draft).to_string();
        draft.preview_destination_amount_cents = 0;
        draft.preview_destination_account_id = None;
        clear_transfer_category_identity(draft, categories);
    } else if generated_category_rule_is_auto_applied(draft) {
        clear_transfer_category_identity(draft, categories);
    }
    clear_generated_transfer_feedback(draft);
    true
}

fn unauthorized_transfer_demoted_type(draft: &ImportPreviewDraft) -> &'static str {
    if draft.preview_amount_cents < 0 {
        return "支出";
    }
    if draft.preview_destination_amount_cents > 0 {
        return "支出";
    }
    "收入"
}

fn is_manually_annotated_preview(draft: &ImportPreviewDraft) -> bool {
    draft
        .preview_matching_feedback
        .get("annotation")
        .and_then(Value::as_object)
        .and_then(|annotation| annotation.get("is_manually_annotated"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn clear_transfer_category_identity(
    draft: &mut ImportPreviewDraft,
    categories: &[ImportIntelligenceCategory],
) {
    let category_id_is_transfer = draft.category_id.is_some_and(|category_id| {
        categories
            .iter()
            .any(|category| category.id == category_id && category.type_code == 4)
    });
    let category_text_is_transfer = categories.iter().any(|category| {
        category.type_code == 4
            && category.main_category.trim() == draft.preview_main_category.trim()
            && category.sub_category.trim() == draft.preview_sub_category.trim()
    });

    if category_id_is_transfer || category_text_is_transfer {
        draft.category_id = None;
        draft.preview_main_category.clear();
        draft.preview_sub_category.clear();
    }
}

/// 移除未授权自动转账产生的 feedback family，只保留人工或结构化匹配允许的证据。
fn clear_generated_transfer_feedback(draft: &mut ImportPreviewDraft) {
    let category_rule_auto_applied = generated_category_rule_is_auto_applied(draft);
    let Some(feedback) = draft.preview_matching_feedback.as_object_mut() else {
        return;
    };
    feedback.remove("transfer");
    feedback.remove("stage2_baseline");
    feedback.remove("identity_validation");
    if category_rule_auto_applied {
        feedback.remove("category_rule");
    }
    if let Some(account_rule) = feedback.get_mut("account_rule").and_then(Value::as_object_mut) {
        account_rule.remove("destination");
        if let Some(source) = account_rule.get_mut("source").and_then(Value::as_object_mut) {
            if source
                .get("transaction_type")
                .and_then(Value::as_str)
                .is_some_and(|transaction_type| {
                    transaction_type.trim().eq_ignore_ascii_case("transfer")
                        || transaction_type.trim() == "转账"
                })
            {
                source.remove("transaction_type");
            }
        }
        if account_rule.is_empty() {
            feedback.remove("account_rule");
        }
    }
    if let Some(account) = feedback.get_mut("account").and_then(Value::as_object_mut) {
        account.remove("destination_account_id");
        account.remove("destination_account_name");
    }
    if feedback
        .get("annotation")
        .and_then(Value::as_object)
        .and_then(|annotation| annotation.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|annotation_type| annotation_type == "transfer_account_direction")
    {
        feedback.remove("annotation");
    }
}

fn generated_category_rule_is_auto_applied(draft: &ImportPreviewDraft) -> bool {
    draft
        .preview_matching_feedback
        .get("category_rule")
        .and_then(Value::as_object)
        .is_some_and(|category_rule| {
            category_rule
                .get("review_status")
                .and_then(Value::as_str)
                .is_some_and(|status| status.eq_ignore_ascii_case("auto_applied"))
        })
}
