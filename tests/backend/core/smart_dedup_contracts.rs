use bill_analyser_core::primitives::Money;
use bill_analyser_core::{
    find_cross_batch_transfer_pairs, find_database_duplicates,
    find_import_reconciliation_candidates, DedupBill, DeduplicationType,
    ReconciliationCandidateType, SmartDeduplicationEngine,
};
use serde_json::json;
use std::time::{Duration, Instant};

fn money(yuan: &str) -> Money {
    Money::from_yuan_str(yuan).unwrap()
}

fn bill(parser_id: &str, date: &str, amount: &str) -> DedupBill {
    DedupBill {
        date: date.to_string(),
        amount: money(amount),
        transaction_type: if amount.starts_with('-') {
            "支出".to_string()
        } else {
            "收入".to_string()
        },
        source_account_id: parser_id.to_string(),
        parser_id: parser_id.to_string(),
        source: parser_id.to_string(),
        counterparty: "测试商户".to_string(),
        payment_method: parser_id.to_string(),
        description: "测试交易".to_string(),
        ..Default::default()
    }
}

#[test]
fn exact_duplicates_keep_highest_source_priority() {
    let mut wechat = bill("wechat", "2026-01-02 08:00:00", "-18.60");
    let mut icbc = bill("icbc", "2026-01-02 08:00:00", "-18.60");
    wechat.counterparty = "早餐店".to_string();
    icbc.counterparty = "早餐店".to_string();
    wechat.description = "早餐".to_string();
    icbc.description = "早餐".to_string();

    let result = SmartDeduplicationEngine.process(vec![icbc, wechat]);

    assert_eq!(result.original_count, 2);
    assert_eq!(result.removed_count, 1);
    assert_eq!(
        result.duplicate_groups[0].dedup_type,
        DeduplicationType::Exact
    );
    assert_eq!(result.duplicate_groups[0].keep_index, 1);
    assert_eq!(result.kept_bills[0].parser_id, "wechat");
}

#[test]
fn platform_bank_duplicate_keeps_platform_but_transfer_intent_pairs_transfer() {
    let mut platform = bill("alipay", "2026-01-02 08:00:00", "-20.00");
    let mut bank = bill("abc", "2026-01-02 08:00:20", "-20.00");
    platform.counterparty = "便利店".to_string();
    bank.counterparty = "便利店".to_string();

    let result = SmartDeduplicationEngine.process(vec![platform, bank]);

    assert_eq!(result.removed_count, 1);
    assert_eq!(
        result.duplicate_groups[0].dedup_type,
        DeduplicationType::PlatformBank
    );
    assert_eq!(result.kept_bills[0].parser_id, "alipay");
    assert_eq!(
        result.kept_bills[0].dedup_type.as_deref(),
        Some("platform_bank")
    );

    let mut outgoing = bill("wechat", "2026-01-02 09:00:00", "-100.00");
    let mut incoming = bill("icbc", "2026-01-02 09:00:10", "100.00");
    outgoing.description = "转账到银行卡".to_string();
    incoming.description = "银行卡转入".to_string();

    let transfer_result = SmartDeduplicationEngine.process(vec![outgoing, incoming]);

    assert!(transfer_result.duplicate_groups.is_empty());
    assert_eq!(transfer_result.transfer_pairs.len(), 1);
    assert_eq!(transfer_result.kept_bills[0].transaction_type, "转账");
    assert_eq!(
        transfer_result.kept_bills[0].dedup_type.as_deref(),
        Some("transfer")
    );
    assert_eq!(
        transfer_result.kept_bills[0].transfer_pair_order.as_deref(),
        Some("outgoing_first")
    );
    assert_eq!(transfer_result.kept_bills[0].destination_parser_id, "icbc");
    assert_eq!(transfer_result.kept_bills[0].transfer_pair_sources.len(), 2);
}

