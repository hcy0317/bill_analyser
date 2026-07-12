#[cfg(test)]
mod preview_draft_tests {
    use super::*;

    fn transfer_bill(parser_id: &str, amount: &str, account_id: &str) -> DedupBill {
        DedupBill {
            id: Some(format!("db-{parser_id}")),
            date: "2026-05-04 10:00:05".to_string(),
            amount: Money::from_yuan_str(amount).unwrap(),
            transaction_type: if amount.starts_with('-') {
                "支出".to_string()
            } else {
                "收入".to_string()
            },
            source_account_id: account_id.to_string(),
            parser_id: parser_id.to_string(),
            source: parser_id.to_string(),
            counterparty: format!("{parser_id} counterparty"),
            payment_method: format!("{parser_id} payment"),
            description: format!("{parser_id} description"),
            main_category: "一般转账".to_string(),
            sub_category: "电子支付".to_string(),
            ..DedupBill::default()
        }
    }

    fn history_row(bill: DedupBill) -> ImportHistoryBillRow {
        ImportHistoryBillRow {
            history_bill_id: 88,
            history_bill_version: 3,
            snapshot: serde_json::json!({
                "id": 88,
                "amount": bill.amount.to_yuan_string(),
            }),
            bill,
        }
    }

    #[test]
    fn current_bill_transfer_preview_uses_stored_outgoing_when_import_is_income() {
        let imported = DedupBill {
            template_id: Some("901".to_string()),
            session_id: Some("session-transfer".to_string()),
            ..transfer_bill("wechat", "300.00", "2002")
        };
        let stored = history_row(transfer_bill("icbc", "-300.00", "1001"));

        let draft = preview_draft_from_history_transfer(ImportHistoryTransferPreviewInput {
            imported_bill: &imported,
            history_bill: &stored,
            candidate_id: "candidate",
            group_key: "group",
            time_diff_seconds: 5,
            score_percent: 99,
            level: "high",
            reason: "opposite_amount|same_day|time_close",
        });

        assert_eq!(draft.preview_type, "转账");
        assert_eq!(draft.preview_source_account_id, Some(1001));
        assert_eq!(draft.preview_destination_account_id, Some(2002));
        assert_eq!(draft.dedup_type.as_deref(), Some("transfer_cross_batch"));
        assert_eq!(
            draft.preview_matching_feedback["reconciliation"]["history_role"],
            "outgoing"
        );
        assert_eq!(
            draft.preview_matching_feedback["transfer"]["source_chain"][0]["description"],
            "icbc description"
        );
        assert_eq!(
            draft.preview_matching_feedback["transfer"]["source_chain"][1]["description"],
            "wechat description"
        );
    }

    #[test]
    fn dedup_preview_draft_preserves_amount_cents_and_transfer_destination_cents() {
        let bill = DedupBill {
            transaction_type: "转账".to_string(),
            destination_account_id: Some("2002".to_string()),
            template_id: Some("901".to_string()),
            merged_from: vec![bill_analyser_core::MergedBillSource {
                source: "alipay".to_string(),
                date: "2026-05-04 10:00:05".to_string(),
                amount: Some("300.00".to_string()),
                template_id: Some("902".to_string()),
            }],
            ..transfer_bill("icbc", "-300.00", "1001")
        };

        let draft = preview_draft_from_dedup_bill(&bill);

        assert_eq!(draft.preview_amount_cents, 30000);
        assert_eq!(draft.preview_destination_amount_cents, 30000);
        assert_eq!(draft.preview_source_account_id, Some(1001));
        assert_eq!(draft.preview_destination_account_id, Some(2002));
        assert_eq!(
            draft.preview_matching_feedback["dedup"]["source_chain"][0]["amount_cents"],
            serde_json::json!(-30000)
        );
        assert_eq!(
            draft.preview_matching_feedback["dedup"]["source_chain"][1]["source_amount_text"],
            "300.00"
        );
    }

    #[test]
    fn history_duplicate_preview_preserves_current_bill_cents_payloads() {
        let imported = transfer_bill("wechat", "-18.50", "2002");
        let stored = history_row(transfer_bill("icbc", "-18.50", "1001"));

        let draft = preview_draft_from_history_duplicate(ImportHistoryDuplicatePreviewInput {
            imported_bill: &imported,
            history_bill: &stored,
            candidate_id: "candidate",
            group_key: "group",
            time_diff_seconds: 2,
            score_percent: 96,
            level: "high",
            reason: "same_amount|same_day",
        });

        assert_eq!(draft.preview_amount_cents, 1850);
        assert_eq!(draft.preview_destination_amount_cents, 0);
        assert_eq!(
            draft.preview_matching_feedback["dedup"]["source_chain"][0]["amount_cents"],
            serde_json::json!(-1850)
        );
        assert_eq!(
            draft.preview_matching_feedback["dedup"]["source_chain"][1]["amount_cents"],
            serde_json::json!(-1850)
        );
        assert_eq!(
            draft.preview_matching_feedback["reconciliation"]["history_summary"],
            serde_json::json!({
                "bill_id": 88,
                "date_time": "2026-05-04 10:00:05",
                "amount_cents": -1850,
                "currency": "CNY",
                "category_name": "一般转账 / 电子支付",
                "category_status": "known",
                "source_account_name": "",
                "source_account_status": "unknown",
                "destination_account_name": null,
                "destination_account_status": "unknown",
                "identity_source": "snapshot_fallback",
                "counterparty": "icbc counterparty",
                "description": "icbc description"
            })
        );
    }
}
