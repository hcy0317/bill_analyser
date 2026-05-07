use chrono::{Datelike, Local, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use unicode_normalization::UnicodeNormalization;

pub const OCR_DISABLED_PROVIDER_NAME: &str = "disabled";
pub const OCR_DEFAULT_LANG: &str = "chi_sim+eng";
pub const OCR_AVAILABLE_PROVIDERS: [&str; 2] = ["cloud_stub", "tesseract"];
pub const LLM_AVAILABLE_PROVIDERS: [&str; 13] = [
    "openai",
    "claude",
    "anthropic",
    "deepseek",
    "ollama",
    "xai",
    "google",
    "openrouter",
    "openai_compatible",
    "openai-compatible",
    "azure",
    "azure_openai",
    "azure-openai",
];

const LLM_PROMPT_TEXT_LIMIT: usize = 12_000;
const OCR_ERROR_PROVIDER_UNCONFIGURED: &str = "provider_unconfigured";
const OCR_ERROR_TIMEOUT: &str = "timeout";
const OCR_ERROR_PARSE: &str = "parse_error";
const OCR_ERROR_CANCELLED: &str = "cancelled";
const OCR_ERROR_RATE_LIMITED: &str = "rate_limited";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrConfigContract {
    pub provider: String,
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentScreenshotParseContract {
    /// Yuan-unit OCR extraction for Python API parity; storage conversions to cents happen later.
    pub amount: Option<f64>,
    pub trade_time: Option<String>,
    pub description: Option<String>,
    pub payment_platform: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRouteResponse {
    pub status_code: u16,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderConfigContract {
    pub provider: String,
    pub normalized_provider: String,
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub provider_name: String,
}

pub fn normalize_ocr_config(value: Option<&Value>) -> OcrConfigContract {
    let object = value.and_then(Value::as_object);
    let provider = object
        .and_then(|item| item.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or(OCR_DISABLED_PROVIDER_NAME);
    let lang = object
        .and_then(|item| item.get("lang"))
        .and_then(Value::as_str)
        .unwrap_or(OCR_DEFAULT_LANG);

    OcrConfigContract {
        provider: normalize_ocr_provider_name(provider),
        lang: normalize_ocr_lang(lang),
    }
}

pub fn normalize_ocr_provider_name(provider: &str) -> String {
    let normalized = provider.trim().to_lowercase();
    if normalized.is_empty() || matches!(normalized.as_str(), "none" | "off") {
        return OCR_DISABLED_PROVIDER_NAME.to_string();
    }
    if normalized == OCR_DISABLED_PROVIDER_NAME
        || OCR_AVAILABLE_PROVIDERS.contains(&normalized.as_str())
    {
        normalized
    } else {
        OCR_DISABLED_PROVIDER_NAME.to_string()
    }
}

pub fn normalize_ocr_lang(lang: &str) -> String {
    let normalized = lang.trim();
    if normalized.is_empty()
        || normalized.chars().count() > 64
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '.' | '-'))
    {
        return OCR_DEFAULT_LANG.to_string();
    }
    normalized.to_string()
}

pub fn ocr_available_providers_with_disabled() -> Vec<String> {
    std::iter::once(OCR_DISABLED_PROVIDER_NAME)
        .chain(OCR_AVAILABLE_PROVIDERS)
        .map(str::to_string)
        .collect()
}

pub fn build_ocr_config_response_payload(config: &OcrConfigContract) -> Value {
    json!({
        "provider": config.provider,
        "lang": config.lang,
        "available_providers": ocr_available_providers_with_disabled(),
        "configured": config.provider != OCR_DISABLED_PROVIDER_NAME,
    })
}

pub fn build_ocr_config_success_response(config: &OcrConfigContract) -> AiRouteResponse {
    AiRouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": build_ocr_config_response_payload(config),
        }),
    }
}

pub fn build_unknown_ocr_provider_response() -> AiRouteResponse {
    AiRouteResponse {
        status_code: 400,
        body: json!({
            "success": false,
            "error": "Bad Request",
            "message": "Unknown OCR provider",
        }),
    }
}