#[test]
fn similar_duplicates_and_split_groups_preserve_python_contract_edges() {
    let mut wechat = bill("wechat", "2026-01-03 10:00:00", "-8.80");
    let mut alipay = bill("alipay", "2026-01-03 10:00:20", "-8.80");
    wechat.counterparty = "咖啡店".to_string();
    alipay.counterparty = "咖啡店".to_string();
    wechat.template_id = Some("tpl-wechat".to_string());
    alipay.template_id = Some("tpl-alipay".to_string());

    let similar_result = SmartDeduplicationEngine.process(vec![wechat, alipay]);

    assert_eq!(similar_result.removed_count, 1);
    assert_eq!(
        similar_result.duplicate_groups[0].dedup_type,
        DeduplicationType::Similar
    );
    assert_eq!(
        similar_result.kept_bills[0].dedup_type.as_deref(),
        Some("similar")
    );
    assert_eq!(
        similar_result.kept_bills[0].dedup_source_ids(),
        vec!["tpl-wechat".to_string(), "tpl-alipay".to_string()]
    );

    let total = DedupBill {
        source_account_id: "bank-total".to_string(),
        parser_id: "icbc".to_string(),
        source: "icbc".to_string(),
        counterparty: "合并商户".to_string(),
        description: "总账单".to_string(),
        ..bill("", "2026-01-03 18:00:00", "-30.00")
    };
    let split_a = DedupBill {
        source_account_id: "wallet-splits".to_string(),
        parser_id: "wechat".to_string(),
        source: "wechat".to_string(),
        description: "分账 A".to_string(),
        ..bill("", "2026-01-03 18:00:05", "-10.00")
    };
    let split_b = DedupBill {
        source_account_id: "wallet-splits".to_string(),
        parser_id: "wechat".to_string(),
        source: "wechat".to_string(),
        description: "分账 B".to_string(),
        ..bill("", "2026-01-03 18:00:10", "-20.00")
    };

    let split_result = SmartDeduplicationEngine.process(vec![total, split_a, split_b]);

    assert_eq!(split_result.split_groups.len(), 1);
    assert_eq!(split_result.split_groups[0].total_index, 0);
    assert_eq!(split_result.split_groups[0].split_indices, vec![1, 2]);
    assert_eq!(
        serde_json::to_value(&split_result.split_groups[0]).unwrap()["total_amount"],
        -30.0
    );
    assert_eq!(split_result.kept_bills.len(), 2);
    assert!(split_result
        .kept_bills
        .iter()
        .all(|item| item.dedup_type.as_deref() == Some("split")));
}

#[test]
fn income_split_groups_preserve_split_contract_edges() {
    let total = DedupBill {
        source_account_id: "bank-total".to_string(),
        parser_id: "icbc".to_string(),
        source: "icbc".to_string(),
        counterparty: "合并收入".to_string(),
        description: "收入总账单".to_string(),
        ..bill("", "2026-01-03 19:00:00", "30.00")
    };
    let split_a = DedupBill {
        source_account_id: "wallet-splits".to_string(),
        parser_id: "wechat".to_string(),
        source: "wechat".to_string(),
        description: "收入分账 A".to_string(),
        ..bill("", "2026-01-03 19:00:05", "10.00")
    };
    let split_b = DedupBill {
        source_account_id: "wallet-splits".to_string(),
        parser_id: "wechat".to_string(),
        source: "wechat".to_string(),
        description: "收入分账 B".to_string(),
        ..bill("", "2026-01-03 19:00:10", "20.00")
    };

    let split_result = SmartDeduplicationEngine.process(vec![total, split_a, split_b]);

    assert_eq!(split_result.split_groups.len(), 1);
    assert_eq!(split_result.split_groups[0].total_index, 0);
    assert_eq!(split_result.split_groups[0].split_indices, vec![1, 2]);
    assert_eq!(
        serde_json::to_value(&split_result.split_groups[0]).unwrap()["total_amount"],
        30.0
    );
    assert_eq!(split_result.kept_bills.len(), 2);
    assert!(split_result
        .kept_bills
        .iter()
        .all(|item| item.dedup_type.as_deref() == Some("split")));
}

