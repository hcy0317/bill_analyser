use bill_analyser_core::ai_ocr_llm::{
    build_llm_analysis_response, build_llm_candidate_list_response,
    build_llm_candidate_reject_response, build_llm_classification_prompt,
    build_llm_config_get_response, build_llm_contract_error_response,
    build_llm_import_preview_recommendation_prompt, build_llm_preview_recommend_response,
    build_llm_provider_config, build_llm_rule_expression_synthesis_prompt,
    build_llm_rule_induction_prompt, build_ocr_config_response_payload,
    build_ocr_config_success_response, build_ocr_error_response,
    build_ocr_recognition_success_response, build_ocr_recognition_success_response_with_context,
    build_runtime_llm_config_from_saved_config, build_unknown_ocr_provider_response,
    copy_runtime_llm_config, llm_available_providers, llm_review_endpoint_requires_live_provider,
    normalize_llm_advanced_settings, normalize_llm_provider_name, normalize_ocr_config,
    ocr_available_providers_with_disabled, ocr_error_http_status, parse_llm_json_array_response,
    parse_payment_screenshot_text, render_llm_prompt_template, safe_llm_config_payload,
    OcrProviderTextLine, OcrProviderTextResult, ReceiptDraftAccount, ReceiptDraftCategory,
    ReceiptDraftCategoryRule, ReceiptDraftContext, ReceiptDraftTag,
};
use serde_json::{json, Value};
use std::env;

struct EnvVarRestore {
    name: &'static str,
    value: Option<String>,
}

impl EnvVarRestore {
    fn capture(name: &'static str) -> Self {
        Self {
            name,
            value: env::var(name).ok(),
        }
    }
}

impl Drop for EnvVarRestore {
    fn drop(&mut self) {
        if let Some(value) = &self.value {
            env::set_var(self.name, value);
        } else {
            env::remove_var(self.name);
        }
    }
}

#[test]
fn ocr_config_and_disabled_safe_errors_match_receipt_routes() {
    let default_config = normalize_ocr_config(None);
    assert_eq!(default_config.provider, "disabled");
    assert_eq!(default_config.lang, "chi_sim+eng");

    let configured = normalize_ocr_config(Some(&json!({
        "provider": " TESSERACT ",
        "lang": "eng+chi_sim",
    })));
    assert_eq!(configured.provider, "tesseract");
    assert_eq!(configured.lang, "eng+chi_sim");
    assert_eq!(
        ocr_available_providers_with_disabled(),
        vec!["disabled", "cloud_stub", "tesseract", "local_json_ocr"]
    );
    assert_eq!(
        build_ocr_config_response_payload(&configured),
        json!({
            "provider": "tesseract",
            "lang": "eng+chi_sim",
            "available_providers": ["disabled", "cloud_stub", "tesseract", "local_json_ocr"],
            "configured": true,
        })
    );
    assert_eq!(
        build_ocr_config_success_response(&configured).body["result"]["configured"],
        true
    );

    let invalid = normalize_ocr_config(Some(&json!({
        "provider": "unknown-provider",
        "lang": "chi sim with spaces",
    })));
    assert_eq!(invalid.provider, "disabled");
    assert_eq!(invalid.lang, "chi_sim+eng");
    assert_eq!(build_unknown_ocr_provider_response().status_code, 400);
    assert_eq!(
        build_unknown_ocr_provider_response().body["message"],
        "Unknown OCR provider"
    );

    let disabled_error = build_ocr_error_response("provider_unconfigured", None);
    assert_eq!(disabled_error.status_code, 501);
    assert_eq!(disabled_error.body["errorCode"], "provider_unconfigured");
    assert_eq!(
        disabled_error.body["message"],
        "Receipt recognition not implemented"
    );
    assert_eq!(
        build_ocr_error_response("provider_unconfigured", Some("ocr provider not configured")).body
            ["message"],
        "ocr provider not configured"
    );
    assert_eq!(ocr_error_http_status("timeout"), 504);
    assert_eq!(ocr_error_http_status("parse_error"), 422);
    assert_eq!(ocr_error_http_status("cancelled"), 499);
    assert_eq!(ocr_error_http_status("rate_limited"), 429);
}

