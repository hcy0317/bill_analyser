use bill_analyser_core::ai_ocr_llm::{
    build_llm_account_rule_induction_prompt, build_llm_category_rule_induction_prompt,
    build_llm_classification_prompt, build_llm_import_preview_recommendation_prompt,
    build_llm_provider_config, build_llm_rule_expression_synthesis_prompt,
    build_llm_rule_induction_prompt, build_receipt_transaction_draft,
    build_runtime_llm_config_from_saved_config, copy_runtime_llm_config, llm_available_providers,
    normalize_llm_advanced_settings, normalize_llm_provider_name, normalize_ocr_config,
    ocr_available_providers_with_disabled, parse_llm_json_array_response,
    parse_payment_screenshot_text, render_llm_prompt_template, safe_llm_config_payload,
    validate_llm_vision_base_url, OcrProviderTextResult, ReceiptDraftAccount, ReceiptDraftContext,
};
use serde_json::json;
use std::env;
use std::sync::{Mutex, MutexGuard};

static LLM_ALLOWLIST_ENV_LOCK: Mutex<()> = Mutex::new(());

fn lock_llm_allowlist_env() -> MutexGuard<'static, ()> {
    LLM_ALLOWLIST_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

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
fn ocr_config_normalization_and_provider_catalog_match_receipt_routes() {
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
        vec![
            "disabled",
            "cloud_stub",
            "tesseract",
            "local_json_ocr",
            "llm_vision"
        ]
    );
    let invalid = normalize_ocr_config(Some(&json!({
        "provider": "unknown-provider",
        "lang": "chi sim with spaces",
    })));
    assert_eq!(invalid.provider, "disabled");
    assert_eq!(invalid.lang, "chi_sim+eng");
}