#[test]
fn database_duplicate_and_cross_batch_transfer_helpers_match_existing_contract() {
    let mut imported = bill("wechat", "2026-01-04 08:00:00", "-12.50");
    let mut existing = bill("icbc", "2026-01-04 08:03:00", "12.50");
    imported.counterparty = "转账".to_string();
    existing.counterparty = "转账".to_string();
    existing.id = Some("db-1".to_string());

    let mut imported_for_cross_batch = vec![imported.clone()];
    let cross_batch =
        find_cross_batch_transfer_pairs(&mut imported_for_cross_batch, &[existing.clone()]);

    assert_eq!(cross_batch.len(), 1);
    assert_eq!(cross_batch[0].imported_index, 0);
    assert_eq!(cross_batch[0].existing_bill_id.as_deref(), Some("db-1"));
    assert_eq!(
        imported_for_cross_batch[0].dedup_type.as_deref(),
        Some("transfer_cross_batch")
    );
    assert_eq!(
        imported_for_cross_batch[0].cross_batch_db_id.as_deref(),
        Some("db-1")
    );

    let mut imported_duplicate = bill("alipay", "2026-01-04 09:00:00", "-66.00");
    let mut existing_duplicate = bill("abc", "2026-01-04 09:04:00", "66.00");
    imported_duplicate.counterparty = "商户订单".to_string();
    existing_duplicate.counterparty = "商户订单".to_string();
    existing_duplicate.id = Some("db-2".to_string());

    let mut imported_for_db = vec![imported_duplicate];
    let db_matches = find_database_duplicates(&mut imported_for_db, &[existing_duplicate]);

    assert_eq!(db_matches.len(), 1);
    assert!(db_matches[0].is_platform_bank_pair);
    assert_eq!(
        db_matches[0].dedup_type,
        DeduplicationType::DatabaseDuplicate
    );
    assert!(imported_for_db[0].removed);
    assert_eq!(
        imported_for_db[0].duplicate_of_db_id.as_deref(),
        Some("db-2")
    );
}

#[test]
fn python_shaped_hidden_fields_round_trip_and_match_dedup_source_ids() {
    let value = json!({
        "date": "2026-01-05 12:00:00",
        "amount": -20.50,
        "type": "支出",
        "source_account_id": 7,
        "_parser_id": "wechat",
        "_template_id": 101,
        "_parser_tags": ["parser:wechat", "account:wallet"],
        "_dedup_type": "similar",
        "_removed": false,
        "_merged_template_ids": [102, "103"],
        "counterparty": "商户",
        "description": "午餐"
    });

    let bill: DedupBill = serde_json::from_value(value).unwrap();

    assert_eq!(bill.amount, money("-20.50"));
    assert_eq!(bill.source_account_id, "7");
    assert_eq!(bill.parser_id, "wechat");
    assert_eq!(bill.template_id.as_deref(), Some("101"));
    assert_eq!(
        bill.dedup_source_ids(),
        vec!["101".to_string(), "102".to_string(), "103".to_string()]
    );

    let serialized = serde_json::to_value(&bill).unwrap();
    assert_eq!(serialized["_parser_id"], "wechat");
    assert_eq!(serialized["_template_id"], "101");
    assert_eq!(serialized["amount"], -20.5);
}

#[test]
fn same_source_large_import_skips_cross_source_quadratic_work() {
    let bills = (0..4_000)
        .map(|index| {
            let day = (index % 28) + 1;
            let hour = (index / 28) % 24;
            let minute = (index / (28 * 24)) % 60;
            let second = index % 60;
            let amount_cents = 100 + (index % 50_000);
            let amount = format!("-{}.{:02}", amount_cents / 100, amount_cents % 100);
            let mut item = bill(
                "alipay",
                &format!("2026-02-{day:02} {hour:02}:{minute:02}:{second:02}"),
                &amount,
            );
            item.counterparty = format!("商户{index}");
            item.payment_method = "支付宝".to_string();
            item.description = format!("批量导入交易{index}");
            item
        })
        .collect::<Vec<_>>();

    let started_at = Instant::now();
    let result = SmartDeduplicationEngine.process(bills);
    let elapsed = started_at.elapsed();

    assert_eq!(result.original_count, 4_000);
    assert_eq!(result.kept_bills.len(), 4_000);
    assert!(result.transfer_pairs.is_empty());
    assert!(result.duplicate_groups.is_empty());
    assert!(result.split_groups.is_empty());
    assert!(
        elapsed < Duration::from_secs(5),
        "same-source import dedup should skip cross-source quadratic passes, elapsed={elapsed:?}"
    );
}

