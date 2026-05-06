use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use crate::import_learning::{build_composite_match_features, normalize_import_learning_text};
use crate::primitives::parse_bill_datetime;

pub const CANDIDATE_KIND_ORDER: &[&str] = &["reconciliation", "transfer", "learning", "recurring"];
pub const LEGACY_SUMMARY_KIND_ORDER: &[&str] = &["transfer", "learning", "recurring"];
pub const TRANSFER_PAIR_TYPE: &str = "transfer";
pub const INVESTMENT_PAIR_TYPE: &str = "investment";
pub const MANUAL_PAIR_SOURCE: &str = "manual";
pub const TRANSFER_PAIR_LOOKBACK_DAYS: i64 = 3;
pub const INVESTMENT_PAIR_LOOKBACK_DAYS: i64 = 3;
pub const TRANSFER_AMOUNT_TOLERANCE: f64 = 0.01;
pub const MIN_RECURRING_OCCURRENCES: usize = 3;
pub const MAX_RECURRING_GAP_RATIO: f64 = 2.0;
pub const MAX_RECURRING_INTERVAL_VARIATION: f64 = 0.5;
pub const MIN_RECURRING_PATTERN_CONFIDENCE: f64 = 0.6;

pub const DEFAULT_INVESTMENT_PLATFORM_KEYWORDS: &[&str] = &[
    "蚂蚁财富",
    "天天基金",
    "理财通",
    "东方财富",
    "同花顺",
    "雪球",
    "基金销售平台",
    "证券账户",
    "余额宝",
    "招财宝",
    "国债逆回购",
    "京东金融",
    "肯特瑞",
    "且慢",
    "蛋卷基金",
    "陆金所",
    "度小满理财",
    "招银理财",
    "工银理财",
    "建信理财",
    "中银理财",
    "农银理财",
    "华泰证券",
    "招商证券",
    "中信证券",
    "广发证券",
    "国泰君安",
    "富途证券",
    "老虎证券",
];
pub const DEFAULT_INVESTMENT_PRODUCT_KEYWORDS: &[&str] = &[
    "基金",
    "理财",
    "定投",
    "申购",
    "赎回",
    "买入",
    "卖出",
    "货币基金",
    "指数基金",
    "债券基金",
    "混合基金",
    "REITs",
    "REIT",
    "ETF",
    "LOF",
    "QDII",
    "债券",
    "股票",
    "黄金",
    "可转债",
    "固收+",
    "现金管理",
    "养老目标",
    "国债",
    "逆回购",
    "组合",
    "计划",
];
pub const DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS: &[&str] = &[
    "还款",
    "借呗",
    "花呗",
    "贷款",
    "信用卡",
    "房贷",
    "车贷",
    "待还款",
    "分期",
    "账单",
    "生活缴费",
    "水费",
    "电费",
    "话费",
];
pub const INVESTMENT_ACTION_KEYWORDS: &[&str] = &[
    "买入",
    "卖出",
    "申购",
    "赎回",
    "定投",
    "扣款",
    "自动定投",
    "转入",
    "转出",
    "分红",
    "分红发放",
    "收益",
    "收益发放",
    "亏损",
    "确认份额",
    "购买",
];
pub const EXPLICIT_INVESTMENT_TYPES: &[&str] = &["投资", "investment", "5"];
const GENERIC_INVESTMENT_PLATFORMS: &[&str] = &["基金销售平台", "证券账户"];
const INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS: &[&str] = &[
    "服务费",
    "账户服务",
    "账户管理",
    "开户",
    "销户",
    "生活缴费",
    "话费",
];
const INVESTMENT_PNL_GAIN_KEYWORDS: &[&str] = &[
    "收益",
    "收益发放",
    "分红",
    "分红发放",
    "红利",
    "派息",
    "盈利",
    "利润",
];
const INVESTMENT_PNL_LOSS_KEYWORDS: &[&str] = &["亏损", "亏损调整", "亏损扣款", "损失", "浮亏"];
const INVESTMENT_CONTEXT_KEYWORDS: &[&str] = &[
    "投资", "基金", "理财", "证券", "股票", "债券", "黄金", "etf", "lof", "reits",
];
const ORDINARY_BANK_CONTEXT_KEYWORDS: &[&str] = &[
    "银行",
    "银行卡",
    "储蓄卡",
    "借记卡",
    "活期",
    "存款",
    "账户",
    "结算户",
];
const ORDINARY_BANK_INTEREST_KEYWORDS: &[&str] = &[
    "结息",
    "账户结息",
    "银行结息",
    "存款结息",
    "活期结息",
    "利息收入",
    "利息入账",
    "银行利息",
    "存款利息",
    "活期利息",
];
const PLATFORM_ALIASES: &[(&str, &[&str])] = &[
    (
        "蚂蚁财富",
        &["蚂蚁财富", "蚂蚁（杭州）基金销售有限公司", "蚂蚁基金"],
    ),
    ("天天基金", &["天天基金", "上海天天基金销售有限公司"]),
    ("理财通", &["理财通", "微信理财通", "腾讯理财通"]),
    ("东方财富", &["东方财富", "东方财富证券"]),
    ("同花顺", &["同花顺"]),
    ("雪球", &["雪球"]),
    ("余额宝", &["余额宝"]),
    ("招财宝", &["招财宝"]),
    ("国债逆回购", &["国债逆回购", "逆回购"]),
    ("证券账户", &["证券", "证券账户"]),
    ("基金销售平台", &["基金销售有限公司", "基金销售"]),
    (
        "京东金融",
        &["京东金融", "京东小金库", "京东肯特瑞", "肯特瑞"],
    ),
    ("且慢", &["且慢", "盈米基金", "盈米财富"]),
    ("蛋卷基金", &["蛋卷基金", "雪球基金", "蛋卷"]),
    ("陆金所", &["陆金所", "陆基金", "陆金所基金"]),
    ("度小满理财", &["度小满", "度小满理财", "百度理财"]),
    ("招银理财", &["招银理财", "招商银行理财", "朝朝宝"]),
    ("工银理财", &["工银理财", "工商银行理财", "工银瑞信"]),
    ("建信理财", &["建信理财", "建设银行理财", "建信基金"]),
    ("中银理财", &["中银理财", "中国银行理财"]),
    ("农银理财", &["农银理财", "农业银行理财"]),
    ("华泰证券", &["华泰证券", "涨乐财富通"]),
    ("招商证券", &["招商证券", "智远一户通"]),
    ("中信证券", &["中信证券", "信e投"]),
    ("广发证券", &["广发证券", "易淘金"]),
    ("国泰君安", &["国泰君安", "君弘"]),
    ("富途证券", &["富途", "富途证券"]),
    ("老虎证券", &["老虎证券", "tiger trade"]),
];
const PRODUCT_ALIASES: &[(&str, &[&str])] = &[
    ("货币基金", &["货币基金"]),
    ("指数基金", &["指数基金", "指数增强", "宽基指数"]),
    ("债券基金", &["债券基金"]),
    ("混合基金", &["混合基金"]),
    ("REITs", &["reits", "reit"]),
    ("ETF", &["etf"]),
    ("LOF", &["lof"]),
    ("QDII", &["qdii"]),
    ("基金", &["基金"]),
    ("理财", &["理财", "理财产品"]),
    ("债券", &["债券"]),
    ("股票", &["股票"]),
    ("黄金", &["黄金"]),
    ("定投", &["定投"]),
    ("可转债", &["可转债"]),
    ("固收+", &["固收+", "固收"]),
    ("现金管理", &["现金管理"]),
    ("养老目标基金", &["养老目标", "养老fof"]),
    ("国债逆回购", &["国债逆回购", "逆回购"]),
    ("组合", &["组合", "策略组合"]),
    ("计划", &["计划", "资管计划"]),
];
const FREQUENCY_PATTERNS: &[(&str, f64, f64)] = &[
    ("weekly", 7.0, 0.3),
    ("biweekly", 14.0, 0.2),
    ("monthly", 30.0, 0.25),
    ("bimonthly", 60.0, 0.2),
    ("quarterly", 90.0, 0.15),
    ("semiannual", 180.0, 0.15),
    ("annual", 365.0, 0.1),
];

