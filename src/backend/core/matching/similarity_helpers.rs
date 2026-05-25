// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

fn hex_prefix(bytes: &[u8], length: usize) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .chars()
        .take(length)
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
fn insert_i64_if_present(object: &mut Map<String, Value>, key: &str, value: Option<&Value>) {
    if let Some(value) = value.and_then(|value| value_to_i64(Some(value))) {
        object.insert(key.to_string(), json!(value));
    }
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
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