#[test]
fn dense_multi_source_same_timestamp_import_uses_amount_narrowed_candidates() {
    let sources = ["alipay", "wechat", "icbc", "abc"];
    let bills = (0..4_000)
        .map(|index| {
            let source = sources[index % sources.len()];
            let amount_cents = 100_000 + index * 10;
            let amount = format!("-{}.{:02}", amount_cents / 100, amount_cents % 100);
            let mut item = bill(source, "2026-02-01 12:00:00", &amount);
            item.counterparty = format!("密集商户{index}");
            item.payment_method = source.to_string();
            item.description = format!("同一秒导入交易{index}");
            item
        })
        .collect::<Vec<_>>();

    let started_at = Instant::now();
    let result = SmartDeduplicationEngine.process(bills);
    let elapsed = started_at.elapsed();

    assert_eq!(result.original_count, 4_000);
    assert_eq!(result.kept_bills.len(), 4_000);
    assert!(result.transfer_pairs.is_empty());
    assert!(result.duplicate_groups.is_empty());
    assert!(result.split_groups.is_empty());
    assert!(
        elapsed < Duration::from_secs(5),
        "dense multi-source same-timestamp import should stay near-linear, elapsed={elapsed:?}"
    );
}

#[test]
fn db_and_cross_batch_helpers_skip_removed_and_in_batch_transfers() {
    let mut removed = bill("wechat", "2026-01-06 08:00:00", "-11.00");
    let mut already_transfer = bill("wechat", "2026-01-06 09:00:00", "-22.00");
    let mut existing = bill("icbc", "2026-01-06 08:01:00", "11.00");
    let mut transfer_existing = bill("abc", "2026-01-06 09:01:00", "22.00");
    removed.removed = true;
    already_transfer.dedup_type = Some("transfer".to_string());
    existing.id = Some("db-removed".to_string());
    transfer_existing.id = Some("db-transfer".to_string());

    let mut imported_for_db = vec![removed.clone()];
    assert!(find_database_duplicates(&mut imported_for_db, &[existing]).is_empty());

    let mut imported_for_cross_batch = vec![already_transfer];
    assert!(
        find_cross_batch_transfer_pairs(&mut imported_for_cross_batch, &[transfer_existing])
            .is_empty()
    );
}

#[test]
fn reconciliation_candidate_contract_classifies_duplicates_and_transfers() {
    let mut imported_duplicate = bill("wechat", "2026-01-07 10:00:00", "-30.00");
    let mut existing_duplicate = bill("icbc", "2026-01-07 10:00:20", "-30.00");
    imported_duplicate.counterparty = "咖啡店".to_string();
    existing_duplicate.counterparty = "咖啡店".to_string();
    imported_duplicate.session_id = Some("session-1".to_string());
    imported_duplicate.template_id = Some("501".to_string());
    existing_duplicate.id = Some("db-dup".to_string());

    let candidates =
        find_import_reconciliation_candidates(&[imported_duplicate.clone()], &[existing_duplicate]);

    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].candidate_type,
        ReconciliationCandidateType::Duplicate
    );
    assert_eq!(
        candidates[0].import_bill_key,
        "session:session-1:template:501"
    );
    assert_eq!(candidates[0].existing_bill_id.as_deref(), Some("db-dup"));
    assert_eq!(
        serde_json::to_value(&candidates[0]).unwrap()["amount_abs"],
        30.0
    );

    let mut imported_transfer = bill("alipay", "2026-01-07 11:00:00", "-30.00");
    let mut existing_transfer = bill("abc", "2026-01-07 11:00:20", "30.00");
    imported_transfer.description = "转账到银行卡".to_string();
    existing_transfer.id = Some("db-transfer".to_string());

    let transfer_candidates =
        find_import_reconciliation_candidates(&[imported_transfer], &[existing_transfer]);

    assert_eq!(transfer_candidates.len(), 1);
    assert_eq!(
        transfer_candidates[0].candidate_type,
        ReconciliationCandidateType::Transfer
    );
}
