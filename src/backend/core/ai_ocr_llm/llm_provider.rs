// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::Value;
use std::env;

use super::types::{LlmProviderConfigContract, LLM_AVAILABLE_PROVIDERS};
use super::value_helpers::first_non_empty_field;
use crate::{OutboundHostClass, OutboundHttpUrl, OutboundHttpUrlParseError};

const LLM_BASE_URL_ALLOWLIST_ENV: &str = "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST";

/// 返回当前运行时支持的 LLM provider 标识列表，供配置页和错误提示使用。
#[tracing::instrument(level = "debug", skip_all)]
pub fn llm_available_providers() -> Vec<String> {
    LLM_AVAILABLE_PROVIDERS
        .iter()
        .map(|item| (*item).to_string())
        .collect()
}

/// 归一化 provider 别名，兼容前端表单和历史配置中的命名差异。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_llm_provider_name(provider: &str) -> String {
    match provider.trim().to_lowercase().as_str() {
        "" => "openai".to_string(),
        "anthropic" => "claude".to_string(),
        "openai-compatible" => "openai_compatible".to_string(),
        "azure_openai" | "azure-openai" => "azure".to_string(),
        normalized => normalized.to_string(),
    }
}

/// 构建 LLM provider 运行时合同，校验 base_url、默认模型和 provider kind。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_llm_provider_config(
    provider: &str,
    provider_config: Option<&Value>,
) -> Result<LlmProviderConfigContract, String> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_llm_provider_config",
        "business operation entered"
    );
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
            | "qwen"
            | "siliconflow"
            | "zhipu"
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
        "qwen" => Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
        "siliconflow" => Some("https://api.siliconflow.cn/v1"),
        "zhipu" => Some("https://open.bigmodel.cn/api/paas/v4"),
        _ => None,
    }
}

/// 校验显式 LLM base_url 的 SSRF 边界，只允许默认域名、allowlist 或安全本地端点。
#[tracing::instrument(level = "debug", skip_all)]
fn validate_llm_base_url(provider: &str, base_url: &str, explicit: bool) -> Result<(), String> {
    let parsed = parse_llm_base_url(base_url)?;
    if llm_url_host_is_never_allowed(&parsed) {
        return Err("LLM provider base_url host is not allowed".to_string());
    }
    if !explicit {
        return Ok(());
    }
    if provider == "ollama" && parsed.same_origin("http://localhost:11434") {
        return Ok(());
    }
    if let Some(default_url) = default_llm_base_url(provider) {
        if provider != "openai_compatible" && parsed.same_origin(default_url) {
            return Ok(());
        }
    }
    if provider == "openai_compatible" && parsed.is_https() && !llm_url_host_is_forbidden(&parsed) {
        return Ok(());
    }
    if llm_url_is_allowlisted(&parsed) {
        if parsed.is_https() || llm_url_is_local_plain_http_endpoint(&parsed) {
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

/// 校验 LLM vision OCR 使用的 base_url，禁止未经 allowlist 的内网、metadata 和凭据 URL。
#[tracing::instrument(level = "debug", skip_all)]
pub fn validate_llm_vision_base_url(base_url: &str) -> Result<(), String> {
    let parsed = parse_llm_base_url(base_url)?;
    if llm_url_host_is_never_allowed(&parsed) {
        return Err("LLM provider base_url host is not allowed".to_string());
    }
    if llm_url_matches_openai_compatible_default(&parsed) {
        return Ok(());
    }
    if llm_url_is_allowlisted(&parsed) {
        if parsed.is_https() || llm_url_is_local_plain_http_endpoint(&parsed) {
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

/// 解析 provider URL 并拒绝控制字符、反斜杠、非 http/https、凭据和无 host 输入。
#[tracing::instrument(level = "debug", skip_all)]
fn parse_llm_base_url(base_url: &str) -> Result<OutboundHttpUrl, String> {
    OutboundHttpUrl::parse(base_url).map_err(|error| match error {
        OutboundHttpUrlParseError::Empty => "LLM provider base_url is required".to_string(),
        OutboundHttpUrlParseError::UnsafeText => "LLM provider base_url is not allowed".to_string(),
        OutboundHttpUrlParseError::Invalid => {
            "LLM provider base_url must be a valid URL".to_string()
        }
        OutboundHttpUrlParseError::UnsupportedScheme => {
            "LLM provider base_url must use http or https".to_string()
        }
        OutboundHttpUrlParseError::Credentials => {
            "LLM provider base_url must not contain credentials".to_string()
        }
        OutboundHttpUrlParseError::MissingHost => {
            "LLM provider base_url must include a host".to_string()
        }
    })
}

fn llm_url_host_is_never_allowed(parsed: &OutboundHttpUrl) -> bool {
    matches!(
        parsed.host_class(),
        OutboundHostClass::Metadata
            | OutboundHostClass::Unspecified
            | OutboundHostClass::LinkLocal
            | OutboundHostClass::Multicast
    )
}

fn llm_url_host_is_forbidden(parsed: &OutboundHttpUrl) -> bool {
    matches!(
        parsed.host_class(),
        OutboundHostClass::Localhost
            | OutboundHostClass::Metadata
            | OutboundHostClass::Unspecified
            | OutboundHostClass::Loopback
            | OutboundHostClass::Private
            | OutboundHostClass::LinkLocal
            | OutboundHostClass::Multicast
    )
}

fn llm_url_is_allowlisted(parsed: &OutboundHttpUrl) -> bool {
    parsed.matches_exact_or_origin_allowlist(
        &env::var(LLM_BASE_URL_ALLOWLIST_ENV).unwrap_or_default(),
    )
}

fn llm_url_matches_openai_compatible_default(parsed: &OutboundHttpUrl) -> bool {
    [
        "openai",
        "deepseek",
        "xai",
        "google",
        "openrouter",
        "qwen",
        "siliconflow",
        "zhipu",
    ]
    .iter()
    .filter_map(|provider| default_llm_base_url(provider))
    .any(|default_url| parsed.same_origin(default_url))
}

fn llm_url_is_local_plain_http_endpoint(parsed: &OutboundHttpUrl) -> bool {
    !parsed.is_https()
        && matches!(
            parsed.host_class(),
            OutboundHostClass::Localhost | OutboundHostClass::Loopback
        )
}

/// 检查 Azure OpenAI base_url 是否为 https 且 host 位于 openai.azure.com 名下。
#[tracing::instrument(level = "debug", skip_all)]
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
        "qwen" => Some("qwen-plus"),
        "siliconflow" => Some("deepseek-ai/DeepSeek-V3"),
        "zhipu" => Some("glm-4-flash"),
        _ => None,
    }
}
