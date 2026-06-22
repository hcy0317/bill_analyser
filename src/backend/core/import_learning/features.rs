use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportLearningTrainingSample {
    pub sample_id: i64,
    pub features: BTreeMap<String, String>,
    pub semantic_label: String,
    pub route_label: String,
}

#[tracing::instrument(level = "debug", skip_all)]
/// 归一普通学习文本，用于分类、账户和规则标签的稳定比较。
pub fn normalize_learning_text(raw_value: Option<&Value>) -> String {
    let text = value_to_string(raw_value).trim().to_lowercase();
    collapse_whitespace(&text)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 归一导入学习文本，额外保留复合字段中的管道分隔语义。
pub fn normalize_import_learning_text(raw_value: Option<&Value>) -> String {
    let text = value_to_string(raw_value).trim().to_lowercase();
    if text.is_empty() {
        return String::new();
    }
    let pipe_parts: Vec<String> = text
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    let normalized = if pipe_parts.is_empty() {
        text
    } else {
        pipe_parts.join(" | ")
    };
    collapse_whitespace(&normalized)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 从 JSON 值中提取合法的正数学习建议 ID。
pub fn normalize_import_learning_suggestion_id(raw_value: Option<&Value>) -> Option<i64> {
    let normalized = optional_positive_i64(raw_value);
    normalized.filter(|value| *value > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 构造复合匹配特征，至少保留两个非空维度才允许进入学习匹配。
pub fn build_composite_match_features(
    parser_id: &str,
    counterparty: &str,
    description: &str,
    payment_method: &str,
) -> Option<BTreeMap<String, String>> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_composite_match_features",
        "business operation entered"
    );
    let mut features = BTreeMap::new();
    for (key, raw_value) in [
        ("parser_id", parser_id),
        ("counterparty", counterparty),
        ("description", description),
        ("payment_method", payment_method),
    ] {
        let normalized =
            normalize_import_learning_text(Some(&Value::String(raw_value.to_string())));
        if !normalized.is_empty() {
            features.insert(key.to_string(), normalized);
        }
    }
    if features.len() < 2 {
        None
    } else {
        Some(features)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 基于复合匹配特征生成稳定哈希字符串，供导入学习候选复用。
pub fn build_composite_match_hash(
    parser_id: &str,
    counterparty: &str,
    description: &str,
    payment_method: &str,
) -> Option<String> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_composite_match_hash",
        "business operation entered"
    );
    build_composite_match_features(parser_id, counterparty, description, payment_method)
        .map(|features| composite_hash_from_features(&features))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将复合特征 map 序列化为稳定短键格式，避免顺序差异影响匹配。
pub fn composite_hash_from_features(features: &BTreeMap<String, String>) -> String {
    features
        .iter()
        .filter_map(|(key, value)| {
            let alias = match key.as_str() {
                "parser_id" => "p",
                "counterparty" => "c",
                "description" => "d",
                "payment_method" => "m",
                _ => return None,
            };
            Some(format!("{alias}={value}"))
        })
        .collect::<Vec<_>>()
        .join("|")
}

#[tracing::instrument(level = "debug", skip_all)]
/// 解析历史复合匹配值，兼容短键和旧式字段名。
pub fn parse_composite_match_value(raw_value: Option<&Value>) -> Option<BTreeMap<String, String>> {
    let mut features = BTreeMap::new();
    for part in value_to_string(raw_value).split('|') {
        let Some((raw_key, raw_feature_value)) = part.split_once('=') else {
            continue;
        };
        let key = match normalize_import_learning_text(Some(&Value::String(raw_key.to_string())))
            .as_str()
        {
            "p" | "parser" | "parser_id" => "parser_id",
            "c" | "counterparty" => "counterparty",
            "d" | "description" => "description",
            "m" | "payment" | "payment_method" => "payment_method",
            _ => continue,
        };
        let value =
            normalize_import_learning_text(Some(&Value::String(raw_feature_value.to_string())));
        if !value.is_empty() {
            features.insert(key.to_string(), value);
        }
    }
    if features.len() < 2 {
        None
    } else {
        Some(features)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将金额分桶为粗粒度区间，避免学习特征直接绑定精确金额。
pub fn amount_cents_bucket(raw_amount_cents: Option<&Value>) -> &'static str {
    let amount_cents = raw_amount_cents.map(value_to_i64).unwrap_or_default().abs();
    if amount_cents == 0 {
        "zero"
    } else if amount_cents < 2_000 {
        "lt20"
    } else if amount_cents < 10_000 {
        "lt100"
    } else if amount_cents < 50_000 {
        "lt500"
    } else {
        "gte500"
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 构造分类语义标签，记录学习样本的交易类型和分类 ID。
pub fn build_semantic_label(row: &Map<String, Value>) -> String {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_semantic_label",
        "business operation entered"
    );
    let learned_type = normalize_learning_text(row.get("annotated_type"));
    let category_id = optional_nonnegative_i64(row.get("annotated_category_id")).unwrap_or(0);
    format!("type={learned_type}|category={category_id}")
}

#[tracing::instrument(level = "debug", skip_all)]
/// 构造账户路由标签，记录学习样本的来源和目标账户 ID。
pub fn build_route_label(row: &Map<String, Value>) -> String {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_route_label",
        "business operation entered"
    );
    let source_id = optional_nonnegative_i64(row.get("annotated_source_account_id")).unwrap_or(0);
    let destination_id =
        optional_nonnegative_i64(row.get("annotated_destination_account_id")).unwrap_or(0);
    format!("source={source_id}|destination={destination_id}")
}

#[tracing::instrument(level = "debug", skip_all)]
/// 解析分类语义标签，供模型评估或回放时恢复业务字段。
pub fn parse_semantic_label(label: &str) -> SemanticLabelParts {
    let mut result = SemanticLabelParts::default();
    for part in label.split('|') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "type" => result.transaction_type = value.to_string(),
            "category" => {
                result.category_id =
                    optional_nonnegative_i64(Some(&Value::String(value.to_string())))
                        .filter(|value| *value > 0);
            }
            _ => {}
        }
    }
    result
}

#[tracing::instrument(level = "debug", skip_all)]
/// 解析账户路由标签，供模型评估或回放时恢复账户字段。
pub fn parse_route_label(label: &str) -> RouteLabelParts {
    let mut result = RouteLabelParts::default();
    for part in label.split('|') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        let normalized_id = optional_nonnegative_i64(Some(&Value::String(value.to_string())))
            .filter(|value| *value > 0);
        match key {
            "source" => result.source_account_id = normalized_id,
            "destination" => result.destination_account_id = normalized_id,
            _ => {}
        }
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SemanticLabelParts {
    #[serde(rename = "type")]
    pub transaction_type: String,
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RouteLabelParts {
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[tracing::instrument(level = "debug", skip_all)]
/// 汇总学习样本的文本、金额区间和预览类型特征。
pub fn build_feature_payload(row: &Map<String, Value>) -> BTreeMap<String, String> {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_feature_payload",
        "business operation entered"
    );
    let snapshot = snapshot_payload(row.get("source_snapshot_json"));
    let preview_type = normalize_learning_text(snapshot.get("preview_type"));
    let preview_type = if preview_type.is_empty() {
        normalize_learning_text(row.get("annotated_type"))
    } else {
        preview_type
    };
    BTreeMap::from([
        (
            "parser_id".to_string(),
            normalize_learning_text(row.get("parser_id")),
        ),
        (
            "counterparty".to_string(),
            normalize_learning_text(row.get("counterparty")),
        ),
        (
            "description".to_string(),
            normalize_learning_text(row.get("description")),
        ),
        (
            "payment_method".to_string(),
            normalize_learning_text(row.get("payment_method")),
        ),
        (
            "amount_bucket".to_string(),
            amount_cents_bucket(snapshot.get("preview_amount_cents")).to_string(),
        ),
        ("preview_type".to_string(), preview_type),
    ])
}

#[tracing::instrument(level = "debug", skip_all)]
/// 从数据库行集合中筛出可训练样本，并丢弃无标签或特征不足的数据。
pub fn prepare_training_samples(rows: &[Value]) -> Vec<ImportLearningTrainingSample> {
    rows.iter()
        .filter_map(Value::as_object)
        .filter_map(|row| {
            let sample_id = optional_nonnegative_i64(row.get("id")).unwrap_or(0);
            if sample_id <= 0 {
                return None;
            }
            let semantic_label = build_semantic_label(row);
            let route_label = build_route_label(row);
            if semantic_label == "type=|category=0" && route_label == "source=0|destination=0" {
                return None;
            }
            let features = build_feature_payload(row);
            let text_feature_count = ["parser_id", "counterparty", "description", "payment_method"]
                .iter()
                .filter(|key| features.get(**key).is_some_and(|value| !value.is_empty()))
                .count();
            if text_feature_count < 2 {
                return None;
            }
            Some(ImportLearningTrainingSample {
                sample_id,
                features,
                semantic_label,
                route_label,
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将特征 map 展开为召回 token，供向量或规则学习索引消费。
pub fn iter_feature_tokens(features: &BTreeMap<String, String>) -> Vec<String> {
    let mut tokens = Vec::new();
    if let Some(parser_id) = features
        .get("parser_id")
        .map(|value| normalize_learning_text(Some(&Value::String(value.clone()))))
        .filter(|value| !value.is_empty())
    {
        tokens.push(format!("parser_id={parser_id}"));
    }
    for field in ["counterparty", "description", "payment_method"] {
        tokens.extend(iter_text_tokens(
            field,
            features.get(field).map(String::as_str).unwrap_or(""),
        ));
    }
    if let Some(amount_bucket) = features
        .get("amount_bucket")
        .map(|value| normalize_learning_text(Some(&Value::String(value.clone()))))
        .filter(|value| !value.is_empty())
    {
        tokens.push(format!("amount_bucket={amount_bucket}"));
    }
    if let Some(preview_type) = features
        .get("preview_type")
        .map(|value| normalize_learning_text(Some(&Value::String(value.clone()))))
        .filter(|value| !value.is_empty())
    {
        tokens.push(format!("preview_type={preview_type}"));
    }
    tokens
}
