#[cfg(test)]
mod ai_response_contract_tests {
    use super::*;
    use bill_analyser_core::OcrProviderTextLine;

    #[test]
    fn ocr_provider_failure_response_keeps_legacy_http_contract() {
        let cases = [
            (
                OcrProviderFailure::Unavailable {
                    message: "provider unavailable".to_string(),
                },
                501,
                "provider_unconfigured",
                "provider unavailable",
            ),
            (
                OcrProviderFailure::TimedOut {
                    message: "provider timeout".to_string(),
                },
                504,
                "timeout",
                "provider timeout",
            ),
            (
                OcrProviderFailure::InvalidOutput {
                    message: "invalid output".to_string(),
                },
                422,
                "parse_error",
                "invalid output",
            ),
            (
                OcrProviderFailure::ReauthenticationRequired {
                    message: "sign in again".to_string(),
                },
                401,
                "provider_relogin_required",
                "sign in again",
            ),
        ];

        for (failure, expected_status, expected_code, expected_message) in cases {
            let response = ocr_provider_failure_response(failure);
            assert_eq!(response.status_code, expected_status);
            assert_eq!(response.body["errorCode"], expected_code);
            assert_eq!(response.body["message"], expected_message);
        }
    }

    #[test]
    fn ai_ocr_transport_contract_keeps_status_envelopes_and_secret_redaction() {
        let config = OcrConfigContract {
            provider: "llm_vision".to_string(),
            lang: "eng".to_string(),
            model: "gpt-4o-mini".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            parameters: json!({
                "temperature": 0,
                "api_key": "parameter-secret",
                "headers": {
                    "Authorization": "Bearer nested-secret",
                    "safe_header": "visible"
                }
            }),
            credential_config: json!({
                "access_token": "credential-secret",
                "refresh_headers": {"x-api-key": "refresh-header-secret"}
            }),
        };
        let response = build_ocr_config_success_response(&config);
        let result = &response.body["result"];
        let serialized = serde_json::to_string(&response.body).expect("safe OCR response JSON");

        assert_eq!(response.status_code, 200);
        assert_eq!(result["configured"], true);
        assert_eq!(result["parameters"]["headers"]["safe_header"], "visible");
        assert_eq!(result["parameters"]["api_key"], "********");
        assert_eq!(result["credential_config"]["access_token"], "********");
        for secret in [
            "parameter-secret",
            "nested-secret",
            "credential-secret",
            "refresh-header-secret",
        ] {
            assert!(!serialized.contains(secret), "response leaked {secret}");
        }
        assert_eq!(
            result["available_providers"],
            json!(["disabled", "cloud_stub", "tesseract", "local_json_ocr", "llm_vision"])
        );

        let unknown = build_unknown_ocr_provider_response();
        assert_eq!(unknown.status_code, 400);
        assert_eq!(unknown.body["message"], "Unknown OCR provider");

        for (code, expected_status) in [
            ("provider_unconfigured", 501),
            ("timeout", 504),
            ("parse_error", 422),
            ("cancelled", 499),
            ("rate_limited", 429),
            ("provider_relogin_required", 401),
            ("unknown", 500),
        ] {
            let error = build_ocr_error_response(code, None);
            assert_eq!(error.status_code, expected_status, "status for {code}");
            assert_eq!(error.body["errorCode"], code);
        }
        assert_eq!(
            build_ocr_error_response("provider_unconfigured", None).body["message"],
            "Receipt recognition not implemented"
        );
        assert_eq!(
            build_ocr_error_response("timeout", Some(" provider timeout ")).body["message"],
            "provider timeout"
        );
    }