#[test]
fn payment_screenshot_parser_extracts_wechat_and_alipay_contract_fields() {
    let wechat = parse_payment_screenshot_text(
        r#"
        微信支付
        支付成功
        ￥18.80
        交易对方
        瑞幸咖啡
        支付时间 2026年5月4日 08:09:10
        "#,
    );
    assert_eq!(wechat.amount, Some(18.80));
    assert_eq!(wechat.trade_time.as_deref(), Some("2026-05-04 08:09:10"));
    assert_eq!(wechat.description.as_deref(), Some("瑞幸咖啡"));
    assert_eq!(wechat.payment_platform.as_deref(), Some("wechat_pay"));
    assert_eq!(wechat.confidence, 1.0);
    let serialized_wechat = serde_json::to_value(&wechat).expect("payment OCR JSON");
    assert_eq!(serialized_wechat["trade_time"], "2026-05-04 08:09:10");
    assert_eq!(serialized_wechat["payment_platform"], "wechat_pay");
    assert!(serialized_wechat.get("tradeTime").is_none());
    assert!(serialized_wechat.get("paymentPlatform").is_none());

    let alipay =
        parse_payment_screenshot_text("支付宝\n商品: 拿铁咖啡\n付款金额 12.34\n2025-01-02 10:30");
    assert_eq!(alipay.amount, Some(12.34));
    assert_eq!(alipay.trade_time.as_deref(), Some("2025-01-02 10:30"));
    assert_eq!(alipay.description.as_deref(), Some("拿铁咖啡"));
    assert_eq!(alipay.payment_platform.as_deref(), Some("alipay"));
    assert_eq!(alipay.confidence, 1.0);

    let dated_yuan_text = parse_payment_screenshot_text("2026-05-04 便利店 12.34元");
    assert_eq!(dated_yuan_text.amount, Some(12.34));
    assert_eq!(dated_yuan_text.trade_time.as_deref(), Some("2026-05-04"));

    let cny_after_date = parse_payment_screenshot_text("2026-05-04 CNY 12.34 便利店");
    assert_eq!(cny_after_date.amount, Some(12.34));
    assert_eq!(cny_after_date.trade_time.as_deref(), Some("2026-05-04"));

    let rmb_after_date = parse_payment_screenshot_text("2026-05-04 RMB 56.78 便利店");
    assert_eq!(rmb_after_date.amount, Some(56.78));
    assert_eq!(rmb_after_date.trade_time.as_deref(), Some("2026-05-04"));

    let empty = parse_payment_screenshot_text("");
    assert_eq!(empty.amount, None);
    assert_eq!(empty.trade_time, None);
    assert_eq!(empty.description, None);
    assert_eq!(empty.payment_platform, None);
    assert_eq!(empty.confidence, 0.0);
}

#[test]
fn ocr_recognition_success_response_matches_receipt_route_payload() {
    let provider_result = OcrProviderTextResult {
        text: "支付宝\n商品: 拿铁咖啡\n付款金额 12.34\n2025-01-02 10:30".to_string(),
        confidence: 0.42,
        model: "tesseract".to_string(),
        raw_provider_response: json!({
            "engine": "tesseract",
            "lang": "chi_sim+eng",
        }),
        lines: Vec::new(),
    };
    let response =
        build_ocr_recognition_success_response("tesseract", &provider_result, "rust-ocr-7");
    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["success"], true);
    assert_eq!(response.body["result"]["amount"], 12.34);
    assert_eq!(response.body["result"]["trade_time"], "2025-01-02 10:30");
    assert_eq!(response.body["result"]["description"], "拿铁咖啡");
    assert_eq!(response.body["result"]["payment_platform"], "alipay");
    assert_eq!(
        response.body["result"]["provenance"]["provider"],
        "tesseract"
    );
    assert_eq!(response.body["result"]["provenance"]["model"], "tesseract");
    assert_eq!(
        response.body["result"]["provenance"]["request_id"],
        "rust-ocr-7"
    );
    assert_eq!(
        response.body["result"]["raw_provider_response"]["engine"],
        "tesseract"
    );
    assert_eq!(response.body["result"]["confidence"], 1.0);
    assert_eq!(
        response.body["result"]["draft"]["auto_fill"]["amount"]["unit"],
        "yuan"
    );
    assert_eq!(
        response.body["result"]["draft"]["auto_fill"]["type"]["value"],
        "expense"
    );
    assert!(response.body["result"]["draft"]["auto_fill"]["category_id"].is_null());
}

