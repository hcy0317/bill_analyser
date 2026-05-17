fn open_runtime(state: &HttpAppState) -> Result<SqliteRuntime, ImportV2RouteResponse> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        import_v2_error_response(
            503,
            "Rust import DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        )
    })?;
    let db_path = SqliteDbPath::application_file(db_path)
        .map_err(|error| import_v2_error_response(503, &error.to_string()))?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: state.config.timeout,
    })
    .map_err(db_error_response)
}

fn init_import_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_import_staging_schema(runtime.connection()).map_err(db_error_response)
}

fn init_ocr_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_app_settings_schema(runtime.connection()).map_err(db_error_response)
}

fn init_llm_config_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_llm_runtime_schema(runtime.connection()).map_err(db_error_response)
}

fn user_id_from_headers(
    headers: &HeaderMap,
    config: &HttpShellConfig,
) -> Result<UserId, ImportV2RouteResponse> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER)
        .map_err(|error| import_v2_error_response(error.status, &error.message))
}

fn db_error_response(_error: impl std::fmt::Display) -> ImportV2RouteResponse {
    import_v2_error_response(500, "Rust import route runtime DB error")
}

fn non_negative_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

#[derive(Debug, Default, Deserialize)]
pub struct ImportConfigQuery {
    file_format: Option<String>,
    #[serde(rename = "fileFormat")]
    file_format_camel: Option<String>,
    limit: Option<usize>,
}

impl ImportConfigQuery {
    fn file_format(&self) -> Option<&String> {
        self.file_format
            .as_ref()
            .or(self.file_format_camel.as_ref())
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct ImportLearningRulesQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    limit: Option<usize>,
    enabled_only: Option<bool>,
    #[serde(rename = "enabledOnly")]
    enabled_only_camel: Option<bool>,
}

impl ImportLearningRulesQuery {
    fn page_size(&self) -> usize {
        self.page_size
            .or(self.page_size_camel)
            .or(self.limit)
            .unwrap_or(100)
    }

