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
