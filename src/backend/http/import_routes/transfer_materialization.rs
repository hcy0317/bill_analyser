// 中文导读：导入 stage2 的转账 materialization 辅助逻辑。
// 维护重点：同批转账和历史转账只生成 preview/decision 证据，不直接改写正式账单。
// 不变式：历史账单真实更新、删除和余额同步由后续确认事务切片接管。

#[derive(Debug, Clone)]
struct HistoryTransferPreviewPlan {
    import_bill_key: String,
    import_template_id: Option<i64>,
    history_bill_id: i64,
    history_role: String,
    import_role: String,
    group_key: String,
    preview_draft: ImportPreviewDraft,
    materialization: ImportHistoryMaterializationDraft,
}

fn build_history_transfer_preview_plan(
    imported_bills: &[DedupBill],
    history_bills: &[ImportHistoryBillRow],
    excluded_import_keys: &HashSet<String>,
    excluded_history_ids: &HashSet<i64>,
) -> Vec<HistoryTransferPreviewPlan> {
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
    let mut used_import_keys = excluded_import_keys.clone();
    let mut used_history_ids = excluded_history_ids.clone();
    let mut plans = Vec::new();

    for candidate in candidates {
        if candidate.candidate_type != ReconciliationCandidateType::Transfer {
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
        if !used_import_keys.insert(candidate.import_bill_key.clone())
            || !used_history_ids.insert(history_bill.history_bill_id)
        {
            continue;
        }

        let import_is_outgoing = imported_bill.amount.is_negative();
        let preview_draft =
            preview_draft_from_history_transfer(ImportHistoryTransferPreviewInput {
                imported_bill,
                history_bill,
                candidate_id: &candidate.candidate_id,
                group_key: &candidate.group_key,
                time_diff_seconds: candidate.time_diff_seconds,
                score_percent: candidate.score_percent,
                level: &candidate.level,
                reason: &candidate.reason,
            });
        plans.push(HistoryTransferPreviewPlan {
            import_bill_key: candidate.import_bill_key.clone(),
            import_template_id: imported_bill
                .template_id
                .as_deref()
                .and_then(|value| value.parse::<i64>().ok()),
            history_bill_id: history_bill.history_bill_id,
            history_role: if import_is_outgoing {
                "incoming".to_string()
            } else {
                "outgoing".to_string()
            },
            import_role: if import_is_outgoing {
                "outgoing".to_string()
            } else {
                "incoming".to_string()
            },
            group_key: candidate.group_key.clone(),
            materialization: ImportHistoryMaterializationDraft {
                history_bill_id: history_bill.history_bill_id,
                history_bill_version: history_bill.history_bill_version,
                materialized_payload: json!({
                    "candidate_id": &candidate.candidate_id,
                    "group_key": &candidate.group_key,
                    "operation": "merge_transfer_history",
                    "history_role": if import_is_outgoing { "incoming" } else { "outgoing" },
                    "import_role": if import_is_outgoing { "outgoing" } else { "incoming" },
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

fn refresh_history_transfer_materialization_payloads(
    history_plan: &mut [HistoryTransferPreviewPlan],
    preview_drafts: &[ImportPreviewDraft],
) {
    let preview_by_group_key = build_preview_draft_by_reconciliation_group_key(preview_drafts);
    for plan in history_plan {
        let Some(final_preview) = preview_by_group_key.get(&plan.group_key).copied() else {
            continue;
        };
        let Some(payload) = plan.materialization.materialized_payload.as_object_mut() else {
            continue;
        };
        payload.insert("merged_preview".to_string(), json!(final_preview));
    }
}

fn build_same_batch_transfer_decision_group(
    session_id: &str,
    pair: &TransferPair,
    templates_by_index: &[&bill_analyser_db::ImportParserTemplateRow],
    template_standard_rows: &HashMap<i64, i64>,
    preview_by_source_id: &HashMap<i64, i64>,
) -> Option<ImportDecisionGroupDraft> {
    let outgoing_template = templates_by_index.get(pair.outgoing_index)?;
    let incoming_template = templates_by_index.get(pair.incoming_index)?;
    let source_ids = vec![outgoing_template.id, incoming_template.id];
    let base_preview_row_id = preview_by_source_id.get(&outgoing_template.id).copied();
    let source_chain = [
        ("outgoing", outgoing_template),
        ("incoming", incoming_template),
    ]
    .into_iter()
    .map(|(role, template)| {
        json!({
            "role": role,
            "template_id": template.id,
            "standard_row_id": template_standard_rows.get(&template.id).copied(),
            "parser_id": template.parser_id,
            "parser_name": parser_source_label(&template.parser_id),
            "date": template.parser_date,
            "amount": template.parser_amount,
            "counterparty": template.parser_counterparty,
            "payment_method": template.parser_payment_method,
            "description": template.parser_description,
        })
    })
    .collect::<Vec<_>>();
    let source_label = format!(
        "匹配 | {} | {}",
        parser_source_label(&outgoing_template.parser_id),
        parser_source_label(&incoming_template.parser_id)
    );
    let members = [
        ("outgoing", outgoing_template),
        ("incoming", incoming_template),
    ]
    .into_iter()
    .map(|(role, template)| ImportDecisionGroupMemberDraft {
        preview_row_id: if role == "outgoing" {
            base_preview_row_id
        } else {
            None
        },
        standard_row_id: template_standard_rows.get(&template.id).copied(),
        history_bill_id: None,
        member_role: role.to_string(),
        parser_name: parser_source_label(&template.parser_id).into_owned(),
        metadata: json!({
            "template_id": template.id,
            "parser_id": template.parser_id,
            "dedup_type": "transfer",
        }),
    })
    .collect::<Vec<_>>();
    Some(ImportDecisionGroupDraft {
        group_type: "same_batch_transfer".to_string(),
        group_key: format!(
            "same_batch_transfer:{}:{}-{}",
            session_id, outgoing_template.id, incoming_template.id
        ),
        decision_status: if base_preview_row_id.is_some() {
            "matched".to_string()
        } else {
            "pending".to_string()
        },
        base_preview_row_id,
        signal_payload: json!({
            "signal": "transfer",
            "dedup_type": "transfer",
            "planned_operation": "merge_transfer_history",
            "source_ids": source_ids,
            "source_chain": source_chain,
            "source_label": source_label,
        }),
        members,
    })
}

fn build_history_transfer_decision_groups(
    history_plan: &[HistoryTransferPreviewPlan],
    template_standard_rows: &HashMap<i64, i64>,
    preview_by_reconciliation_group_key: &HashMap<String, &ImportPreviewRow>,
) -> Vec<ImportDecisionGroupDraft> {
    history_plan
        .iter()
        .filter_map(|plan| {
            let preview = preview_by_reconciliation_group_key.get(&plan.group_key).copied()?;
            let standard_row_id = plan
                .import_template_id
                .and_then(|template_id| template_standard_rows.get(&template_id).copied());
            let source_label = preview
                .preview_matching_feedback
                .pointer("/transfer/source_label")
                .and_then(Value::as_str)
                .unwrap_or("匹配 | history_db | import")
                .to_string();
            Some(ImportDecisionGroupDraft {
                group_type: "historical_transfer".to_string(),
                group_key: plan.group_key.clone(),
                decision_status: "pending".to_string(),
                base_preview_row_id: Some(preview.id),
                signal_payload: json!({
                    "signal": "historical_transfer",
                    "history_bill_id": plan.history_bill_id,
                    "planned_operation": "merge_transfer_history",
                    "preview_row_id": preview.id,
                    "source_label": source_label,
                    "notice": "将改写/合并历史账单",
                }),
                members: vec![
                    ImportDecisionGroupMemberDraft {
                        preview_row_id: Some(preview.id),
                        standard_row_id: None,
                        history_bill_id: Some(plan.history_bill_id),
                        member_role: format!("history_{}", plan.history_role),
                        parser_name: "history_db".to_string(),
                        metadata: json!({
                            "planned_operation": "merge_transfer_history",
                            "role": plan.history_role,
                        }),
                    },
                    ImportDecisionGroupMemberDraft {
                        preview_row_id: Some(preview.id),
                        standard_row_id,
                        history_bill_id: None,
                        member_role: format!("import_{}", plan.import_role),
                        parser_name: "import".to_string(),
                        metadata: json!({
                            "template_id": plan.import_template_id,
                            "role": plan.import_role,
                        }),
                    },
                ],
            })
        })
        .collect()
}

#[cfg(test)]
mod transfer_materialization_tests {
    use super::*;

    fn parser_template(id: i64, amount: f64) -> bill_analyser_db::ImportParserTemplateRow {
        bill_analyser_db::ImportParserTemplateRow {
            id,
            session_id: "session".to_string(),
            user_id: 1,
            parser_date: "2026-07-10 10:00:00".to_string(),
            parser_amount: amount,
            parser_type: "转账".to_string(),
            parser_description: "账户间转账".to_string(),
            parser_id: "fixture".to_string(),
            parser_tags: vec![],
            parser_counterparty: "本人".to_string(),
            parser_payment_method: "银行卡".to_string(),
            parser_original_type: "转账".to_string(),
            parser_original_category: String::new(),
            parser_account_id: String::new(),
            parser_is_processed: false,
            created_at: "2026-07-10 10:00:00".to_string(),
        }
    }

    #[test]
    fn same_batch_transfer_writer_uses_canonical_history_operation() {
        let outgoing = parser_template(11, -100.0);
        let incoming = parser_template(12, 100.0);
        let templates = vec![&outgoing, &incoming];
        let standard_rows = HashMap::from([(11, 21), (12, 22)]);
        let preview_rows = HashMap::from([(11, 31)]);
        let group = build_same_batch_transfer_decision_group(
            "session",
            &TransferPair {
                outgoing_index: 0,
                incoming_index: 1,
                cross_batch_db_id: None,
            },
            &templates,
            &standard_rows,
            &preview_rows,
        )
        .expect("same-batch transfer decision group");

        assert_eq!(
            group.signal_payload["planned_operation"],
            "merge_transfer_history"
        );
    }
}