pub fn build_ocr_error_response(code: &str, message: Option<&str>) -> AiRouteResponse {
    let message_text = message.unwrap_or_default().trim();
    let fallback = if code == OCR_ERROR_PROVIDER_UNCONFIGURED {
        "Receipt recognition not implemented"
    } else {
        code
    };
    let resolved_message = if message_text.is_empty() {
        fallback
    } else {
        message_text
    };

    AiRouteResponse {
        status_code: ocr_error_http_status(code),
        body: json!({
            "success": false,
            "errorCode": code,
            "errorMessage": resolved_message,
            "message": resolved_message,
        }),
    }
}

pub fn ocr_error_http_status(code: &str) -> u16 {
    match code {
        OCR_ERROR_PROVIDER_UNCONFIGURED => 501,
        OCR_ERROR_TIMEOUT => 504,
        OCR_ERROR_PARSE => 422,
        OCR_ERROR_CANCELLED => 499,
        OCR_ERROR_RATE_LIMITED => 429,
        _ => 500,
    }
}

pub fn parse_payment_screenshot_text(text: &str) -> PaymentScreenshotParseContract {
    let normalized_text = normalize_text(text);
    if normalized_text.is_empty() {
        return PaymentScreenshotParseContract {
            amount: None,
            trade_time: None,
            description: None,
            payment_platform: None,
            confidence: 0.0,
        };
    }

    let lines = normalize_lines(&normalized_text);
    let amount = parse_amount(&normalized_text);
    let trade_time = parse_trade_time(&normalized_text);
    let description = parse_description(&lines);
    let payment_platform = detect_payment_platform(&normalized_text).map(str::to_string);
    let confidence = score_ocr_confidence(
        amount,
        trade_time.as_deref(),
        description.as_deref(),
        payment_platform.as_deref(),
    );

    PaymentScreenshotParseContract {
        amount,
        trade_time,
        description,
        payment_platform,
        confidence,
    }
}

pub fn llm_available_providers() -> Vec<String> {
    LLM_AVAILABLE_PROVIDERS
        .iter()
        .map(|item| (*item).to_string())
        .collect()
}

pub fn normalize_llm_provider_name(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "" => "openai".to_string(),
        "anthropic" => "claude".to_string(),
        "openai-compatible" => "openai_compatible".to_string(),
        "azure_openai" | "azure-openai" => "azure".to_string(),
        normalized => normalized.to_string(),
    }
}

pub fn build_llm_provider_config(
    provider: &str,
    provider_config: Option<&Value>,
) -> Result<LlmProviderConfigContract, String> {
    let normalized_provider = normalize_llm_provider_name(provider);
    if !is_llm_provider_creatable(&normalized_provider) {
        return Err(format!(
            "Unknown provider '{provider}'. Available: {:?}",
            llm_available_providers()
        ));
    }

    let config = provider_config.and_then(Value::as_object);
    let explicit_base_url = first_non_empty_field(config, "base_url");
    let base_url = if normalized_provider == "azure" {
        let base_url = explicit_base_url
            .ok_or_else(|| "Azure provider requires explicit base_url".to_string())?;
        validate_azure_base_url(&base_url)?;
        base_url
    } else {
        explicit_base_url
            .or_else(|| default_llm_base_url(&normalized_provider).map(str::to_string))
            .unwrap_or_default()
    };
    let model = first_non_empty_field(config, "model")
        .or_else(|| default_llm_model(&normalized_provider).map(str::to_string))
        .unwrap_or_default();
    let provider_kind = if normalized_provider == "claude" {
        "claude"
    } else if normalized_provider == "ollama" {
        "ollama"
    } else {
        "openai_compatible"
    };
    let provider_name = if provider_kind == "openai_compatible" {
        normalized_provider.clone()
    } else {
        provider_kind.to_string()
    };

    Ok(LlmProviderConfigContract {
        provider: provider.trim().to_lowercase(),
        normalized_provider,
        provider_kind: provider_kind.to_string(),
        base_url,
        model,
        provider_name,
    })
}

