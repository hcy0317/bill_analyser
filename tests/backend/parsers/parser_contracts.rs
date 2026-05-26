use bill_analyser_parsers::{
    aggregate_description, build_parser_tags, detect_dedicated_import_bytes, normalize_amount_text,
    normalize_parser_tags, normalize_transaction_type, parse_dedicated_import_bytes,
    parser_registry, parser_source_label, post_process_raw_bills, resolve_parser_tags,
    serialize_parser_tags, RawBill, StandardBill,
};
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

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
fn dedicated_parser_sources_are_split_by_source_family() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dedicated_dir = manifest_dir.join("dedicated");

    for filename in [
        "WeChat.rs",
        "Alipay.rs",
        "ICBC.rs",
        "CMBC.rs",
        "ABC.rs",
        "CCB.rs",
        "common.rs",
        "mod.rs",
    ] {
        assert!(
            dedicated_dir.join(filename).exists(),
            "{filename} should live in dedicated parser modules"
        );
    }
    assert!(
        !manifest_dir.join("dedicated.rs").exists(),
        "dedicated parser runtime should not fall back to a monolithic source file"
    );
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

#[test]
fn post_process_preserves_no_income_expenditure_as_transfer_before_staging_suppression() {
    let processed = post_process_raw_bills(
        "alipay",
        &[RawBill {
            date: "2019-02-09 11:52:47".to_string(),
            amount: "813.22".to_string(),
            transaction_type: "不计收支".to_string(),
            description: "自动还款-花呗2019年02月账单".to_string(),
            counterparty: "支付宝（中国）网络技术有限公司".to_string(),
            payment_method: "跨行支付".to_string(),
            original_category: "信用借还".to_string(),
            ..Default::default()
        }],
    );

    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0].original_type, "不计收支");
    assert_eq!(processed[0].transaction_type, "转账");
    assert_ne!(processed[0].transaction_type, "收入");
}

#[test]
fn dedicated_rust_parsers_detect_and_parse_repository_fixtures() {
    let cases = [
        ("wechat_statement_sample.csv", "wechat", 7),
        ("wechat_statement_sample.xlsx", "wechat", 2),
        ("alipay_statement_sample.csv", "alipay", 6),
        ("icbc_statement_sample.csv", "icbc", 2),
        ("icbc_statement_sample.xlsx", "icbc", 2),
        ("cmbc_statement_sample.csv", "cmbc", 2),
        ("cmbc_statement_sample.xls", "cmbc", 2),
        ("abc_statement_sample.csv", "abc", 2),
        ("abc_statement_sample.xlsx", "abc", 2),
        ("ccb_statement_sample.xlsx", "ccb", 2),
    ];

    for (filename, parser_id, expected_count) in cases {
        let bytes = std::fs::read(import_sample_path(filename)).expect("fixture reads");
        let parsed = parse_dedicated_import_bytes(filename, &bytes, "auto")
            .unwrap_or_else(|| panic!("{filename} should match a dedicated Rust parser"));

        assert_eq!(parsed.parser_id, parser_id, "{filename}");
        assert_eq!(parsed.bills.len(), expected_count, "{filename}");
        assert!(
            parsed
                .bills
                .iter()
                .any(|bill| !bill.date.trim().is_empty() && bill.amount.to_cents() != 0),
            "{filename}"
        );
    }
}

#[test]
fn dedicated_rust_parser_rejects_generic_csv_fixture() {
    let filename = "generic_statement_sample.csv";
    let bytes = std::fs::read(import_sample_path(filename)).expect("fixture reads");

    assert!(parse_dedicated_import_bytes(filename, &bytes, "auto").is_none());
}

#[test]
fn dedicated_parser_detector_records_exactly_one_no_match_and_conflict_evidence() {
    let wechat_bytes =
        std::fs::read(import_sample_path("wechat_statement_sample.csv")).expect("fixture reads");
    let matched =
        detect_dedicated_import_bytes("wechat_statement_sample.csv", &wechat_bytes, "auto");
    assert_eq!(matched.status, "matched");
    assert_eq!(matched.selected_parser_id.as_deref(), Some("wechat"));
    assert_eq!(matched.candidates.len(), 1);
    assert_eq!(matched.candidates[0].parser_id, "wechat");
    assert_eq!(matched.candidates[0].parser_label, "微信");
    assert_eq!(matched.candidates[0].parsed_count, 7);
    assert!(matched.candidates[0]
        .evidence
        .iter()
        .any(|item| item == "parsed_count=7"));

    let generic_bytes =
        std::fs::read(import_sample_path("generic_statement_sample.csv")).expect("fixture reads");
    let no_match =
        detect_dedicated_import_bytes("generic_statement_sample.csv", &generic_bytes, "auto");
    assert_eq!(no_match.status, "no_match");
    assert!(no_match.selected_parser_id.is_none());
    assert!(no_match.candidates.is_empty());

    let conflict_csv = "\
交易日期,交易金额,对手信息,对方户名,对方账号
2026-05-04,12.34,张三,张三,6222000000000000
";
    let conflict =
        detect_dedicated_import_bytes("ambiguous-bank.csv", conflict_csv.as_bytes(), "auto");
    assert_eq!(conflict.status, "conflict");
    assert!(conflict.selected_parser_id.is_none());
    assert_eq!(
        conflict
            .candidates
            .iter()
            .map(|candidate| candidate.parser_id.as_str())
            .collect::<Vec<_>>(),
        ["icbc", "abc"]
    );
    assert_eq!(conflict.conflict_group, ["icbc", "abc"]);
    assert!(
        parse_dedicated_import_bytes("ambiguous-bank.csv", conflict_csv.as_bytes(), "auto")
            .is_none()
    );
}

