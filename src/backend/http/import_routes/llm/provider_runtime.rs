// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

#[derive(Debug, Clone)]
struct LlmProviderRequestContext {
    config: LlmProviderConfigContract,
    api_key: String,
    credential_config: Value,
    config_id: Option<i64>,
    user_id: Option<i64>,
    system_prompt: String,
    temperature: f64,
    max_tokens: i64,
    reasoning_depth: String,
    api_protocol: String,
}

#[derive(Debug, Clone)]
struct LlmProviderRuntimeResponse {
    content: String,
    model: String,
    provider: String,
}

#[derive(Debug)]
struct LlmPreviewPrepared {
    session_id: String,
    provider: LlmProviderRequestContext,
    prompt: String,
    selected_preview_ids: Vec<i64>,
    account_ids: BTreeMap<String, i64>,
    missing_identities: BTreeMap<i64, LlmPreviewMissingIdentity>,
}

#[derive(Debug, Clone, Copy)]
struct LlmPreviewMissingIdentity {
    category: bool,
    source_account: bool,
    destination_account: bool,
}

#[derive(Debug, Clone)]
struct RuleInductionGroup {
    candidate_type: &'static str,
    category_id: Option<i64>,
    category_name: String,
    main_category: String,
    sub_category: String,
    account_id: Option<i64>,
    account_name: String,
    account_role: String,
    source_ids: Vec<i64>,
    transactions: Vec<Value>,
}

#[derive(Debug)]
enum LlmAnalyzePrepared {
    ImportSession {
        provider: LlmProviderRequestContext,
        session_id: String,
        groups: Vec<RuleInductionGroup>,
        rule_prompt_template: String,
    },
    Persisted {
        provider: LlmProviderRequestContext,
        transactions: Vec<Value>,
        bill_ids: Option<Vec<i64>>,
        classification_prompt_template: String,
    },
}

impl LlmAnalyzePrepared {
    fn provider_call_count(&self) -> usize {
        match self {
            Self::ImportSession { groups, .. } => groups.len(),
            Self::Persisted { transactions, .. } => usize::from(!transactions.is_empty()),
        }
    }
}

#[derive(Debug)]
struct LlmRuleSynthesisPrepared {
    provider: LlmProviderRequestContext,
    knowledge_pack: Value,
    categories: Vec<Value>,
    limit: usize,
}

fn llm_contract_error_response(
    message: &str,
    code: &str,
    status_code: u16,
) -> ImportV2RouteResponse {
    build_llm_contract_error_response(message, code, status_code)
}

fn rule_synthesis_empty_response(knowledge_pack: Value) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": {
                "mode": "rule_synthesis",
                "knowledge_summary_pack": knowledge_pack,
                "candidates_created": 0,
                "candidates": [],
            },
            "total": 0,
        }),
    }
}

fn rule_synthesis_has_learning_evidence(knowledge_pack: &Value) -> bool {
    knowledge_pack
        .get("categories")
        .and_then(Value::as_array)
        .is_some_and(|values| !values.is_empty())
}

fn llm_limit_from_object(
    object: &Map<String, Value>,
    default_limit: usize,
    max_limit: usize,
) -> Result<usize, ImportV2RouteResponse> {
    let raw_limit = first_value(object, &["limit"])
        .and_then(value_to_i64)
        .unwrap_or(i64::try_from(default_limit).unwrap_or(i64::MAX));
    if raw_limit <= 0 {
        return Err(llm_contract_error_response(
            "limit must be a positive integer",
            "INVALID_REQUEST",
            400,
        ));
    }
    let limit = usize::try_from(raw_limit).unwrap_or(usize::MAX);
    if limit > max_limit {
        return Err(llm_contract_error_response(
            &format!("limit cannot exceed {max_limit}"),
            "INVALID_REQUEST",
            400,
        ));
    }
    Ok(limit)
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_import_session_exists(
    runtime: &ImportRuntime,
    session_id: &str,
    user_id: UserId,
) -> Result<(), ImportV2RouteResponse> {
    match get_import_session(runtime.connection(), session_id, user_id) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(llm_contract_error_response(
            "Import session not found",
            "IMPORT_SESSION_NOT_FOUND",
            404,
        )),
        Err(error) => Err(db_error_response(error)),
    }
}

async fn effective_llm_runtime_config(
    state: &HttpAppState,
    user_id: i64,
) -> Result<Value, ImportV2RouteResponse> {
    let config = if let Some(config) = state.get_llm_runtime_config(user_id) {
        config
    } else {
        let runtime = open_postgres_runtime(state)?;
        effective_postgres_llm_config_from_saved(runtime.pool(), user_id)
            .await
            .map_err(db_error_response)?
    };
    if !config
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(llm_contract_error_response(
            "LLM service is not enabled",
            "LLM_DISABLED",
            400,
        ));
    }
    Ok(config)
}

