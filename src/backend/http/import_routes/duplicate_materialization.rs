// 中文导读：导入 stage2 的重复账单 materialization 辅助逻辑。
// 维护重点：handler 只调用本文件构建 preview、history materialization 和 decision group draft。
// 不变式：这里只生成预览证据和待确认计划，不直接改写正式账单。

#[derive(Debug, Clone)]
struct HistoryDuplicatePreviewPlan {
    import_bill_key: String,
    import_template_id: Option<i64>,
    history_bill_id: i64,
    group_key: String,
    preview_draft: ImportPreviewDraft,
    materialization: ImportHistoryMaterializationDraft,
}

struct ImportMatchDecisionGroupInput<'a> {
    session_id: &'a str,
    duplicate_groups: &'a [DuplicateGroup],
    transfer_pairs: &'a [TransferPair],
    templates: &'a [bill_analyser_db::ImportParserTemplateRow],
    standard_rows: &'a [bill_analyser_db::ImportStandardRow],
    preview_rows: &'a [ImportPreviewRow],
    history_duplicate_plan: &'a [HistoryDuplicatePreviewPlan],
    history_transfer_plan: &'a [HistoryTransferPreviewPlan],
}

fn build_history_duplicate_preview_plan(
    imported_bills: &[DedupBill],
    history_bills: &[ImportHistoryBillRow],
) -> Vec<HistoryDuplicatePreviewPlan> {
    if imported_bills.is_empty() || history_bills.is_empty() {
        return Vec::new();
    }
    let candidates = find_import_reconciliation_candidates(
        imported_bills,
        &history_bills
            .iter()
            .map(|row| row.bill.clone())
            .collect::<Vec<_>>(),
    );
    let imported_by_key = imported_bills
        .iter()
        .map(|bill| (import_reconciliation_key_for_bill(bill), bill))
        .collect::<HashMap<_, _>>();
    let history_by_id = history_bills
        .iter()
        .map(|row| (row.history_bill_id.to_string(), row))
        .collect::<HashMap<_, _>>();
    let mut used_import_keys = HashSet::new();
    let mut used_history_ids = HashSet::new();
    let mut plans = Vec::new();

    for candidate in candidates {
        if candidate.candidate_type != ReconciliationCandidateType::Duplicate {
            continue;
        }
        let Some(imported_bill) = imported_by_key.get(&candidate.import_bill_key).copied() else {
            continue;
        };
        let Some(history_bill_id) = candidate.existing_bill_id.as_deref() else {
            continue;
        };
        let Some(history_bill) = history_by_id.get(history_bill_id).copied() else {
            continue;
        };
        if !same_amount_direction(imported_bill, &history_bill.bill)
            || !used_import_keys.insert(candidate.import_bill_key.clone())
            || !used_history_ids.insert(history_bill.history_bill_id)
        {
            continue;
        }

        let preview_draft = preview_draft_from_history_duplicate(
            ImportHistoryDuplicatePreviewInput {
                imported_bill,
                history_bill,
                candidate_id: &candidate.candidate_id,
                group_key: &candidate.group_key,
                time_diff_seconds: candidate.time_diff_seconds,
                score_percent: candidate.score_percent,
                level: &candidate.level,
                reason: &candidate.reason,
            },
        );
        plans.push(HistoryDuplicatePreviewPlan {
            import_bill_key: candidate.import_bill_key.clone(),
            import_template_id: imported_bill
                .template_id
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok()),
            history_bill_id: history_bill.history_bill_id,
            group_key: candidate.group_key.clone(),
            materialization: ImportHistoryMaterializationDraft {
                history_bill_id: history_bill.history_bill_id,
                history_bill_version: history_bill.history_bill_version,
                materialized_payload: json!({
                    "candidate_id": &candidate.candidate_id,
                    "group_key": &candidate.group_key,
                    "operation": "update_history",
                    "history_snapshot": &history_bill.snapshot,
                    "import_source_ids": imported_bill.dedup_source_ids(),
                    "merged_preview": &preview_draft,
                }),
                rewrite_reason: candidate.reason.clone(),
            },
            preview_draft,
        });
    }

    plans
}

fn same_amount_direction(left: &DedupBill, right: &DedupBill) -> bool {
    let left_cents = i128::from(left.amount.to_cents());
    let right_cents = i128::from(right.amount.to_cents());
    (left_cents - right_cents).abs() <= 1
        && ((left_cents >= 0 && right_cents >= 0) || (left_cents < 0 && right_cents < 0))
}

