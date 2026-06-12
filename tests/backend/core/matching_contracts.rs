use std::collections::{BTreeMap, BTreeSet};

use bill_analyser_core::{
    bill_pair_feedback_payload_is_related, build_bill_pair_feedback_payload,
    build_duplicate_bill_candidate, build_duplicate_bill_candidates,
    build_formal_duplicate_candidate_id, build_formal_investment_candidate_id,
    build_formal_learning_candidate_id, build_formal_transfer_candidate_id,
    build_investment_pair_candidate, build_investment_pair_candidates,
    build_learning_candidate_for_bill, build_learning_candidates_for_bill,
    build_learning_rule_revision, build_matching_candidate_action_payload,
    build_matching_session_candidates, build_transfer_pair_candidate,
    build_transfer_pair_candidates, build_user_investment_keyword_settings,
    classify_investment_pnl_change, compute_recurring_pattern_hash, detect_recurring_frequency,
    detect_recurring_patterns_with_today, estimate_next_recurring_date, extract_investment_profile,
    is_ordinary_bank_interest_income, normalize_keyword_list, normalize_learning_rule_revision,
    normalize_reconcile_history_families, normalize_transfer_pair_bill_ids,
    parse_manual_pair_request, parse_matching_candidate_id, parse_reconciliation_candidates_query,
    parse_recurring_date, score_investment_candidate, serialize_keyword_list,
    serialize_recurring_suggestion,
};
use chrono::NaiveDate;
use serde_json::{json, Map, Value};

fn object(value: Value) -> Map<String, Value> {
    value.as_object().expect("object").clone()
}

#[test]
fn candidate_ids_parse_all_s12_families() {
    assert_eq!(
        build_formal_transfer_candidate_id(11, 12),
        "bill:11:transfer:12"
    );
    assert_eq!(
        build_formal_investment_candidate_id(11, 12),
        "bill:11:investment:12"
    );
    assert_eq!(
        build_formal_duplicate_candidate_id(11, 12),
        "bill:11:duplicate:12"
    );
    assert_eq!(normalize_learning_rule_revision(" rev-1_! "), "rev1");
    assert_eq!(
        build_formal_learning_candidate_id(11, 5, " rev-1_! "),
        "bill:11:learning:5:rev1"
    );

    let rule = object(json!({
        "composite_match_hash": "hash",
        "learned_category_id": 7,
        "source_account_id": 3,
        "destination_account_id": null,
        "match_type": "keyword",
        "confidence": 0.93,
        "sample_count": 4,
        "support_count": 2
    }));
    let revision = build_learning_rule_revision(&rule);
    assert_eq!(revision.len(), 16);
    assert!(revision
        .chars()
        .all(|character| character.is_ascii_hexdigit()));

    let preview = parse_matching_candidate_id("preview:7:learning").expect("preview descriptor");
    assert_eq!(preview.scope, "preview");
    assert_eq!(preview.kind, "learning");
    assert_eq!(preview.preview_id, Some(7));

    let transfer = parse_matching_candidate_id("bill:11:transfer:12").expect("transfer descriptor");
    assert_eq!(transfer.scope, "bill");
    assert_eq!(transfer.bill_id, Some(11));
    assert_eq!(transfer.candidate_bill_id, Some(12));

    let duplicate =
        parse_matching_candidate_id("bill:11:duplicate:12").expect("duplicate descriptor");
    assert_eq!(duplicate.scope, "bill");
    assert_eq!(duplicate.kind, "duplicate");
    assert_eq!(duplicate.candidate_bill_id, Some(12));

    let learning =
        parse_matching_candidate_id("bill:11:learning:5:rev1").expect("learning descriptor");
    assert_eq!(learning.rule_id, Some(5));
    assert_eq!(learning.rule_revision.as_deref(), Some("rev1"));

    let reconciliation =
        parse_matching_candidate_id("reconcile:import:duplicate:bill:42:import-key")
            .expect("reconciliation descriptor");
    assert_eq!(reconciliation.scope, "reconciliation");
    assert_eq!(reconciliation.kind, "duplicate");
    assert_eq!(reconciliation.existing_bill_id, Some(42));
    assert_eq!(
        reconciliation.import_key_hash.as_deref(),
        Some("import-key")
    );
}

