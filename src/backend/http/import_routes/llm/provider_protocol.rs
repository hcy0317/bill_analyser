use super::{
    llm_contract_error_response, ImportV2RouteResponse, LlmProviderRequestContext,
    LlmProviderRuntimeResponse,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenAiApiProtocol {
    Responses,
    ChatCompletions,
}

pub(super) fn llm_api_protocol_attempts(
    context: &LlmProviderRequestContext,
) -> Vec<OpenAiApiProtocol> {
    if context.config.provider_kind != "openai_compatible" {
        return vec![OpenAiApiProtocol::ChatCompletions];
    }
    match context.api_protocol.trim() {
        "responses" => vec![OpenAiApiProtocol::Responses],
        "chat_completions" => vec![OpenAiApiProtocol::ChatCompletions],
        _ => vec![
            OpenAiApiProtocol::Responses,
            OpenAiApiProtocol::ChatCompletions,
        ],
    }
}

pub(super) fn llm_status_allows_protocol_fallback(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::BAD_REQUEST
            | reqwest::StatusCode::NOT_FOUND
            | reqwest::StatusCode::METHOD_NOT_ALLOWED
            | reqwest::StatusCode::UNSUPPORTED_MEDIA_TYPE
            | reqwest::StatusCode::UNPROCESSABLE_ENTITY
            | reqwest::StatusCode::NOT_IMPLEMENTED
    )
}

fn normalize_openai_generation_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    let lower = trimmed.to_ascii_lowercase();
    for suffix in ["/chat/completions", "/responses"] {
        if lower.ends_with(suffix) {
            return trimmed[..trimmed.len() - suffix.len()]
                .trim_end_matches('/')
                .to_string();
        }
    }
    trimmed.to_string()
}

pub(super) fn llm_provider_http_request(
    context: &LlmProviderRequestContext,
    prompt: &str,
    protocol: OpenAiApiProtocol,
) -> (String, Vec<(&'static str, String)>, Value) {
    let base_url = context.config.base_url.trim_end_matches('/');
    match context.config.provider_kind.as_str() {
        "claude" => {
            let mut payload = json!({
                "model": context.config.model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": context.max_tokens,
                "temperature": context.temperature,
            });
            if !context.system_prompt.trim().is_empty() {
                payload["system"] = json!(context.system_prompt);
            }
            (
                format!("{base_url}/messages"),
                vec![
                    ("x-api-key", context.api_key.clone()),
                    ("anthropic-version", "2023-06-01".to_string()),
                    ("content-type", "application/json".to_string()),
                ],
                payload,
            )
        }
        "ollama" => {
            let mut payload = json!({
                "model": context.config.model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "temperature": context.temperature,
                    "num_predict": context.max_tokens,
                },
            });
            if !context.system_prompt.trim().is_empty() {
                payload["system"] = json!(context.system_prompt);
            }
            (
                format!("{base_url}/api/generate"),
                vec![("content-type", "application/json".to_string())],
                payload,
            )
        }
        _ => {
            let generation_base_url = normalize_openai_generation_base_url(base_url);
            let headers = vec![
                ("authorization", format!("Bearer {}", context.api_key)),
                ("content-type", "application/json".to_string()),
            ];
            match protocol {
                OpenAiApiProtocol::Responses => {
                    let mut input = Vec::new();
                    if !context.system_prompt.trim().is_empty() {
                        input.push(json!({"role": "system", "content": context.system_prompt}));
                    }
                    input.push(json!({"role": "user", "content": prompt}));
                    let mut payload = json!({
                        "model": context.config.model,
                        "input": input,
                        "temperature": context.temperature,
                        "max_output_tokens": context.max_tokens,
                        "stream": false,
                    });
                    if !context.reasoning_depth.trim().is_empty() {
                        payload["reasoning"] = json!({"effort": context.reasoning_depth});
                    }
                    (
                        format!("{generation_base_url}/responses"),
                        headers,
                        payload,
                    )
                }
                OpenAiApiProtocol::ChatCompletions => {
                    let mut messages = Vec::new();
                    if !context.system_prompt.trim().is_empty() {
                        messages.push(json!({"role": "system", "content": context.system_prompt}));
                    }
                    messages.push(json!({"role": "user", "content": prompt}));
                    let mut payload = json!({
                        "model": context.config.model,
                        "messages": messages,
                        "temperature": context.temperature,
                        "max_tokens": context.max_tokens,
                    });
                    if !context.reasoning_depth.trim().is_empty()
                        && matches!(context.config.provider_name.as_str(), "openai" | "azure")
                    {
                        payload["reasoning_effort"] = json!(context.reasoning_depth);
                    }
                    (
                        format!("{generation_base_url}/chat/completions"),
                        headers,
                        payload,
                    )
                }
            }
        }
    }
}