#[test]
fn ocr_recognition_success_response_maps_taxonomy_to_auto_fill_and_candidates() {
    let provider_result = OcrProviderTextResult {
        text: "支付宝\n付款方式 招商银行\n商品: 瑞幸咖啡 拿铁\n付款金额 12.34\n2025-01-02 10:30"
            .to_string(),
        confidence: 0.72,
        model: "local-json-fixture".to_string(),
        raw_provider_response: json!({"engine": "local_json_ocr"}),
        lines: vec![
            OcrProviderTextLine {
                text: "付款方式 招商银行".to_string(),
                confidence: Some(0.96),
                bbox: None,
            },
            OcrProviderTextLine {
                text: "商品: 瑞幸咖啡 拿铁".to_string(),
                confidence: Some(0.95),
                bbox: None,
            },
        ],
    };
    let context = ReceiptDraftContext {
        categories: vec![
            ReceiptDraftCategory {
                id: "10".to_string(),
                type_code: 3,
                label: "餐饮 / 咖啡".to_string(),
            },
            ReceiptDraftCategory {
                id: "11".to_string(),
                type_code: 3,
                label: "购物 / 超市".to_string(),
            },
        ],
        category_rules: vec![ReceiptDraftCategoryRule {
            id: "501".to_string(),
            category_id: "10".to_string(),
            category_type: 3,
            label: "餐饮 / 咖啡".to_string(),
            priority: 1,
            rule_expression: "OR:瑞幸|拿铁".to_string(),
            regex_enabled: false,
        }],
        accounts: vec![ReceiptDraftAccount {
            id: "200".to_string(),
            name: "招商银行".to_string(),
            aliases: vec!["招商银行".to_string(), "招行".to_string()],
        }],
        tags: vec![ReceiptDraftTag {
            id: "7".to_string(),
            name: "咖啡".to_string(),
        }],
    };
    let response = build_ocr_recognition_success_response_with_context(
        "local_json_ocr",
        &provider_result,
        "rust-ocr-8",
        &context,
    );
    assert_eq!(response.status_code, 200);
    assert_eq!(
        response.body["result"]["draft"]["auto_fill"]["category_id"]["value"],
        "10"
    );
    assert_eq!(
        response.body["result"]["draft"]["auto_fill"]["source_account_id"]["value"],
        "200"
    );
    assert_eq!(
        response.body["result"]["draft"]["auto_fill"]["tag_ids"]["value"],
        json!(["7"])
    );
    assert_eq!(
        response.body["result"]["draft"]["auto_fill"]["amount"]["unit"],
        "yuan"
    );
}

