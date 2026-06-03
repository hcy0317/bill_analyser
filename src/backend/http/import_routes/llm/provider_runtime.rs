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
}

#[derive(Debug, Clone)]
struct RuleInductionGroup {
    category_name: String,
    main_category: String,
    sub_category: String,
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
    let response = build_llm_contract_error_response(message, code, status_code);
    ImportV2RouteResponse {
        status_code: response.status_code,
        body: response.body,
    }
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
    for attempt in 0..3 {
        let (url, headers, payload) = llm_provider_http_request(&context, prompt);
        let mut request = client.post(&url);
        for (key, value) in &headers {
            request = request.header(*key, value);
        }
        let response = request.body(payload.to_string()).send().await;
        match response {
            Ok(response) => {
                let status = response.status();
                if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    return Err(llm_contract_error_response(
                        "Rate limit exceeded",
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
                    let raw_text = read_limited_llm_provider_body(response).await?;
                    let raw_response = serde_json::from_str::<Value>(&raw_text).map_err(|_| {
                        llm_contract_error_response(
                            "LLM provider returned invalid JSON",
                            "LLM_PROVIDER_UNAVAILABLE",
                            503,
                        )
                    })?;
                    return llm_provider_runtime_response(&context, raw_response);
                }
                last_error = format!("provider status {}", status.as_u16());
                if !status.is_server_error() || attempt == 2 {
                    break;
                }
            }
            Err(error) => {
                last_error = if error.is_timeout() {
                    "provider timeout".to_string()
                } else {
                    "provider request failed".to_string()
                };
                if attempt == 2 {
                    break;
                }
            }
        }
        sleep(StdDuration::from_millis(
            50 * u64::try_from(attempt + 1).unwrap_or(1),
        ))
        .await;
    }
    Err(llm_contract_error_response(
        &format!("LLM provider unavailable: {last_error}"),
        "LLM_PROVIDER_UNAVAILABLE",
        503,
    ))
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

fn llm_provider_http_request(
    context: &LlmProviderRequestContext,
    prompt: &str,
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
                format!("{base_url}/chat/completions"),
                vec![
                    ("authorization", format!("Bearer {}", context.api_key)),
                    ("content-type", "application/json".to_string()),
                ],
                payload,
            )
        }
    }
}

fn llm_provider_runtime_response(
    context: &LlmProviderRequestContext,
    raw_response: Value,
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
        _ => raw_response
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
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