pub(super) fn llm_provider_runtime_response(
    context: &LlmProviderRequestContext,
    raw_response: Value,
    protocol: OpenAiApiProtocol,
) -> Result<LlmProviderRuntimeResponse, ImportV2RouteResponse> {
    let content = match context.config.provider_kind.as_str() {
        "claude" => raw_response
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|block| block.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default(),
        "ollama" => raw_response
            .get("response")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ if protocol == OpenAiApiProtocol::Responses => responses_output_text(&raw_response),
        _ => raw_response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| {
                choice
                    .get("message")
                    .and_then(|message| message.get("content"))
                    .or_else(|| choice.get("text"))
            })
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    };
    if content.trim().is_empty() {
        return Err(llm_contract_error_response(
            "LLM provider returned empty content",
            "LLM_PROVIDER_UNAVAILABLE",
            503,
        ));
    }
    Ok(LlmProviderRuntimeResponse {
        content,
        model: context.config.model.clone(),
        provider: context.config.provider_name.clone(),
    })
}

fn responses_output_text(raw_response: &Value) -> String {
    if let Some(output_text) = raw_response
        .get("output_text")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        return output_text.to_string();
    }
    raw_response
        .get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|item| {
            item.get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter(|part| {
            part.get("type")
                .and_then(Value::as_str)
                .is_none_or(|part_type| part_type == "output_text")
        })
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use bill_analyser_core::LlmProviderConfigContract;

    fn context() -> LlmProviderRequestContext {
        LlmProviderRequestContext {
            config: LlmProviderConfigContract {
                provider: "qwen".to_string(),
                normalized_provider: "qwen".to_string(),
                provider_kind: "openai_compatible".to_string(),
                base_url: "https://gateway.example.test/v1/responses".to_string(),
                model: "gpt-test".to_string(),
                provider_name: "qwen".to_string(),
            },
            api_key: "secret-api-key".to_string(),
            credential_config: json!({}),
            config_id: Some(1),
            user_id: Some(2),
            system_prompt: String::new(),
            temperature: 0.0,
            max_tokens: 16,
            reasoning_depth: String::new(),
            api_protocol: "auto".to_string(),
        }
    }

    #[test]
    fn openai_protocol_requests_normalize_full_endpoints_and_parse_both_response_shapes() {
        let mut context = context();
        assert_eq!(
            llm_api_protocol_attempts(&context),
            vec![OpenAiApiProtocol::Responses, OpenAiApiProtocol::ChatCompletions]
        );

        let (responses_url, _, responses_payload) =
            llm_provider_http_request(&context, "hello", OpenAiApiProtocol::Responses);
        assert_eq!(responses_url, "https://gateway.example.test/v1/responses");
        assert_eq!(responses_payload["model"], "gpt-test");
        assert_eq!(responses_payload["input"][0]["role"], "user");
        assert_eq!(responses_payload["max_output_tokens"], 16);
        assert!(responses_payload.get("messages").is_none());

        let (chat_url, _, chat_payload) =
            llm_provider_http_request(&context, "hello", OpenAiApiProtocol::ChatCompletions);
        assert_eq!(chat_url, "https://gateway.example.test/v1/chat/completions");
        assert_eq!(chat_payload["messages"][0]["role"], "user");
        assert_eq!(chat_payload["max_tokens"], 16);
        assert!(chat_payload.get("input").is_none());

        let responses = llm_provider_runtime_response(
            &context,
            json!({
                "output": [{
                    "type": "message",
                    "content": [{"type": "output_text", "text": "responses-ok"}]
                }]
            }),
            OpenAiApiProtocol::Responses,
        )
        .expect("Responses API output");
        assert_eq!(responses.content, "responses-ok");

        let chat = llm_provider_runtime_response(
            &context,
            json!({"choices": [{"message": {"content": "chat-ok"}}]}),
            OpenAiApiProtocol::ChatCompletions,
        )
        .expect("Chat Completions output");
        assert_eq!(chat.content, "chat-ok");

        context.api_protocol = "responses".to_string();
        assert_eq!(llm_api_protocol_attempts(&context), vec![OpenAiApiProtocol::Responses]);
        context.api_protocol = "chat_completions".to_string();
        assert_eq!(
            llm_api_protocol_attempts(&context),
            vec![OpenAiApiProtocol::ChatCompletions]
        );
        assert!(llm_status_allows_protocol_fallback(reqwest::StatusCode::NOT_FOUND));
        assert!(llm_status_allows_protocol_fallback(reqwest::StatusCode::BAD_REQUEST));
        assert!(!llm_status_allows_protocol_fallback(reqwest::StatusCode::UNAUTHORIZED));
    }

    #[test]
    fn provider_adapters_cover_claude_ollama_reasoning_and_empty_edges() {
        let mut context = context();
        context.config.provider_kind = "claude".to_string();
        context.config.base_url = "https://api.anthropic.test/v1".to_string();
        context.system_prompt = "system".to_string();
        assert_eq!(
            llm_api_protocol_attempts(&context),
            vec![OpenAiApiProtocol::ChatCompletions]
        );
        let (url, headers, payload) =
            llm_provider_http_request(&context, "hello", OpenAiApiProtocol::ChatCompletions);
        assert_eq!(url, "https://api.anthropic.test/v1/messages");
        assert!(headers.iter().any(|(name, _)| *name == "x-api-key"));
        assert_eq!(payload["system"], "system");
        let response = llm_provider_runtime_response(
            &context,
            json!({"content": [
                {"type": "text", "text": "claude-"},
                {"type": "tool_use", "text": "ignored"},
                {"type": "text", "text": "ok"}
            ]}),
            OpenAiApiProtocol::ChatCompletions,
        )
        .expect("Claude response");
        assert_eq!(response.content, "claude-ok");

        context.config.provider_kind = "ollama".to_string();
        context.config.base_url = "http://127.0.0.1:11434/".to_string();
        let (url, headers, payload) =
            llm_provider_http_request(&context, "hello", OpenAiApiProtocol::ChatCompletions);
        assert_eq!(url, "http://127.0.0.1:11434/api/generate");
        assert_eq!(headers[0].0, "content-type");
        assert_eq!(payload["prompt"], "hello");
        assert_eq!(payload["system"], "system");
        let response = llm_provider_runtime_response(
            &context,
            json!({"response": "ollama-ok"}),
            OpenAiApiProtocol::ChatCompletions,
        )
        .expect("Ollama response");
        assert_eq!(response.content, "ollama-ok");

        context.config.provider_kind = "openai_compatible".to_string();
        context.config.provider_name = "openai".to_string();
        context.config.base_url = "https://gateway.example.test/v1/chat/completions".to_string();
        context.reasoning_depth = "high".to_string();
        let (url, _, payload) =
            llm_provider_http_request(&context, "hello", OpenAiApiProtocol::Responses);
        assert_eq!(url, "https://gateway.example.test/v1/responses");
        assert_eq!(payload["input"][0]["role"], "system");
        assert_eq!(payload["reasoning"]["effort"], "high");
        let (_, _, payload) =
            llm_provider_http_request(&context, "hello", OpenAiApiProtocol::ChatCompletions);
        assert_eq!(payload["reasoning_effort"], "high");

        let response = llm_provider_runtime_response(
            &context,
            json!({"output_text": "responses-top-level"}),
            OpenAiApiProtocol::Responses,
        )
        .expect("top-level Responses output");
        assert_eq!(response.content, "responses-top-level");
        let response = llm_provider_runtime_response(
            &context,
            json!({"choices": [{"text": "legacy-chat"}]}),
            OpenAiApiProtocol::ChatCompletions,
        )
        .expect("legacy Chat Completions output");
        assert_eq!(response.content, "legacy-chat");
        assert!(llm_provider_runtime_response(
            &context,
            json!({"output": []}),
            OpenAiApiProtocol::Responses,
        )
        .is_err());
    }
}