#[test]
fn llm_provider_alias_defaults_match_python_factory() {
    assert!(llm_available_providers().contains(&"anthropic".to_string()));
    assert!(llm_available_providers().contains(&"openai-compatible".to_string()));
    assert!(llm_available_providers().contains(&"azure-openai".to_string()));
    assert_eq!(normalize_llm_provider_name("anthropic"), "claude");
    assert_eq!(
        normalize_llm_provider_name("openai-compatible"),
        "openai_compatible"
    );
    assert_eq!(normalize_llm_provider_name("azure_openai"), "azure");
    assert_eq!(normalize_llm_provider_name("azure-openai"), "azure");

    let anthropic = build_llm_provider_config("anthropic", Some(&json!({"api_key": "secret"})))
        .expect("anthropic alias");
    assert_eq!(anthropic.normalized_provider, "claude");
    assert_eq!(anthropic.provider_kind, "claude");
    assert_eq!(anthropic.base_url, "https://api.anthropic.com/v1");
    assert_eq!(anthropic.model, "claude-sonnet-4-20250514");

    for (provider_name, base_url, model) in [
        ("deepseek", "https://api.deepseek.com/v1", "deepseek-chat"),
        ("xai", "https://api.x.ai/v1", "grok-3-mini"),
        (
            "google",
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "gemini-2.0-flash",
        ),
        (
            "openrouter",
            "https://openrouter.ai/api/v1",
            "openai/gpt-4o-mini",
        ),
    ] {
        let config = build_llm_provider_config(
            provider_name,
            Some(&json!({"api_key": "secret", "base_url": "", "model": ""})),
        )
        .expect("openai-compatible defaults");
        assert_eq!(config.provider_kind, "openai_compatible");
        assert_eq!(config.provider_name, provider_name);
        assert_eq!(config.base_url, base_url);
        assert_eq!(config.model, model);
    }

    let default_openai_compatible = build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "",
            "model": "custom-chat",
        })),
    )
    .expect("default custom-compatible endpoint");
    assert_eq!(
        default_openai_compatible.base_url,
        "https://api.openai.com/v1"
    );
    assert_eq!(default_openai_compatible.model, "custom-chat");
    assert_eq!(default_openai_compatible.provider_name, "openai_compatible");
    assert!(build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "https://llm.example.test/v1",
            "model": "custom-chat",
        })),
    )
    .is_err());
    assert!(build_llm_provider_config(
        "openai",
        Some(&json!({
            "api_key": "secret",
            "base_url": "http://127.0.0.1:11434/v1",
            "model": "gpt-test",
        })),
    )
    .is_err());
    let _allowlist_restore = EnvVarRestore::capture("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");
    env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", "llm.example.test");
    assert!(build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "https://llm.example.test/v1",
            "model": "custom-chat",
        })),
    )
    .is_err());
    env::set_var(
        "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
        "http://llm.example.test",
    );
    assert!(build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "http://llm.example.test/v1",
            "model": "custom-chat",
        })),
    )
    .is_err());
    env::set_var(
        "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
        "https://llm.example.test",
    );
    assert!(build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "https://llm.example.test/v1",
            "model": "custom-chat",
        })),
    )
    .is_ok());
    env::set_var(
        "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
        "http://127.0.0.1:11434",
    );
    assert!(build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "http://127.0.0.1:11434/v1",
            "model": "custom-chat",
        })),
    )
    .is_ok());
    assert!(build_llm_provider_config("azure", None).is_err());
    assert!(build_llm_provider_config("azure", Some(&json!({"base_url": ""}))).is_err());
    let azure = build_llm_provider_config(
        "azure_openai",
        Some(&json!({
            "base_url": "https://example-resource.openai.azure.com/openai/deployments/chat",
            "model": "gpt-4o-mini",
        })),
    )
    .expect("azure requires explicit endpoint");
    assert_eq!(azure.normalized_provider, "azure");
    assert_eq!(
        azure.base_url,
        "https://example-resource.openai.azure.com/openai/deployments/chat"
    );
    for unsafe_base_url in [
        "http://127.0.0.1",
        "http://169.254.169.254",
        "https://api.openai.com/v1",
        "https://llm.example.test/v1",
        "https://example-resource.openai.azure.com.evil.test/openai/deployments/chat",
        "https://evil.test\\example-resource.openai.azure.com/openai/deployments/chat",
        "https://example-resource.openai.azure.com\\@evil.test/openai/deployments/chat",
        "https://example-resource.openai.azure.com\u{0008}.evil.test/openai/deployments/chat",
        "https://evil\u{007f}.openai.azure.com/openai/deployments/chat",
    ] {
        assert!(
            build_llm_provider_config(
                "azure",
                Some(&json!({"base_url": unsafe_base_url, "model": "gpt-4o-mini"})),
            )
            .is_err(),
            "unsafe Azure endpoint should fail closed: {unsafe_base_url}"
        );
    }
    assert!(build_llm_provider_config("not-a-provider", None).is_err());
}

