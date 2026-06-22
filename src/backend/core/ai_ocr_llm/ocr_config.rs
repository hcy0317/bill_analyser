// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Value};

use super::ocr_parser::parse_payment_screenshot_text;
use super::provider_auth::{normalize_provider_auth_config, redact_provider_auth_config};
use super::receipt_draft::build_receipt_transaction_draft;
use super::secret_redaction::redact_secrets_in_value;
use super::types::{
    AiRouteResponse, OcrConfigContract, OcrProviderTextResult, ReceiptDraftContext,
    OCR_AVAILABLE_PROVIDERS, OCR_DEFAULT_LANG, OCR_DISABLED_PROVIDER_NAME,
};

const OCR_ERROR_PROVIDER_UNCONFIGURED: &str = "provider_unconfigured";
const OCR_ERROR_TIMEOUT: &str = "timeout";
const OCR_ERROR_PARSE: &str = "parse_error";
const OCR_ERROR_CANCELLED: &str = "cancelled";
const OCR_ERROR_RATE_LIMITED: &str = "rate_limited";
const OCR_ERROR_PROVIDER_RELOGIN_REQUIRED: &str = "provider_relogin_required";

/// 归一化 OCR 配置，兼容 camelCase/legacy 字段并规范 provider auth profile。
#[tracing::instrument(level = "debug", skip_all)]
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
    let model = object
        .and_then(|item| item.get("model"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let base_url = object
        .and_then(|item| {
            item.get("base_url")
                .or_else(|| item.get("baseUrl"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .trim()
        .to_string();
    let parameters = object
        .and_then(|item| item.get("parameters").or_else(|| item.get("params")))
        .cloned()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let credential_config = normalize_provider_auth_config(object.and_then(|item| {
        item.get("credential_config")
            .or_else(|| item.get("credentialConfig"))
            .or_else(|| item.get("auth_profile"))
            .or_else(|| item.get("authProfile"))
    }));

    OcrConfigContract {
        provider: normalize_ocr_provider_name(provider),
        lang: normalize_ocr_lang(lang),
        model,
        base_url,
        parameters,
        credential_config,
    }
}

/// 归一化 OCR provider 名称，未知 provider 统一退回 disabled，避免误触外部调用。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 校验 OCR 语言参数，只允许短 ASCII 标识并在异常时回退默认语言。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 返回包含 disabled 的 OCR provider 列表，供前端配置页展示完整选择。
#[tracing::instrument(level = "debug", skip_all)]
pub fn ocr_available_providers_with_disabled() -> Vec<String> {
    std::iter::once(OCR_DISABLED_PROVIDER_NAME)
        .chain(OCR_AVAILABLE_PROVIDERS)
        .map(str::to_string)
        .collect()
}

/// 构建 OCR 配置响应载荷，递归脱敏 parameters 和 credential_config。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_ocr_config_response_payload(config: &OcrConfigContract) -> Value {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_ocr_config_response_payload",
        "business operation entered"
    );
    let mut safe_parameters = config.parameters.clone();
    redact_secrets_in_value(&mut safe_parameters);
    json!({
        "provider": config.provider,
        "lang": config.lang,
        "model": config.model,
        "base_url": config.base_url,
        "parameters": safe_parameters,
        "credential_config": redact_provider_auth_config(&config.credential_config),
        "available_providers": ocr_available_providers_with_disabled(),
        "configured": config.provider != OCR_DISABLED_PROVIDER_NAME,
    })
}

/// 构建 OCR 配置读取/保存成功响应，维持 success/result envelope。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_ocr_config_success_response(config: &OcrConfigContract) -> AiRouteResponse {
    AiRouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": build_ocr_config_response_payload(config),
        }),
    }
}

/// 构建未知 OCR provider 的 400 响应，避免 route 层重复错误形状。
#[tracing::instrument(level = "debug", skip_all)]
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

/// 构建 OCR 错误响应，并按错误码映射 HTTP 状态与默认文案。
#[tracing::instrument(level = "debug", skip_all)]
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
            "message": resolved_message,
        }),
    }
}

/// 构建无上下文的 OCR 识别成功响应，用于旧调用点保持行为兼容。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_ocr_recognition_success_response(
    provider_name: &str,
    provider_result: &OcrProviderTextResult,
    request_id: &str,
) -> AiRouteResponse {
    build_ocr_recognition_success_response_with_context(
        provider_name,
        provider_result,
        request_id,
        &ReceiptDraftContext::default(),
    )
}

/// 将 OCR 文本结果解析为交易草稿响应，明确金额仍以元值进入草稿字段。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_ocr_recognition_success_response_with_context(
    provider_name: &str,
    provider_result: &OcrProviderTextResult,
    request_id: &str,
    draft_context: &ReceiptDraftContext,
) -> AiRouteResponse {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_ocr_recognition_success_response_with_context",
        "business operation entered"
    );
    let parsed = parse_payment_screenshot_text(&provider_result.text);
    let draft = build_receipt_transaction_draft(&parsed, provider_result, draft_context);
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
                "draft": draft,
                "raw_provider_response": provider_result.raw_provider_response,
            },
        }),
    }
}

/// 将 OCR 业务错误码映射为 HTTP 状态码，保持 provider/runtime 错误对前端稳定。
#[tracing::instrument(level = "debug", skip_all)]
pub fn ocr_error_http_status(code: &str) -> u16 {
    match code {
        OCR_ERROR_PROVIDER_UNCONFIGURED => 501,
        OCR_ERROR_TIMEOUT => 504,
        OCR_ERROR_PARSE => 422,
        OCR_ERROR_CANCELLED => 499,
        OCR_ERROR_RATE_LIMITED => 429,
        OCR_ERROR_PROVIDER_RELOGIN_REQUIRED => 401,
        _ => 500,
    }
}