type RecurringGroupEntry = (Map<String, Value>, NaiveDate, i64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchingCandidateDescriptor {
    pub scope: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bill_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_bill_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub existing_bill_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_key_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualPairRequest {
    pub bill_id: i64,
    pub candidate_bill_id: i64,
    pub pair_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestmentProfile {
    pub platform: String,
    pub product: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvestmentSignal {
    pub score: f64,
    pub reason: String,
    pub hint_text: String,
    pub platform: String,
    pub product: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrequencyDetection {
    pub frequency: String,
    pub average_interval_days: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecurringPattern {
    pub pattern_hash: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub amount: f64,
    pub source_account_id: Option<i64>,
    pub destination_account_id: String,
    pub counterparty: String,
    pub frequency: String,
    pub detected_interval_days: f64,
    pub confidence_score: f64,
    pub sample_count: usize,
    pub sample_bill_ids: Vec<i64>,
    pub first_occurrence: String,
    pub last_occurrence: String,
    pub suggested_next_date: String,
}

pub fn build_formal_transfer_candidate_id(anchor_bill_id: i64, candidate_bill_id: i64) -> String {
    format!("bill:{anchor_bill_id}:transfer:{candidate_bill_id}")
}

pub fn build_formal_investment_candidate_id(anchor_bill_id: i64, candidate_bill_id: i64) -> String {
    format!("bill:{anchor_bill_id}:investment:{candidate_bill_id}")
}

pub fn normalize_learning_rule_revision(raw_rule_revision: &str) -> String {
    let revision: String = raw_rule_revision
        .trim()
        .chars()
        .filter(|character| character.is_alphanumeric())
        .collect();
    if revision.is_empty() {
        "0".to_string()
    } else {
        revision
    }
}

pub fn build_learning_rule_revision(rule: &Map<String, Value>) -> String {
    let nullable_id = |key: &str| -> String {
        match rule.get(key) {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(text)) if text.trim().is_empty() || text.trim() == "0" => {
                String::new()
            }
            Some(Value::Number(number)) if number.as_i64() == Some(0) => String::new(),
            Some(value) => {
                value_to_i64(Some(value)).map_or_else(String::new, |value| value.to_string())
            }
        }
    };
    let payload = [
        (
            "composite_match_hash",
            value_to_string(rule.get("composite_match_hash")),
        ),
        ("learned_category_id", nullable_id("learned_category_id")),
        (
            "learned_destination_account_id",
            nullable_id("learned_destination_account_id"),
        ),
        (
            "learned_source_account_id",
            nullable_id("learned_source_account_id"),
        ),
        ("learned_type", value_to_string(rule.get("learned_type"))),
        (
            "match_features_json",
            value_to_string(rule.get("match_features_json")),
        ),
        ("match_type", value_to_string(rule.get("match_type"))),
        ("match_value", value_to_string(rule.get("match_value"))),
        (
            "normalized_match_value",
            value_to_string(rule.get("normalized_match_value")),
        ),
        ("parser_id", value_to_string(rule.get("parser_id"))),
    ];
    let revision_source = python_json_object_string(&payload);
    let digest = Sha1::digest(revision_source.as_bytes());
    hex_prefix(&digest, 16)
}

pub fn build_formal_learning_candidate_id(
    anchor_bill_id: i64,
    rule_id: i64,
    rule_revision: &str,
) -> String {
    format!(
        "bill:{anchor_bill_id}:learning:{rule_id}:{}",
        normalize_learning_rule_revision(rule_revision)
    )
}

pub fn parse_matching_candidate_id(candidate_id: &str) -> Option<MatchingCandidateDescriptor> {
    let normalized = candidate_id.trim();
    if normalized.is_empty() {
        return None;
    }
    let parts: Vec<&str> = normalized.split(':').collect();
    if parts.len() == 6 && parts[0] == "reconcile" && parts[1] == "import" && parts[3] == "bill" {
        let kind = parts[2].trim().to_lowercase();
        let bill_id = parse_positive_i64(parts[4])?;
        let import_key_hash = parts[5].trim();
        if matches!(kind.as_str(), "transfer" | "duplicate") && !import_key_hash.is_empty() {
            return Some(MatchingCandidateDescriptor {
                scope: "reconciliation".to_string(),
                kind,
                preview_id: None,
                bill_id: None,
                candidate_bill_id: None,
                rule_id: None,
                rule_revision: None,
                existing_bill_id: Some(bill_id),
                import_key_hash: Some(import_key_hash.to_string()),
            });
        }
    }
    if parts.len() == 3 && parts[0] == "preview" {
        let preview_id = parse_positive_i64(parts[1])?;
        return Some(MatchingCandidateDescriptor {
            scope: "preview".to_string(),
            kind: parts[2].trim().to_lowercase(),
            preview_id: Some(preview_id),
            bill_id: None,
            candidate_bill_id: None,
            rule_id: None,
            rule_revision: None,
            existing_bill_id: None,
            import_key_hash: None,
        });
    }
    if parts.len() == 5 && parts[0] == "bill" && parts[2].trim().eq_ignore_ascii_case("learning") {
        let bill_id = parse_positive_i64(parts[1])?;
        let rule_id = parse_positive_i64(parts[3])?;
        let rule_revision = parts[4].trim();
        if !rule_revision.is_empty() {
            return Some(MatchingCandidateDescriptor {
                scope: "bill".to_string(),
                kind: "learning".to_string(),
                preview_id: None,
                bill_id: Some(bill_id),
                candidate_bill_id: None,
                rule_id: Some(rule_id),
                rule_revision: Some(rule_revision.to_string()),
                existing_bill_id: None,
                import_key_hash: None,
            });
        }
    }
    if parts.len() == 4 && parts[0] == "bill" {
        let bill_id = parse_positive_i64(parts[1])?;
        let target_id = parse_positive_i64(parts[3])?;
        let kind = parts[2].trim().to_lowercase();
        return Some(MatchingCandidateDescriptor {
            scope: "bill".to_string(),
            kind: kind.clone(),
            preview_id: None,
            bill_id: Some(bill_id),
            candidate_bill_id: (kind != "learning").then_some(target_id),
            rule_id: (kind == "learning").then_some(target_id),
            rule_revision: None,
            existing_bill_id: None,
            import_key_hash: None,
        });
    }
    None
}

pub fn normalize_transfer_pair_bill_ids(
    bill_id: i64,
    candidate_bill_id: i64,
) -> Result<(i64, i64), &'static str> {
    if bill_id == candidate_bill_id {
        return Err("billId and candidateBillId must be different");
    }
    Ok((
        bill_id.min(candidate_bill_id),
        bill_id.max(candidate_bill_id),
    ))
}

pub fn build_bill_pair_feedback_payload(
    kind: &str,
    bill_id: i64,
    candidate_bill_id: i64,
    pair: Option<&Map<String, Value>>,
) -> Value {
    let mut payload = json!({
        "scope": "bill",
        "kind": kind.trim(),
        "bill_id": bill_id,
        "candidate_bill_id": candidate_bill_id,
    });
    if let Some(pair) = pair {
        payload["pair"] = json!({
            "id": value_to_i64(pair.get("id")).unwrap_or(0),
            "pair_type": value_to_string(pair.get("pair_type")),
            "source": value_to_string(pair.get("source")),
            "left_bill_id": value_to_i64(pair.get("left_bill_id")).unwrap_or(0),
            "right_bill_id": value_to_i64(pair.get("right_bill_id")).unwrap_or(0),
        });
    }
    payload
}

pub fn bill_pair_feedback_payload_is_related(payload: &Value, bill_id: i64) -> bool {
    let Some(object) = payload.as_object() else {
        return false;
    };
    let mut related_ids = BTreeSet::new();
    for key in ["bill_id", "candidate_bill_id"] {
        if let Some(value) = value_to_i64(object.get(key)) {
            related_ids.insert(value);
        }
    }
    if let Some(pair) = object.get("pair").and_then(Value::as_object) {
        for key in ["left_bill_id", "right_bill_id"] {
            if let Some(value) = value_to_i64(pair.get(key)) {
                related_ids.insert(value);
            }
        }
    }
    related_ids.contains(&bill_id)
}

pub fn parse_manual_pair_request(data: &Value) -> Result<ManualPairRequest, &'static str> {
    let Some(object) = data.as_object() else {
        return Err("Invalid request");
    };
    let bill_id = parse_positive_request_int(object, "billId")?;
    let candidate_bill_id = parse_positive_request_int(object, "candidateBillId")?;
    let pair_type = object
        .get("pairType")
        .map(value_to_string_value)
        .unwrap_or_else(|| TRANSFER_PAIR_TYPE.to_string())
        .trim()
        .to_lowercase();
    if !matches!(
        pair_type.as_str(),
        TRANSFER_PAIR_TYPE | INVESTMENT_PAIR_TYPE
    ) {
        return Err("Invalid pairType");
    }
    if bill_id == candidate_bill_id {
        return Err("billId and candidateBillId must be different");
    }
    Ok(ManualPairRequest {
        bill_id,
        candidate_bill_id,
        pair_type,
    })
}

pub fn build_transfer_pair_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
) -> Option<Value> {
    if is_explicit_transfer_type(anchor_bill.get("type"))
        || is_explicit_transfer_type(candidate_bill.get("type"))
    {
        return None;
    }
    let anchor_id = value_to_i64(anchor_bill.get("id")).filter(|value| *value > 0)?;
    let candidate_id = value_to_i64(candidate_bill.get("id")).filter(|value| *value > 0)?;
    if anchor_id == candidate_id {
        return None;
    }
    let anchor_amount = value_to_f64(anchor_bill.get("amount"));
    let candidate_amount = value_to_f64(candidate_bill.get("amount"));
    if (anchor_amount.abs() - candidate_amount.abs()).abs() > TRANSFER_AMOUNT_TOLERANCE
        || anchor_amount * candidate_amount >= 0.0
    {
        return None;
    }
    let anchor_source_account_id =
        value_to_i64(anchor_bill.get("source_account_id")).filter(|value| *value > 0)?;
    let candidate_source_account_id =
        value_to_i64(candidate_bill.get("source_account_id")).filter(|value| *value > 0)?;
    if anchor_source_account_id == candidate_source_account_id {
        return None;
    }
    let time_diff_seconds =
        pair_time_diff_seconds(anchor_bill, candidate_bill, TRANSFER_PAIR_LOOKBACK_DAYS)?;
    let max_window_seconds = (TRANSFER_PAIR_LOOKBACK_DAYS * 24 * 60 * 60) as f64;
    let time_score = (1.0 - (time_diff_seconds as f64 / max_window_seconds)).max(0.0);
    let score = round2((0.85 + (0.14 * time_score)).min(0.99));
    let mut reason_parts = vec!["opposite_amount", "different_source_account"];
    if time_diff_seconds <= 3600 {
        reason_parts.push("time_close");
    } else {
        reason_parts.push("date_window");
    }
    Some(json!({
        "candidate_id": build_formal_transfer_candidate_id(anchor_id, candidate_id),
        "bill_id": candidate_id,
        "score": score,
        "level": derive_level(score),
        "reason": reason_parts.join("|"),
        "time_diff_seconds": time_diff_seconds,
        "bill": {
            "id": candidate_id,
            "date": value_to_string(candidate_bill.get("date")),
            "type": value_to_string(candidate_bill.get("type")),
            "amount": candidate_amount,
            "counterparty": value_to_string(candidate_bill.get("counterparty")),
            "description": value_to_string(candidate_bill.get("description")),
            "payment_method": value_to_string(candidate_bill.get("payment_method")),
            "main_category": value_to_string(candidate_bill.get("main_category")),
            "sub_category": value_to_string(candidate_bill.get("sub_category")),
            "source_account_id": candidate_source_account_id,
            "destination_account_id": value_to_i64(candidate_bill.get("destination_account_id")).unwrap_or(0),
        },
    }))
}

pub fn build_transfer_pair_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
) -> Vec<Value> {
    let mut candidates: Vec<Value> = candidate_bills
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|candidate_bill| build_transfer_pair_candidate(anchor_bill, candidate_bill))
        .collect();
    candidates.sort_by(|left, right| {
        let left_score = value_to_f64(left.get("score"));
        let right_score = value_to_f64(right.get("score"));
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                value_to_i64(left.get("time_diff_seconds"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(right.get("time_diff_seconds")).unwrap_or(0))
            })
            .then_with(|| {
                value_to_i64(left.get("bill_id"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(right.get("bill_id")).unwrap_or(0))
            })
    });
    for candidate in &mut candidates {
        if let Some(object) = candidate.as_object_mut() {
            object.remove("time_diff_seconds");
        }
    }
    candidates
}

