use crate::{parser_source_label, StandardBill};

/// 生成 dedicated parser 候选的中文可读证据，供导入预览和调试定位 parser 命中依据。
pub(super) fn parser_evidence(parser_id: &str, bills: &[StandardBill]) -> Vec<String> {
    let mut evidence = vec![
        format!("parsed_count={}", bills.len()),
        format!("parser_label={}", parser_source_label(parser_id)),
    ];
    if bills.iter().any(|bill| {
        bill.parser_tags
            .iter()
            .any(|tag| tag == &format!("parser:{parser_id}"))
    }) {
        evidence.push(format!("parser_tag=parser:{parser_id}"));
    }
    if bills.iter().any(|bill| !bill.date.trim().is_empty()) {
        evidence.push("has_transaction_time=true".to_string());
    }
    if bills.iter().any(|bill| bill.amount.to_cents() != 0) {
        evidence.push("has_amount=true".to_string());
    }
    evidence
}