#[test]
fn duplicate_candidates_match_identical_formal_bills_only() {
    let anchor = object(json!({
        "id": 30,
        "date": "2026-05-01 10:00:00",
        "type": "支出",
        "amount_cents": -8850,
        "destination_amount_cents": 0,
        "source_account_id": 7,
        "destination_account_id": 0,
        "counterparty": "咖啡店",
        "description": "拿铁",
        "payment_method": "微信",
        "main_category": "餐饮",
        "sub_category": "咖啡"
    }));
    let identical = json!({
        "id": 31,
        "date": "2026-05-01 10:00:00",
        "type": "支出",
        "amount_cents": -8850,
        "destination_amount_cents": 0,
        "source_account_id": 7,
        "destination_account_id": 0,
        "counterparty": "咖啡店",
        "description": "拿铁",
        "payment_method": "微信",
        "main_category": "餐饮",
        "sub_category": "咖啡"
    });
    let different_description = json!({
        "id": 32,
        "date": "2026-05-01 10:00:00",
        "type": "支出",
        "amount_cents": -8850,
        "destination_amount_cents": 0,
        "source_account_id": 7,
        "destination_account_id": 0,
        "counterparty": "咖啡店",
        "description": "美式",
        "payment_method": "微信",
        "main_category": "餐饮",
        "sub_category": "咖啡"
    });

    let duplicate =
        build_duplicate_bill_candidate(&anchor, identical.as_object().expect("candidate object"))
            .expect("duplicate candidate");
    assert_eq!(duplicate["candidate_id"], "bill:30:duplicate:31");
    assert_eq!(duplicate["kind"], "duplicate");
    assert_eq!(duplicate["score"], 1.0);
    assert_eq!(duplicate["bill"]["id"], 31);

    let candidates = build_duplicate_bill_candidates(&anchor, &[identical, different_description]);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["candidate_id"], "bill:30:duplicate:31");
}

#[test]
fn transfer_candidates_preserve_current_pair_rules_and_stable_sort() {
    let anchor = object(json!({
        "id": 10,
        "date": "2026-05-01 10:00:00",
        "type": "支出",
        "amount_cents": -10000,
        "source_account_id": 1,
        "destination_account_id": 0,
        "counterparty": "A",
        "description": "转出"
    }));
    let different_day = json!({
        "id": 12,
        "date": "2026-05-03 10:00:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 2,
        "destination_account_id": 0,
        "counterparty": "B",
        "description": "转入"
    });
    let near = json!({
        "id": 11,
        "date": "2026-05-01 10:04:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 2,
        "destination_account_id": 0,
        "counterparty": "B",
        "description": "转入"
    });
    let outside_import_window = json!({
        "id": 17,
        "date": "2026-05-01 10:06:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 2,
        "destination_account_id": 0,
        "counterparty": "B",
        "description": "转入"
    });
    let different_amount = json!({
        "id": 18,
        "date": "2026-05-01 10:04:00",
        "type": "收入",
        "amount_cents": 10100,
        "source_account_id": 2,
        "destination_account_id": 0,
        "counterparty": "B",
        "description": "转入"
    });
    let invalid_same_account = json!({
        "id": 13,
        "date": "2026-05-01 10:04:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 1
    });

    let candidates = build_transfer_pair_candidates(
        &anchor,
        &[
            different_day,
            near,
            outside_import_window,
            different_amount,
            invalid_same_account,
        ],
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["candidate_id"], "bill:10:transfer:11");
    assert_eq!(
        candidates[0]["reason"],
        "opposite_amount|different_source_account|time_close"
    );
    assert!(candidates[0].get("time_diff_seconds").is_none());

    let explicit_transfer = object(json!({
        "id": 14,
        "date": "2026-05-01 10:20:00",
        "type": "transfer",
        "amount_cents": 10000,
        "source_account_id": 2
    }));
    assert!(build_transfer_pair_candidates(&anchor, &[json!(explicit_transfer)]).is_empty());
    assert_eq!(normalize_transfer_pair_bill_ids(12, 10), Ok((10, 12)));
    assert_eq!(
        normalize_transfer_pair_bill_ids(12, 12),
        Err("billId and candidateBillId must be different")
    );

    let same_id = object(json!({
        "id": 10,
        "date": "2026-05-01 10:10:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 2
    }));
    assert!(build_transfer_pair_candidate(&anchor, &same_id).is_none());

    let same_sign = object(json!({
        "id": 15,
        "date": "2026-05-01 10:10:00",
        "type": "收入",
        "amount_cents": -10000,
        "source_account_id": 2
    }));
    assert!(build_transfer_pair_candidate(&anchor, &same_sign).is_none());

    let tie_a = json!({
        "id": 16,
        "date": "2026-05-01 10:03:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 2
    });
    let tie_b = json!({
        "id": 15,
        "date": "2026-05-01 10:03:00",
        "type": "收入",
        "amount_cents": 10000,
        "source_account_id": 2
    });
    let tied_candidates = build_transfer_pair_candidates(&anchor, &[tie_a, tie_b]);
    assert_eq!(tied_candidates[0]["candidate_id"], "bill:10:transfer:15");
}