fn llm_provider_context_from_config(
    config: &Value,
) -> Result<LlmProviderRequestContext, ImportV2RouteResponse> {
    let copied = copy_runtime_llm_config(config);
    let object = copied.as_object();
    let provider = object
        .and_then(|item| item.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or("openai");
    let provider_config = object
        .and_then(|item| item.get("provider_config"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let provider_contract = build_llm_provider_config(provider, Some(&provider_config))
        .map_err(|error| llm_contract_error_response(&error, "INVALID_REQUEST", 400))?;
    let provider_object = provider_config.as_object();
    let credential_config = normalize_provider_auth_config(
        object.and_then(|item| item.get("credential_config")),
    );
    let api_key = provider_object
        .and_then(|item| item.get("api_key"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| provider_auth_access_token(&credential_config))
        .unwrap_or_default()
        .to_string();
    let advanced = object
        .and_then(|item| item.get("advanced_settings"))
        .and_then(Value::as_object);
    let system_prompt = advanced
        .and_then(|item| item.get("system_prompt"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(LLM_SYSTEM_PROMPT)
        .to_string();
    let temperature = advanced
        .and_then(|item| item.get("temperature"))
        .and_then(value_to_f64)
        .unwrap_or(0.3);
    let max_tokens = advanced
        .and_then(|item| item.get("max_tokens"))
        .and_then(value_to_i64)
        .unwrap_or(2048);
    let reasoning_depth = advanced
        .and_then(|item| item.get("reasoning_depth"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let api_protocol = advanced
        .and_then(|item| item.get("api_protocol"))
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .to_string();
    Ok(LlmProviderRequestContext {
        config: provider_contract,
        api_key,
        credential_config,
        config_id: object
            .and_then(|item| item.get("id"))
            .and_then(Value::as_i64),
        user_id: object
            .and_then(|item| item.get("user_id"))
            .and_then(Value::as_i64),
        system_prompt,
        temperature,
        max_tokens,
        reasoning_depth,
        api_protocol,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
async fn execute_llm_provider_request(
    state: &HttpAppState,
    context: &LlmProviderRequestContext,
    prompt: &str,
) -> Result<LlmProviderRuntimeResponse, ImportV2RouteResponse> {
    let client = reqwest::Client::builder()
        .timeout(state.config.timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| {
            llm_contract_error_response("LLM provider unavailable", "LLM_PROVIDER_UNAVAILABLE", 503)
        })?;
    let mut context = context.clone();
    if provider_auth_is_expired(&context.credential_config, Utc::now()) {
        if provider_auth_has_refresh_credential(&context.credential_config) {
            context = refresh_llm_provider_context(state, &client, &context).await?;
        } else {
            return Err(llm_relogin_required_response());
        }
    }
    let mut did_refresh_after_unauthorized = false;
    let mut last_error = String::new();
    let protocols = llm_api_protocol_attempts(&context);
    'protocols: for (protocol_index, protocol) in protocols.iter().copied().enumerate() {
        let has_alternate = protocol_index + 1 < protocols.len();
        for attempt in 0..3 {
            let (url, headers, payload) = llm_provider_http_request(&context, prompt, protocol);
            let mut request = client.post(&url);
            for (key, value) in &headers {
                request = request.header(*key, value);
            }
            let response = request.body(payload.to_string()).send().await;
            match response {
                Ok(response) => {
                    let status = response.status();
                    let raw_text = read_limited_llm_provider_body(response).await?;
                    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        let detail = llm_provider_error_diagnostic(&raw_text, &context)
                            .map(|detail| format!(": {detail}"))
                            .unwrap_or_default();
                        return Err(llm_contract_error_response(
                            &format!("Rate limit exceeded{detail}"),
                            "LLM_RATE_LIMITED",
                            429,
                        ));
                    }
                    if status == reqwest::StatusCode::UNAUTHORIZED {
                        if !did_refresh_after_unauthorized
                            && provider_auth_has_refresh_credential(&context.credential_config)
                        {
                            context = refresh_llm_provider_context(state, &client, &context).await?;
                            did_refresh_after_unauthorized = true;
                            continue;
                        }
                        return Err(llm_relogin_required_response());
                    }
                    if status.is_success() {
                        let raw_response = match serde_json::from_str::<Value>(&raw_text) {
                            Ok(value) => value,
                            Err(_) if has_alternate => {
                                last_error = "provider protocol returned invalid JSON".to_string();
                                continue 'protocols;
                            }
                            Err(_) => {
                                return Err(llm_contract_error_response(
                                    "LLM provider returned invalid JSON",
                                    "LLM_PROVIDER_UNAVAILABLE",
                                    503,
                                ));
                            }
                        };
                        match llm_provider_runtime_response(&context, raw_response, protocol) {
                            Ok(response) => return Ok(response),
                            Err(_) if has_alternate => {
                                last_error = "provider protocol returned incompatible content".to_string();
                                continue 'protocols;
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    last_error = format!("provider status {}", status.as_u16());
                    if let Some(detail) = llm_provider_error_diagnostic(&raw_text, &context) {
                        last_error.push_str(": ");
                        last_error.push_str(&detail);
                    }
                    if has_alternate && llm_status_allows_protocol_fallback(status) {
                        continue 'protocols;
                    }
                    if !status.is_server_error() || attempt == 2 {
                        break 'protocols;
                    }
                }
                Err(error) => {
                    last_error = if error.is_timeout() {
                        "provider timeout".to_string()
                    } else {
                        "provider request failed".to_string()
                    };
                    if attempt == 2 {
                        break 'protocols;
                    }
                }
            }
            sleep(StdDuration::from_millis(
                50 * u64::try_from(attempt + 1).unwrap_or(1),
            ))
            .await;
        }
    }
    Err(llm_contract_error_response(
        &format!("LLM provider unavailable: {last_error}"),
        "LLM_PROVIDER_UNAVAILABLE",
        503,
    ))
}

fn llm_provider_error_diagnostic(
    raw_text: &str,
    context: &LlmProviderRequestContext,
) -> Option<String> {
    let body = serde_json::from_str::<Value>(raw_text).ok()?;
    let error = body.get("error").unwrap_or(&body);
    let message = error
        .get("message")
        .or_else(|| body.get("message"))
        .and_then(Value::as_str)
        .or_else(|| error.as_str());
    let code = error
        .get("code")
        .or_else(|| body.get("code"))
        .and_then(|value| value.as_str().map(str::to_string).or_else(|| value.as_i64().map(|item| item.to_string())));
    let mut secrets = vec![context.api_key.as_str()];
    collect_provider_secret_values(&context.credential_config, &mut secrets);
    let message = message.and_then(|value| sanitize_llm_provider_error_text(value, 240, &secrets));
    let code = code
        .as_deref()
        .and_then(|value| sanitize_llm_provider_error_text(value, 64, &secrets));
    match (message, code) {
        (Some(message), Some(code)) => Some(format!("{message} (code: {code})")),
        (Some(message), None) => Some(message),
        (None, Some(code)) => Some(format!("provider error code: {code}")),
        (None, None) => None,
    }
}

fn collect_provider_secret_values<'a>(value: &'a Value, secrets: &mut Vec<&'a str>) {
    let Some(object) = value.as_object() else {
        return;
    };
    for (key, value) in object {
        let normalized_key = key.to_ascii_lowercase();
        if (normalized_key.contains("secret")
            || normalized_key.contains("token")
            || normalized_key.contains("password")
            || normalized_key.contains("api_key"))
            && value.as_str().is_some_and(|text| !text.trim().is_empty())
        {
            secrets.push(value.as_str().unwrap_or_default());
        }
        if value.is_object() {
            collect_provider_secret_values(value, secrets);
        }
    }
}

fn sanitize_llm_provider_error_text(
    value: &str,
    max_chars: usize,
    secrets: &[&str],
) -> Option<String> {
    let mut sanitized = value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for secret in secrets
        .iter()
        .map(|secret| secret.trim())
        .filter(|secret| secret.chars().count() >= 4)
    {
        sanitized = sanitized.replace(secret, "[REDACTED]");
    }
    if sanitized.is_empty() {
        return None;
    }
    let mut truncated = sanitized.chars().take(max_chars).collect::<String>();
    if sanitized.chars().count() > max_chars {
        truncated.push('…');
    }
    Some(truncated)
}

fn llm_relogin_required_response() -> ImportV2RouteResponse {
    llm_contract_error_response(
        "LLM provider authorization expired; sign in again or refresh credentials",
        "LLM_RELOGIN_REQUIRED",
        401,
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn refresh_llm_provider_context(
    state: &HttpAppState,
    client: &reqwest::Client,
    context: &LlmProviderRequestContext,
) -> Result<LlmProviderRequestContext, ImportV2RouteResponse> {
    let refreshed = refresh_provider_auth_profile(
        client,
        &context.credential_config,
        &context.config.base_url,
    )
    .await
    .map_err(|_| llm_relogin_required_response())?;
    let Some(api_key) = provider_auth_access_token(&refreshed) else {
        return Err(llm_relogin_required_response());
    };
    persist_refreshed_llm_credentials(state, context, &refreshed).await;
    let mut updated = context.clone();
    updated.api_key = api_key;
    updated.credential_config = refreshed;
    Ok(updated)
}

#[tracing::instrument(level = "debug", skip_all)]
async fn persist_refreshed_llm_credentials(
    state: &HttpAppState,
    context: &LlmProviderRequestContext,
    credential_config: &Value,
) {
    let (Some(config_id), Some(user_id)) = (context.config_id, context.user_id) else {
        return;
    };
    let Ok(runtime) = open_postgres_runtime(state) else {
        return;
    };
    let _ = update_postgres_llm_config(
        runtime.pool(),
        config_id,
        user_id,
        &LlmConfigUpdate {
            credential_config: Some(credential_config.clone()),
            ..LlmConfigUpdate::default()
        },
    )
    .await;
}

#[tracing::instrument(level = "debug", skip_all)]
async fn read_limited_llm_provider_body(
    mut response: reqwest::Response,
) -> Result<String, ImportV2RouteResponse> {
    if response
        .content_length()
        .is_some_and(|length| length > LLM_PROVIDER_RESPONSE_MAX_BYTES as u64)
    {
        return Err(llm_contract_error_response(
            "LLM provider response is too large",
            "LLM_PROVIDER_UNAVAILABLE",
            503,
        ));
    }
    let mut bytes = BytesMut::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| {
        llm_contract_error_response(
            "LLM provider returned invalid response",
            "LLM_PROVIDER_UNAVAILABLE",
            503,
        )
    })? {
        if bytes.len().saturating_add(chunk.len()) > LLM_PROVIDER_RESPONSE_MAX_BYTES {
            return Err(llm_contract_error_response(
                "LLM provider response is too large",
                "LLM_PROVIDER_UNAVAILABLE",
                503,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        llm_contract_error_response(
            "LLM provider returned invalid JSON",
            "LLM_PROVIDER_UNAVAILABLE",
            503,
        )
    })
}

fn reserve_llm_rate_limit(user_id: i64, slots: usize) -> Result<(), ImportV2RouteResponse> {
    if slots == 0 {
        return Ok(());
    }
    let buckets = LLM_RATE_LIMIT_BUCKETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut buckets = buckets.lock().map_err(|_| {
        llm_contract_error_response("Rate limit state unavailable", "LLM_RATE_LIMITED", 429)
    })?;
    let now = Instant::now();
    let bucket = buckets.entry(user_id).or_default();
    bucket.retain(|instant| now.duration_since(*instant) < StdDuration::from_secs(60));
    if bucket.len().saturating_add(slots) > 10 {
        return Err(llm_contract_error_response(
            "Rate limit exceeded: max 10 calls per 60s",
            "LLM_RATE_LIMITED",
            429,
        ));
    }
    for _ in 0..slots {
        bucket.push_back(now);
    }
    Ok(())
}

#[cfg(test)]
mod provider_runtime_tests {
    use super::*;

    fn context_with_secrets() -> LlmProviderRequestContext {
        LlmProviderRequestContext {
            config: LlmProviderConfigContract {
                provider: "qwen".to_string(),
                normalized_provider: "qwen".to_string(),
                provider_kind: "openai_compatible".to_string(),
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                model: "qwen-plus".to_string(),
                provider_name: "qwen".to_string(),
            },
            api_key: "secret-api-key".to_string(),
            credential_config: json!({"nested": {"refresh_token": "secret-refresh"}}),
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
    fn provider_error_diagnostic_extracts_code_and_redacts_known_credentials() {
        let diagnostic = llm_provider_error_diagnostic(
            r#"{"error":{"message":"bad secret-api-key and secret-refresh\nrequest","code":"invalid_auth"}}"#,
            &context_with_secrets(),
        )
        .expect("provider diagnostic");

        assert_eq!(
            diagnostic,
            "bad [REDACTED] and [REDACTED] request (code: invalid_auth)"
        );
        assert!(!diagnostic.contains("secret"));
    }

    #[test]
    fn provider_error_diagnostic_ignores_non_json_and_truncates_messages() {
        assert!(llm_provider_error_diagnostic("upstream html", &context_with_secrets()).is_none());
        let diagnostic = llm_provider_error_diagnostic(
            &json!({"message": "x".repeat(300)}).to_string(),
            &context_with_secrets(),
        )
        .expect("truncated diagnostic");
        assert_eq!(diagnostic.chars().count(), 241);
        assert!(diagnostic.ends_with('…'));
    }
}
