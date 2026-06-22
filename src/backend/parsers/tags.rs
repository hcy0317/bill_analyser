use serde_json::Value;

/// 归一 parser tag 列表，统一小写、去空值并保持首次出现顺序去重。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_parser_tags(raw_tags: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<String> {
    let mut normalized = Vec::new();
    for raw_tag in raw_tags {
        let tag = raw_tag.as_ref().trim().to_lowercase();
        if !tag.is_empty() && !normalized.contains(&tag) {
            normalized.push(tag);
        }
    }
    normalized
}

/// 从 JSON 值中恢复 parser tag 列表，兼容字符串、数组和空值三类输入。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_parser_tags_value(raw_tags: Option<&Value>) -> Vec<String> {
    match raw_tags {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(text)) => normalize_parser_tags_text(text),
        Some(Value::Array(items)) => normalize_parser_tags(items.iter().map(normalize_json_tag)),
        _ => Vec::new(),
    }
}

/// 从文本字段中解析 parser tag，优先兼容 JSON 文本，失败时按逗号切分。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_parser_tags_text(raw_tags: &str) -> Vec<String> {
    let text = raw_tags.trim();
    if text.is_empty() {
        return Vec::new();
    }

    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return normalize_parser_tags_value(Some(&value));
    }

    normalize_parser_tags(text.split(','))
}

/// 根据 parser id、支付方式和渠道生成默认 parser/channel 标签。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_parser_tags(parser_id: &str, payment_method: &str, channel: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let normalized_parser_id = parser_id.trim().to_lowercase();
    if !normalized_parser_id.is_empty() {
        tags.push(format!("parser:{normalized_parser_id}"));
    }

    if let Some(channel_tag) = detect_channel_tag(parser_id, payment_method, channel) {
        tags.push(format!("channel:{channel_tag}"));
    }

    normalize_parser_tags(tags)
}

/// 解析来源 tag 字段；缺失时按 parser id 与渠道信息生成默认标签。
#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_parser_tags(
    raw_tags: Option<&Value>,
    parser_id: &str,
    payment_method: &str,
    channel: &str,
) -> Vec<String> {
    let normalized = normalize_parser_tags_value(raw_tags);
    if normalized.is_empty() {
        build_parser_tags(parser_id, payment_method, channel)
    } else {
        normalized
    }
}

/// 将最终 parser tag 列表序列化为 JSON 文本，供旧表字段和导入 evidence 保存。
#[tracing::instrument(level = "debug", skip_all)]
pub fn serialize_parser_tags(
    raw_tags: Option<&Value>,
    parser_id: &str,
    payment_method: &str,
    channel: &str,
) -> String {
    serde_json::to_string(&resolve_parser_tags(
        raw_tags,
        parser_id,
        payment_method,
        channel,
    ))
    .unwrap_or_else(|_| "[]".to_string())
}

fn detect_channel_tag(
    parser_id: &str,
    payment_method: &str,
    channel: &str,
) -> Option<&'static str> {
    match parser_id.trim().to_lowercase().as_str() {
        "wechat" | "alipay" => return Some("wallet"),
        "abc" | "ccb" | "cmbc" | "icbc" => return Some("bank"),
        _ => {}
    }

    let combined = format!("{payment_method} {channel}").to_lowercase();
    if ["wechat", "微信", "alipay", "支付宝", "零钱", "wallet"]
        .iter()
        .any(|keyword| combined.contains(keyword))
    {
        return Some("wallet");
    }
    if ["信用卡", "credit"]
        .iter()
        .any(|keyword| combined.contains(keyword))
    {
        return Some("credit_card");
    }
    if ["银行", "bank", "借记卡", "储蓄卡"]
        .iter()
        .any(|keyword| combined.contains(keyword))
    {
        return Some("bank");
    }
    None
}

fn normalize_json_tag(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(false) => String::new(),
        Value::Bool(true) => "true".to_string(),
        Value::Number(number) if is_json_zero_number(number) => String::new(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn is_json_zero_number(number: &serde_json::Number) -> bool {
    number.as_i64() == Some(0) || number.as_u64() == Some(0) || number.as_f64() == Some(0.0)
}