fn import_reconciliation_key_for_bill(bill: &DedupBill) -> String {
    if let Some(preview_id) = &bill.preview_id {
        if !preview_id.trim().is_empty() {
            return format!("preview:{}", preview_id.trim());
        }
    }
    if let (Some(session_id), Some(template_id)) = (&bill.session_id, &bill.template_id) {
        if !session_id.trim().is_empty() && !template_id.trim().is_empty() {
            return format!(
                "session:{}:template:{}",
                session_id.trim(),
                template_id.trim()
            );
        }
    }
    format!(
        "fallback:{}|{}|{}|{}",
        bill.date,
        bill.amount.to_cents(),
        bill.parser_id,
        bill.counterparty
    )
}

fn build_import_match_decision_groups(
    input: ImportMatchDecisionGroupInput<'_>,
) -> Vec<ImportDecisionGroupDraft> {
    let template_standard_rows =
        build_template_standard_row_map(input.templates, input.standard_rows);
    let preview_by_source_id = build_preview_by_source_id(input.preview_rows);
    let templates_by_index = input.templates.iter().collect::<Vec<_>>();
    let mut groups = input
        .duplicate_groups
        .iter()
        .filter_map(|group| {
            build_same_batch_decision_group(
                input.session_id,
                group,
                &templates_by_index,
                &template_standard_rows,
                &preview_by_source_id,
            )
        })
        .collect::<Vec<_>>();
    groups.extend(input.transfer_pairs.iter().filter_map(|pair| {
        build_same_batch_transfer_decision_group(
            input.session_id,
            pair,
            &templates_by_index,
            &template_standard_rows,
            &preview_by_source_id,
        )
    }));
    groups.extend(build_history_duplicate_decision_groups(
        input.history_duplicate_plan,
        &template_standard_rows,
        input.preview_rows,
    ));
    groups.extend(build_history_transfer_decision_groups(
        input.history_transfer_plan,
        &template_standard_rows,
        input.preview_rows,
    ));
    groups
}

fn refresh_history_duplicate_materialization_payloads(
    history_plan: &mut [HistoryDuplicatePreviewPlan],
    preview_drafts: &[ImportPreviewDraft],
) {
    for plan in history_plan {
        let Some(final_preview) = preview_drafts.iter().find(|draft| {
            draft
                .preview_matching_feedback
                .pointer("/reconciliation/group_key")
                .and_then(Value::as_str)
                == Some(plan.group_key.as_str())
        }) else {
            continue;
        };
        let Some(payload) = plan.materialization.materialized_payload.as_object_mut() else {
            continue;
        };
        payload.insert("merged_preview".to_string(), json!(final_preview));
    }
}

fn build_same_batch_decision_group(
    session_id: &str,
    group: &DuplicateGroup,
    templates_by_index: &[&bill_analyser_db::ImportParserTemplateRow],
    template_standard_rows: &HashMap<i64, i64>,
    preview_by_source_id: &HashMap<i64, i64>,
) -> Option<ImportDecisionGroupDraft> {
    if group.dedup_type.as_str() != "same_batch" || group.remove_indices.is_empty() {
        return None;
    }
    let source_ids = group
        .bill_indices
        .iter()
        .filter_map(|index| templates_by_index.get(*index).map(|template| template.id))
        .collect::<Vec<_>>();
    if source_ids.len() <= 1 {
        return None;
    }
    let base_template_id = templates_by_index.get(group.keep_index)?.id;
    let base_preview_row_id = preview_by_source_id.get(&base_template_id).copied();
    let source_chain = group
        .bill_indices
        .iter()
        .filter_map(|index| {
            let template = templates_by_index.get(*index)?;
            let role = if *index == group.keep_index {
                "base"
            } else {
                "duplicate"
            };
            Some(json!({
                "role": role,
                "template_id": template.id,
                "standard_row_id": template_standard_rows.get(&template.id).copied(),
                "parser_id": template.parser_id,
                "parser_name": parser_source_label(&template.parser_id),
                "date": template.parser_date,
                "amount": template.parser_amount,
            }))
        })
        .collect::<Vec<_>>();
    let source_label = source_chain
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let parser_name = value.get("parser_name").and_then(Value::as_str)?;
            Some(format!("来源{}: {}", index + 1, parser_name))
        })
        .collect::<Vec<_>>()
        .join(" | ");
    let members = group
        .bill_indices
        .iter()
        .filter_map(|index| {
            let template = templates_by_index.get(*index)?;
            Some(ImportDecisionGroupMemberDraft {
                preview_row_id: if *index == group.keep_index {
                    base_preview_row_id
                } else {
                    None
                },
                standard_row_id: template_standard_rows.get(&template.id).copied(),
                history_bill_id: None,
                member_role: if *index == group.keep_index {
                    "base".to_string()
                } else {
                    "duplicate".to_string()
                },
                parser_name: parser_source_label(&template.parser_id).into_owned(),
                metadata: json!({
                    "template_id": template.id,
                    "parser_id": template.parser_id,
                    "dedup_type": group.dedup_type.as_str(),
                }),
            })
        })
        .collect::<Vec<_>>();
    Some(ImportDecisionGroupDraft {
        group_type: "duplicate".to_string(),
        group_key: format!(
            "same_batch:{}:{}",
            session_id,
            source_ids
                .iter()
                .map(i64::to_string)
                .collect::<Vec<_>>()
                .join("-")
        ),
        decision_status: if base_preview_row_id.is_some() {
            "merged".to_string()
        } else {
            "pending".to_string()
        },
        base_preview_row_id,
        signal_payload: json!({
            "signal": "duplicate",
            "dedup_type": group.dedup_type.as_str(),
            "reason": group.reason,
            "source_ids": source_ids,
            "source_chain": source_chain,
            "source_label": source_label,
        }),
        members,
    })
}