#[test]
fn historical_investment_and_learning_candidates_pin_formal_bill_contracts() {
    let anchor = object(json!({
        "id": 20,
        "date": "2026-05-01 10:00:00",
        "type": "投资",
        "amount_cents": -10000,
        "source_account_id": 1,
        "counterparty": "天天基金",
        "description": "买入 沪深300ETF 申购"
    }));
    let candidate = json!({
        "id": 21,
        "date": "2026-05-01 10:20:00",
        "type": "投资",
        "amount_cents": 10000,
        "source_account_id": 2,
        "destination_account_id": 0,
        "counterparty": "天天基金",
        "description": "卖出 沪深300ETF 赎回"
    });
    let investment_candidates = build_investment_pair_candidates(&anchor, &[candidate], None);
    assert_eq!(investment_candidates.len(), 1);
    assert_eq!(
        investment_candidates[0]["candidate_id"],
        "bill:20:investment:21"
    );
    assert_eq!(investment_candidates[0]["kind"], "investment");
    assert_eq!(
        investment_candidates[0]["reason"],
        "investment_keyword|opposite_amount|different_source_account|time_close"
    );
    assert_eq!(
        investment_candidates[0]["bill"]["source_account_id"],
        json!(2)
    );

    let far_candidate = object(json!({
        "id": 22,
        "date": "2026-05-03 10:00:00",
        "type": "投资",
        "amount_cents": 10000,
        "source_account_id": 2,
        "counterparty": "天天基金",
        "description": "卖出 沪深300ETF 赎回"
    }));
    let near_candidate = object(json!({
        "id": 23,
        "date": "2026-05-01 10:20:00",
        "type": "投资",
        "amount_cents": 10000,
        "source_account_id": 2,
        "counterparty": "天天基金",
        "description": "卖出 沪深300ETF 赎回"
    }));
    assert_eq!(
        build_investment_pair_candidate(&anchor, &far_candidate, None)
            .expect("far investment pair")["reason"],
        "investment_keyword|opposite_amount|different_source_account|date_window"
    );
    let sorted_investments = build_investment_pair_candidates(
        &anchor,
        &[json!(far_candidate), json!(near_candidate)],
        None,
    );
    assert_eq!(
        sorted_investments[0]["candidate_id"],
        "bill:20:investment:23"
    );

    let bill = object(json!({
        "id": 30,
        "counterparty": "星巴克",
        "description": "拿铁",
        "payment_method": "支付宝"
    }));
    let rule = object(json!({
        "id": 8,
        "match_type": "composite",
        "match_features_json": "{\"counterparty\":\"星巴克\",\"description\":\"拿铁\",\"payment_method\":\"支付宝\"}",
        "learned_type": "支出",
        "learned_category_id": 6,
        "learned_source_account_id": 2,
        "learned_destination_account_id": null,
        "composite_match_hash": "c=星巴克|d=拿铁|m=支付宝"
    }));
    let categories = vec![json!({"id": 6, "main_category": "餐饮", "sub_category": "咖啡"})];
    let accounts = vec![
        json!({"id": 2, "name": "支付宝"}),
        json!({"id": 3, "name": "招商银行"}),
    ];
    let learning_rules = vec![json!(rule.clone())];
    let candidates = build_learning_candidates_for_bill(
        &bill,
        &learning_rules,
        &BTreeMap::new(),
        &categories,
        &accounts,
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0]["kind"], "learning");
    assert_eq!(candidates[0]["rule_id"], 8);
    assert_eq!(candidates[0]["score"], 1.0);
    assert_eq!(candidates[0]["level"], "high");
    assert_eq!(candidates[0]["summary"], "支出 | 餐饮/咖啡 | 支付宝");
    assert!(candidates[0]["candidate_id"]
        .as_str()
        .unwrap()
        .starts_with("bill:30:learning:8:"));

    let revision = build_learning_rule_revision(&rule);
    let suppressed = BTreeMap::from([(8, revision)]);
    assert!(build_learning_candidates_for_bill(
        &bill,
        &learning_rules,
        &suppressed,
        &categories,
        &accounts
    )
    .is_empty());

    assert_eq!(
        normalize_reconcile_history_families(Some(&json!([
            "investment",
            "bad",
            "learning",
            "investment"
        ]))),
        vec!["investment", "learning"]
    );
    assert_eq!(
        normalize_reconcile_history_families(Some(&json!("investment"))),
        vec!["transfer"]
    );

    let transfer_bill = object(json!({
        "id": 31,
        "counterparty": "工资",
        "description": "公司/报销_五月",
        "payment_method": "银行卡"
    }));
    let transfer_rule = object(json!({
        "id": 9,
        "match_type": "composite",
        "match_features_json": "{\"counterparty\":\"工资\",\"description\":\"公司 报销 五月\",\"payment_method\":\"银行卡\"}",
        "learned_type": "转账",
        "learned_source_account_id": 2,
        "learned_destination_account_id": 3,
        "composite_match_hash": "c=工资|d=公司 报销 五月|m=银行卡"
    }));
    let transfer_candidates = build_learning_candidates_for_bill(
        &transfer_bill,
        &[json!(transfer_rule)],
        &BTreeMap::new(),
        &[],
        &accounts,
    );
    assert_eq!(transfer_candidates.len(), 1);
    assert_eq!(
        transfer_candidates[0]["summary"],
        "转账 | 支付宝 → 招商银行"
    );

    let income_bill = object(json!({
        "id": 32,
        "counterparty": "Coffee Shop",
        "description": "Latte",
        "payment_method": "card"
    }));
    let income_rule = object(json!({
        "id": 11,
        "match_type": "composite",
        "match_features_json": "{\"counterparty\":\"Coffee Shop\",\"description\":\"Latte\",\"payment_method\":\"card\"}",
        "learned_type": "income",
        "learned_category_id": 7,
        "learned_source_account_id": null,
        "learned_destination_account_id": 3,
        "composite_match_hash": "income-rule"
    }));
    let income_categories = vec![json!({"id": 7, "main_category": "Income", "sub_category": ""})];
    let income_candidate = build_learning_candidate_for_bill(
        &income_bill,
        &income_rule,
        None,
        &income_categories,
        &accounts,
    )
    .expect("income learning candidate");
    assert_eq!(income_candidate["summary"], "income | Income | 招商银行");

    let duplicate_rule = object(json!({
        "id": 12,
        "match_type": "composite",
        "match_features_json": "{\"counterparty\":\"Coffee Shop\",\"description\":\"Latte\",\"payment_method\":\"card\"}",
        "learned_type": "income",
        "composite_match_hash": "income-rule-2"
    }));
    let sorted_learning = build_learning_candidates_for_bill(
        &income_bill,
        &[json!(income_rule.clone()), json!(duplicate_rule)],
        &BTreeMap::new(),
        &income_categories,
        &accounts,
    );
    assert_eq!(sorted_learning[0]["rule_id"], 12);
    assert!(build_learning_candidate_for_bill(
        &income_bill,
        &object(json!({
            "id": 13,
            "match_type": "composite",
            "match_features_json": "{bad json"
        })),
        None,
        &[],
        &[]
    )
    .is_none());
}