pub fn normalize_llm_advanced_settings(settings: Option<&Value>) -> Map<String, Value> {
    let loaded = decode_settings_object(settings);
    let mut normalized = Map::new();

    if let Some(reasoning_depth) = loaded
        .get("reasoning_depth")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !matches!(value.as_str(), "" | "auto" | "default" | "none"))
        .filter(|value| matches!(value.as_str(), "low" | "medium" | "high"))
    {
        normalized.insert("reasoning_depth".to_string(), json!(reasoning_depth));
    }

    if let Some(temperature) = normalize_float_setting(loaded.get("temperature"), 0.0, 2.0) {
        normalized.insert("temperature".to_string(), json!(temperature));
    }

    if let Some(max_tokens) = normalize_int_setting(loaded.get("max_tokens"), 1, 200_000) {
        normalized.insert("max_tokens".to_string(), json!(max_tokens));
    }

    for key in [
        "system_prompt",
        "classification_prompt_template",
        "rule_prompt_template",
    ] {
        if let Some(prompt) = loaded
            .get(key)
            .and_then(Value::as_str)
            .map(normalize_prompt_text)
            .filter(|value| !value.is_empty())
        {
            normalized.insert(key.to_string(), json!(prompt));
        }
    }

    normalized
}

pub fn safe_llm_config_payload(config: &Value) -> Value {
    let mut safe_config = config.as_object().cloned().unwrap_or_default();
    safe_config.insert(
        "advanced_settings".to_string(),
        Value::Object(normalize_llm_advanced_settings(
            safe_config.get("advanced_settings"),
        )),
    );
    let has_api_key = object_has_non_empty_secret(&safe_config);
    redact_secrets_in_map(&mut safe_config);
    safe_config.insert("has_api_key".to_string(), json!(has_api_key));
    safe_config
        .entry("api_key".to_string())
        .or_insert_with(|| json!(""));
    Value::Object(safe_config)
}

pub fn build_runtime_llm_config_from_saved_config(config: &Value) -> Value {
    let object = config.as_object();
    json!({
        "enabled": true,
        "provider": string_field_or(object, "provider", "openai"),
        "advanced_settings": Value::Object(normalize_llm_advanced_settings(
            object.and_then(|item| item.get("advanced_settings")),
        )),
        "provider_config": {
            "api_key": string_field_or(object, "api_key", ""),
            "base_url": string_field_or(object, "base_url", ""),
            "model": string_field_or(object, "model", ""),
        },
    })
}

