use bill_analyser_core::{
    aggregate_description, build_parser_tags, normalize_amount_text, normalize_parser_tags,
    normalize_transaction_type, parser_registry, parser_source_label, post_process_raw_bills,
    resolve_parser_tags, serialize_parser_tags, RawBill, StandardBill,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct ParserGoldenCase {
    sample_family: String,
    parser_id: String,
    class_name: String,
    source_label: String,
    date: String,
    expected_date: String,
    amount: String,
    expected_amount: String,
    transaction_type: String,
    expected_type: String,
    description: String,
    counterparty: String,
    payment_method: String,
    original_category: String,
    expected_tags: Vec<String>,
}

#[test]
fn parser_registry_preserves_python_detection_order_and_metadata() {
    let registry = parser_registry();
    let ids: Vec<&str> = registry.iter().map(|item| item.id).collect();

    assert_eq!(ids, ["wechat", "alipay", "icbc", "cmbc", "abc", "ccb"]);
    assert_eq!(registry[0].name, "微信支付");
    assert_eq!(registry[0].source_label, "微信");
    assert_eq!(registry[0].class_name, "WeChatParser");
    assert_eq!(registry[0].supported_extensions, [".csv", ".xlsx"]);
    assert_eq!(registry[5].channel_tag, "bank");
    assert_eq!(registry[5].source_label, "建设银行");
    assert_eq!(registry[5].supported_extensions, [".xlsx", ".xls"]);
}

#[test]
fn parser_tags_match_python_contract_defaults_and_dedupe() {
    assert_eq!(
        normalize_parser_tags(["parser:WeChat", "channel:wallet", "parser:wechat"]),
        ["parser:wechat", "channel:wallet"]
    );
    assert_eq!(
        build_parser_tags("cmbc", "", ""),
        ["parser:cmbc", "channel:bank"]
    );
    assert_eq!(build_parser_tags("", "微信零钱", ""), ["channel:wallet"]);
    assert_eq!(
        resolve_parser_tags(Some(&json!("parser:alipay, channel:wallet")), "", "", "",),
        ["parser:alipay", "channel:wallet"]
    );
    assert_eq!(
        resolve_parser_tags(
            Some(&json!(["parser:WeChat", 123, 0, true, false, null])),
            "",
            "",
            "",
        ),
        ["parser:wechat", "123", "true"]
    );
    assert_eq!(
        serialize_parser_tags(
            Some(&json!(["parser:WeChat", "channel:wallet"])),
            "",
            "",
            "",
        ),
        "[\"parser:wechat\",\"channel:wallet\"]"
    );
}

#[test]
fn standard_bill_json_round_trip_matches_existing_keys() {
    let bill = StandardBill::from_json_value(&json!({
        "date": "2026-03-28 00:00:00",
        "amount": "12.34",
        "type": "支出",
        "description": "早餐",
        "source_account_id": "wechat",
        "payment_method": "零钱",
        "parser_tags": ["parser:wechat", "channel:wallet"]
    }));
    let serialized = serde_json::to_value(&bill).unwrap();

    assert_eq!(bill.amount.to_yuan_string(), "12.34");
    assert_eq!(serialized["amount"], json!(12.34));
    assert_eq!(serialized["type"], "支出");
    assert_eq!(serialized["description"], "早餐");
    assert_eq!(
        serialized["parser_tags"],
        json!(["parser:wechat", "channel:wallet"])
    );
}

#[test]
fn base_normalization_matches_python_parser_helpers() {
    let mut raw = RawBill {
        date: "2026/01/02".to_string(),
        amount: "¥1,234.50".to_string(),
        transaction_type: "支出".to_string(),
        description: "早餐".to_string(),
        counterparty: "测试早餐店".to_string(),
        remark: "早餐".to_string(),
        payment_method: "零钱".to_string(),
        ..Default::default()
    };

    assert_eq!(
        normalize_amount_text("¥1,234.50").to_yuan_string(),
        "1234.50"
    );
    assert_eq!(normalize_amount_text("not-a-number").to_cents(), 0);
    assert_eq!(normalize_transaction_type("投资理财"), "投资");
    assert_eq!(normalize_transaction_type("未知类型"), "支出");
    assert_eq!(
        aggregate_description(&raw),
        "早餐 | 测试早餐店 | 零钱 | 支出"
    );

    let processed = post_process_raw_bills("dummy", &[raw.clone()]);
    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0].date, "2026-01-02 00:00:00");
    assert_eq!(processed[0].amount.to_yuan_string(), "-1234.50");
    assert_eq!(processed[0].source_account_id, "dummy");
    assert!(processed[0]
        .parser_tags
        .contains(&"parser:dummy".to_string()));

    raw.date = "2026-01-03 09:00:00".to_string();
    raw.amount = "88.00".to_string();
    raw.transaction_type = "投资理财".to_string();
    raw.original_category = "投资理财".to_string();
    let processed = post_process_raw_bills("dummy", &[raw]);
    assert_eq!(processed[0].transaction_type, "收入");
    assert_eq!(processed[0].amount.to_yuan_string(), "88.00");
}