#[test]
fn session_candidates_project_preview_families_without_removed_investment() {
    let previews = vec![
        json!("bad-preview"),
        json!({
            "id": 8,
            "preview_date": "2026-05-01",
            "preview_type": "支出",
            "preview_amount_cents": -1850,
            "preview_counterparty": "超市",
            "preview_description": "午餐",
            "matching": {
                "reconciliation": {
                    "candidate_id": "reconcile:import:duplicate:bill:42:abc",
                    "candidate_type": "duplicate",
                    "score": 0.91,
                    "level": "high",
                    "reason": "duplicate",
                    "status": "pending"
                },
                "transfer": {
                    "candidate_type": "transfer",
                    "score": 0.88,
                    "level": "medium",
                    "reason": "opposite_amount",
                    "review_status": "accepted",
                    "suppressed": false
                },
                "learning": {
                    "rule_id": 5,
                    "score": 0.72,
                    "level": "medium",
                    "reason": "rule",
                    "recommended_type": "支出",
                    "summary": "rule summary"
                },
                "recurring": {
                    "id": 99,
                    "name": "会员",
                    "candidate_count": 3,
                    "match_score": 0.93,
                    "match_reasons": "monthly",
                    "matched_date": "2026-05-01"
                },
                "investment": {
                    "score": 1.0
                }
            }
        }),
    ];

    let payload = build_matching_session_candidates("session-a", &previews);
    assert_eq!(payload["session_id"], "session-a");
    assert_eq!(payload["summary"]["preview_count"], 2);
    assert_eq!(payload["summary"]["candidate_count"], 4);
    assert_eq!(payload["summary"]["counts_by_kind"]["reconciliation"], 1);

    let candidates = payload["candidates"].as_array().expect("candidates");
    assert_eq!(
        candidates[0]["candidate_id"],
        "reconcile:import:duplicate:bill:42:abc"
    );
    assert_eq!(candidates[1]["candidate_id"], "preview:8:transfer");
    assert_eq!(candidates[1]["status"], "accepted");
    assert_eq!(candidates[2]["candidate_id"], "preview:8:learning");
    assert_eq!(candidates[2]["status"], "pending");
    assert_eq!(candidates[3]["candidate_id"], "preview:8:recurring");
    assert_eq!(candidates[3]["status"], "confirmed");
    assert!(candidates
        .iter()
        .all(|candidate| candidate["kind"] != "investment"));
}