pub fn copy_runtime_llm_config(config: &Value) -> Value {
    let object = config.as_object();
    let provider_config = object
        .and_then(|item| item.get("provider_config"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let advanced_source = object
        .and_then(|item| item.get("advanced_settings"))
        .filter(|value| !is_falsy_settings_value(value))
        .or_else(|| provider_config.get("advanced_settings"));

    json!({
        "enabled": object
            .and_then(|item| item.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        "provider": string_field_or(object, "provider", "openai"),
        "provider_config": provider_config,
        "advanced_settings": Value::Object(normalize_llm_advanced_settings(advanced_source)),
    })
}

pub fn build_llm_config_get_response(config: &Value) -> AiRouteResponse {
    let copied = copy_runtime_llm_config(config);
    let copied_object = copied.as_object();
    let provider_config = copied_object
        .and_then(|item| item.get("provider_config"))
        .and_then(Value::as_object);
    AiRouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": {
                "enabled": copied_object
                    .and_then(|item| item.get("enabled"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "provider": string_field_or(copied_object, "provider", "openai"),
                "model": string_field_or(provider_config, "model", ""),
                "advanced_settings": copied
                    .get("advanced_settings")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
                "available_providers": llm_available_providers(),
            },
        }),
    }
}

pub fn build_llm_contract_error_response(
    message: &str,
    code: &str,
    status_code: u16,
) -> AiRouteResponse {
    AiRouteResponse {
        status_code,
        body: json!({
            "success": false,
            "error": message,
            "code": code,
            "error_code": code,
        }),
    }
}

pub fn build_llm_preview_recommend_response(session_id: &str, suggestions: Vec<Value>) -> Value {
    json!({
        "success": true,
        "data": {
            "session_id": session_id,
            "count": suggestions.len(),
            "suggestions": suggestions,
        },
    })
}

pub fn build_llm_candidate_list_response(candidates: Vec<Value>, total: i64) -> Value {
    json!({
        "success": true,
        "data": candidates,
        "total": total,
    })
}

pub fn build_llm_candidate_reject_response(rejected: bool) -> Value {
    json!({
        "success": true,
        "data": {
            "rejected": rejected,
        },
    })
}

pub fn llm_review_endpoint_requires_live_provider(endpoint: &str) -> bool {
    let normalized = endpoint.trim_matches('/');
    if matches!(
        normalized,
        "preview-recommend/accept"
            | "preview-recommend/reject"
            | "candidates/accept"
            | "candidates/reject"
    ) {
        return false;
    }

    let endpoint_parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    !matches!(
        endpoint_parts.as_slice(),
        ["candidates", _, "accept"] | ["candidates", _, "reject"]
    )
}

pub fn build_llm_analysis_response(candidates: Vec<Value>, context: &Value) -> Value {
    let context_object = context.as_object();
    let session_id = context_object
        .and_then(|item| item.get("session_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let bill_ids_present = context_object
        .and_then(|item| item.get("bill_ids"))
        .is_some_and(|value| !value.is_null());
    let mode = if !session_id.is_empty() {
        "import_session"
    } else if bill_ids_present {
        "persisted_selection"
    } else {
        "persisted_uncategorized"
    };
    let mut response_payload = Map::new();
    response_payload.insert("candidates_created".to_string(), json!(candidates.len()));
    response_payload.insert("candidates".to_string(), Value::Array(candidates.clone()));
    response_payload.insert("mode".to_string(), json!(mode));
    if !session_id.is_empty() {
        response_payload.insert("session_id".to_string(), json!(session_id));
    }

    json!({
        "success": true,
        "data": response_payload,
        "total": candidates.len(),
    })
}

fn normalize_text(text: &str) -> String {
    text.nfkc()
        .collect::<String>()
        .replace('\u{00a0}', " ")
        .trim()
        .to_string()
}

fn normalize_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(collapse_whitespace)
        .map(|line| {
            line.trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '：'))
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn detect_payment_platform(text: &str) -> Option<&'static str> {
    let lowered = text.to_lowercase();
    if text.contains("微信支付") || text.contains("财付通") || lowered.contains("wechat pay")
    {
        Some("wechat_pay")
    } else if text.contains("支付宝")
        || lowered.contains("alipay")
        || text.contains("花呗")
        || text.contains("余额宝")
    {
        Some("alipay")
    } else {
        None
    }
}

fn parse_amount(text: &str) -> Option<f64> {
    for marker in [
        "付款金额",
        "支付金额",
        "实付金额",
        "实付款",
        "订单金额",
        "转账金额",
        "收款金额",
        "金额",
        "合计",
        "总计",
    ] {
        if let Some(index) = text.find(marker) {
            if let Some(amount) = parse_number_after_prefix(text, index + marker.len()) {
                return Some(amount.abs());
            }
        }
    }

    for currency_marker in ['¥', '￥'] {
        if let Some(index) = text.find(currency_marker) {
            if let Some(amount) =
                parse_number_after_prefix(text, index + currency_marker.len_utf8())
            {
                return Some(amount.abs());
            }
        }
    }

    if contains_any_case_insensitive(text, &["元", "CNY", "RMB"]) {
        parse_currency_adjacent_amount(text).map(f64::abs)
    } else {
        None
    }
}

fn parse_number_after_prefix(text: &str, start: usize) -> Option<f64> {
    let mut number_start = None;
    for (offset, ch) in text[start..].char_indices() {
        if ch.is_ascii_digit() || matches!(ch, '+' | '-') {
            number_start = Some(start + offset);
            break;
        }
        if !(ch.is_whitespace() || matches!(ch, ':' | '：' | '¥' | '￥')) {
            return None;
        }
    }
    number_start.and_then(|index| parse_number_at(text, index))
}

fn parse_currency_adjacent_amount(text: &str) -> Option<f64> {
    for marker in ["CNY", "cny", "RMB", "rmb"] {
        for (index, _) in text.match_indices(marker) {
            if let Some(amount) = parse_number_after_prefix(text, index + marker.len())
                .or_else(|| parse_number_before_index(text, index))
            {
                return Some(amount);
            }
        }
    }

    for (index, _) in text.match_indices("元") {
        if let Some(amount) = parse_number_before_index(text, index)
            .or_else(|| parse_number_after_prefix(text, index + "元".len()))
        {
            return Some(amount);
        }
    }

    None
}

fn parse_number_before_index(text: &str, end: usize) -> Option<f64> {
    let mut trimmed_end = end;
    while let Some((index, ch)) = text[..trimmed_end].char_indices().next_back() {
        if ch.is_whitespace() {
            trimmed_end = index;
        } else {
            break;
        }
    }

    let mut start = trimmed_end;
    let mut has_digit = false;
    let mut has_decimal = false;
    for (index, ch) in text[..trimmed_end].char_indices().rev() {
        if ch.is_ascii_digit() {
            start = index;
            has_digit = true;
        } else if ch == '.' && !has_decimal {
            start = index;
            has_decimal = true;
        } else if ch == ',' {
            start = index;
        } else if matches!(ch, '+' | '-') {
            start = index;
            break;
        } else {
            break;
        }
    }

    has_digit.then(|| parse_number_at(text, start)).flatten()
}

fn parse_number_at(text: &str, start: usize) -> Option<f64> {
    let mut raw = String::new();
    let mut has_digit = false;
    let mut has_decimal = false;
    for ch in text[start..].chars() {
        if ch.is_ascii_digit() {
            has_digit = true;
            raw.push(ch);
        } else if matches!(ch, '+' | '-') && raw.is_empty() {
            raw.push(ch);
        } else if ch == '.' && !has_decimal {
            has_decimal = true;
            raw.push(ch);
        } else if ch == ',' {
            continue;
        } else {
            break;
        }
    }
    if has_digit {
        raw.parse::<f64>().ok()
    } else {
        None
    }
}

fn parse_trade_time(text: &str) -> Option<String> {
    let search_text = normalize_datetime_text(text);
    let tokens = search_text.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        let Some(date_parts) = parse_date_token(token) else {
            continue;
        };
        let time_parts = tokens
            .get(index + 1)
            .and_then(|next| parse_time_token(next));
        return Some(format_date_time(date_parts, time_parts));
    }
    None
}

fn normalize_datetime_text(text: &str) -> String {
    text.nfkc()
        .collect::<String>()
        .chars()
        .map(|ch| match ch {
            '年' | '月' | '/' | '.' => '-',
            '日' => ' ',
            _ => ch,
        })
        .collect()
}

fn parse_date_token(token: &str) -> Option<(i32, u32, u32)> {
    let cleaned = trim_to_ascii_date_token(token);
    let parts = cleaned.split('-').collect::<Vec<_>>();
    match parts.as_slice() {
        [year, month, day] if year.len() == 4 => {
            let parsed = (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
            validate_date(parsed)
        }
        [month, day] => {
            let parsed = (
                Local::now().date_naive().year(),
                month.parse().ok()?,
                day.parse().ok()?,
            );
            validate_date(parsed)
        }
        _ => None,
    }
}

fn parse_time_token(token: &str) -> Option<(u32, u32, u32, bool)> {
    let cleaned = trim_to_ascii_time_token(token);
    let parts = cleaned.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [hour, minute] => {
            let parsed = (hour.parse().ok()?, minute.parse().ok()?, 0, false);
            validate_time(parsed)
        }
        [hour, minute, second] => {
            let parsed = (
                hour.parse().ok()?,
                minute.parse().ok()?,
                second.parse().ok()?,
                true,
            );
            validate_time(parsed)
        }
        _ => None,
    }
}

fn trim_to_ascii_date_token(token: &str) -> String {
    token
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
        .collect()
}

fn trim_to_ascii_time_token(token: &str) -> String {
    token
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || *ch == ':')
        .collect()
}