#[test]
fn parser_golden_cases_cover_each_python_parser_source_label_and_tags() {
    let cases: Vec<ParserGoldenCase> =
        serde_json::from_str(include_str!("fixtures/parser_golden_contracts.json")).unwrap();
    let registry = parser_registry();

    assert_eq!(cases.len(), 6);

    for case in cases {
        let info = registry
            .iter()
            .find(|item| item.id == case.parser_id)
            .unwrap_or_else(|| panic!("missing parser info for {}", case.parser_id));
        let raw_bill = RawBill {
            date: case.date.clone(),
            amount: case.amount.clone(),
            transaction_type: case.transaction_type.clone(),
            description: case.description.clone(),
            counterparty: case.counterparty.clone(),
            payment_method: case.payment_method.clone(),
            original_category: case.original_category.clone(),
            transaction_id: format!("{}-golden-transaction", case.parser_id),
            ..Default::default()
        };

        let processed = post_process_raw_bills(&case.parser_id, &[raw_bill]);

        assert_eq!(info.class_name, case.class_name, "{}", case.sample_family);
        assert_eq!(
            info.source_label, case.source_label,
            "{}",
            case.sample_family
        );
        assert_eq!(
            parser_source_label(&case.parser_id).as_ref(),
            case.source_label,
            "{}",
            case.sample_family
        );
        assert_eq!(processed.len(), 1, "{}", case.sample_family);

        let bill = &processed[0];
        assert_eq!(bill.date, case.expected_date, "{}", case.sample_family);
        assert_eq!(
            bill.amount.to_yuan_string(),
            case.expected_amount,
            "{}",
            case.sample_family
        );
        assert_eq!(
            bill.transaction_type, case.expected_type,
            "{}",
            case.sample_family
        );
        assert_eq!(
            bill.source_account_id, case.parser_id,
            "{}",
            case.sample_family
        );
        assert_eq!(
            bill.parser_tags, case.expected_tags,
            "{}",
            case.sample_family
        );
        assert!(
            bill.description.contains(&case.description),
            "{}",
            case.sample_family
        );
    }

    assert_eq!(parser_source_label("generic").as_ref(), "通用来源");
    assert_eq!(
        parser_source_label(" CustomParser ").as_ref(),
        "customparser"
    );
    assert_eq!(parser_source_label("").as_ref(), "");
}

#[test]
fn post_process_skips_missing_dates_and_zero_amounts() {
    let processed = post_process_raw_bills(
        "wechat",
        &[
            RawBill {
                amount: "1".to_string(),
                transaction_type: "支出".to_string(),
                ..Default::default()
            },
            RawBill {
                date: "2026-01-05".to_string(),
                amount: "0".to_string(),
                transaction_type: "收入".to_string(),
                ..Default::default()
            },
        ],
    );

    assert!(processed.is_empty());
}