#[test]
fn route_request_contracts_validate_pair_query_action_and_feedback_payloads() {
    let request = parse_manual_pair_request(&json!({
        "billId": "11",
        "candidateBillId": 12
    }))
    .expect("manual pair");
    assert_eq!(request.bill_id, 11);
    assert_eq!(request.candidate_bill_id, 12);
    assert_eq!(request.pair_type, "transfer");
    assert_eq!(
        parse_manual_pair_request(&json!({"billId": 11, "candidateBillId": 11})),
        Err("billId and candidateBillId must be different")
    );
    assert_eq!(
        parse_manual_pair_request(&json!({
            "billId": 11,
            "candidateBillId": 12,
            "pairType": "duplicate"
        })),
        Err("Invalid pairType")
    );

    let query = object(json!({
        "candidateType": "TRANSFER",
        "status": "accepted",
        "limit": 999,
        "billId": "42",
        "previewId": "8",
        "sessionId": "session-a"
    }));
    let parsed_query = parse_reconciliation_candidates_query(&query).expect("query");
    assert_eq!(parsed_query["candidate_type"], "transfer");
    assert_eq!(parsed_query["limit"], 500);
    assert_eq!(
        parse_reconciliation_candidates_query(&object(json!({"candidateType": "other"}))),
        Err("Invalid candidateType".to_string())
    );

    let action_payload = build_matching_candidate_action_payload(
        "preview:8:transfer",
        &object(json!({
            "action": "accepted",
            "candidate_id": "candidate-x",
            "preview_id": 8,
            "session_id": "session-a",
            "review_status": "accepted",
            "suppressed": true,
            "bill": {
                "id": 22,
                "date": "2026-05-01",
                "type": "支出",
                "amount_cents": -1990,
                "counterparty": "超市",
                "description": "晚餐",
                "payment_method": "支付宝",
                "main_category": "餐饮",
                "sub_category": "晚餐",
                "source_account_id": 3,
                "destination_account_id": 0
            },
            "pair": {
                "id": 7,
                "pair_type": "investment",
                "source": "manual",
                "left_bill_id": 11,
                "right_bill_id": 12,
                "other_bill_id": 12
            }
        })),
    );
    assert_eq!(action_payload["candidateId"], "candidate-x");
    assert_eq!(action_payload["previewId"], 8);
    assert_eq!(action_payload["reviewStatus"], "accepted");
    assert_eq!(action_payload["suppressed"], true);
    assert_eq!(action_payload["pair"]["pairType"], "investment");
    assert_eq!(action_payload["pair"]["otherBillId"], 12);
    assert_eq!(action_payload["bill"]["paymentMethod"], "支付宝");
    assert_eq!(action_payload["bill"]["mainCategory"], "餐饮");
    assert_eq!(action_payload["bill"]["sourceAccountId"], 3);
    assert_eq!(action_payload["bill"]["amountCents"], -1990);
    assert!(action_payload["bill"].get("amount").is_none());
    assert!(action_payload["bill"].get("payment_method").is_none());

    let pair = object(json!({
        "id": 7,
        "pair_type": "transfer",
        "source": "manual",
        "left_bill_id": 11,
        "right_bill_id": 12
    }));
    let feedback = build_bill_pair_feedback_payload("transfer", 11, 12, Some(&pair));
    assert!(bill_pair_feedback_payload_is_related(&feedback, 11));
    assert!(bill_pair_feedback_payload_is_related(&feedback, 12));
    assert!(!bill_pair_feedback_payload_is_related(&feedback, 13));
}

