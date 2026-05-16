use serde_json::Value;
use std::{env, net::IpAddr};
use url::Url;

use super::types::{LlmProviderConfigContract, LLM_AVAILABLE_PROVIDERS};
use super::value_helpers::first_non_empty_field;

const LLM_BASE_URL_ALLOWLIST_ENV: &str = "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST";

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
