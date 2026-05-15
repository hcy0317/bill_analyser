use chrono::{Datelike, Local, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{env, net::IpAddr};
use unicode_normalization::UnicodeNormalization;
use url::Url;

pub const OCR_DISABLED_PROVIDER_NAME: &str = "disabled";
pub const OCR_DEFAULT_LANG: &str = "chi_sim+eng";
pub const LLM_SYSTEM_PROMPT: &str = "你是 Bill Analyser 的智能分类助手。Bill Analyser 是一个个人/家庭账单管理系统，支持收入、支出、转账三种交易类型。\n每笔交易包含：日期、金额（单位：元）、交易对方、描述、支付方式、主分类、子分类。\n你的任务是根据交易信息推断最合适的分类，或根据已分类样本归纳关键词匹配规则。\n请始终以 JSON 格式返回结果，不要包含额外的解释文字。";
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
const LLM_BASE_URL_ALLOWLIST_ENV: &str = "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST";
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrProviderTextResult {
    pub text: String,
    pub confidence: f64,
    pub model: String,
    pub raw_provider_response: Value,
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

pub fn build_ocr_recognition_success_response(
    provider_name: &str,
    provider_result: &OcrProviderTextResult,
    request_id: &str,
) -> AiRouteResponse {
    let parsed = parse_payment_screenshot_text(&provider_result.text);
    let confidence = provider_result
        .confidence
        .max(parsed.confidence)
        .clamp(0.0, 1.0);
    AiRouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": {
                "amount": parsed.amount,
                "trade_time": parsed.trade_time,
                "description": parsed.description,
                "payment_platform": parsed.payment_platform,
                "provenance": {
                    "provider": provider_name,
                    "model": provider_result.model,
                    "request_id": request_id,
                },
                "confidence": confidence,
                "raw_provider_response": provider_result.raw_provider_response,
            },
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
        let base_url = explicit_base_url
            .clone()
            .or_else(|| default_llm_base_url(&normalized_provider).map(str::to_string))
            .unwrap_or_default();
        validate_llm_base_url(&normalized_provider, &base_url, explicit_base_url.is_some())?;
        base_url
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

pub fn build_llm_classification_prompt(transactions: &[Value]) -> String {
    let transactions_block = transactions
        .iter()
        .enumerate()
        .map(|(index, txn)| {
            format!(
                "  {}. id={}, date={}, amount={}元, counterparty=\"{}\", description=\"{}\", payment_method=\"{}\"",
                index + 1,
                prompt_value(txn, "id"),
                prompt_value(txn, "date"),
                prompt_value(txn, "amount"),
                prompt_value(txn, "counterparty"),
                prompt_value(txn, "description"),
                prompt_value(txn, "payment_method"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "以下是一批未分类的交易记录，请为每笔交易推荐最合适的主分类和子分类。\n\n交易列表：\n{transactions_block}\n\n请以如下 JSON 格式返回（数组，每个元素对应一笔交易）：\n[\n  {{\n    \"bill_id\": <交易ID>,\n    \"suggested_main_category\": \"<推荐主分类>\",\n    \"suggested_sub_category\": \"<推荐子分类>\",\n    \"confidence\": <0.0-1.0之间的置信度>\n  }}\n]\n\n分类应尽可能贴合中文个人财务常见分类体系（如：餐饮美食、交通出行、日用百货、住房物业、医疗健康、教育培训、休闲娱乐、人情往来、工资薪酬等）。\n只返回 JSON，不要有其他文字。"
    )
}

pub fn build_llm_rule_induction_prompt(category_name: &str, transactions: &[Value]) -> String {
    let samples_block = transactions
        .iter()
        .enumerate()
        .map(|(index, txn)| {
            format!(
                "  {}. counterparty=\"{}\", description=\"{}\", payment_method=\"{}\"",
                index + 1,
                prompt_value(txn, "counterparty"),
                prompt_value(txn, "description"),
                prompt_value(txn, "payment_method"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "以下是已被归类为「{category_name}」的交易样本，请分析它们的共同模式，归纳出一组关键词匹配规则。\n\n样本列表：\n{samples_block}\n\n规则表达式语法说明：\n- OR={{关键词1,关键词2}} 表示匹配任一关键词\n- AND={{关键词1,关键词2}} 表示必须同时包含所有关键词\n- NOT={{关键词1}} 表示排除包含这些关键词的交易\n- 多个条件用 + 连接，如：OR={{美团,饿了么}}+NOT={{退款}}\n\n请以如下 JSON 格式返回（可返回多条规则建议）：\n[\n  {{\n    \"rule_name\": \"<规则名称>\",\n    \"rule_expression\": \"<规则表达式>\",\n    \"confidence\": <0.0-1.0之间的置信度>,\n    \"explanation\": \"<简短说明为什么归纳出这条规则>\"\n  }}\n]\n\n只返回 JSON，不要有其他文字。"
    )
}

pub fn build_llm_import_preview_recommendation_prompt(
    transactions: &[Value],
    existing_categories: &[String],
    existing_accounts: &[String],
    memory_context: &[Value],
) -> String {
    let transactions_block = transactions
        .iter()
        .enumerate()
        .map(|(index, txn)| {
            format!(
                "  {}. preview_id={}, date=\"{}\", amount={}元, type={}, counterparty=\"{}\", description=\"{}\", payment_method=\"{}\"",
                index + 1,
                prompt_value(txn, "id"),
                prompt_value(txn, "date"),
                prompt_value(txn, "amount"),
                prompt_value(txn, "type"),
                prompt_value(txn, "counterparty"),
                prompt_value(txn, "description"),
                prompt_value(txn, "payment_method"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let categories_block = if existing_categories.is_empty() {
        String::new()
    } else {
        format!(
            "\n已有分类体系（优先从中选择）：\n{}\n",
            existing_categories
                .iter()
                .take(50)
                .map(|category| format!("  - {category}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let accounts_block = if existing_accounts.is_empty() {
        String::new()
    } else {
        format!(
            "\n已有账户（若需要给出账户路由，请优先使用这些账户名）：\n{}\n",
            existing_accounts
                .iter()
                .take(50)
                .map(|account| format!("  - {account}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let memory_lines = memory_context
        .iter()
        .take(20)
        .filter_map(|memory| {
            let decision = prompt_value(memory, "decision");
            let category = prompt_value(memory, "suggested_main_category");
            if decision.is_empty() || category.is_empty() {
                return None;
            }
            Some(format!(
                "  - {decision}: \"{}\" -> {}/{}",
                prompt_value(memory, "description_hint"),
                category,
                prompt_value(memory, "suggested_sub_category"),
            ))
        })
        .collect::<Vec<_>>();
    let memory_block = if memory_lines.is_empty() {
        String::new()
    } else {
        format!(
            "\n历史记忆（你过去的推荐和用户反馈，请从中学习）：\n{}\n",
            memory_lines.join("\n")
        )
    };

    format!(
        "以下是一批待导入的交易记录，请为每笔交易推荐最合适的主分类、子分类和账户路由。\n{categories_block}{accounts_block}{memory_block}\n交易列表：\n{transactions_block}\n\n请以如下 JSON 格式返回（数组，每个元素对应一笔交易）：\n[\n  {{\n    \"preview_id\": <预览行ID>,\n    \"suggested_main_category\": \"<推荐主分类>\",\n    \"suggested_sub_category\": \"<推荐子分类>\",\n    \"suggested_source_account\": \"<推荐来源账户，可为空>\",\n    \"suggested_destination_account\": \"<推荐目标账户，可为空>\",\n    \"confidence\": <0.0-1.0之间的置信度>,\n    \"reason\": \"<简短推荐理由>\"\n  }}\n]\n\n分类应尽可能贴合中文个人财务常见分类体系。如果历史记忆中有相似交易的反馈，优先参考用户的纠正。\n只返回 JSON，不要有其他文字。"
    )
}

pub fn build_llm_rule_expression_synthesis_prompt(
    knowledge_summary_pack: &Value,
    max_candidates: usize,
) -> String {
    let summary_json =
        serde_json::to_string_pretty(knowledge_summary_pack).unwrap_or_else(|_| "{}".to_string());
    format!(
        "以下是 Bill Analyser 的长期学习知识摘要（KnowledgeSummaryPack）。\n请基于这些长期学习证据，为规则中心归纳出可人工审核的分类规则候选。\n\n约束：\n- 只能输出“候选规则”，不要假设会自动写入正式规则系统。\n- 候选必须兼容现有规则表达式语法：\n  - OR={{关键词1,关键词2}}\n  - AND={{关键词1,关键词2}}\n  - NOT={{关键词1}}\n  - REGEX={{模式1,模式2}}\n  - 可以使用 +、/、|、× 和括号组合\n- 不要输出无效语法、空表达式或与知识摘要明显冲突的规则。\n- 优先覆盖证据稳定、反馈正向、可复用的模式。\n- 推荐分类必须严格来自 knowledge_summary_pack.existing_categories 中已有的分类路径。\n- 如果证据不足，请少提，不要为了凑数量强行生成。\n- 最多输出 {max_candidates} 条候选。\n\nKnowledgeSummaryPack:\n{summary_json}\n\n请以如下 JSON 格式返回：\n[\n  {{\n    \"rule_name\": \"<候选名称>\",\n    \"suggested_main_category\": \"<主分类>\",\n    \"suggested_sub_category\": \"<子分类，可为空>\",\n    \"rule_expression\": \"<规则表达式>\",\n    \"confidence\": <0.0-1.0之间的置信度>,\n    \"reason\": \"<简短说明归纳依据>\"\n  }}\n]\n\n只返回 JSON，不要有其他文字。"
    )
}

pub fn render_llm_prompt_template(
    template: &str,
    default_prompt: &str,
    transactions: &[Value],
    category_name: &str,
) -> String {
    if template.trim().is_empty() {
        return default_prompt.to_string();
    }
    let transactions_json =
        serde_json::to_string(transactions).unwrap_or_else(|_| "[]".to_string());
    let transactions_text = transactions
        .iter()
        .map(|item| serde_json::to_string(item).unwrap_or_else(|_| "{}".to_string()))
        .collect::<Vec<_>>()
        .join("\n");
    template
        .replace("{default_prompt}", default_prompt)
        .replace("{transactions_json}", &transactions_json)
        .replace("{transactions_text}", &transactions_text)
        .replace("{category_name}", category_name)
}

pub fn parse_llm_json_array_response(content: &str) -> Result<Vec<Value>, String> {
    let stripped = strip_json_code_fence(content.trim());
    let candidate = if let (Some(start), Some(end)) = (stripped.find('['), stripped.rfind(']')) {
        if start <= end {
            &stripped[start..=end]
        } else {
            stripped
        }
    } else {
        stripped
    };
    serde_json::from_str::<Vec<Value>>(candidate)
        .map_err(|error| format!("Unable to parse LLM JSON array response: {error}"))
}

fn strip_json_code_fence(content: &str) -> &str {
    if !content.starts_with("```") {
        return content;
    }
    let Some(first_newline) = content.find('\n') else {
        return content;
    };
    let body = &content[first_newline + 1..];
    if let Some(last_fence) = body.rfind("```") {
        body[..last_fence].trim()
    } else {
        body.trim()
    }
}

fn prompt_value(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| match value {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            Value::Array(_) | Value::Object(_) => Some(value.to_string()),
        })
        .unwrap_or_default()
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

fn validate_llm_base_url(provider: &str, base_url: &str, explicit: bool) -> Result<(), String> {
    let parsed = parse_llm_base_url(base_url)?;
    if !explicit {
        return Ok(());
    }
    if provider == "ollama" && llm_url_origin_matches(&parsed, "http://localhost:11434") {
        return Ok(());
    }
    if let Some(default_url) = default_llm_base_url(provider) {
        if provider != "openai_compatible" && llm_url_origin_matches(&parsed, default_url) {
            return Ok(());
        }
    }
    if llm_url_is_allowlisted(&parsed) {
        if parsed.scheme() == "https" || llm_url_is_local_plain_http_endpoint(&parsed) {
            return Ok(());
        }
        return Err(
            "LLM provider base_url must use https unless allowlisting a local endpoint".to_string(),
        );
    }
    if llm_url_host_is_forbidden(&parsed) {
        return Err("LLM provider base_url host is not allowed".to_string());
    }
    Err(format!(
        "LLM provider base_url is not allowed; configure {LLM_BASE_URL_ALLOWLIST_ENV}"
    ))
}

fn parse_llm_base_url(base_url: &str) -> Result<Url, String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return Err("LLM provider base_url is required".to_string());
    }
    if trimmed.contains('\\') || trimmed.chars().any(char::is_control) {
        return Err("LLM provider base_url is not allowed".to_string());
    }
    let parsed =
        Url::parse(trimmed).map_err(|_| "LLM provider base_url must be a valid URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("LLM provider base_url must use http or https".to_string());
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("LLM provider base_url must not contain credentials".to_string());
    }
    let Some(host) = parsed.host_str() else {
        return Err("LLM provider base_url must include a host".to_string());
    };
    if host.eq_ignore_ascii_case("metadata.google.internal") {
        return Err("LLM provider base_url host is not allowed".to_string());
    }
    Ok(parsed)
}

fn llm_url_host_is_forbidden(parsed: &Url) -> bool {
    let Some(host) = parsed.host_str() else {
        return true;
    };
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.eq_ignore_ascii_case("metadata.google.internal")
    {
        return true;
    }
    let Ok(address) = host.parse::<IpAddr>() else {
        return false;
    };
    if address.is_unspecified() || address.is_loopback() || address.is_multicast() {
        return true;
    }
    match address {
        IpAddr::V4(address) => address.is_private() || address.is_link_local(),
        IpAddr::V6(address) => address.is_unique_local() || address.is_unicast_link_local(),
    }
}

fn llm_url_origin_matches(parsed: &Url, allowed_url: &str) -> bool {
    Url::parse(allowed_url)
        .map(|allowed| {
            parsed.scheme() == allowed.scheme()
                && parsed.host_str().map(str::to_ascii_lowercase)
                    == allowed.host_str().map(str::to_ascii_lowercase)
                && parsed.port_or_known_default() == allowed.port_or_known_default()
        })
        .unwrap_or(false)
}

fn llm_url_is_allowlisted(parsed: &Url) -> bool {
    let origin = llm_url_origin(parsed);
    let full = parsed.as_str().trim_end_matches('/').to_ascii_lowercase();
    env::var(LLM_BASE_URL_ALLOWLIST_ENV)
        .unwrap_or_default()
        .split([',', ';'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.trim_end_matches('/').to_ascii_lowercase())
        .any(|entry| entry == full || entry == origin)
}

fn llm_url_is_local_plain_http_endpoint(parsed: &Url) -> bool {
    if parsed.scheme() != "http" {
        return false;
    }
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return true;
    }
    host.parse::<IpAddr>()
        .map(|address| address.is_loopback())
        .unwrap_or(false)
}

fn llm_url_origin(parsed: &Url) -> String {
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    match parsed.port() {
        Some(port) => format!("{}://{}:{port}", parsed.scheme(), host),
        None => format!("{}://{}", parsed.scheme(), host),
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