fn build_history_duplicate_decision_groups(
    history_plan: &[HistoryDuplicatePreviewPlan],
    template_standard_rows: &HashMap<i64, i64>,
    preview_rows: &[ImportPreviewRow],
) -> Vec<ImportDecisionGroupDraft> {
    history_plan
        .iter()
        .filter_map(|plan| {
            let preview = preview_rows.iter().find(|row| {
                row.preview_matching_feedback
                    .pointer("/reconciliation/group_key")
                    .and_then(Value::as_str)
                    == Some(plan.group_key.as_str())
            })?;
            let standard_row_id = plan
                .import_template_id
                .and_then(|template_id| template_standard_rows.get(&template_id).copied());
            Some(ImportDecisionGroupDraft {
                group_type: "historical_duplicate".to_string(),
                group_key: plan.group_key.clone(),
                decision_status: "pending".to_string(),
                base_preview_row_id: Some(preview.id),
                signal_payload: json!({
                    "signal": "historical_duplicate",
                    "history_bill_id": plan.history_bill_id,
                    "planned_operation": "update_history",
                    "preview_row_id": preview.id,
                    "notice": "将改写/合并历史账单",
                }),
                members: vec![
                    ImportDecisionGroupMemberDraft {
                        preview_row_id: Some(preview.id),
                        standard_row_id: None,
                        history_bill_id: Some(plan.history_bill_id),
                        member_role: "history_base".to_string(),
                        parser_name: "history_db".to_string(),
                        metadata: json!({
                            "planned_operation": "update_history",
                        }),
                    },
                    ImportDecisionGroupMemberDraft {
                        preview_row_id: Some(preview.id),
                        standard_row_id,
                        history_bill_id: None,
                        member_role: "import_duplicate".to_string(),
                        parser_name: "import".to_string(),
                        metadata: json!({
                            "template_id": plan.import_template_id,
                        }),
                    },
                ],
            })
        })
        .collect()
}

fn build_preview_by_source_id(preview_rows: &[ImportPreviewRow]) -> HashMap<i64, i64> {
    let mut map = HashMap::new();
    for row in preview_rows {
        for source_id in &row.dedup_source_ids {
            map.entry(*source_id).or_insert(row.id);
        }
    }
    map
}

fn build_template_standard_row_map(
    templates: &[bill_analyser_db::ImportParserTemplateRow],
    standard_rows: &[bill_analyser_db::ImportStandardRow],
) -> HashMap<i64, i64> {
    let mut standard_by_key: HashMap<String, Vec<i64>> = HashMap::new();
    for row in standard_rows {
        standard_by_key
            .entry(standard_row_match_key(
                &row.occurred_at,
                row.amount_cents,
                &row.parser_id,
                &row.merchant,
                &row.payment_method,
                &row.description,
            ))
            .or_default()
            .push(row.id);
    }
    let mut result = HashMap::new();
    for template in templates {
        let amount_cents = (template.parser_amount * 100.0).round() as i64;
        let key = standard_row_match_key(
            &template.parser_date,
            amount_cents,
            &template.parser_id,
            &template.parser_counterparty,
            &template.parser_payment_method,
            &template.parser_description,
        );
        if let Some(ids) = standard_by_key.get_mut(&key) {
            if let Some(id) = ids.first().copied() {
                result.insert(template.id, id);
                ids.remove(0);
            }
        }
    }
    result
}

fn standard_row_match_key(
    date: &str,
    amount_cents: i64,
    parser_id: &str,
    counterparty: &str,
    payment_method: &str,
    description: &str,
) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        date.trim(),
        amount_cents,
        parser_id.trim(),
        counterparty.trim(),
        payment_method.trim(),
        description.trim()
    )
}