#[test]
fn investment_keywords_profiles_scores_and_pnl_match_current_fixtures() {
    assert_eq!(
        normalize_keyword_list(Some(&json!("基金, ETF，基金|黄金；股票、债券")), &[]),
        vec!["基金", "ETF", "黄金", "股票", "债券"]
    );
    assert_eq!(
        normalize_keyword_list(Some(&json!("")), &["fallback"]),
        vec!["fallback"]
    );
    let serialized = serialize_keyword_list(Some(&json!(["基金", " ETF ", "基金"])));
    assert_eq!(
        serde_json::from_str::<Vec<String>>(&serialized).unwrap(),
        vec!["基金", "ETF"]
    );

    let user = object(json!({
        "investment_platform_keywords": "[\"自定义平台\", \"天天基金\"]",
        "investment_product_keywords": "沪深300ETF\n基金",
        "investment_exclude_keywords": ["服务费", "贷款"]
    }));
    let settings = build_user_investment_keyword_settings(Some(&user));
    assert_eq!(settings["platform_keywords"][0], "自定义平台");
    assert_eq!(settings["product_keywords"][0], "沪深300ETF");

    let generic_profile = extract_investment_profile("基金销售平台 账户服务费", None);
    assert_eq!(generic_profile.platform, "基金销售平台");
    assert_eq!(generic_profile.product, "");
    let empty_profile = extract_investment_profile("", None);
    assert_eq!(empty_profile.platform, "");
    assert_eq!(empty_profile.product, "");

    let investment_bill = object(json!({
        "type": "支出",
        "counterparty": "天天基金",
        "description": "买入 沪深300ETF 申购 扣款",
        "original_category": "基金申购"
    }));
    let signal =
        score_investment_candidate(&investment_bill, false, None).expect("investment score");
    assert_eq!(signal.score, 0.92);
    assert_eq!(
        signal.reason,
        "platform:天天基金, product:沪深300ETF/基金/ETF"
    );
    assert!(score_investment_candidate(
        &object(json!({"type": "transfer", "description": "天天基金 ETF"})),
        true,
        None
    )
    .is_none());
    assert!(score_investment_candidate(
        &object(json!({"type": "投资", "description": "天天基金 ETF"})),
        false,
        None
    )
    .is_none());
    assert!(score_investment_candidate(&object(json!({"type": "支出"})), false, None).is_none());

    let alias_profile = extract_investment_profile("天天基金 指数增强 申购", None);
    assert_eq!(alias_profile.product, "指数增强");
    let alias_bill = object(json!({
        "type": "支出",
        "counterparty": "天天基金",
        "description": "指数增强 申购"
    }));
    assert!(score_investment_candidate(&alias_bill, false, None).is_some());

    let compact_profile = extract_investment_profile("天天基金买入沪深300ETF申购扣款", None);
    assert_eq!(compact_profile.product, "沪深300ETF");

    let service_fee = object(json!({
        "type": "支出",
        "counterparty": "基金销售平台",
        "description": "账户服务费"
    }));
    assert!(score_investment_candidate(&service_fee, false, None).is_none());
    let fee_with_product = object(json!({
        "type": "支出",
        "counterparty": "天天基金",
        "description": "买入 沪深300ETF 账户服务费 申购"
    }));
    assert!(score_investment_candidate(&fee_with_product, false, None).is_none());

    let bank_interest = object(json!({
        "type": "收入",
        "counterparty": "招商银行",
        "description": "活期结息 利息入账"
    }));
    assert!(is_ordinary_bank_interest_income(&bank_interest, None));
    assert!(score_investment_candidate(&bank_interest, false, None).is_none());
    assert!(!is_ordinary_bank_interest_income(
        &object(json!({
            "type": "expense",
            "counterparty": "招商银行",
            "description": "利息扣款"
        })),
        None
    ));

    let pnl_gain = object(json!({
        "type": "投资",
        "counterparty": "天天基金",
        "description": "沪深300ETF 收益发放"
    }));
    let pnl_signal = classify_investment_pnl_change(&pnl_gain, Some(&settings)).expect("pnl");
    assert_eq!(pnl_signal.signal_type.as_deref(), Some("pnl_change"));
    assert_eq!(pnl_signal.direction.as_deref(), Some("gain"));
    assert!(score_investment_candidate(&pnl_gain, true, Some(&settings)).is_none());
    let pnl_loss = object(json!({
        "type": "投资",
        "counterparty": "天天基金",
        "description": "沪深300ETF 亏损调整"
    }));
    let loss_signal = classify_investment_pnl_change(&pnl_loss, Some(&settings)).expect("loss");
    assert_eq!(loss_signal.direction.as_deref(), Some("loss"));
    assert!(classify_investment_pnl_change(
        &object(json!({"type": "transfer", "description": "天天基金 收益"})),
        Some(&settings)
    )
    .is_none());
    assert!(classify_investment_pnl_change(
        &object(json!({"type": "支出", "description": "收益"})),
        Some(&settings)
    )
    .is_none());
}