pub fn build_investment_pair_candidate(
    anchor_bill: &Map<String, Value>,
    candidate_bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> Option<Value> {
    score_investment_candidate(anchor_bill, true, keyword_config)?;
    score_investment_candidate(candidate_bill, true, keyword_config)?;

    let anchor_id = value_to_i64(anchor_bill.get("id")).filter(|value| *value > 0)?;
    let candidate_id = value_to_i64(candidate_bill.get("id")).filter(|value| *value > 0)?;
    if anchor_id == candidate_id {
        return None;
    }
    let anchor_amount = value_to_f64(anchor_bill.get("amount"));
    let candidate_amount = value_to_f64(candidate_bill.get("amount"));
    if (anchor_amount.abs() - candidate_amount.abs()).abs() > TRANSFER_AMOUNT_TOLERANCE
        || anchor_amount * candidate_amount >= 0.0
    {
        return None;
    }
    let anchor_source_account_id =
        value_to_i64(anchor_bill.get("source_account_id")).filter(|value| *value > 0)?;
    let candidate_source_account_id =
        value_to_i64(candidate_bill.get("source_account_id")).filter(|value| *value > 0)?;
    if anchor_source_account_id == candidate_source_account_id {
        return None;
    }
    let time_diff_seconds =
        pair_time_diff_seconds(anchor_bill, candidate_bill, INVESTMENT_PAIR_LOOKBACK_DAYS)?;
    let max_window_seconds = (INVESTMENT_PAIR_LOOKBACK_DAYS * 24 * 60 * 60) as f64;
    let time_score = (1.0 - (time_diff_seconds as f64 / max_window_seconds)).max(0.0);
    let score = round2((0.86 + (0.12 * time_score)).min(0.98));
    let mut reason_parts = vec![
        "investment_keyword",
        "opposite_amount",
        "different_source_account",
    ];
    if time_diff_seconds <= 3600 {
        reason_parts.push("time_close");
    } else {
        reason_parts.push("date_window");
    }
    Some(json!({
        "candidate_id": build_formal_investment_candidate_id(anchor_id, candidate_id),
        "kind": "investment",
        "bill_id": candidate_id,
        "score": score,
        "level": derive_level(score),
        "reason": reason_parts.join("|"),
        "bill": build_historical_candidate_bill_snapshot(candidate_bill, candidate_source_account_id),
        "time_diff_seconds": time_diff_seconds,
        "suppressed": false,
    }))
}

pub fn build_investment_pair_candidates(
    anchor_bill: &Map<String, Value>,
    candidate_bills: &[Value],
    keyword_config: Option<&Value>,
) -> Vec<Value> {
    let mut candidates: Vec<Value> = candidate_bills
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|candidate_bill| {
            build_investment_pair_candidate(anchor_bill, candidate_bill, keyword_config)
        })
        .collect();
    candidates.sort_by(|left, right| {
        let left_score = value_to_f64(left.get("score"));
        let right_score = value_to_f64(right.get("score"));
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                value_to_i64(left.get("time_diff_seconds"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(right.get("time_diff_seconds")).unwrap_or(0))
            })
            .then_with(|| {
                value_to_i64(left.get("bill_id"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(right.get("bill_id")).unwrap_or(0))
            })
    });
    for candidate in &mut candidates {
        if let Some(object) = candidate.as_object_mut() {
            object.remove("time_diff_seconds");
        }
    }
    candidates
}

pub fn build_matching_session_candidates(session_id: &str, previews: &[Value]) -> Value {
    let mut candidates = Vec::new();
    let mut counts_by_kind: BTreeMap<&str, i64> =
        CANDIDATE_KIND_ORDER.iter().map(|kind| (*kind, 0)).collect();
    for preview in previews {
        let Some(preview_object) = preview.as_object() else {
            continue;
        };
        let matching = preview_object
            .get("matching")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let context = build_candidate_context(&matching);
        for kind in CANDIDATE_KIND_ORDER {
            let details = matching
                .get(*kind)
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if !should_include_candidate(kind, &details) {
                continue;
            }
            candidates.push(build_session_candidate(
                session_id,
                preview_object,
                kind,
                &details,
                &context,
            ));
            *counts_by_kind.entry(kind).or_insert(0) += 1;
        }
    }
    let mut summary_counts = Map::new();
    for kind in LEGACY_SUMMARY_KIND_ORDER {
        summary_counts.insert((*kind).to_string(), json!(counts_by_kind[kind]));
    }
    if counts_by_kind["reconciliation"] > 0 {
        summary_counts.insert(
            "reconciliation".to_string(),
            json!(counts_by_kind["reconciliation"]),
        );
    }
    json!({
        "session_id": session_id,
        "summary": {
            "preview_count": previews.len(),
            "candidate_count": candidates.len(),
            "counts_by_kind": summary_counts,
        },
        "candidates": candidates,
    })
}

pub fn parse_reconciliation_candidates_query(query: &Map<String, Value>) -> Result<Value, String> {
    let candidate_type = optional_lower_query(query, "candidateType");
    if let Some(candidate_type) = candidate_type.as_deref() {
        if !matches!(candidate_type, "transfer" | "duplicate") {
            return Err("Invalid candidateType".to_string());
        }
    }
    let status = optional_lower_query(query, "status");
    if let Some(status) = status.as_deref() {
        if !matches!(
            status,
            "pending" | "accepted" | "rejected" | "merged" | "rolled_back" | "superseded"
        ) {
            return Err("Invalid status".to_string());
        }
    }
    let limit = parse_optional_positive_query_int(query, "limit")?
        .unwrap_or(200)
        .min(500);
    Ok(json!({
        "session_id": optional_string_query(query, "sessionId"),
        "preview_id": parse_optional_positive_query_int(query, "previewId")?,
        "existing_bill_id": parse_optional_positive_query_int(query, "billId")?,
        "candidate_type": candidate_type,
        "status": status,
        "limit": limit,
    }))
}

pub fn build_matching_candidate_action_payload(
    candidate_id: &str,
    result: &Map<String, Value>,
) -> Value {
    let mut payload = json!({
        "candidateId": value_to_string(result.get("candidate_id")).if_empty(candidate_id),
        "action": value_to_string(result.get("action")),
    });
    let object = payload.as_object_mut().expect("json object");
    insert_i64_if_present(object, "previewId", result.get("preview_id"));
    insert_string_if_present(object, "sessionId", result.get("session_id"));
    insert_i64_if_present(object, "recurringId", result.get("recurring_id"));
    insert_string_if_present(object, "reviewStatus", result.get("review_status"));
    if let Some(value) = result.get("suppressed") {
        object.insert(
            "suppressed".to_string(),
            json!(value.as_bool().unwrap_or(false)),
        );
    }
    for (source_key, target_key) in [
        ("preview_item", "previewItem"),
        ("projection", "projection"),
    ] {
        if let Some(value) = result.get(source_key).filter(|value| value.is_object()) {
            object.insert(target_key.to_string(), value.clone());
        }
    }
    if let Some(bill) = result.get("bill").and_then(Value::as_object) {
        object.insert("bill".to_string(), serialize_bill_snapshot(bill));
    }
    if let Some(value) = result.get("preview").filter(|value| value.is_array()) {
        object.insert("preview".to_string(), value.clone());
    }
    if let Some(pair) = result.get("pair").and_then(Value::as_object) {
        object.insert("pair".to_string(), serialize_bill_pair(pair));
    }
    payload
}

pub fn build_learning_candidate_for_bill(
    bill: &Map<String, Value>,
    rule: &Map<String, Value>,
    suppression_created_at: Option<&str>,
    categories: &[Value],
    accounts: &[Value],
) -> Option<Value> {
    let bill_id = value_to_i64(bill.get("id")).filter(|value| *value > 0)?;
    let rule_id = value_to_i64(rule.get("id")).filter(|value| *value > 0)?;
    if value_to_string(rule.get("match_type")) != "composite"
        || value_to_string(rule.get("match_features_json"))
            .trim()
            .is_empty()
    {
        return None;
    }
    let rule_revision = build_learning_rule_revision(rule);
    if suppression_created_at
        .map(str::trim)
        .is_some_and(|value| value == rule_revision)
    {
        return None;
    }
    let bill_features = build_composite_match_features(
        "",
        &value_to_string(bill.get("counterparty")),
        &value_to_string(bill.get("description")),
        &value_to_string(bill.get("payment_method")),
    )?;
    let rule_features = deserialize_learning_match_features(rule)?;
    let score_payload = score_learning_rule_similarity(&bill_features, &rule_features)?;
    let score = value_to_f64(score_payload.get("score"));
    if score < 0.72 {
        return None;
    }
    let level = if score >= 0.9 {
        "high"
    } else if score >= 0.82 {
        "medium"
    } else {
        "low"
    };
    Some(json!({
        "candidate_id": build_formal_learning_candidate_id(bill_id, rule_id, &rule_revision),
        "kind": "learning",
        "rule_id": rule_id,
        "score": score,
        "level": level,
        "reason": score_payload
            .get("reason_parts")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(value_to_string_value).collect::<Vec<_>>().join(", "))
            .unwrap_or_default(),
        "recommended_type": value_to_string(rule.get("learned_type")),
        "summary": build_learning_rule_result_summary(rule, categories, accounts),
        "suppressed": false,
    }))
}

pub fn build_learning_candidates_for_bill(
    bill: &Map<String, Value>,
    rules: &[Value],
    suppression_revisions: &BTreeMap<i64, String>,
    categories: &[Value],
    accounts: &[Value],
) -> Vec<Value> {
    let mut candidates: Vec<Value> = rules
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|rule| {
            let rule_id = value_to_i64(rule.get("id")).unwrap_or(0);
            build_learning_candidate_for_bill(
                bill,
                rule,
                suppression_revisions.get(&rule_id).map(String::as_str),
                categories,
                accounts,
            )
        })
        .collect();
    candidates.sort_by(|left, right| {
        let left_score = value_to_f64(left.get("score"));
        let right_score = value_to_f64(right.get("score"));
        right_score
            .partial_cmp(&left_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| {
                value_to_i64(right.get("rule_id"))
                    .unwrap_or(0)
                    .cmp(&value_to_i64(left.get("rule_id")).unwrap_or(0))
            })
    });
    candidates
}