fn validate_date(parts: (i32, u32, u32)) -> Option<(i32, u32, u32)> {
    NaiveDate::from_ymd_opt(parts.0, parts.1, parts.2).map(|_| parts)
}

fn validate_time(parts: (u32, u32, u32, bool)) -> Option<(u32, u32, u32, bool)> {
    NaiveTime::from_hms_opt(parts.0, parts.1, parts.2).map(|_| parts)
}

fn format_date_time(
    date_parts: (i32, u32, u32),
    time_parts: Option<(u32, u32, u32, bool)>,
) -> String {
    let (year, month, day) = date_parts;
    if let Some((hour, minute, second, has_second)) = time_parts {
        if has_second {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
        } else {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
        }
    } else {
        format!("{year:04}-{month:02}-{day:02}")
    }
}

fn parse_description(lines: &[String]) -> Option<String> {
    parse_labeled_description(lines).or_else(|| {
        lines
            .iter()
            .find(|line| is_description_candidate(line))
            .map(|line| clean_description(line))
    })
}

fn parse_labeled_description(lines: &[String]) -> Option<String> {
    for (index, line) in lines.iter().enumerate() {
        for label in description_labels() {
            if let Some(value) = value_after_label(line, label) {
                if is_description_candidate(value) {
                    return Some(clean_description(value));
                }
            }
            if line == label {
                if let Some(next_line) = lines
                    .get(index + 1)
                    .filter(|item| is_description_candidate(item))
                {
                    return Some(clean_description(next_line));
                }
            }
        }
    }
    None
}