#[test]
fn recurring_detection_preserves_current_hash_frequency_and_suggestion_shape() {
    let hash = compute_recurring_pattern_hash("支出", 1999, " Netflix ", Some(3));
    assert_eq!(hash.len(), 16);
    assert_eq!(
        hash,
        compute_recurring_pattern_hash("支出", 1999, "netflix", Some(3))
    );

    let monthly = detect_recurring_frequency(&[30.0, 31.0, 29.0]);
    assert_eq!(monthly.frequency, "monthly");
    assert!(monthly.confidence >= 0.6);
    assert_eq!(
        detect_recurring_frequency(&[45.0, 45.0]).frequency,
        "every_45_days"
    );
    assert_eq!(
        detect_recurring_frequency(&[7.0, 30.0, 90.0]).frequency,
        "irregular"
    );
    let unknown = detect_recurring_frequency(&[]);
    assert_eq!(unknown.frequency, "unknown");
    assert_eq!(unknown.confidence, 0.0);

    let last = NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
    assert_eq!(
        estimate_next_recurring_date(last, "monthly", 30.0),
        NaiveDate::from_ymd_opt(2026, 5, 31).unwrap()
    );
    assert_eq!(
        estimate_next_recurring_date(last, "weekly", 0.0),
        NaiveDate::from_ymd_opt(2026, 5, 8).unwrap()
    );
    assert_eq!(
        estimate_next_recurring_date(last, "annual", 0.0),
        NaiveDate::from_ymd_opt(2027, 5, 1).unwrap()
    );
    assert_eq!(
        estimate_next_recurring_date(last, "custom", 45.0),
        NaiveDate::from_ymd_opt(2026, 6, 15).unwrap()
    );
    assert_eq!(
        estimate_next_recurring_date(last, "custom", 0.0),
        NaiveDate::from_ymd_opt(2026, 5, 31).unwrap()
    );
    assert!(parse_recurring_date(&json!(7)).is_none());

    let bills = vec![
        json!("bad-row"),
        json!({
            "id": 98,
            "date": "bad-date",
            "type": "支出",
            "amount_cents": -1999,
            "source_account_id": 3,
            "counterparty": "Netflix"
        }),
        json!({
            "id": 97,
            "date": "2026-01-01",
            "type": "",
            "amount_cents": 0,
            "source_account_id": 3,
            "counterparty": ""
        }),
        json!({
            "id": 1,
            "date": "2026-01-01",
            "type": "支出",
            "amount_cents": -1999,
            "source_account_id": 3,
            "destination_account_id": null,
            "counterparty": "Netflix",
            "description": "subscription"
        }),
        json!({
            "id": 2,
            "date": "2026-01-31",
            "type": "支出",
            "amount_cents": -1999,
            "source_account_id": 3,
            "destination_account_id": null,
            "counterparty": "Netflix",
            "description": "subscription"
        }),
        json!({
            "id": 3,
            "date": "2026-03-02",
            "type": "支出",
            "amount_cents": -1999,
            "source_account_id": 3,
            "destination_account_id": null,
            "counterparty": "Netflix",
            "description": "subscription"
        }),
        json!({
            "id": 99,
            "date": "2026-01-01",
            "type": "支出",
            "amount_cents": -8800,
            "source_account_id": 3,
            "counterparty": "ignored"
        }),
        json!({
            "id": 4,
            "date": "2026-04-01",
            "type": "支出",
            "amount_cents": -999,
            "source_account_id": 3,
            "counterparty": "Weekly",
            "description": "weekly subscription"
        }),
        json!({
            "id": 5,
            "date": "2026-04-08",
            "type": "支出",
            "amount_cents": -999,
            "source_account_id": 3,
            "counterparty": "Weekly",
            "description": "weekly subscription"
        }),
        json!({
            "id": 6,
            "date": "2026-04-15",
            "type": "支出",
            "amount_cents": -999,
            "source_account_id": 3,
            "counterparty": "Weekly",
            "description": "weekly subscription"
        }),
    ];
    let patterns = detect_recurring_patterns_with_today(
        &bills,
        3,
        &BTreeSet::new(),
        NaiveDate::from_ymd_opt(2026, 5, 7).unwrap(),
    );
    assert_eq!(patterns.len(), 2);
    let netflix = patterns
        .iter()
        .find(|pattern| pattern.counterparty == "Netflix")
        .expect("netflix pattern");
    assert_eq!(netflix.frequency, "monthly");
    assert_eq!(netflix.amount_cents, 1999);
    assert_eq!(netflix.sample_count, 3);
    assert_eq!(netflix.sample_bill_ids, vec![1, 2, 3]);
    assert_eq!(netflix.suggested_next_date, "2026-04-01");

    let serialized = serialize_recurring_suggestion(&object(json!({
        "id": 7,
        "pattern_hash": netflix.pattern_hash,
        "name": "Netflix",
        "description": "subscription",
        "type": "支出",
        "amount_cents": 1999,
        "source_account_id": "3",
        "destination_account_id": "",
        "counterparty": "Netflix",
        "frequency": "monthly",
        "detected_interval_days": 30.0,
        "confidence_score": 0.91,
        "sample_count": 3,
        "sample_bill_ids_json": "[1,2,3]",
        "first_occurrence": "2026-01-01",
        "last_occurrence": "2026-03-02",
        "suggested_next_date": "2026-04-01",
        "status": "pending",
        "created_at": "2026-05-07T10:00:00",
        "updated_at": "2026-05-07T10:00:00"
    })));
    assert_eq!(serialized["patternHash"], netflix.pattern_hash);
    assert_eq!(serialized["amountCents"], 1999);
    assert_eq!(serialized["sampleBillIds"], json!([1, 2, 3]));
    assert_eq!(serialized["sourceAccountId"], "3");
    assert_eq!(serialized["destinationAccountId"], "");
    assert_eq!(serialized["suggestedNextDate"], "2026-04-01");
    assert_eq!(serialized["createdAt"], "2026-05-07T10:00:00");
    assert!(serialized.get("pattern_hash").is_none());

    let skipped = detect_recurring_patterns_with_today(
        &bills,
        3,
        &BTreeSet::from([2, 5]),
        NaiveDate::from_ymd_opt(2026, 5, 7).unwrap(),
    );
    assert!(skipped.is_empty());
}