#[test]
fn llm_vision_base_url_reuses_llm_ssrf_allowlist_contract() {
    let _allowlist_lock = lock_llm_allowlist_env();
    let _allowlist_restore = EnvVarRestore::capture("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");
    env::remove_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");

    assert!(validate_llm_vision_base_url("https://api.openai.com/v1").is_ok());
    assert!(validate_llm_vision_base_url("https://api.deepseek.com/v1").is_ok());

    for unsafe_base_url in [
        "",
        "http://127.0.0.1:11434/v1",
        "http://169.254.169.254/latest/meta-data",
        "http://metadata.google.internal/computeMetadata/v1",
        "https://llm.example.test/v1",
        "https://evil.test\\api.openai.com/v1",
        "https://api.openai.com\u{0008}.evil.test/v1",
        "ftp://api.openai.com/v1",
        "https://user:pass@api.openai.com/v1",
    ] {
        assert!(
            validate_llm_vision_base_url(unsafe_base_url).is_err(),
            "unsafe llm_vision base URL should fail closed: {unsafe_base_url:?}"
        );
    }

    env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", "llm.example.test");
    assert!(validate_llm_vision_base_url("https://llm.example.test/v1").is_err());
    env::set_var(
        "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
        "https://llm.example.test",
    );
    assert!(validate_llm_vision_base_url("https://llm.example.test/v1").is_ok());
    env::set_var(
        "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
        "http://127.0.0.1:11434",
    );
    assert!(validate_llm_vision_base_url("http://127.0.0.1:11434/v1").is_ok());

    env::set_var(
        "BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST",
        "https://metadata.google.internal;https://169.254.169.254;https://0.0.0.0;https://224.0.0.1",
    );
    for never_allowed in [
        "https://metadata.google.internal/v1",
        "https://169.254.169.254/v1",
        "https://0.0.0.0/v1",
        "https://224.0.0.1/v1",
    ] {
        assert!(
            validate_llm_vision_base_url(never_allowed).is_err(),
            "allowlist must not override non-routable or metadata host: {never_allowed}"
        );
    }
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
fn full_image_payment_screenshots_partition_amount_time_payment_method_and_account() {
    let wechat_text = r#"15:11 M
85.7
KB/s
四川农商银行
泷口卤肉烟酒副食
-6.00
当前状态
支付成功
支付时间
2026年8月27日10:54:49
商品
泷口卤肉烟酒副食
商户全称
梓潼县泷口副食店
支付方式
零钱
交易单号
4500000341202608271740217914
本服务由财付通提供"#;
    let wechat = parse_payment_screenshot_text(wechat_text);
    assert_eq!(wechat.amount, Some(6.0));
    assert_eq!(wechat.trade_time.as_deref(), Some("2026-08-27 10:54:49"));
    assert_eq!(
        wechat.description.as_deref(),
        Some("泷口卤肉烟酒副食 - 梓潼县泷口副食店")
    );
    assert_eq!(wechat.payment_method.as_deref(), Some("零钱"));
    assert_eq!(wechat.payment_platform.as_deref(), Some("wechat_pay"));

    let alipay_text = r#"15:13 M
35.5
KB/s
账单详情
蚂蚁财富-蚂蚁（杭州）基金销售有限公司
-35.00
交易成功
创建时间
2026-08-2609:06:15
付款方式
中国农业银行储蓄卡(4071)>
服务详情
蚂蚁财富-广发中证养老产业A-定投...查看详情>
订单号
20260826001080012204310011999172
账单分类
投资理财>"#;
    let alipay = parse_payment_screenshot_text(alipay_text);
    assert_eq!(alipay.amount, Some(35.0));
    assert_eq!(alipay.trade_time.as_deref(), Some("2026-08-26 09:06:15"));
    assert_eq!(
        alipay.description.as_deref(),
        Some("蚂蚁财富-蚂蚁(杭州)基金销售有限公司 - 蚂蚁财富-广发中证养老产业A-定投")
    );
    assert_eq!(
        alipay.payment_method.as_deref(),
        Some("中国农业银行储蓄卡(4071)")
    );
    assert_eq!(alipay.payment_platform.as_deref(), Some("alipay"));

    for label in ["商品说明", "交易详情"] {
        let parsed = parse_payment_screenshot_text(&format!(
            "支付宝\n-12.00\n{label}\n测试商品或服务\n创建时间\n2026-08-26 09:06:15"
        ));
        assert_eq!(
            parsed.description.as_deref(),
            Some("测试商品或服务"),
            "支付宝 {label} 布局"
        );
    }

    let context = ReceiptDraftContext {
        accounts: vec![
            ReceiptDraftAccount {
                id: "17000000003".to_string(),
                name: "农业银行".to_string(),
            },
            ReceiptDraftAccount {
                id: "17000000038".to_string(),
                name: "农业银行信用卡".to_string(),
            },
            ReceiptDraftAccount {
                id: "17000000045".to_string(),
                name: "微信".to_string(),
            },
            ReceiptDraftAccount {
                id: "17000000312".to_string(),
                name: "支付宝".to_string(),
            },
        ],
        ..ReceiptDraftContext::default()
    };
    let wechat_provider = OcrProviderTextResult {
        text: wechat_text.to_string(),
        confidence: 0.98,
        model: "PP-OCRv6".to_string(),
        raw_provider_response: json!({}),
        lines: Vec::new(),
    };
    let wechat_draft = build_receipt_transaction_draft(&wechat, &wechat_provider, &context);
    assert_eq!(
        wechat_draft.auto_fill["source_account_id"].value,
        "17000000045"
    );

    let alipay_provider = OcrProviderTextResult {
        text: alipay_text.to_string(),
        confidence: 0.98,
        model: "PP-OCRv6".to_string(),
        raw_provider_response: json!({}),
        lines: Vec::new(),
    };
    let alipay_draft = build_receipt_transaction_draft(&alipay, &alipay_provider, &context);
    assert_eq!(
        alipay_draft.auto_fill["source_account_id"].value,
        "17000000003"
    );
    assert_eq!(alipay_draft.auto_fill["type"].value, "investment");
}

#[test]
fn llm_provider_alias_defaults_match_current_factory() {
    let _allowlist_lock = lock_llm_allowlist_env();
    let _allowlist_restore = EnvVarRestore::capture("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");
    env::remove_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST");

    assert!(llm_available_providers().contains(&"anthropic".to_string()));
    assert!(llm_available_providers().contains(&"openai-compatible".to_string()));
    assert!(llm_available_providers().contains(&"azure-openai".to_string()));
    assert!(llm_available_providers().contains(&"qwen".to_string()));
    assert!(llm_available_providers().contains(&"siliconflow".to_string()));
    assert!(llm_available_providers().contains(&"zhipu".to_string()));
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
        (
            "qwen",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen-plus",
        ),
        (
            "siliconflow",
            "https://api.siliconflow.cn/v1",
            "deepseek-ai/DeepSeek-V3",
        ),
        (
            "zhipu",
            "https://open.bigmodel.cn/api/paas/v4",
            "glm-4-flash",
        ),
    ] {
        let config = build_llm_provider_config(
            provider_name,
            Some(&json!({"api_key": "secret", "base_url": "", "model": ""})),
        )
        .expect("provider defaults");
        assert_eq!(config.provider_kind, "openai_compatible");
        assert_eq!(config.provider_name, provider_name);
        assert_eq!(config.base_url, base_url);
        assert_eq!(config.model, model);
        let explicit_default_origin = build_llm_provider_config(
            provider_name,
            Some(&json!({"base_url": format!("{base_url}/"), "model": model})),
        )
        .expect("explicit provider default origin is SSRF-safe");
        assert_eq!(explicit_default_origin.provider_name, provider_name);
    }

    let default_openai_compatible = build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "",
            "model": "custom-chat",
        })),
    )
    .expect("default custom endpoint");
    assert_eq!(
        default_openai_compatible.base_url,
        "https://api.openai.com/v1"
    );
    assert_eq!(default_openai_compatible.model, "custom-chat");
    assert_eq!(default_openai_compatible.provider_name, "openai_compatible");
    let arbitrary_public_https = build_llm_provider_config(
        "openai_compatible",
        Some(&json!({
            "api_key": "secret",
            "base_url": "https://llm.example.test/v1",
            "model": "custom-chat",
        })),
    )
    .expect("custom OpenAI-compatible public HTTPS endpoint");
    assert_eq!(
        arbitrary_public_https.base_url,
        "https://llm.example.test/v1"
    );
    for unsafe_custom_url in [
        "http://llm.example.test/v1",
        "https://localhost/v1",
        "https://metadata.google.internal/v1",
        "https://169.254.169.254/v1",
        "https://user:password@llm.example.test/v1",
    ] {
        assert!(
            build_llm_provider_config(
                "openai_compatible",
                Some(&json!({
                    "api_key": "secret",
                    "base_url": unsafe_custom_url,
                    "model": "custom-chat",
                })),
            )
            .is_err(),
            "unsafe custom URL should fail closed: {unsafe_custom_url}"
        );
    }
    assert!(build_llm_provider_config(
        "openai",
        Some(&json!({
            "api_key": "secret",
            "base_url": "http://127.0.0.1:11434/v1",
            "model": "gpt-test",
        })),
    )
    .is_err());
    env::set_var("BILL_ANALYSER_LLM_BASE_URL_ALLOWLIST", "llm.example.test");
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
        "api_protocol": "RESPONSES",
        "reasoning_depth": "HIGH",
        "temperature": "0.55",
        "max_tokens": "1234",
        "system_prompt": " 系统提示词 ",
        "classification_prompt_template": "分类 {transactions_json}",
        "rule_prompt_template": "规则 {category_name}",
        "invalid": "ignored",
    })));
    assert_eq!(normalized["api_protocol"], "responses");
    assert_eq!(normalized["reasoning_depth"], "high");
    assert_eq!(normalized["temperature"], 0.55);
    assert_eq!(normalized["max_tokens"], 1234);
    assert_eq!(normalized["system_prompt"], "系统提示词");
    assert!(!normalized.contains_key("invalid"));
    assert_eq!(
        normalize_llm_advanced_settings(Some(&json!({"api_protocol": "chat-completions"})))
            ["api_protocol"],
        "chat_completions"
    );
    assert!(
        !normalize_llm_advanced_settings(Some(&json!({"api_protocol": "legacy"})))
            .contains_key("api_protocol")
    );
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
        &[json!({
            "id": 1,
            "date": "2026-05-01",
            "amount_cents": 1850,
            "type": "支出",
            "counterparty": "cafe",
            "description": "latte",
            "payment_method": "现金",
        })],
        &[json!({"id": 42, "type": "支出", "path": "餐饮/咖啡"})],
        &[json!({"id": 7, "name": "现金"})],
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
    assert!(preview_prompt.contains("\"amount_cents\":1850"));
    assert!(preview_prompt.contains("\"category_id\""));
    assert!(preview_prompt.contains("\"source_account_id\""));
    assert!(preview_prompt.contains("\"destination_account_id\""));
    assert!(!preview_prompt.contains("suggested_main_category"));
    assert!(!preview_prompt.contains("suggested_source_account"));

    let preview_prompt_without_context =
        build_llm_import_preview_recommendation_prompt(&transactions, &[], &[], &[]);
    assert!(!preview_prompt_without_context.contains("已有分类体系"));
    assert!(!preview_prompt_without_context.contains("历史记忆（你过去"));

    let category_rule_prompt =
        build_llm_category_rule_induction_prompt(42, "餐饮/咖啡", &transactions);
    assert!(category_rule_prompt.contains("target_category_id"));
    assert!(category_rule_prompt.contains("\"target_category_id\":42"));
    assert!(category_rule_prompt.contains("\"target_category_path\":\"餐饮/咖啡\""));
    assert!(category_rule_prompt.contains("rule_expression"));
    assert!(!category_rule_prompt.contains("target_account_id"));

    let account_rule_prompt =
        build_llm_account_rule_induction_prompt(7, "现金", "source", &transactions);
    assert!(account_rule_prompt.contains("target_account_id"));
    assert!(account_rule_prompt.contains("\"target_account_id\":7"));
    assert!(account_rule_prompt.contains("\"account_role\":\"source\""));
    assert!(account_rule_prompt.contains("rule_expression"));
    assert!(!account_rule_prompt.contains("suggested_main_category"));

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