#[test]
fn dedicated_dispatcher_respects_requested_parser_and_rejects_non_dedicated_ids() {
    let filename = "wechat_statement_sample.csv";
    let bytes = std::fs::read(import_sample_path(filename)).expect("fixture reads");

    let parsed = parse_dedicated_import_bytes(filename, &bytes, "wechat")
        .expect("requested parser should parse matching WeChat fixture");
    assert_eq!(parsed.parser_id, "wechat");

    assert!(parse_dedicated_import_bytes(filename, &bytes, "generic").is_none());
    assert!(parse_dedicated_import_bytes(filename, &bytes, "unknown-parser").is_none());
    assert!(parse_dedicated_import_bytes(filename, &bytes, "abc").is_none());
}

#[test]
fn abc_parser_edges_are_source_local_before_standardization() {
    assert!(
        parse_dedicated_import_bytes("abc_statement_sample.json", b"{}", "abc").is_none(),
        "ABC should reject unsupported extensions before content probing"
    );
    assert!(
        parse_dedicated_import_bytes(
            "abc_missing_header.csv",
            "农业银行\n无表头".as_bytes(),
            "abc"
        )
        .is_none(),
        "ABC should require a recognizable header"
    );
    assert!(
        parse_dedicated_import_bytes("abc_bad.xlsx", b"not a workbook", "abc").is_none(),
        "ABC xlsx parser should reject unreadable workbooks"
    );

    let csv = "\
农业银行
交易日期,交易时间,交易金额,对手信息,交易用途
,09:00:00,1.00,缺少日期,跳过
2026-02-01,,12.50,收款方,单金额收入
2026-02-02,,-3.00,付款方,单金额支出
2026-02-03,,0,零金额,跳过
2026-02-04,,,缺少金额,跳过
";
    let parsed = parse_dedicated_import_bytes("abc_edge_statement.csv", csv.as_bytes(), "abc")
        .expect("ABC edge sample should parse valid rows");

    assert_eq!(parsed.parser_id, "abc");
    assert_eq!(parsed.bills.len(), 2);
    assert_eq!(parsed.bills[0].transaction_type, "收入");
    assert_eq!(parsed.bills[0].amount.to_yuan_string(), "12.50");
    assert_eq!(parsed.bills[1].transaction_type, "支出");
    assert_eq!(parsed.bills[1].amount.to_yuan_string(), "-3.00");
}

#[test]
fn ccb_parser_edges_are_source_local_before_standardization() {
    assert!(
        parse_dedicated_import_bytes(
            "ccb_bad.xls",
            b"<table><tr><td>not a statement</td></tr></table>",
            "ccb",
        )
        .is_none(),
        "CCB should reject content without bank markers or required columns"
    );

    let html = "\
<table>
  <tr><th>记账日</th><th>交易日期</th><th>交易时间</th><th>支出</th><th>收入</th><th>摘要</th><th>对方户名</th></tr>
  <tr><td></td><td></td><td></td><td></td><td>1.00</td><td>缺少日期</td><td>跳过</td></tr>
  <tr><td>20260201</td><td></td><td></td><td></td><td></td><td>缺少金额</td><td>跳过</td></tr>
  <tr><td>20260202</td><td></td><td></td><td></td><td>8.00</td><td>工资</td><td></td></tr>
  <tr><td></td><td>20260203</td><td>123456</td><td>3.00</td><td></td><td>消费</td><td>商户</td></tr>
</table>
";
    let parsed = parse_dedicated_import_bytes("ccb_edge_statement.xls", html.as_bytes(), "ccb")
        .expect("CCB edge html should parse valid rows");

    assert_eq!(parsed.parser_id, "ccb");
    assert_eq!(parsed.bills.len(), 2);
    assert_eq!(parsed.bills[0].date, "2026-02-02 00:00:00");
    assert_eq!(parsed.bills[0].counterparty, "工资");
    assert_eq!(parsed.bills[0].amount.to_yuan_string(), "8.00");
    assert_eq!(parsed.bills[1].date, "2026-02-03 12:34:56");
    assert_eq!(parsed.bills[1].counterparty, "商户");
    assert_eq!(parsed.bills[1].amount.to_yuan_string(), "-3.00");
}

fn import_sample_path(filename: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/import_samples")
        .join(filename)
}