fn value_after_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let remainder = line.strip_prefix(label)?;
    if remainder.is_empty() {
        return None;
    }
    let mut chars = remainder.chars();
    if let Some(first) = chars.next() {
        if !(first.is_whitespace() || matches!(first, ':' | '：')) {
            return None;
        }
    }
    let value = remainder.trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '：'));
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn is_description_candidate(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty() || text.chars().count() > 80 {
        return false;
    }
    if noise_tokens().iter().any(|token| text.contains(token)) {
        return false;
    }
    if description_labels().contains(&text) {
        return false;
    }
    if parse_amount(text).is_some() || parse_trade_time(text).is_some() {
        return false;
    }
    if text.chars().count() >= 8
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
    {
        return false;
    }
    text.chars().any(|ch| ch.is_alphabetic() || is_cjk(ch))
}

fn clean_description(value: &str) -> String {
    collapse_whitespace(value)
        .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '：'))
        .chars()
        .take(60)
        .collect()
}

fn score_ocr_confidence(
    amount: Option<f64>,
    trade_time: Option<&str>,
    description: Option<&str>,
    payment_platform: Option<&str>,
) -> f64 {
    let mut score = 0.0;
    if amount.is_some() {
        score += 0.3;
    }
    if trade_time.is_some_and(|value| !value.is_empty()) {
        score += 0.25;
    }
    if description.is_some_and(|value| !value.is_empty()) {
        score += 0.25;
    }
    if payment_platform.is_some_and(|value| !value.is_empty()) {
        score += 0.2;
    }
    f64::min(1.0, score)
}

fn description_labels() -> &'static [&'static str] {
    &[
        "交易对方",
        "收款方",
        "付款方",
        "商户",
        "商家",
        "对方账户",
        "商品",
        "商品说明",
        "订单名称",
        "备注",
        "说明",
    ]
}

fn noise_tokens() -> &'static [&'static str] {
    &[
        "微信支付",
        "支付宝",
        "支付成功",
        "交易成功",
        "商家服务",
        "当前状态",
        "付款方式",
        "支付方式",
        "交易单号",
        "商户单号",
        "订单号",
        "创建时间",
        "支付时间",
        "交易时间",
        "付款时间",
        "收款时间",
        "账单详情",
        "交易详情",
    ]
}

fn is_cjk(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn contains_any_case_insensitive(text: &str, needles: &[&str]) -> bool {
    let lowered = text.to_lowercase();
    needles
        .iter()
        .any(|needle| lowered.contains(&needle.to_lowercase()))
}

fn is_llm_provider_creatable(provider: &str) -> bool {
    matches!(
        provider,
        "openai"
            | "claude"
            | "deepseek"
            | "ollama"
            | "xai"
            | "google"
            | "openrouter"
            | "openai_compatible"
            | "azure"
    )
}

fn default_llm_base_url(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" | "openai_compatible" => Some("https://api.openai.com/v1"),
        "claude" => Some("https://api.anthropic.com/v1"),
        "ollama" => Some("http://localhost:11434"),
        "deepseek" => Some("https://api.deepseek.com/v1"),
        "xai" => Some("https://api.x.ai/v1"),
        "google" => Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        "openrouter" => Some("https://openrouter.ai/api/v1"),
        _ => None,
    }
}

fn validate_azure_base_url(base_url: &str) -> Result<(), String> {
    let trimmed = base_url.trim();
    if trimmed.contains('\\') || trimmed.chars().any(char::is_control) {
        return Err("Azure provider base_url must use an Azure OpenAI host".to_string());
    }
    let without_scheme = trimmed
        .strip_prefix("https://")
        .ok_or_else(|| "Azure provider base_url must use https".to_string())?;
    let authority = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if authority.is_empty() || authority.contains('@') {
        return Err("Azure provider base_url must use an Azure OpenAI host".to_string());
    }
    let host = authority.split(':').next().unwrap_or_default();
    if !host.ends_with(".openai.azure.com") {
        return Err("Azure provider base_url must use an Azure OpenAI host".to_string());
    }
    Ok(())
}