pub fn deserialize_learning_match_features(
    rule: &Map<String, Value>,
) -> Option<BTreeMap<String, String>> {
    let raw_payload = value_to_string(rule.get("match_features_json"));
    if raw_payload.trim().is_empty() {
        return None;
    }
    let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(&raw_payload) else {
        return None;
    };
    let mut normalized = BTreeMap::new();
    for (key, value) in payload {
        let normalized_value =
            normalize_import_learning_text(Some(&Value::String(value_to_string_value(&value))));
        if !normalized_value.is_empty() {
            normalized.insert(key, normalized_value);
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn score_learning_rule_similarity(
    bill_features: &BTreeMap<String, String>,
    rule_features: &BTreeMap<String, String>,
) -> Option<Value> {
    let feature_weights = BTreeMap::from([
        ("parser_id", 0.35),
        ("counterparty", 0.30),
        ("description", 0.20),
        ("payment_method", 0.15),
    ]);
    let bill_feature_keys: Vec<&str> = feature_weights
        .keys()
        .copied()
        .filter(|field| {
            bill_features
                .get(*field)
                .is_some_and(|value| !value.is_empty())
        })
        .collect();
    if bill_feature_keys.len() < 2 {
        return None;
    }
    let total_possible_weight: f64 = bill_feature_keys
        .iter()
        .filter_map(|field| feature_weights.get(field).copied())
        .sum();
    let mut weighted_score = 0.0;
    let mut matched_fields = Vec::new();
    let mut reason_parts = Vec::new();
    for field in bill_feature_keys {
        let bill_value = bill_features.get(field).map(String::as_str).unwrap_or("");
        let rule_value = rule_features.get(field).map(String::as_str).unwrap_or("");
        if rule_value.is_empty() {
            continue;
        }
        let similarity = calculate_learning_feature_similarity(field, bill_value, rule_value);
        if similarity <= 0.0 {
            continue;
        }
        weighted_score += feature_weights[field] * similarity;
        if similarity >= 0.8 {
            matched_fields.push(field.to_string());
            let match_label = if similarity >= 0.999 {
                "exact".to_string()
            } else {
                format!("similar({similarity:.2})")
            };
            reason_parts.push(format!("{field}:{match_label}"));
        }
    }
    if matched_fields.len() < 2 || total_possible_weight <= 0.0 {
        return None;
    }
    Some(json!({
        "score": round2(weighted_score / total_possible_weight),
        "matched_fields": matched_fields,
        "reason_parts": reason_parts,
    }))
}

pub fn build_learning_rule_result_summary(
    rule: &Map<String, Value>,
    categories: &[Value],
    accounts: &[Value],
) -> String {
    let mut parts = Vec::new();
    let learned_type = value_to_string(rule.get("learned_type")).trim().to_string();
    if !learned_type.is_empty() {
        parts.push(learned_type.clone());
    }
    if let Some(category_id) = value_to_i64(rule.get("learned_category_id")) {
        if let Some(category) = find_object_by_id(categories, category_id) {
            let main_category = value_to_string(category.get("main_category"));
            let sub_category = value_to_string(category.get("sub_category"));
            if !main_category.is_empty() && !sub_category.is_empty() {
                parts.push(format!("{main_category}/{sub_category}"));
            } else if !main_category.is_empty() {
                parts.push(main_category);
            }
        }
    }
    let source_account = value_to_i64(rule.get("learned_source_account_id"))
        .and_then(|account_id| find_object_by_id(accounts, account_id));
    let destination_account = value_to_i64(rule.get("learned_destination_account_id"))
        .and_then(|account_id| find_object_by_id(accounts, account_id));
    let source_account_name =
        source_account.map_or_else(String::new, |account| value_to_string(account.get("name")));
    let destination_account_name = destination_account
        .map_or_else(String::new, |account| value_to_string(account.get("name")));
    if !source_account_name.is_empty() || !destination_account_name.is_empty() {
        if matches!(
            learned_type.to_lowercase().as_str(),
            "转账" | "投资" | "transfer" | "investment"
        ) && !source_account_name.is_empty()
            && !destination_account_name.is_empty()
        {
            parts.push(format!(
                "{source_account_name} → {destination_account_name}"
            ));
        } else if !source_account_name.is_empty() {
            parts.push(source_account_name);
        } else {
            parts.push(destination_account_name);
        }
    }
    parts.join(" | ")
}

pub fn normalize_reconcile_history_families(raw_families: Option<&Value>) -> Vec<String> {
    let allowed = ["transfer", "investment", "learning"];
    let families = match raw_families {
        Some(Value::Array(values)) => values.as_slice(),
        _ => return vec!["transfer".to_string()],
    };
    let mut normalized = Vec::new();
    for family in families {
        let family = value_to_string_value(family).trim().to_lowercase();
        if allowed.contains(&family.as_str()) && !normalized.contains(&family) {
            normalized.push(family);
        }
    }
    if normalized.is_empty() {
        vec!["transfer".to_string()]
    } else {
        normalized
    }
}

pub fn normalize_keyword_list(raw_value: Option<&Value>, fallback: &[&str]) -> Vec<String> {
    let values: Vec<String> = match raw_value {
        None | Some(Value::Null) => fallback.iter().map(|value| (*value).to_string()).collect(),
        Some(Value::String(text)) if text.trim().is_empty() => {
            fallback.iter().map(|value| (*value).to_string()).collect()
        }
        Some(Value::Array(values)) => values.iter().map(value_to_string_value).collect(),
        Some(Value::String(text)) => {
            let trimmed = text.trim();
            if trimmed.starts_with('[') {
                if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(trimmed) {
                    return dedupe_keywords(values.iter().map(value_to_string_value));
                }
            }
            let normalized = ["\n", ",", "，", "|", "、", ";", "；"]
                .iter()
                .fold(trimmed.to_string(), |text, separator| {
                    text.replace(separator, "\n")
                });
            normalized.lines().map(ToOwned::to_owned).collect()
        }
        Some(_) => fallback.iter().map(|value| (*value).to_string()).collect(),
    };
    dedupe_keywords(values)
}

pub fn serialize_keyword_list(raw_value: Option<&Value>) -> String {
    serde_json::to_string(&normalize_keyword_list(raw_value, &[]))
        .unwrap_or_else(|_| "[]".to_string())
}

pub fn build_user_investment_keyword_settings(user: Option<&Map<String, Value>>) -> Value {
    let user = user.cloned().unwrap_or_default();
    json!({
        "platform_keywords": normalize_keyword_list(user.get("investment_platform_keywords"), DEFAULT_INVESTMENT_PLATFORM_KEYWORDS),
        "product_keywords": normalize_keyword_list(user.get("investment_product_keywords"), DEFAULT_INVESTMENT_PRODUCT_KEYWORDS),
        "exclude_keywords": normalize_keyword_list(user.get("investment_exclude_keywords"), DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS),
    })
}

pub fn extract_investment_profile(text: &str, keyword_config: Option<&Value>) -> InvestmentProfile {
    let raw_text = text.trim();
    if raw_text.is_empty() {
        return InvestmentProfile {
            platform: String::new(),
            product: String::new(),
        };
    }
    let text_lower = raw_text.to_lowercase();
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    let platform_keywords = value_array_strings(config.get("platform_keywords"));
    let product_keywords = value_array_strings(config.get("product_keywords"));

    let mut platform = String::new();
    let mut best_platform_score = (-1, 0usize);
    for (canonical, aliases) in PLATFORM_ALIASES {
        let mut matched_aliases: Vec<&str> = vec![canonical];
        matched_aliases.extend_from_slice(aliases);
        let Some(best_alias) = matched_aliases
            .into_iter()
            .filter(|alias| text_lower.contains(&alias.to_lowercase()))
            .max_by_key(|alias| alias.chars().count())
        else {
            continue;
        };
        let score = (
            if GENERIC_INVESTMENT_PLATFORMS.contains(canonical) {
                0
            } else {
                1
            },
            best_alias.chars().count(),
        );
        if score > best_platform_score {
            best_platform_score = score;
            platform = (*canonical).to_string();
        }
    }
    if platform.is_empty() {
        for keyword in sort_by_len_desc(&platform_keywords) {
            if text_lower.contains(&keyword.to_lowercase()) {
                platform = keyword;
                break;
            }
        }
    }

    let mut product = sort_by_len_desc(&product_keywords)
        .into_iter()
        .find(|keyword| text_lower.contains(&keyword.to_lowercase()))
        .unwrap_or_default();
    if let Some(named_product) = extract_named_product(raw_text, &platform) {
        if product.is_empty() || named_product.chars().count() > product.chars().count() {
            product = named_product;
        }
    }
    let mut best_alias_product = String::new();
    let mut best_alias_product_score = 0usize;
    for (canonical, aliases) in PRODUCT_ALIASES {
        let mut alias_candidates: Vec<&str> = vec![canonical];
        alias_candidates.extend_from_slice(aliases);
        let Some(best_alias) = alias_candidates
            .into_iter()
            .filter(|alias| text_lower.contains(&alias.to_lowercase()))
            .max_by_key(|alias| alias.chars().count())
        else {
            continue;
        };
        if best_alias.chars().count() > best_alias_product_score {
            best_alias_product_score = best_alias.chars().count();
            best_alias_product = best_alias.to_string();
        }
    }
    if product.is_empty()
        || best_alias_product_score > product.chars().count()
        || matches!(
            product.as_str(),
            "基金" | "理财" | "债券" | "股票" | "黄金" | "组合" | "计划"
        )
    {
        product = best_alias_product;
    }
    product = clean_investment_product_name(&product, &platform);
    if GENERIC_INVESTMENT_PLATFORMS.contains(&platform.as_str())
        && matches!(
            product.as_str(),
            "基金" | "理财" | "债券" | "股票" | "黄金" | "组合" | "计划"
        )
        && !contains_any(&text_lower, INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS).is_empty()
    {
        product.clear();
    }
    InvestmentProfile { platform, product }
}

pub fn is_ordinary_bank_interest_income(
    bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> bool {
    let current_type = value_to_string(bill.get("type")).trim().to_lowercase();
    if matches!(current_type.as_str(), "转账" | "transfer" | "4") {
        return false;
    }
    let evidence_text = join_nonempty([
        value_to_string(bill.get("counterparty")),
        value_to_string(bill.get("payment_method")),
        value_to_string(bill.get("description")),
        value_to_string(bill.get("original_category")),
    ]);
    if evidence_text.is_empty() {
        return false;
    }
    let evidence_lower = evidence_text.to_lowercase();
    let has_settlement_interest = !contains_any(&evidence_lower, ORDINARY_BANK_INTEREST_KEYWORDS)
        .is_empty()
        || evidence_lower.contains("结息");
    let has_generic_interest = evidence_lower.contains("利息");
    if !has_settlement_interest && !has_generic_interest {
        return false;
    }
    if has_generic_interest && !has_settlement_interest {
        let amount = value_to_f64(bill.get("amount"));
        if matches!(current_type.as_str(), "支出" | "expense" | "3") || amount < 0.0 {
            return false;
        }
    }
    if !ORDINARY_BANK_CONTEXT_KEYWORDS
        .iter()
        .any(|keyword| evidence_lower.contains(&keyword.to_lowercase()))
    {
        return false;
    }
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    let profile = extract_investment_profile(&evidence_text, Some(&config));
    if !profile.platform.is_empty() || !profile.product.is_empty() {
        return false;
    }
    let mut investment_keywords = value_array_strings(config.get("platform_keywords"));
    investment_keywords.extend(value_array_strings(config.get("product_keywords")));
    investment_keywords.extend(
        INVESTMENT_CONTEXT_KEYWORDS
            .iter()
            .map(|item| (*item).to_string()),
    );
    !investment_keywords
        .iter()
        .any(|keyword| evidence_lower.contains(&keyword.to_lowercase()))
}

pub fn score_investment_candidate(
    bill: &Map<String, Value>,
    allow_existing_investment: bool,
    keyword_config: Option<&Value>,
) -> Option<InvestmentSignal> {
    let current_type = value_to_string(bill.get("type")).trim().to_lowercase();
    if matches!(current_type.as_str(), "转账" | "transfer" | "4") {
        return None;
    }
    let is_explicit_investment_type = EXPLICIT_INVESTMENT_TYPES.contains(&current_type.as_str());
    if !allow_existing_investment && is_explicit_investment_type {
        return None;
    }
    let evidence_text = join_nonempty([
        value_to_string(bill.get("counterparty")),
        value_to_string(bill.get("payment_method")),
        value_to_string(bill.get("description")),
        value_to_string(bill.get("original_category")),
    ]);
    let assigned_category_text = join_nonempty([
        value_to_string(bill.get("main_category")),
        value_to_string(bill.get("sub_category")),
    ]);
    let all_text = join_nonempty([evidence_text.clone(), assigned_category_text]);
    if evidence_text.is_empty() {
        return None;
    }
    let evidence_lower = evidence_text.to_lowercase();
    let all_text_lower = all_text.to_lowercase();
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    if is_ordinary_bank_interest_income(bill, Some(&config))
        || classify_investment_pnl_change(bill, Some(&config)).is_some()
    {
        return None;
    }
    let profile = extract_investment_profile(&evidence_text, Some(&config));
    let platform_keywords = value_array_strings(config.get("platform_keywords"));
    let product_keywords: Vec<String> = value_array_strings(config.get("product_keywords"))
        .into_iter()
        .filter(|keyword| !INVESTMENT_ACTION_KEYWORDS.contains(&keyword.as_str()))
        .collect();
    let exclude_keywords = value_array_strings(config.get("exclude_keywords"));
    let action_matches = contains_any(&evidence_lower, INVESTMENT_ACTION_KEYWORDS);
    let intrinsic_negative_matches =
        contains_any(&all_text_lower, INTRINSIC_INVESTMENT_NEGATIVE_KEYWORDS);
    let mut matched_platforms = Vec::new();
    append_unique(&mut matched_platforms, &profile.platform);
    for keyword in &platform_keywords {
        if evidence_lower.contains(&keyword.to_lowercase()) {
            append_unique(&mut matched_platforms, keyword);
        }
    }
    let mut matched_products = Vec::new();
    append_unique(&mut matched_products, &profile.product);
    for keyword in &product_keywords {
        if evidence_lower.contains(&keyword.to_lowercase()) {
            append_unique(&mut matched_products, keyword);
        }
    }
    let matched_excludes: Vec<String> = exclude_keywords
        .into_iter()
        .filter(|keyword| all_text_lower.contains(&keyword.to_lowercase()))
        .collect();
    let has_specific_platform = matched_platforms
        .iter()
        .any(|platform| !GENERIC_INVESTMENT_PLATFORMS.contains(&platform.as_str()));
    let has_named_or_specific_product = matched_products.iter().any(|product| {
        product.chars().count() > 2
            && !matches!(
                product.as_str(),
                "基金" | "理财" | "债券" | "股票" | "黄金" | "组合" | "计划"
            )
    });
    let mut score = 0.0;
    if has_specific_platform {
        score += (0.38 * matched_platforms.iter().take(2).count() as f64).min(0.65);
    } else if !matched_platforms.is_empty() {
        score += 0.28;
    }
    if !matched_products.is_empty() {
        score += (0.18 * matched_products.iter().take(3).count() as f64).min(0.42);
    }
    if !action_matches.is_empty() {
        score += 0.12;
    }
    let explicit_boost = if allow_existing_investment
        && is_explicit_investment_type
        && has_specific_platform
        && !action_matches.is_empty()
    {
        0.08
    } else {
        0.0
    };
    score += explicit_boost;
    if !matched_excludes.is_empty() {
        score -= (0.28 * matched_excludes.iter().take(2).count() as f64).min(0.48);
    }
    if !intrinsic_negative_matches.is_empty() {
        score -= (0.35 * intrinsic_negative_matches.iter().take(2).count() as f64).min(0.55);
    }
    if matched_platforms.is_empty() && matched_products.len() < 2 {
        return None;
    }
    if !matched_platforms.is_empty()
        && !has_specific_platform
        && !has_named_or_specific_product
        && action_matches.is_empty()
    {
        return None;
    }
    if score < 0.55 {
        return None;
    }
    let mut reason_parts = Vec::new();
    if !matched_platforms.is_empty() {
        reason_parts.push(format!(
            "platform:{}",
            matched_platforms
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !matched_products.is_empty() {
        reason_parts.push(format!(
            "product:{}",
            matched_products
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !matched_excludes.is_empty() {
        reason_parts.push(format!(
            "exclude:{}",
            matched_excludes
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if !intrinsic_negative_matches.is_empty() {
        reason_parts.push(format!(
            "negative:{}",
            intrinsic_negative_matches
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join("/")
        ));
    }
    if explicit_boost > 0.0 {
        reason_parts.push("type:investment".to_string());
    }
    let mut hint_tokens = matched_platforms
        .iter()
        .take(1)
        .cloned()
        .collect::<Vec<_>>();
    hint_tokens.extend(matched_products.iter().take(2).cloned());
    Some(InvestmentSignal {
        score: round2(score.min(1.0)),
        reason: reason_parts.join(", "),
        hint_text: hint_tokens.join(" "),
        platform: profile.platform,
        product: profile.product,
        signal_type: None,
        label: None,
        direction: None,
        direction_label: None,
    })
}

pub fn classify_investment_pnl_change(
    bill: &Map<String, Value>,
    keyword_config: Option<&Value>,
) -> Option<InvestmentSignal> {
    let current_type = value_to_string(bill.get("type")).trim().to_lowercase();
    if matches!(current_type.as_str(), "转账" | "transfer" | "4") {
        return None;
    }
    let evidence_text = join_nonempty([
        value_to_string(bill.get("counterparty")),
        value_to_string(bill.get("payment_method")),
        value_to_string(bill.get("description")),
        value_to_string(bill.get("original_category")),
    ]);
    let assigned_category_text = join_nonempty([
        value_to_string(bill.get("main_category")),
        value_to_string(bill.get("sub_category")),
    ]);
    let all_text = join_nonempty([evidence_text.clone(), assigned_category_text]);
    if evidence_text.is_empty() {
        return None;
    }
    let all_text_lower = all_text.to_lowercase();
    let config = keyword_config
        .cloned()
        .unwrap_or_else(|| build_user_investment_keyword_settings(None));
    if is_ordinary_bank_interest_income(bill, Some(&config)) {
        return None;
    }
    let loss_matches = contains_any(&all_text_lower, INVESTMENT_PNL_LOSS_KEYWORDS);
    let gain_matches = contains_any(&all_text_lower, INVESTMENT_PNL_GAIN_KEYWORDS);
    if loss_matches.is_empty() && gain_matches.is_empty() {
        return None;
    }
    let profile = extract_investment_profile(&evidence_text, Some(&config));
    let has_config_platform = value_array_strings(config.get("platform_keywords"))
        .iter()
        .any(|keyword| all_text_lower.contains(&keyword.to_lowercase()));
    let has_config_product = value_array_strings(config.get("product_keywords"))
        .iter()
        .any(|keyword| all_text_lower.contains(&keyword.to_lowercase()));
    let has_explicit_context = EXPLICIT_INVESTMENT_TYPES.contains(&current_type.as_str())
        || INVESTMENT_CONTEXT_KEYWORDS
            .iter()
            .any(|keyword| all_text_lower.contains(keyword));
    if !(!profile.platform.is_empty()
        || !profile.product.is_empty()
        || has_config_platform
        || has_config_product
        || has_explicit_context)
    {
        return None;
    }
    let (direction, direction_label, matched_keywords) = if loss_matches.is_empty() {
        ("gain", "收益", gain_matches)
    } else {
        ("loss", "亏损", loss_matches)
    };
    let mut score: f64 = 0.72;
    if !profile.platform.is_empty() || has_config_platform {
        score += 0.08;
    }
    if !profile.product.is_empty() || has_config_product {
        score += 0.06;
    }
    if EXPLICIT_INVESTMENT_TYPES.contains(&current_type.as_str()) {
        score += 0.04;
    }
    let mut reason_parts = vec![format!("盈亏变化:{direction_label}")];
    if !profile.platform.is_empty() {
        reason_parts.push(format!("platform:{}", profile.platform));
    }
    if !profile.product.is_empty() {
        reason_parts.push(format!("product:{}", profile.product));
    }
    reason_parts.push(format!(
        "keyword:{}",
        matched_keywords
            .into_iter()
            .take(2)
            .collect::<Vec<_>>()
            .join("/")
    ));
    let hint_text = [profile.platform.clone(), profile.product.clone()]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some(InvestmentSignal {
        score: round2(score.min(1.0)),
        reason: reason_parts.join(", "),
        hint_text,
        platform: profile.platform,
        product: profile.product,
        signal_type: Some("pnl_change".to_string()),
        label: Some("盈亏变化".to_string()),
        direction: Some(direction.to_string()),
        direction_label: Some(direction_label.to_string()),
    })
}

pub fn parse_recurring_date(value: &Value) -> Option<NaiveDate> {
    match value {
        Value::String(text) => NaiveDate::parse_from_str(text.trim().get(..10)?, "%Y-%m-%d").ok(),
        _ => None,
    }
}

pub fn compute_recurring_pattern_hash(
    transaction_type: &str,
    amount_cents: i64,
    counterparty: &str,
    account_id: Option<i64>,
) -> String {
    let key = format!(
        "{}|{}|{}|{}",
        transaction_type,
        amount_cents,
        counterparty.trim().to_lowercase(),
        account_id.unwrap_or(0)
    );
    let digest = Sha256::digest(key.as_bytes());
    hex_prefix(&digest, 16)
}

pub fn detect_recurring_frequency(intervals: &[f64]) -> FrequencyDetection {
    if intervals.is_empty() {
        return FrequencyDetection {
            frequency: "unknown".to_string(),
            average_interval_days: 0.0,
            confidence: 0.0,
        };
    }
    let avg_interval = mean(intervals);
    let interval_stdev = sample_stdev(intervals);
    let interval_variation = if avg_interval > 0.0 {
        interval_stdev / avg_interval
    } else {
        0.0
    };
    if interval_variation > MAX_RECURRING_INTERVAL_VARIATION {
        return FrequencyDetection {
            frequency: "irregular".to_string(),
            average_interval_days: avg_interval,
            confidence: 0.0,
        };
    }
    let mut best: Option<FrequencyDetection> = None;
    for (label, nominal_days, tolerance) in FREQUENCY_PATTERNS {
        let deviation = (avg_interval - nominal_days).abs() / nominal_days;
        if deviation <= *tolerance {
            let closeness = 1.0 - deviation;
            let consistency = 1.0 - (interval_stdev / nominal_days).min(1.0);
            let score = closeness * 0.6 + consistency * 0.4;
            if best
                .as_ref()
                .is_none_or(|current| score > current.confidence)
            {
                best = Some(FrequencyDetection {
                    frequency: (*label).to_string(),
                    average_interval_days: avg_interval,
                    confidence: score,
                });
            }
        }
    }
    if let Some(best) = best {
        return best;
    }
    if avg_interval > 0.0 && interval_stdev / avg_interval < 0.4 {
        return FrequencyDetection {
            frequency: format!("every_{}_days", avg_interval.round() as i64),
            average_interval_days: avg_interval,
            confidence: (1.0 - (interval_stdev / avg_interval).min(1.0)) * 0.5,
        };
    }
    FrequencyDetection {
        frequency: "irregular".to_string(),
        average_interval_days: avg_interval,
        confidence: 0.0,
    }
}

pub fn estimate_next_recurring_date(
    last_date: NaiveDate,
    frequency: &str,
    avg_interval: f64,
) -> NaiveDate {
    let days = match frequency {
        "weekly" => 7,
        "biweekly" => 14,
        "monthly" => 30,
        "bimonthly" => 60,
        "quarterly" => 90,
        "semiannual" => 180,
        "annual" => 365,
        _ => {
            if avg_interval > 0.0 {
                avg_interval.round() as i64
            } else {
                30
            }
        }
    };
    last_date + Duration::days(days)
}

pub fn detect_recurring_patterns(
    bills: &[Value],
    min_occurrences: usize,
    existing_recurring_ids: &BTreeSet<i64>,
) -> Vec<RecurringPattern> {
    detect_recurring_patterns_with_today(
        bills,
        min_occurrences,
        existing_recurring_ids,
        Local::now().date_naive(),
    )
}

pub fn detect_recurring_patterns_with_today(
    bills: &[Value],
    min_occurrences: usize,
    existing_recurring_ids: &BTreeSet<i64>,
    today: NaiveDate,
) -> Vec<RecurringPattern> {
    let mut group_order = Vec::new();
    let mut groups: BTreeMap<String, Vec<RecurringGroupEntry>> = BTreeMap::new();
    for bill in bills {
        let Some(object) = bill.as_object() else {
            continue;
        };
        let bill_id = value_to_i64(object.get("id")).unwrap_or(0);
        if existing_recurring_ids.contains(&bill_id) {
            continue;
        }
        let Some(bill_date) = object.get("date").and_then(parse_recurring_date) else {
            continue;
        };
        let transaction_type = value_to_string(object.get("type")).trim().to_string();
        let amount_cents = (value_to_f64(object.get("amount")).abs() * 100.0).round() as i64;
        let counterparty = value_to_string(object.get("counterparty"))
            .trim()
            .to_string();
        if transaction_type.is_empty() || amount_cents == 0 || counterparty.is_empty() {
            continue;
        }
        let account_id = value_to_i64(object.get("source_account_id"));
        let pattern_hash = compute_recurring_pattern_hash(
            &transaction_type,
            amount_cents,
            &counterparty,
            account_id,
        );
        if !groups.contains_key(&pattern_hash) {
            group_order.push(pattern_hash.clone());
        }
        groups
            .entry(pattern_hash)
            .or_default()
            .push((object.clone(), bill_date, amount_cents));
    }
    let mut patterns = Vec::new();
    for pattern_hash in group_order {
        let Some(group_bills) = groups.get_mut(&pattern_hash) else {
            continue;
        };
        if group_bills.len() < min_occurrences {
            continue;
        }
        group_bills.sort_by_key(|(_, bill_date, _)| *bill_date);
        let dates: Vec<NaiveDate> = group_bills
            .iter()
            .map(|(_, bill_date, _)| *bill_date)
            .collect();
        let intervals: Vec<f64> = dates
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).num_days() as f64)
            .collect();
        if intervals.is_empty() {
            continue;
        }
        let avg_raw = mean(&intervals);
        if avg_raw <= 0.0 {
            continue;
        }
        let raw_variation = sample_stdev(&intervals) / avg_raw;
        if raw_variation > MAX_RECURRING_INTERVAL_VARIATION {
            continue;
        }
        let valid_intervals: Vec<f64> = intervals
            .into_iter()
            .filter(|interval| *interval <= avg_raw * MAX_RECURRING_GAP_RATIO && *interval > 0.0)
            .collect();
        if valid_intervals.len() < min_occurrences.saturating_sub(1) {
            continue;
        }
        let frequency = detect_recurring_frequency(&valid_intervals);
        if matches!(frequency.frequency.as_str(), "unknown" | "irregular")
            || frequency.confidence < 0.3
        {
            continue;
        }
        let recency_days = (today - *dates.last().expect("date")).num_days();
        let recency_score = (1.0 - (recency_days as f64 / 180.0)).max(0.0);
        let count_score = (group_bills.len() as f64 / 12.0).min(1.0);
        let confidence =
            round3((frequency.confidence * 0.5 + recency_score * 0.3 + count_score * 0.2).min(1.0));
        if confidence < MIN_RECURRING_PATTERN_CONFIDENCE {
            continue;
        }
        let sample = &group_bills[0].0;
        let counterparty = value_to_string(sample.get("counterparty"))
            .trim()
            .to_string();
        let description = [
            value_to_string(sample.get("description")),
            value_to_string(sample.get("main_category")),
        ]
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
        let next_date = estimate_next_recurring_date(
            *dates.last().expect("date"),
            &frequency.frequency,
            frequency.average_interval_days,
        );
        patterns.push(RecurringPattern {
            pattern_hash,
            name: if counterparty.is_empty() {
                value_to_string(sample.get("description")).if_empty("Unknown")
            } else {
                counterparty.clone()
            },
            description,
            transaction_type: value_to_string(sample.get("type")),
            amount: value_to_f64(sample.get("amount")).abs(),
            source_account_id: value_to_i64(sample.get("source_account_id")),
            destination_account_id: value_to_string(sample.get("destination_account_id")),
            counterparty,
            frequency: frequency.frequency,
            detected_interval_days: round1(frequency.average_interval_days),
            confidence_score: confidence,
            sample_count: group_bills.len(),
            sample_bill_ids: group_bills
                .iter()
                .filter_map(|(bill, _, _)| value_to_i64(bill.get("id")))
                .collect(),
            first_occurrence: dates.first().expect("date").to_string(),
            last_occurrence: dates.last().expect("date").to_string(),
            suggested_next_date: next_date.to_string(),
        });
    }
    patterns.sort_by(|left, right| {
        right
            .confidence_score
            .partial_cmp(&left.confidence_score)
            .unwrap_or(Ordering::Equal)
    });
    patterns
}

pub fn serialize_recurring_suggestion(item: &Map<String, Value>) -> Value {
    json!({
        "id": value_to_i64(item.get("id")).unwrap_or(0),
        "patternHash": value_to_string(item.get("pattern_hash")),
        "name": value_to_string(item.get("name")),
        "description": value_to_string(item.get("description")),
        "type": value_to_string(item.get("type")),
        "amount": value_to_f64(item.get("amount")),
        "sourceAccountId": item.get("source_account_id").cloned().unwrap_or(Value::Null),
        "destinationAccountId": item.get("destination_account_id").cloned().unwrap_or(Value::Null),
        "counterparty": value_to_string(item.get("counterparty")),
        "frequency": value_to_string(item.get("frequency")),
        "detectedIntervalDays": value_to_f64(item.get("detected_interval_days")),
        "confidenceScore": value_to_f64(item.get("confidence_score")),
        "sampleCount": value_to_i64(item.get("sample_count")).unwrap_or(0),
        "sampleBillIds": recurring_sample_bill_ids(item),
        "firstOccurrence": value_to_string(item.get("first_occurrence")),
        "lastOccurrence": value_to_string(item.get("last_occurrence")),
        "suggestedNextDate": value_to_string(item.get("suggested_next_date")),
        "status": value_to_string(item.get("status")),
        "createdAt": value_to_string(item.get("created_at")),
        "updatedAt": value_to_string(item.get("updated_at")),
    })
}

pub fn serialize_recurring_suggestions(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter_map(Value::as_object)
        .map(serialize_recurring_suggestion)
        .collect()
}

fn build_candidate_context(matching: &Map<String, Value>) -> Value {
    let dedup = matching.get("dedup").and_then(Value::as_object);
    let parser = matching.get("parser").and_then(Value::as_object);
    let annotation = matching.get("annotation").and_then(Value::as_object);
    json!({
        "dedup": {
            "type": dedup.map_or_else(String::new, |value| value_to_string(value.get("type"))),
            "source_ids": normalize_list(dedup.and_then(|value| value.get("source_ids"))),
        },
        "parser": {
            "id": parser.map_or_else(String::new, |value| value_to_string(value.get("id"))),
            "tags": normalize_list(parser.and_then(|value| value.get("tags"))).into_iter().filter(|value| !value_to_string_value(value).is_empty()).collect::<Vec<_>>(),
        },
        "annotation": {
            "is_manually_annotated": annotation.and_then(|value| value.get("is_manually_annotated")).and_then(Value::as_bool).unwrap_or(false),
        },
    })
}

fn build_historical_candidate_bill_snapshot(
    candidate_bill: &Map<String, Value>,
    source_account_id: i64,
) -> Value {
    json!({
        "id": value_to_i64(candidate_bill.get("id")).unwrap_or(0),
        "date": value_to_string(candidate_bill.get("date")),
        "type": value_to_string(candidate_bill.get("type")),
        "amount": value_to_f64(candidate_bill.get("amount")),
        "counterparty": value_to_string(candidate_bill.get("counterparty")),
        "description": value_to_string(candidate_bill.get("description")),
        "payment_method": value_to_string(candidate_bill.get("payment_method")),
        "main_category": value_to_string(candidate_bill.get("main_category")),
        "sub_category": value_to_string(candidate_bill.get("sub_category")),
        "source_account_id": source_account_id,
        "destination_account_id": value_to_i64(candidate_bill.get("destination_account_id")).unwrap_or(0),
    })
}

fn should_include_candidate(kind: &str, details: &Map<String, Value>) -> bool {
    match kind {
        "reconciliation" => !value_to_string(details.get("candidate_id"))
            .trim()
            .is_empty(),
        "transfer" => {
            !value_to_string(details.get("candidate_type"))
                .trim()
                .is_empty()
                || matches!(
                    value_to_string(details.get("review_status"))
                        .trim()
                        .to_lowercase()
                        .as_str(),
                    "accepted" | "rejected"
                )
        }
        "learning" => {
            details.get("rule_id").is_some_and(|value| !value.is_null())
                || value_to_f64(details.get("score")) > 0.0
                || ["level", "reason", "recommended_type", "summary"]
                    .iter()
                    .any(|field| !value_to_string(details.get(*field)).trim().is_empty())
                || matches!(
                    value_to_string(details.get("review_status"))
                        .trim()
                        .to_lowercase()
                        .as_str(),
                    "accepted" | "rejected"
                )
        }
        "recurring" => {
            details.get("id").is_some_and(|value| !value.is_null())
                || value_to_i64(details.get("candidate_count")).unwrap_or(0) > 0
                || ["name", "match_reasons", "matched_date"]
                    .iter()
                    .any(|field| !value_to_string(details.get(*field)).trim().is_empty())
                || value_to_f64(details.get("match_score")) > 0.0
        }
        _ => false,
    }
}

fn build_session_candidate(
    session_id: &str,
    preview: &Map<String, Value>,
    kind: &str,
    details: &Map<String, Value>,
    context: &Value,
) -> Value {
    let preview_id = value_to_i64(preview.get("id")).unwrap_or(0);
    let normalized_details = build_candidate_details(kind, details);
    let (score, level, reason, status) = match kind {
        "reconciliation" => (
            value_to_f64(details.get("score")),
            value_to_string(details.get("level")),
            value_to_string(details.get("reason")),
            value_to_string(normalized_details.get("status")).if_empty("pending"),
        ),
        "recurring" => {
            let score = value_to_f64(normalized_details.get("match_score"));
            (
                score,
                derive_level(score).to_string(),
                value_to_string(normalized_details.get("match_reasons")),
                if normalized_details
                    .get("id")
                    .is_some_and(|value| !value.is_null())
                {
                    "confirmed".to_string()
                } else {
                    "pending".to_string()
                },
            )
        }
        "learning" => (
            value_to_f64(normalized_details.get("score")),
            value_to_string(normalized_details.get("level")),
            value_to_string(normalized_details.get("reason")),
            value_to_string(normalized_details.get("review_status")).if_empty("pending"),
        ),
        _ => (
            value_to_f64(normalized_details.get("score")),
            value_to_string(normalized_details.get("level")),
            value_to_string(normalized_details.get("reason")),
            value_to_string(normalized_details.get("review_status")).if_empty("pending"),
        ),
    };
    let candidate_id = if kind == "reconciliation" {
        value_to_string(normalized_details.get("candidate_id"))
            .if_empty(&format!("preview:{preview_id}:{kind}"))
    } else {
        format!("preview:{preview_id}:{kind}")
    };
    json!({
        "candidate_id": candidate_id,
        "kind": kind,
        "session_id": session_id,
        "preview_id": preview_id,
        "score": score,
        "level": level,
        "reason": reason,
        "status": status,
        "details": normalized_details,
        "preview": build_candidate_preview(preview),
        "context": context,
    })
}

fn build_candidate_details(kind: &str, details: &Map<String, Value>) -> Value {
    match kind {
        "reconciliation" => json!({
            "candidate_id": value_to_string(details.get("candidate_id")),
            "candidate_type": value_to_string(details.get("candidate_type")),
            "status": value_to_string(details.get("status")),
            "existing_bill_id": details.get("existing_bill_id").cloned().unwrap_or(Value::Null),
            "group_id": details.get("group_id").cloned().unwrap_or(Value::Null),
            "signal_label": value_to_string(details.get("signal_label")),
            "source_chain": normalize_list(details.get("source_chain")),
        }),
        "recurring" => json!({
            "id": details.get("id").cloned().unwrap_or(Value::Null),
            "name": value_to_string(details.get("name")),
            "candidate_count": value_to_i64(details.get("candidate_count")).unwrap_or(0),
            "match_score": value_to_f64(details.get("match_score")),
            "match_reasons": value_to_string(details.get("match_reasons")),
            "matched_date": value_to_string(details.get("matched_date")),
        }),
        "transfer" => json!({
            "candidate_type": value_to_string(details.get("candidate_type")),
            "score": value_to_f64(details.get("score")),
            "level": value_to_string(details.get("level")),
            "reason": value_to_string(details.get("reason")),
            "review_status": value_to_string(details.get("review_status")),
            "reviewed_type": value_to_string(details.get("reviewed_type")),
            "suppressed": details.get("suppressed").and_then(Value::as_bool).unwrap_or(false),
        }),
        _ => json!({
            "rule_id": details.get("rule_id").cloned().unwrap_or(Value::Null),
            "score": value_to_f64(details.get("score")),
            "level": value_to_string(details.get("level")),
            "reason": value_to_string(details.get("reason")),
            "recommended_type": value_to_string(details.get("recommended_type")),
            "summary": value_to_string(details.get("summary")),
            "review_status": value_to_string(details.get("review_status")),
            "suppressed": details.get("suppressed").and_then(Value::as_bool).unwrap_or(false),
        }),
    }
}

fn build_candidate_preview(preview: &Map<String, Value>) -> Value {
    json!({
        "preview_date": preview.get("preview_date").cloned().unwrap_or(json!("")),
        "preview_type": preview.get("preview_type").cloned().unwrap_or(json!("")),
        "preview_amount": preview.get("preview_amount").cloned().unwrap_or(json!(0)),
        "preview_destination_amount": preview.get("preview_destination_amount").cloned().unwrap_or(json!(0)),
        "preview_main_category": preview.get("preview_main_category").cloned().unwrap_or(json!("")),
        "preview_sub_category": preview.get("preview_sub_category").cloned().unwrap_or(json!("")),
        "preview_source_account_id": preview.get("preview_source_account_id").cloned().unwrap_or(Value::Null),
        "preview_destination_account_id": preview.get("preview_destination_account_id").cloned().unwrap_or(Value::Null),
        "preview_counterparty": preview.get("preview_counterparty").cloned().unwrap_or(json!("")),
        "preview_payment_method": preview.get("preview_payment_method").cloned().unwrap_or(json!("")),
        "preview_description": preview.get("preview_description").cloned().unwrap_or(json!("")),
        "preview_selected": preview.get("preview_selected").and_then(Value::as_bool).unwrap_or(false),
    })
}

fn serialize_bill_pair(pair: &Map<String, Value>) -> Value {
    let mut serialized = json!({
        "id": value_to_i64(pair.get("id")).unwrap_or(0),
        "pairType": value_to_string(pair.get("pair_type")).if_empty(TRANSFER_PAIR_TYPE),
        "source": value_to_string(pair.get("source")).if_empty(MANUAL_PAIR_SOURCE),
        "leftBillId": value_to_i64(pair.get("left_bill_id")).unwrap_or(0),
        "rightBillId": value_to_i64(pair.get("right_bill_id")).unwrap_or(0),
    });
    insert_i64_if_present(
        serialized.as_object_mut().expect("pair object"),
        "otherBillId",
        pair.get("other_bill_id"),
    );
    serialized
}

fn serialize_bill_snapshot(snapshot: &Map<String, Value>) -> Value {
    json!({
        "id": value_to_i64(snapshot.get("id")).unwrap_or(0),
        "date": value_to_string(snapshot.get("date")),
        "type": value_to_string(snapshot.get("type")),
        "amount": value_to_f64(snapshot.get("amount")),
        "counterparty": value_to_string(snapshot.get("counterparty")),
        "description": value_to_string(snapshot.get("description")),
        "paymentMethod": value_to_string(snapshot.get("payment_method")),
        "mainCategory": value_to_string(snapshot.get("main_category")),
        "subCategory": value_to_string(snapshot.get("sub_category")),
        "sourceAccountId": value_to_i64(snapshot.get("source_account_id")).unwrap_or(0),
        "destinationAccountId": value_to_i64(snapshot.get("destination_account_id")).unwrap_or(0),
    })
}

fn parse_positive_request_int(
    object: &Map<String, Value>,
    field_name: &str,
) -> Result<i64, &'static str> {
    let Some(value) = object.get(field_name) else {
        return Err("billId and candidateBillId are required");
    };
    match value {
        Value::Bool(_) => Err("Invalid request"),
        Value::Number(number) => number
            .as_i64()
            .filter(|value| *value > 0)
            .ok_or("Invalid request"),
        Value::String(text)
            if text
                .trim()
                .chars()
                .all(|character| character.is_ascii_digit()) =>
        {
            text.trim()
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .ok_or("Invalid request")
        }
        _ => Err("Invalid request"),
    }
}

fn parse_optional_positive_query_int(
    query: &Map<String, Value>,
    field_name: &str,
) -> Result<Option<i64>, String> {
    let Some(value) = query.get(field_name) else {
        return Ok(None);
    };
    if value.is_null() || value_to_string(Some(value)).is_empty() {
        return Ok(None);
    }
    let parsed = value_to_i64(Some(value)).ok_or_else(|| format!("Invalid {field_name}"))?;
    if parsed <= 0 {
        return Err(format!("Invalid {field_name}"));
    }
    Ok(Some(parsed))
}

fn pair_time_diff_seconds(
    left_bill: &Map<String, Value>,
    right_bill: &Map<String, Value>,
    lookback_days: i64,
) -> Option<i64> {
    let left_datetime = parse_bill_datetime(&value_to_string(left_bill.get("date")))?;
    let right_datetime = parse_bill_datetime(&value_to_string(right_bill.get("date")))?;
    let diff_seconds = (left_datetime.inner() - right_datetime.inner())
        .num_seconds()
        .abs();
    let max_window_seconds = lookback_days.max(0) * 24 * 60 * 60;
    (max_window_seconds > 0 && diff_seconds <= max_window_seconds).then_some(diff_seconds)
}

fn is_explicit_transfer_type(raw_type: Option<&Value>) -> bool {
    matches!(
        value_to_string(raw_type).trim().to_lowercase().as_str(),
        "转账" | "transfer"
    )
}

fn parse_positive_i64(text: &str) -> Option<i64> {
    text.parse::<i64>().ok().filter(|value| *value > 0)
}

fn value_to_string(value: Option<&Value>) -> String {
    value.map(value_to_string_value).unwrap_or_default()
}

fn value_to_string_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn value_to_i64(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| number.as_f64().map(|value| value as i64)),
        Some(Value::String(text)) if !text.trim().is_empty() => text.trim().parse::<i64>().ok(),
        Some(Value::Bool(true)) => Some(1),
        Some(Value::Bool(false)) => Some(0),
        _ => None,
    }
}

fn value_to_f64(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or_default(),
        Some(Value::String(text)) => text.trim().parse::<f64>().unwrap_or_default(),
        Some(Value::Bool(true)) => 1.0,
        _ => 0.0,
    }
}

fn python_json_object_string(pairs: &[(&str, String)]) -> String {
    let body = pairs
        .iter()
        .map(|(key, value)| {
            format!(
                "{}: {}",
                serde_json::to_string(key).expect("json key"),
                serde_json::to_string(value).expect("json value")
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{body}}}")
}

fn hex_prefix(bytes: &[u8], length: usize) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .chars()
        .take(length)
        .collect()
}

fn normalize_list(value: Option<&Value>) -> Vec<Value> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values.clone(),
        Some(value) => vec![value.clone()],
    }
}

fn recurring_sample_bill_ids(item: &Map<String, Value>) -> Vec<Value> {
    if let Some(Value::Array(values)) = item.get("sample_bill_ids") {
        return values.clone();
    }
    let raw_json = value_to_string(item.get("sample_bill_ids_json"));
    if raw_json.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str::<Value>(&raw_json)
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

fn insert_i64_if_present(object: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    if let Some(value) = value.and_then(|value| value_to_i64(Some(value))) {
        object.insert(key.to_string(), json!(value));
    }
}

fn insert_string_if_present(object: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    let text = value_to_string(value);
    if !text.is_empty() {
        object.insert(key.to_string(), json!(text));
    }
}

fn optional_string_query(query: &Map<String, Value>, key: &str) -> Option<String> {
    let text = value_to_string(query.get(key)).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn optional_lower_query(query: &Map<String, Value>, key: &str) -> Option<String> {
    optional_string_query(query, key).map(|value| value.to_lowercase())
}

fn dedupe_keywords<I>(keywords: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for keyword in keywords {
        let text = keyword.trim().to_string();
        if text.is_empty() {
            continue;
        }
        if seen.insert(text.to_lowercase()) {
            result.push(text);
        }
    }
    result
}

fn sort_by_len_desc(values: &[String]) -> Vec<String> {
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    sorted
}

fn value_array_strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|values| values.iter().map(value_to_string_value).collect())
        .unwrap_or_default()
}

fn calculate_learning_feature_similarity(field: &str, bill_value: &str, rule_value: &str) -> f64 {
    if bill_value.is_empty() || rule_value.is_empty() {
        return 0.0;
    }
    if bill_value == rule_value {
        return 1.0;
    }
    if field == "parser_id" {
        return 0.0;
    }
    if bill_value.contains(rule_value) || rule_value.contains(bill_value) {
        return 0.92;
    }
    let bill_parts = split_learning_feature_parts(bill_value);
    let rule_parts = split_learning_feature_parts(rule_value);
    let overlap_score = if bill_parts.is_empty() || rule_parts.is_empty() {
        0.0
    } else {
        let intersection = bill_parts.intersection(&rule_parts).count() as f64;
        let union = bill_parts.union(&rule_parts).count() as f64;
        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    };
    overlap_score.max(sequence_similarity(bill_value, rule_value))
}

fn split_learning_feature_parts(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, '|' | ',' | '，' | '/' | '、' | '_' | '-' | '－')
        })
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn sequence_similarity(left: &str, right: &str) -> f64 {
    let left_chars: Vec<char> = left.chars().collect();
    let right_chars: Vec<char> = right.chars().collect();
    if left_chars.is_empty() || right_chars.is_empty() {
        return 0.0;
    }
    let matching_chars = sequence_matcher_matching_chars(
        &left_chars,
        0,
        left_chars.len(),
        &right_chars,
        0,
        right_chars.len(),
    );
    (2.0 * matching_chars as f64) / (left_chars.len() + right_chars.len()) as f64
}

fn sequence_matcher_matching_chars(
    left: &[char],
    left_start: usize,
    left_end: usize,
    right: &[char],
    right_start: usize,
    right_end: usize,
) -> usize {
    let (best_left, best_right, best_size) =
        find_longest_contiguous_match(left, left_start, left_end, right, right_start, right_end);
    if best_size == 0 {
        return 0;
    }
    let before = if left_start < best_left && right_start < best_right {
        sequence_matcher_matching_chars(left, left_start, best_left, right, right_start, best_right)
    } else {
        0
    };
    let after_left_start = best_left + best_size;
    let after_right_start = best_right + best_size;
    let after = if after_left_start < left_end && after_right_start < right_end {
        sequence_matcher_matching_chars(
            left,
            after_left_start,
            left_end,
            right,
            after_right_start,
            right_end,
        )
    } else {
        0
    };
    before + best_size + after
}

fn find_longest_contiguous_match(
    left: &[char],
    left_start: usize,
    left_end: usize,
    right: &[char],
    right_start: usize,
    right_end: usize,
) -> (usize, usize, usize) {
    let mut best_left = left_start;
    let mut best_right = right_start;
    let mut best_size = 0usize;
    for left_index in left_start..left_end {
        for right_index in right_start..right_end {
            let mut size = 0usize;
            while left_index + size < left_end
                && right_index + size < right_end
                && left[left_index + size] == right[right_index + size]
            {
                size += 1;
            }
            if size > best_size {
                best_left = left_index;
                best_right = right_index;
                best_size = size;
            }
        }
    }
    (best_left, best_right, best_size)
}

fn find_object_by_id(values: &[Value], target_id: i64) -> Option<&Map<String, Value>> {
    values
        .iter()
        .filter_map(Value::as_object)
        .find(|object| value_to_i64(object.get("id")).is_some_and(|value| value == target_id))
}

fn extract_named_product(raw_text: &str, platform: &str) -> Option<String> {
    let normalized = raw_text.replace(['|', '｜', ',', '，'], " ");
    for token in normalized.split_whitespace() {
        let cleaned = clean_investment_product_name(token, platform);
        if cleaned.chars().count() >= 2
            && [
                "基金", "ETF", "LOF", "REITs", "REIT", "理财", "计划", "组合", "债券", "股票",
                "黄金",
            ]
            .iter()
            .any(|suffix| cleaned.to_lowercase().contains(&suffix.to_lowercase()))
        {
            return Some(cleaned);
        }
    }
    let compact = clean_investment_product_name(raw_text, platform);
    if compact.chars().count() >= 2
        && [
            "基金", "ETF", "LOF", "REITs", "REIT", "理财", "计划", "组合", "债券", "股票", "黄金",
        ]
        .iter()
        .any(|suffix| compact.to_lowercase().contains(&suffix.to_lowercase()))
    {
        return Some(compact);
    }
    None
}

fn clean_investment_product_name(product: &str, platform: &str) -> String {
    let mut cleaned = product
        .trim()
        .trim_matches(['|', '｜', ',', '，', ' '])
        .to_string();
    for (canonical, aliases) in PLATFORM_ALIASES {
        for alias in std::iter::once(canonical).chain(aliases.iter()) {
            if cleaned.to_lowercase().starts_with(&alias.to_lowercase()) {
                cleaned = cleaned[alias.len()..]
                    .trim_start_matches(['-', '－', ':', '：', ' '])
                    .to_string();
            }
        }
    }
    if !platform.is_empty() && cleaned.to_lowercase().starts_with(&platform.to_lowercase()) {
        cleaned = cleaned[platform.len()..]
            .trim_start_matches(['-', '－', ':', '：', ' '])
            .to_string();
    }
    let suffixes = [
        "买入",
        "卖出",
        "申购",
        "赎回",
        "定投",
        "扣款",
        "自动定投",
        "转入",
        "转出",
        "确认份额",
        "分红再投资",
    ];
    loop {
        let original = cleaned.clone();
        for prefix in INVESTMENT_ACTION_KEYWORDS {
            for separator in ["", "-", "－", ":", "：", " "] {
                let candidate = format!("{prefix}{separator}");
                if cleaned
                    .to_lowercase()
                    .starts_with(&candidate.to_lowercase())
                {
                    cleaned = cleaned[candidate.len()..].trim().to_string();
                }
            }
        }
        for suffix in suffixes {
            if cleaned.to_lowercase().ends_with(&suffix.to_lowercase()) {
                let end = cleaned.len().saturating_sub(suffix.len());
                cleaned = cleaned[..end]
                    .trim_end_matches(['-', '－', ':', '：', ' '])
                    .to_string();
            }
        }
        if cleaned == original {
            break;
        }
    }
    cleaned.chars().take(80).collect()
}

fn contains_any(text_lower: &str, keywords: &[&str]) -> Vec<String> {
    let mut matches: Vec<String> = keywords
        .iter()
        .filter(|keyword| text_lower.contains(&keyword.to_lowercase()))
        .map(|keyword| (*keyword).to_string())
        .collect();
    matches.sort_by(|left, right| {
        right
            .chars()
            .count()
            .cmp(&left.chars().count())
            .then_with(|| left.cmp(right))
    });
    matches
}

fn append_unique(items: &mut Vec<String>, value: &str) {
    let normalized = value.trim();
    if !normalized.is_empty() && !items.iter().any(|item| item == normalized) {
        items.push(normalized.to_string());
    }
}

fn join_nonempty<const N: usize>(parts: [String; N]) -> String {
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn sample_stdev(values: &[f64]) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }
    let avg = mean(values);
    let variance = values
        .iter()
        .map(|value| (value - avg).powi(2))
        .sum::<f64>()
        / (values.len() - 1) as f64;
    variance.sqrt()
}

fn derive_level(score: f64) -> &'static str {
    if score >= 0.8 {
        "high"
    } else if score >= 0.65 {
        "medium"
    } else if score > 0.0 {
        "low"
    } else {
        ""
    }
}

fn round1(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

trait StringFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl StringFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}
