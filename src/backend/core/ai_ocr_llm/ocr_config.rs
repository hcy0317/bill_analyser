// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{json, Value};

use super::provider_auth::normalize_provider_auth_config;
use super::types::{
    OcrConfigContract, OCR_AVAILABLE_PROVIDERS, OCR_DEFAULT_LANG, OCR_DISABLED_PROVIDER_NAME,
};

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