fn default_llm_model(provider: &str) -> Option<&'static str> {
    match provider {
        "openai" | "openai_compatible" | "azure" => Some("gpt-4o-mini"),
        "claude" => Some("claude-sonnet-4-20250514"),
        "ollama" => Some("llama3"),
        "deepseek" => Some("deepseek-chat"),
        "xai" => Some("grok-3-mini"),
        "google" => Some("gemini-2.0-flash"),
        "openrouter" => Some("openai/gpt-4o-mini"),
        _ => None,
    }
}

fn first_non_empty_field(object: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    object
        .and_then(|item| item.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn string_field_or(object: Option<&Map<String, Value>>, key: &str, default: &str) -> String {
    first_non_empty_field(object, key).unwrap_or_else(|| default.to_string())
}

fn decode_settings_object(settings: Option<&Value>) -> Map<String, Value> {
    match settings {
        Some(Value::Object(object)) => object.clone(),
        Some(Value::String(text)) if !text.trim().is_empty() => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default(),
        _ => Map::new(),
    }
}

fn is_falsy_settings_value(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(false) => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
        _ => false,
    }
}

fn normalize_float_setting(value: Option<&Value>, minimum: f64, maximum: f64) -> Option<f64> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(text)) => text.parse::<f64>().ok(),
        _ => None,
    }?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

fn normalize_int_setting(value: Option<&Value>, minimum: i64, maximum: i64) -> Option<i64> {
    let parsed = match value {
        Some(Value::Number(number)) => number.as_i64(),
        Some(Value::String(text)) => text.parse::<i64>().ok(),
        _ => None,
    }?;
    (minimum..=maximum).contains(&parsed).then_some(parsed)
}

fn normalize_prompt_text(value: &str) -> String {
    value.trim().chars().take(LLM_PROMPT_TEXT_LIMIT).collect()
}

fn object_has_non_empty_secret(object: &Map<String, Value>) -> bool {
    object.iter().any(|(key, value)| {
        let current_key_has_secret = is_secret_key(key) && secret_value_present(value);
        current_key_has_secret || value_has_non_empty_secret(value)
    })
}

fn value_has_non_empty_secret(value: &Value) -> bool {
    match value {
        Value::Object(object) => object_has_non_empty_secret(object),
        Value::Array(items) => items.iter().any(value_has_non_empty_secret),
        _ => false,
    }
}

fn redact_secrets_in_map(object: &mut Map<String, Value>) {
    for (key, value) in object.iter_mut() {
        if is_secret_key(key) {
            let replacement = if secret_value_present(value) {
                "********"
            } else {
                ""
            };
            *value = json!(replacement);
        } else {
            redact_secrets_in_value(value);
        }
    }
}

fn redact_secrets_in_value(value: &mut Value) {
    match value {
        Value::Object(object) => redact_secrets_in_map(object),
        Value::Array(items) => {
            for item in items {
                redact_secrets_in_value(item);
            }
        }
        _ => {}
    }
}

fn secret_value_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
        _ => true,
    }
}

fn is_secret_key(key: &str) -> bool {
    let normalized = key
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let exact_alias = matches!(
        normalized.as_str(),
        "apikey"
            | "authorization"
            | "xapikey"
            | "apisecret"
            | "secretkey"
            | "credential"
            | "credentials"
            | "proxyauthorization"
            | "subscriptionkey"
            | "ocpapimsubscriptionkey"
            | "accesstoken"
            | "refreshtoken"
            | "bearertoken"
            | "idtoken"
            | "privatekey"
            | "token"
            | "password"
            | "clientsecret"
    );
    if exact_alias {
        return true;
    }

    const SECRET_KEY_SUFFIXES: [&str; 15] = [
        "apikey",
        "xapikey",
        "apisecret",
        "secretkey",
        "authorizationheader",
        "proxyauthorization",
        "subscriptionkey",
        "accesstoken",
        "refreshtoken",
        "bearertoken",
        "idtoken",
        "privatekey",
        "password",
        "clientsecret",
        "credentials",
    ];

    SECRET_KEY_SUFFIXES
        .iter()
        .any(|suffix| normalized.len() > suffix.len() && normalized.ends_with(suffix))
}