    #[test]
    fn ocr_recognition_transport_preserves_yuan_draft_and_provenance() {
        let provider_result = OcrProviderTextResult {
            text: "支付宝\n商品: 拿铁咖啡\n付款金额 12.34\n2025-01-02 10:30".to_string(),
            confidence: 0.42,
            model: "tesseract".to_string(),
            raw_provider_response: json!({"engine": "tesseract", "lang": "chi_sim+eng"}),
            lines: Vec::new(),
        };
        let response = build_ocr_recognition_success_response_with_context(
            "tesseract",
            &provider_result,
            "rust-ocr-7",
            &ReceiptDraftContext::default(),
        );
        let result = &response.body["result"];

        assert_eq!(response.status_code, 200);
        assert_eq!(result["amount"], 12.34);
        assert_eq!(result["trade_time"], "2025-01-02 10:30");
        assert_eq!(result["description"], "拿铁咖啡");
        assert_eq!(result["payment_platform"], "alipay");
        assert_eq!(result["provenance"]["provider"], "tesseract");
        assert_eq!(result["provenance"]["model"], "tesseract");
        assert_eq!(result["provenance"]["request_id"], "rust-ocr-7");
        assert_eq!(result["raw_provider_response"]["engine"], "tesseract");
        assert_eq!(result["confidence"], 1.0);
        assert_eq!(result["draft"]["auto_fill"]["amount"]["unit"], "yuan");
        assert_eq!(result["draft"]["auto_fill"]["type"]["value"], "expense");
    }

    #[test]
    fn ocr_recognition_transport_preserves_taxonomy_account_and_tag_projection() {
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
            categories: vec![ReceiptDraftCategory {
                id: "10".to_string(),
                type_code: 3,
                label: "餐饮 / 咖啡".to_string(),
            }],
            category_rules: vec![ReceiptDraftCategoryRule {
                id: "501".to_string(),
                category_id: "10".to_string(),
                category_type: 3,
                label: "餐饮 / 咖啡".to_string(),
                priority: 1,
                rule_expression: "OR:瑞幸|拿铁".to_string(),
                regex_enabled: false,
            }],
            account_rules: vec![AccountRuleCandidate {
                rule_id: 601,
                account_id: 200,
                rule_expression: "OR={招商银行,招行}".to_string(),
                regex_enabled: false,
                enabled: true,
                priority: 1,
            }],
            accounts: vec![ReceiptDraftAccount {
                id: "200".to_string(),
                name: "招商银行".to_string(),
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
    fn llm_transport_projection_preserves_config_review_and_analysis_shapes() {
        let runtime = json!({
            "enabled": true,
            "provider": "openai",
            "provider_config": {"model": " gpt-test "},
            "credential_config": {
                "access_token": "access-secret",
                "refresh_token": "refresh-secret"
            },
            "advanced_settings": {"temperature": 0.7}
        });
        let config = build_llm_config_get_response(&runtime);
        let serialized = serde_json::to_string(&config.body).expect("safe LLM response JSON");
        assert_eq!(config.status_code, 200);
        assert_eq!(config.body["data"]["provider"], "openai");
        assert_eq!(config.body["data"]["model"], "gpt-test");
        assert_eq!(config.body["data"]["credential_config"]["access_token"], "********");
        assert!(!serialized.contains("access-secret"));
        assert!(!serialized.contains("refresh-secret"));
        assert!(config.body["data"]["available_providers"]
            .as_array()
            .expect("providers")
            .contains(&json!("openrouter")));

        let disabled =
            build_llm_contract_error_response("LLM service is not enabled", "LLM_DISABLED", 400);
        assert_eq!(disabled.status_code, 400);
        assert_eq!(disabled.body["code"], "LLM_DISABLED");
        assert_eq!(disabled.body["error_code"], "LLM_DISABLED");

        let preview = build_llm_preview_recommend_response(
            "session-1",
            vec![json!({"matching": {"llm": {"review_status": "pending"}}})],
        );
        assert_eq!(preview["data"]["session_id"], "session-1");
        assert_eq!(preview["data"]["count"], 1);
        assert_eq!(
            build_llm_candidate_list_response(vec![json!({"id": 9})], 1)["total"],
            1
        );
        assert_eq!(
            build_llm_candidate_reject_response(true),
            json!({"success": true, "data": {"rejected": true}})
        );

        let import_session = build_llm_analysis_response(
            vec![json!({"id": 1})],
            &json!({"session_id": "sess-a", "preview_ids": [3]}),
        );
        assert_eq!(import_session["data"]["mode"], "import_session");
        assert_eq!(import_session["data"]["session_id"], "sess-a");
        assert_eq!(import_session["total"], 1);
        assert_eq!(
            build_llm_analysis_response(Vec::new(), &json!({"bill_ids": [1, 2]}))["data"]
                ["mode"],
            "persisted_selection"
        );
        assert_eq!(
            build_llm_analysis_response(Vec::new(), &json!({}))["data"]["mode"],
            "persisted_uncategorized"
        );
    }
}