    fn enabled_only(&self) -> Option<bool> {
        self.enabled_only.or(self.enabled_only_camel)
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct LearningCenterListQuery {
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    enabled_only: Option<bool>,
    #[serde(rename = "enabledOnly")]
    enabled_only_camel: Option<bool>,
}

impl LearningCenterListQuery {
    fn enabled_only(&self) -> Option<bool> {
        self.enabled_only.or(self.enabled_only_camel)
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PreviewPageQuery {
    page: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    sort_by: Option<String>,
    #[serde(rename = "sortBy")]
    sort_by_camel: Option<String>,
    sort_direction: Option<String>,
    #[serde(rename = "sortDirection")]
    sort_direction_camel: Option<String>,
    preview_ids: Option<String>,
    #[serde(rename = "previewIds")]
    preview_ids_camel: Option<String>,
    selected_only: Option<bool>,
    #[serde(rename = "selectedOnly")]
    selected_only_camel: Option<bool>,
    min_datetime: Option<String>,
    #[serde(rename = "minDatetime")]
    min_datetime_camel: Option<String>,
    max_datetime: Option<String>,
    #[serde(rename = "maxDatetime")]
    max_datetime_camel: Option<String>,
    transaction_type: Option<String>,
    #[serde(rename = "transactionType")]
    transaction_type_camel: Option<String>,
    category: Option<String>,
    account: Option<String>,
    tag: Option<String>,
    signal: Option<String>,
    annotation: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LlmMemoryQuery {
    session_id: Option<String>,
    event_type: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LlmCandidatesQuery {
    status: Option<String>,
    r#type: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_context(provider: &str, provider_config: Value) -> LlmProviderRequestContext {
        LlmProviderRequestContext {
            config: build_llm_provider_config(provider, Some(&provider_config))
                .expect("provider contract"),
            api_key: "secret-key".to_string(),
            system_prompt: "system prompt".to_string(),
            temperature: 0.25,
            max_tokens: 128,
            reasoning_depth: "low".to_string(),
        }
    }

    fn local_openai_context(base_url: String) -> LlmProviderRequestContext {
        LlmProviderRequestContext {
            config: LlmProviderConfigContract {
                provider: "openai_compatible".to_string(),
                normalized_provider: "openai_compatible".to_string(),
                provider_kind: "openai_compatible".to_string(),
                base_url,
                model: "fake".to_string(),
                provider_name: "openai_compatible".to_string(),
            },
            api_key: "secret-key".to_string(),
            system_prompt: "system prompt".to_string(),
            temperature: 0.25,
            max_tokens: 128,
            reasoning_depth: "low".to_string(),
        }
    }

    fn row(
        id: i64,
        main_category: &str,
        sub_category: &str,
        description: &str,
    ) -> ImportPreviewRow {
        ImportPreviewRow {
            id,
            session_id: "session-a".to_string(),
            user_id: 42,
            preview_date: "2026-05-01".to_string(),
            preview_type: "支出".to_string(),
            preview_amount: 12.5,
            preview_destination_amount: 0.0,
            preview_main_category: main_category.to_string(),
            preview_sub_category: sub_category.to_string(),
            preview_source_account_id: None,
            preview_destination_account_id: None,
            preview_counterparty: "canteen".to_string(),
            preview_payment_method: "card".to_string(),
            preview_description: description.to_string(),
            preview_parser_id: "wechat".to_string(),
            preview_parser_tags: Vec::new(),
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: true,
            dedup_type: String::new(),
            dedup_source_ids: Vec::new(),
            preview_matching_feedback: json!({}),
            created_at: "2026-05-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn llm_provider_payloads_responses_and_limits_cover_provider_edges() {
        let openai = provider_context(
            "openai",
            json!({
                "base_url": "https://api.openai.com/v1",
                "model": "gpt-test",
            }),
        );
        let (url, headers, payload) = llm_provider_http_request(&openai, "classify this");
        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
        assert!(headers.iter().any(|(key, _)| *key == "authorization"));
        assert_eq!(payload["reasoning_effort"], "low");
        assert_eq!(
            llm_provider_runtime_response(
                &openai,
                json!({"choices": [{"message": {"content": "[{\"bill_id\":1}]"}}]})
            )
            .expect("openai response")
            .content,
            "[{\"bill_id\":1}]"
        );

        let claude = provider_context(
            "anthropic",
            json!({
                "base_url": "https://api.anthropic.com/v1",
                "model": "claude-test",
            }),
        );
        let (url, headers, payload) = llm_provider_http_request(&claude, "rules");
        assert_eq!(url, "https://api.anthropic.com/v1/messages");
        assert!(headers.iter().any(|(key, _)| *key == "x-api-key"));
        assert_eq!(payload["system"], "system prompt");
        assert_eq!(
            llm_provider_runtime_response(
                &claude,
                json!({"content": [{"type": "text", "text": "[{\"rule_expression\":\"OR={咖啡}\"}]"}, {"type": "tool", "text": "ignored"}]})
            )
            .expect("claude response")
            .content,
            "[{\"rule_expression\":\"OR={咖啡}\"}]"
        );

        let ollama = provider_context(
            "ollama",
            json!({
                "base_url": "http://localhost:11434",
                "model": "llama-test",
            }),
        );
        let (url, headers, payload) = llm_provider_http_request(&ollama, "preview");
        assert_eq!(url, "http://localhost:11434/api/generate");
        assert_eq!(
            headers,
            vec![("content-type", "application/json".to_string())]
        );
        assert_eq!(payload["system"], "system prompt");
        assert_eq!(
            llm_provider_runtime_response(&ollama, json!({"response": "[{\"preview_id\":1}]"}))
                .expect("ollama response")
                .content,
            "[{\"preview_id\":1}]"
        );
        assert!(llm_provider_runtime_response(&ollama, json!({"response": ""})).is_err());

        let user_id = -9_001;
        assert!(reserve_llm_rate_limit(user_id, 0).is_ok());
        assert!(reserve_llm_rate_limit(user_id, 10).is_ok());
        assert!(reserve_llm_rate_limit(user_id, 1).is_err());

        let mut limit_object = Map::new();
        assert_eq!(
            llm_limit_from_object(&limit_object, 8, 20).expect("default limit"),
            8
        );
        limit_object.insert("limit".to_string(), json!(0));
        assert!(llm_limit_from_object(&limit_object, 8, 20).is_err());
        limit_object.insert("limit".to_string(), json!(21));
        assert!(llm_limit_from_object(&limit_object, 8, 20).is_err());
    }

    #[tokio::test]
    async fn llm_provider_execution_covers_transport_and_payload_errors() {
        let app = Router::new().route(
            "/chat/completions",
            post(|axum::Json(payload): axum::Json<Value>| async move {
                let prompt = payload
                    .get("messages")
                    .and_then(Value::as_array)
                    .and_then(|messages| messages.last())
                    .and_then(|message| message.get("content"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                match prompt {
                    "rate" => (StatusCode::TOO_MANY_REQUESTS, "limited").into_response(),
                    "bad-json" => (StatusCode::OK, "not-json").into_response(),
                    "empty" => (
                        StatusCode::OK,
                        Json(json!({"choices": [{"message": {"content": ""}}]})),
                    )
                        .into_response(),
                    "server-error" => (StatusCode::INTERNAL_SERVER_ERROR, "retry").into_response(),
                    _ => (
                        StatusCode::OK,
                        Json(json!({"choices": [{"message": {"content": "[{\"bill_id\":1}]"}}]})),
                    )
                        .into_response(),
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("provider server");
        });
        let context = local_openai_context(format!("http://{addr}"));
        let config = HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            StdDuration::from_millis(50),
            1024,
            crate::config::ImportRouteMode::ImportDbRuntime,
        )
        .expect("config");
        let state = HttpAppState::new(config).expect("state");

        assert!(execute_llm_provider_request(&state, &context, "ok")
            .await
            .is_ok());
        assert!(execute_llm_provider_request(&state, &context, "rate")
            .await
            .is_err());
        assert!(execute_llm_provider_request(&state, &context, "bad-json")
            .await
            .is_err());
        assert!(execute_llm_provider_request(&state, &context, "empty")
            .await
            .is_err());
        assert!(
            execute_llm_provider_request(&state, &context, "server-error")
                .await
                .is_err()
        );

        let refused = local_openai_context("http://127.0.0.1:9".to_string());
        assert!(execute_llm_provider_request(&state, &refused, "transport")
            .await
            .is_err());
        handle.abort();
    }

    #[test]
    fn llm_provider_db_helpers_cover_selection_context_and_duplicates() {
        let connection = Connection::open_in_memory().expect("memory db");
        assert!(selected_preview_rows_for_llm(
            &connection,
            &json!({"preview_ids": []}),
            "session-a",
            UserId::new(42).expect("user"),
            10,
        )
        .is_err());
        assert!(load_existing_category_values(&connection, 42)
            .expect("missing categories")
            .is_empty());
        assert!(load_account_id_map(&connection, 42)
            .expect("missing accounts")
            .is_empty());
        assert!(load_learning_concept_stats(&connection, 42)
            .expect("missing stats")
            .is_empty());

        connection
            .execute_batch(
                "
                CREATE TABLE categories(
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    main_category TEXT,
                    sub_category TEXT
                );
                CREATE TABLE accounts(
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT
                );
                CREATE TABLE import_learning_concept_stats(
                    user_id INTEGER NOT NULL,
                    concept_key TEXT,
                    concept_type TEXT,
                    sample_count INTEGER,
                    accepted_count INTEGER,
                    rejected_count INTEGER,
                    auto_applied_count INTEGER,
                    rollback_count INTEGER,
                    updated_at TEXT
                );
                CREATE TABLE llm_candidates(
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL,
                    type TEXT NOT NULL,
                    source_bill_ids TEXT NOT NULL,
                    suggested_main_category TEXT,
                    suggested_sub_category TEXT,
                    suggested_rule_expression TEXT,
                    confidence REAL,
                    llm_provider TEXT,
                    llm_model TEXT,
                    llm_response_raw TEXT,
                    status TEXT NOT NULL,
                    created_at TEXT
                );
                CREATE TABLE category_rules(
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    category_id INTEGER NOT NULL,
                    rule_expression TEXT NOT NULL
                );
                INSERT INTO categories(id, user_id, main_category, sub_category)
                    VALUES (1, 42, '餐饮', '咖啡');
                INSERT INTO accounts(id, user_id, name)
                    VALUES (7, 42, '现金'), (8, 42, '');
                INSERT INTO import_learning_concept_stats(
                    user_id, concept_key, concept_type, sample_count, accepted_count,
                    rejected_count, auto_applied_count, rollback_count, updated_at
                ) VALUES (42, 'merchant:cafe', 'merchant', 3, 2, 1, 0, 0, '2026-05-01');
                INSERT INTO category_rules(id, user_id, category_id, rule_expression)
                    VALUES (5, 42, 1, 'OR={咖啡}');
                ",
            )
            .expect("schema");

        let categories = load_existing_category_values(&connection, 42).expect("categories");
        assert_eq!(categories[0]["path"], "餐饮/咖啡");
        assert_eq!(
            load_existing_category_paths(&connection, 42).expect("category paths"),
            vec!["餐饮/咖啡".to_string()]
        );
        assert_eq!(
            load_existing_account_names(&connection, 42).expect("account names"),
            vec!["现金".to_string()]
        );
        let account_ids = load_account_id_map(&connection, 42).expect("account ids");
        assert_eq!(account_ids["现金"], 7);
        assert_eq!(
            load_learning_concept_stats(&connection, 42).expect("stats")[0]["concept_key"],
            "merchant:cafe"
        );
        init_import_staging_schema(&connection).expect("llm memory schema");
        bill_analyser_db::create_llm_memory_event(
            &connection,
            &bill_analyser_db::LlmMemoryEventDraft {
                user_id: UserId::new(42).expect("user"),
                session_id: Some("session-a".to_string()),
                preview_id: Some(1),
                event_type: "feedback".to_string(),
                decision: Some("accept".to_string()),
                prompt_text: None,
                llm_response_raw: None,
                llm_provider: Some("openai".to_string()),
                llm_model: Some("gpt".to_string()),
                suggested_main_category: Some("餐饮".to_string()),
                suggested_sub_category: Some("咖啡".to_string()),
                suggested_source_account: None,
                suggested_destination_account: None,
                confidence: 0.9,
                user_correction_category: None,
                user_correction_account: None,
                snapshot_before: None,
                snapshot_after: None,
                metadata: Some(json!({"description": "latte"})),
            },
        )
        .expect("memory event");
        let memory = load_llm_memory_prompt_context(&connection, UserId::new(42).expect("user"))
            .expect("memory context");
        assert_eq!(memory[0]["decision"], "accept");
        assert_eq!(memory[0]["description_hint"], "latte");
        assert!(
            rule_candidate_duplicate(&connection, 42, "餐饮", "咖啡", "OR={咖啡}")
                .expect("rule duplicate")
        );
        assert!(
            rule_candidate_duplicate(&connection, 42, "餐饮", "咖啡", " OR={咖啡} ")
                .expect("trimmed rule duplicate")
        );
        assert!(
            !rule_candidate_duplicate(&connection, 42, "餐饮", "咖啡", "OR={奶茶}")
                .expect("no duplicate")
        );
        let no_rule_tables = Connection::open_in_memory().expect("memory db");
        no_rule_tables
            .execute_batch(
                "
                CREATE TABLE llm_candidates(
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL,
                    type TEXT NOT NULL,
                    source_bill_ids TEXT NOT NULL,
                    suggested_main_category TEXT,
                    suggested_sub_category TEXT,
                    suggested_rule_expression TEXT,
                    confidence REAL,
                    llm_provider TEXT,
                    llm_model TEXT,
                    llm_response_raw TEXT,
                    status TEXT NOT NULL,
                    created_at TEXT
                );
                ",
            )
            .expect("candidate schema");
        assert!(
            !rule_candidate_duplicate(&no_rule_tables, 42, "餐饮", "咖啡", "OR={咖啡}")
                .expect("no category rule table")
        );

        let filled = fill_llm_suggestion_account_ids(
            json!({
                "suggested_source_account": "现金",
                "suggested_destination_account": "不存在"
            }),
            &account_ids,
        );
        assert_eq!(filled["resolved_source_account_id"], 7);
        assert!(filled.get("resolved_destination_account_id").is_none());
        let kept = fill_llm_suggestion_account_ids(
            json!({"resolved_source_account_id": 99, "source_account": "现金"}),
            &account_ids,
        );
        assert_eq!(kept["resolved_source_account_id"], 7);
        let removed = fill_llm_suggestion_account_ids(
            json!({"sourceAccountId": 99, "source_account": "不存在"}),
            &account_ids,
        );
        assert!(removed.get("sourceAccountId").is_none());
        assert!(removed.get("resolved_source_account_id").is_none());
        assert_eq!(
            fill_llm_suggestion_account_ids(json!("plain"), &account_ids),
            json!("plain")
        );
    }

    #[test]
    fn llm_runtime_config_and_session_helpers_cover_disabled_and_missing_edges() {
        let temp = tempfile::NamedTempFile::new().expect("temp db");
        let db_path = SqliteDbPath::temporary_file(temp.path()).expect("temp sqlite path");
        let mut runtime = SqliteRuntime::open(SqliteConnectionConfig {
            path: db_path,
            create_if_missing: true,
            busy_timeout: StdDuration::from_millis(50),
        })
        .expect("runtime");
        init_import_staging_schema(runtime.connection()).expect("import schema");
        runtime
            .connection()
            .execute_batch(
                "
                CREATE TABLE users(id INTEGER PRIMARY KEY, username TEXT, is_active INTEGER);
                INSERT INTO users(id, username, is_active) VALUES (42, 'user-42', 1);
                ",
            )
            .expect("users");
        assert!(ensure_import_session_exists(
            &runtime,
            "missing-session",
            UserId::new(42).expect("user"),
        )
        .is_err());
        assert!(selected_preview_rows_for_llm(
            runtime.connection(),
            &json!({}),
            "missing-session",
            UserId::new(42).expect("user"),
            10,
        )
        .expect("empty preview selection")
        .is_empty());
        bill_analyser_db::create_import_session(
            runtime.connection(),
            &ImportSessionDraft {
                session_id: "session-a".to_string(),
                user_id: UserId::new(42).expect("user"),
                file_count: 1,
            },
        )
        .expect("session");
        insert_preview_bills_batch(
            runtime.connection_mut(),
            "session-a",
            UserId::new(42).expect("user"),
            &[bill_analyser_db::ImportPreviewDraft {
                preview_date: "2026-05-01".to_string(),
                preview_type: "支出".to_string(),
                preview_amount: 12.5,
                preview_main_category: "餐饮".to_string(),
                preview_sub_category: "咖啡".to_string(),
                preview_counterparty: "cafe".to_string(),
                preview_description: "latte".to_string(),
                preview_payment_method: "cash".to_string(),
                ..Default::default()
            }],
        )
        .expect("preview row");
        assert!(load_preview_rows_by_ids(
            runtime.connection(),
            "other-session",
            UserId::new(42).expect("user"),
            &[1],
        )
        .expect("wrong session rows")
        .is_empty());
        let invalid_preview_ids = selected_preview_rows_for_llm(
            runtime.connection(),
            &json!({"preview_ids": "1"}),
            "session-a",
            UserId::new(42).expect("user"),
            10,
        )
        .expect_err("invalid preview id shape");
        assert_eq!(invalid_preview_ids.status_code, 400);
        assert_eq!(invalid_preview_ids.body["code"], "INVALID_REQUEST");
        let limited_preview_rows = selected_preview_rows_for_llm(
            runtime.connection(),
            &json!({"preview_ids": [1, 1]}),
            "session-a",
            UserId::new(42).expect("user"),
            1,
        )
        .expect("deduped preview rows");
        assert_eq!(limited_preview_rows.len(), 1);
        let too_many_preview_ids = selected_preview_rows_for_llm(
            runtime.connection(),
            &json!({"preview_ids": [1, 999]}),
            "session-a",
            UserId::new(42).expect("user"),
            1,
        )
        .expect_err("too many preview ids");
        assert_eq!(too_many_preview_ids.status_code, 422);
        assert_eq!(
            too_many_preview_ids.body["code"],
            "PREVIEW_SELECTION_TOO_LARGE"
        );

        let config = HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            StdDuration::from_millis(50),
            1024,
            crate::config::ImportRouteMode::ImportDbRuntime,
        )
        .expect("config");
        let state = HttpAppState::new(config).expect("state");
        state.set_llm_runtime_config(42, json!({"enabled": false}));
        assert!(effective_llm_runtime_config(&state, &runtime, 42).is_err());

        init_global_learning_runtime_schema(&runtime).expect("global learning schema");
        runtime
            .connection()
            .execute_batch(
                "
                CREATE TABLE import_learning_model_registry (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL,
                    model_key TEXT NOT NULL,
                    model_version TEXT NOT NULL,
                    dataset_snapshot_id INTEGER,
                    status TEXT NOT NULL,
                    metrics_json TEXT,
                    updated_at TEXT NOT NULL
                );
                INSERT INTO import_learning_rules(
                    user_id, match_type, match_value, normalized_match_value, learned_type,
                    learned_category_id, enabled, match_features_json, applied_count,
                    created_at, updated_at
                )
                VALUES (
                    42, 'merchant', 'cafe', 'cafe', '支出', 1, 1,
                    '{\"parser_id\":\"wechat\"}', 3,
                    '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z'
                );
                INSERT INTO import_learning_suggestions(
                    user_id, match_type, match_value, normalized_match_value,
                    match_features_json, suggested_type, suggested_category_id,
                    sample_count, status, created_at, updated_at
                )
                VALUES (
                    42, 'merchant', 'old cafe', 'old cafe',
                    '{\"parser_id\":\"wechat\"}', '支出', 1,
                    2, 'rejected', '2026-05-14T00:00:00Z', '2026-05-14T00:00:00Z'
                );
                INSERT INTO import_learning_concept_stats(
                    user_id, concept_key, concept_type, sample_count, accepted_count,
                    rejected_count, auto_applied_count, rollback_count, updated_at
                )
                VALUES (
                    42, 'rule:1', 'rule', 3, 2, 0, 1, 0,
                    '2026-05-14T00:00:00Z'
                );
                INSERT INTO import_learning_model_registry(
                    user_id, model_key, model_version, dataset_snapshot_id, status,
                    metrics_json, updated_at
                )
                VALUES (
                    42, 'import-learning-dual-head', 'v1', 7, 'active',
                    '{\"feature_schema_version\":\"fs1\",\"policy_version\":\"p1\",\"sample_count\":3}',
                    '2026-05-14T00:00:00Z'
                );
                ",
            )
            .expect("learning evidence");
        bill_analyser_db::create_llm_memory_event(
            runtime.connection(),
            &bill_analyser_db::LlmMemoryEventDraft {
                user_id: UserId::new(42).expect("user"),
                session_id: Some("session-a".to_string()),
                preview_id: Some(1),
                event_type: "feedback".to_string(),
                decision: Some("accept".to_string()),
                prompt_text: None,
                llm_response_raw: None,
                llm_provider: Some("openai".to_string()),
                llm_model: Some("gpt".to_string()),
                suggested_main_category: Some("餐饮".to_string()),
                suggested_sub_category: Some("咖啡".to_string()),
                suggested_source_account: None,
                suggested_destination_account: None,
                confidence: 0.9,
                user_correction_category: None,
                user_correction_account: None,
                snapshot_before: None,
                snapshot_after: None,
                metadata: Some(json!({"counterparty": "cafe"})),
            },
        )
        .expect("memory event");
        let pack = build_rule_synthesis_knowledge_pack(
            runtime.connection(),
            UserId::new(42).expect("user"),
            &[json!({"id": 1, "path": "餐饮/咖啡"})],
        )
        .expect("knowledge pack");
        assert_eq!(pack["recent_llm_feedback"][0]["decision"], "accept");
        assert_eq!(pack["categories"][0]["category_name"], "餐饮/咖啡");
        assert_eq!(
            pack["categories"][0]["evidence"][0]["source"],
            "learning_rule"
        );
        assert_eq!(pack["active_model"]["model_version"], "v1");
        assert!(pack["learning_suggestions"]
            .as_array()
            .expect("learning suggestions")
            .is_empty());
        assert!(rule_synthesis_has_learning_evidence(&pack));
        assert!(!rule_synthesis_has_learning_evidence(&json!({
            "categories": [],
        })));
    }

    #[test]
    fn llm_rule_grouping_and_bill_prompt_helpers_cover_edges() {
        let mut blank_evidence = row(5, "餐饮", "午餐", "");
        blank_evidence.preview_counterparty.clear();
        blank_evidence.preview_payment_method.clear();
        let groups = rule_induction_groups(vec![
            row(1, "餐饮", "午餐", "canteen lunch"),
            row(2, "餐饮", "午餐", ""),
            row(3, "", "", "ignored category"),
            blank_evidence,
        ])
        .expect("rule groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].category_name, "餐饮/午餐");
        assert_eq!(groups[0].source_ids, vec![1, 2]);
        assert!(rule_induction_groups(vec![row(4, "", "", "")]).is_err());
        let too_many_groups = (1..=11)
            .map(|id| row(id, &format!("分类{id}"), "", "evidence"))
            .collect::<Vec<_>>();
        let too_many = rule_induction_groups(too_many_groups).expect_err("too many groups");
        assert_eq!(too_many.status_code, 422);
        assert_eq!(too_many.body["code"], "PREVIEW_SELECTION_TOO_LARGE");

        assert_eq!(category_path("餐饮", ""), "餐饮");
        assert_eq!(category_path("", "咖啡"), "咖啡");
        assert_eq!(category_path("餐饮", "咖啡"), "餐饮/咖啡");

        let connection = Connection::open_in_memory().expect("memory db");
        connection
            .execute_batch(
                "
                CREATE TABLE bills(
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    date TEXT,
                    amount REAL,
                    counterparty TEXT,
                    description TEXT,
                    payment_method TEXT,
                    main_category TEXT
                );
                INSERT INTO bills(id, user_id, date, amount, counterparty, description, payment_method, main_category)
                    VALUES (10, 42, '2026-05-01', 18.5, 'cafe', 'latte', 'cash', ''),
                           (11, 42, '2026-05-02', 8.0, 'shop', 'snack', 'card', '餐饮');
                ",
            )
            .expect("bills schema");
        assert!(
            load_persisted_bill_prompt_values(&connection, 42, Some(&[]), 10)
                .expect("empty selected ids")
                .is_empty()
        );
        let selected = load_persisted_bill_prompt_values(&connection, 42, Some(&[10, -1]), 10)
            .expect("selected bill");
        assert_eq!(selected[0]["id"], 10);
        let deduped_selected =
            load_persisted_bill_prompt_values(&connection, 42, Some(&[10, 10, 11]), 1)
                .expect("deduped selected bill");
        assert_eq!(deduped_selected.len(), 1);
        let uncategorized = load_persisted_bill_prompt_values(&connection, 42, None, 10)
            .expect("uncategorized bills");
        assert_eq!(uncategorized.len(), 1);

        assert_eq!(preview_id_from_llm_value(&json!({"previewId": 9})), 9);
        assert_eq!(
            main_category_from_llm_value(&json!({"mainCategory": "餐饮"})),
            "餐饮"
        );
        assert_eq!(
            sub_category_from_llm_value(&json!({"subCategory": "咖啡"})),
            "咖啡"
        );
        assert_eq!(
            rule_expression_from_llm_value(&json!({"suggestedRuleExpression": "OR={咖啡}"})),
            "OR={咖啡}"
        );
        assert_eq!(confidence_from_value(&json!({"confidence": 2.0})), 1.0);
        assert!(valid_rule_expression("OR={咖啡}"));
        assert!(!valid_rule_expression("(OR={broken}"));
    }
}