#[test]
fn llm_advanced_settings_runtime_isolation_and_secret_redaction_are_pinned() {
    let normalized = normalize_llm_advanced_settings(Some(&json!({
        "reasoning_depth": "HIGH",
        "temperature": "0.55",
        "max_tokens": "1234",
        "system_prompt": " 系统提示词 ",
        "classification_prompt_template": "分类 {transactions_json}",
        "rule_prompt_template": "规则 {category_name}",
        "invalid": "ignored",
    })));
    assert_eq!(normalized["reasoning_depth"], "high");
    assert_eq!(normalized["temperature"], 0.55);
    assert_eq!(normalized["max_tokens"], 1234);
    assert_eq!(normalized["system_prompt"], "系统提示词");
    assert!(!normalized.contains_key("invalid"));
    assert!(normalize_llm_advanced_settings(Some(&json!("[]"))).is_empty());
    assert!(normalize_llm_advanced_settings(Some(&json!("\"plain\""))).is_empty());
    assert!(normalize_llm_advanced_settings(Some(&json!("1"))).is_empty());

    let saved = json!({
        "id": 7,
        "name": "primary",
        "provider": "openai",
        "model": "gpt-test",
        "api_key": "sk-secret-should-not-leak",
        "base_url": "https://example.test/v1",
        "advanced_settings": {
            "reasoning_depth": "low",
            "temperature": 0.7,
        },
    });
    let safe = safe_llm_config_payload(&saved);
    let safe_text = serde_json::to_string(&safe).expect("safe JSON");
    assert!(!safe_text.contains("sk-secret-should-not-leak"));
    assert_eq!(safe["api_key"], "********");
    assert_eq!(safe["has_api_key"], true);
    assert_eq!(safe["advanced_settings"]["reasoning_depth"], "low");

    let runtime_shaped_safe = safe_llm_config_payload(&json!({
        "provider": "openai",
            "provider_config": {
                "api_key": "sk-secret-should-not-leak",
                "apiKey": "sk-alias-should-not-leak",
                "credential": "credential-should-not-leak",
                "credentials": {
                    "tenant": "credentials-object-should-not-leak"
                },
                "proxy_authorization": "proxy-auth-should-not-leak",
                "subscription_key": "subscription-key-should-not-leak",
                "access_token": "token-should-not-leak",
                "api_secret": "api-secret-should-not-leak",
                "bearer_token": "bearer-token-should-not-leak",
                "id_token": "id-token-should-not-leak",
                "private_key": "private-key-should-not-leak",
                "azure_api_key": "azure-api-key-should-not-leak",
                "client_secret_key": "client-secret-key-should-not-leak",
                "proxy_authorization_header": "proxy-auth-header-should-not-leak",
                "ocp_apim_subscription_key": "ocp-apim-underscore-should-not-leak",
                "headers": {
                    "Authorization": "Bearer auth-header-should-not-leak",
                    "x-api-key": "x-api-key-should-not-leak",
                    "Ocp-Apim-Subscription-Key": "ocp-apim-should-not-leak",
                    "safe_header": "kept-visible"
                },
                "max_tokens": 1024,
                "tokens_used": 17,
                "model": "gpt-test",
            },
    }));
    let runtime_shaped_safe_text =
        serde_json::to_string(&runtime_shaped_safe).expect("runtime safe JSON");
    assert!(!runtime_shaped_safe_text.contains("sk-secret-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("sk-alias-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("credential-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("credentials-object-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("proxy-auth-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("subscription-key-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("token-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("api-secret-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("bearer-token-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("id-token-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("private-key-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("azure-api-key-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("client-secret-key-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("proxy-auth-header-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("ocp-apim-underscore-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("auth-header-should-not-leak"));
    assert!(!runtime_shaped_safe_text.contains("x-api-key-should-not-leak"));
    assert_eq!(
        runtime_shaped_safe["provider_config"]["api_key"],
        "********"
    );
    assert_eq!(runtime_shaped_safe["provider_config"]["apiKey"], "********");
    assert_eq!(
        runtime_shaped_safe["provider_config"]["access_token"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["credential"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["credentials"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["proxy_authorization"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["subscription_key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["api_secret"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["bearer_token"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["id_token"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["private_key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["azure_api_key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["client_secret_key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["proxy_authorization_header"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["ocp_apim_subscription_key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["headers"]["Authorization"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["headers"]["x-api-key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["headers"]["Ocp-Apim-Subscription-Key"],
        "********"
    );
    assert_eq!(
        runtime_shaped_safe["provider_config"]["headers"]["safe_header"],
        "kept-visible"
    );
    assert_eq!(runtime_shaped_safe["provider_config"]["max_tokens"], 1024);
    assert_eq!(runtime_shaped_safe["provider_config"]["tokens_used"], 17);
    assert_eq!(runtime_shaped_safe["has_api_key"], true);

    let runtime = build_runtime_llm_config_from_saved_config(&saved);
    assert_eq!(runtime["enabled"], true);
    assert_eq!(
        runtime["provider_config"]["api_key"],
        "sk-secret-should-not-leak"
    );
    assert_eq!(runtime["advanced_settings"]["temperature"], 0.7);

    let copied = copy_runtime_llm_config(&json!({
        "enabled": false,
        "provider": "openai",
        "advanced_settings": {},
        "provider_config": {
            "model": "disabled-global",
            "advanced_settings": {"system_prompt": "provider-local"},
        },
    }));
    assert_eq!(copied["enabled"], false);
    assert_eq!(copied["provider_config"]["model"], "disabled-global");
    assert_eq!(
        copied["advanced_settings"]["system_prompt"],
        "provider-local"
    );

    let get_response = build_llm_config_get_response(&runtime);
    assert_eq!(get_response.status_code, 200);
    assert_eq!(get_response.body["data"]["provider"], "openai");
    assert_eq!(get_response.body["data"]["model"], "gpt-test");
    assert!(get_response.body["data"]["available_providers"]
        .as_array()
        .expect("providers")
        .contains(&json!("openrouter")));
}

#[test]
fn llm_preview_and_candidate_review_route_envelopes_preserve_live_provider_boundary() {
    let disabled =
        build_llm_contract_error_response("LLM service is not enabled", "LLM_DISABLED", 400);
    assert_eq!(disabled.status_code, 400);
    assert_eq!(disabled.body["code"], "LLM_DISABLED");
    assert_eq!(disabled.body["error_code"], "LLM_DISABLED");

    let suggestions = vec![json!({"matching": {"llm": {"review_status": "pending"}}})];
    let preview = build_llm_preview_recommend_response("session-1", suggestions);
    assert_eq!(preview["data"]["session_id"], "session-1");
    assert_eq!(preview["data"]["count"], 1);

    let candidate_list = build_llm_candidate_list_response(vec![json!({"id": 9})], 1);
    assert_eq!(candidate_list["success"], true);
    assert_eq!(candidate_list["total"], 1);
    assert_eq!(
        build_llm_candidate_reject_response(true),
        json!({"success": true, "data": {"rejected": true}})
    );

    assert!(llm_review_endpoint_requires_live_provider(
        "preview-recommend"
    ));
    assert!(llm_review_endpoint_requires_live_provider(
        "analyze-transactions"
    ));
    assert!(llm_review_endpoint_requires_live_provider("rule-synthesis"));
    assert!(!llm_review_endpoint_requires_live_provider(
        "preview-recommend/accept"
    ));
    assert!(!llm_review_endpoint_requires_live_provider(
        "preview-recommend/reject"
    ));
    assert!(!llm_review_endpoint_requires_live_provider(
        "candidates/accept"
    ));
    assert!(!llm_review_endpoint_requires_live_provider(
        "candidates/reject"
    ));
    assert!(!llm_review_endpoint_requires_live_provider(
        "candidates/123/accept"
    ));
    assert!(!llm_review_endpoint_requires_live_provider(
        "candidates/123/reject"
    ));

    let import_session = build_llm_analysis_response(
        vec![json!({"id": 1})],
        &json!({"session_id": "sess-a", "preview_ids": [3]}),
    );
    assert_eq!(import_session["data"]["mode"], "import_session");
    assert_eq!(import_session["data"]["session_id"], "sess-a");
    assert_eq!(import_session["total"], 1);

    let persisted_selection =
        build_llm_analysis_response(Vec::<Value>::new(), &json!({"bill_ids": [1, 2]}));
    assert_eq!(persisted_selection["data"]["mode"], "persisted_selection");

    let uncategorized = build_llm_analysis_response(Vec::<Value>::new(), &json!({}));
    assert_eq!(uncategorized["data"]["mode"], "persisted_uncategorized");
}

#[test]
fn llm_provider_generation_prompt_and_json_array_contracts_are_stable() {
    let transactions = vec![
        json!({
            "id": 1,
            "date": "2026-05-01",
            "amount": 18.5,
            "counterparty": "cafe",
            "description": {"raw": "latte"},
            "payment_method": true,
            "type": "支出",
        }),
        json!({"id": 2, "description": null}),
    ];
    let classification_prompt = build_llm_classification_prompt(&transactions);
    assert!(classification_prompt.contains("id=1"));
    assert!(classification_prompt.contains("payment_method=\"true\""));
    assert!(classification_prompt.contains("description=\"{\"raw\":\"latte\"}\""));

    let rule_prompt = build_llm_rule_induction_prompt("餐饮/咖啡", &transactions);
    assert!(rule_prompt.contains("已被归类为「餐饮/咖啡」"));
    assert!(rule_prompt.contains("OR={关键词1,关键词2}"));

    let preview_prompt = build_llm_import_preview_recommendation_prompt(
        &transactions,
        &["餐饮/咖啡".to_string()],
        &["现金".to_string()],
        &[
            json!({
                "decision": "accept",
                "suggested_main_category": "餐饮",
                "suggested_sub_category": "咖啡",
                "description_hint": "latte",
            }),
            json!({"decision": "", "suggested_main_category": "ignored"}),
        ],
    );
    assert!(preview_prompt.contains("已有分类体系"));
    assert!(preview_prompt.contains("已有账户"));
    assert!(preview_prompt.contains("历史记忆"));

    let preview_prompt_without_context =
        build_llm_import_preview_recommendation_prompt(&transactions, &[], &[], &[]);
    assert!(!preview_prompt_without_context.contains("已有分类体系"));
    assert!(!preview_prompt_without_context.contains("历史记忆（你过去"));

    let prompt = build_llm_rule_expression_synthesis_prompt(
        &json!({
            "knowledge_summary_version": "a6-rule-synthesis-v1",
            "existing_categories": [{"path": "餐饮/咖啡"}],
        }),
        3,
    );
    assert!(prompt.contains("KnowledgeSummaryPack"));
    assert!(prompt.contains("最多输出 3 条候选"));
    assert!(prompt.contains("只返回 JSON"));

    let rendered = render_llm_prompt_template(
        "base={default_prompt}\njson={transactions_json}\ntext={transactions_text}\ncat={category_name}",
        "默认提示",
        &transactions,
        "餐饮/咖啡",
    );
    assert!(rendered.contains("base=默认提示"));
    assert!(rendered.contains("cat=餐饮/咖啡"));
    assert!(rendered.contains("\"id\":1"));
    assert_eq!(
        render_llm_prompt_template("", "默认提示", &transactions, ""),
        "默认提示"
    );

    let parsed =
        parse_llm_json_array_response("```json\n[{\"rule_expression\":\"OR={咖啡}\"}]\n```")
            .expect("json fence parses");
    assert_eq!(parsed[0]["rule_expression"], "OR={咖啡}");

    let prefixed = parse_llm_json_array_response(
        "结果如下：[{\"bill_id\":1,\"suggested_main_category\":\"餐饮\"}]",
    )
    .expect("prefixed json parses");
    assert_eq!(prefixed[0]["bill_id"], 1);

    let unfenced =
        parse_llm_json_array_response("```\n[{\"id\":7}]").expect("unterminated fence body parses");
    assert_eq!(unfenced[0]["id"], 7);
    assert!(parse_llm_json_array_response("```json").is_err());
    assert!(parse_llm_json_array_response("not json").is_err());
}
