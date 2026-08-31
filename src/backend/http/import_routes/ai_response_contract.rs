// 中文导读：HTTP 运行态层，集中投影 AI/OCR/LLM 路由的状态码与 JSON envelope。
// 维护重点：core 只提供配置归一化、识别解析和草稿构建；这里不执行 provider 调用或数据库写入。
// 不变式：所有 AI/OCR/LLM 响应复用 ImportV2RouteResponse 与唯一 route_response Axum 适配器。

const OCR_ERROR_PROVIDER_UNCONFIGURED: &str = "provider_unconfigured";
const OCR_ERROR_TIMEOUT: &str = "timeout";
const OCR_ERROR_PARSE: &str = "parse_error";
const OCR_ERROR_CANCELLED: &str = "cancelled";
const OCR_ERROR_RATE_LIMITED: &str = "rate_limited";
const OCR_ERROR_PROVIDER_RELOGIN_REQUIRED: &str = "provider_relogin_required";

/// 构建 LLM 合同错误响应，同时保留 code 与 error_code 以兼容前端旧字段。
fn build_llm_contract_error_response(
    message: &str,
    code: &str,
    status_code: u16,
) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code,
        body: json!({
            "success": false,
            "error": message,
            "code": code,
            "error_code": code,
        }),
    }
}

/// 构建导入预览 LLM 推荐响应，按 session_id 回传建议数量和建议列表。
fn build_llm_preview_recommend_response(session_id: &str, suggestions: Vec<Value>) -> Value {
    json!({
        "success": true,
        "data": {
            "session_id": session_id,
            "count": suggestions.len(),
            "suggestions": suggestions,
        },
    })
}

/// 构建候选列表响应，保持 data 数组和 total 分页字段的接口合同。
fn build_llm_candidate_list_response(candidates: Vec<Value>, total: i64) -> Value {
    json!({
        "success": true,
        "data": candidates,
        "total": total,
    })
}

/// 构建候选拒绝响应，明确返回本次状态更新是否成功。
fn build_llm_candidate_reject_response(rejected: bool) -> Value {
    json!({
        "success": true,
        "data": {
            "rejected": rejected,
        },
    })
}

/// 构建 LLM 分析响应，按上下文区分导入 session、持久化选择或未分类交易模式。
fn build_llm_analysis_response(candidates: Vec<Value>, context: &Value) -> Value {
    let context_object = context.as_object();
    let session_id = context_object
        .and_then(|item| item.get("session_id"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let bill_ids_present = context_object
        .and_then(|item| item.get("bill_ids"))
        .is_some_and(|value| !value.is_null());
    let mode = if !session_id.is_empty() {
        "import_session"
    } else if bill_ids_present {
        "persisted_selection"
    } else {
        "persisted_uncategorized"
    };
    let mut response_payload = Map::new();
    response_payload.insert("candidates_created".to_string(), json!(candidates.len()));
    response_payload.insert("candidates".to_string(), Value::Array(candidates.clone()));
    response_payload.insert("mode".to_string(), json!(mode));
    if !session_id.is_empty() {
        response_payload.insert("session_id".to_string(), json!(session_id));
    }

    json!({
        "success": true,
        "data": response_payload,
        "total": candidates.len(),
    })
}

/// 构建 LLM 配置读取接口响应；数据投影在 HTTP 层一次完成，避免从完整响应反向拆字段。
fn build_llm_config_get_response(config: &Value) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "data": llm_runtime_config_response_data(config, true),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OcrRuntimePresentation {
    local_json_configured: bool,
    bundled: bool,
    display_name: String,
    model: String,
}

impl OcrRuntimePresentation {
    fn from_env() -> Self {
        let local_json_configured = std::env::var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_COMMAND")
            .is_ok_and(|value| !value.trim().is_empty());
        let bundled = std::env::var("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_BUNDLED")
            .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"));
        Self {
            local_json_configured,
            bundled,
            display_name: safe_ocr_runtime_label(
                "BILL_ANALYSER_RUST_OCR_LOCAL_JSON_DISPLAY_NAME",
                "Local OCR",
            ),
            model: safe_ocr_runtime_label("BILL_ANALYSER_RUST_OCR_LOCAL_JSON_MODEL", ""),
        }
    }
}

fn safe_ocr_runtime_label(key: &str, fallback: &str) -> String {
    let value = std::env::var(key).unwrap_or_default();
    let sanitized = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(120)
        .collect::<String>();
    if sanitized.is_empty() {
        fallback.to_string()
    } else {
        sanitized
    }
}

/// 构建 OCR 配置响应载荷，递归脱敏 parameters 和 credential_config。
fn build_ocr_config_response_payload_with_runtime(
    config: &OcrConfigContract,
    runtime: &OcrRuntimePresentation,
) -> Value {
    let mut safe_parameters = config.parameters.clone();
    redact_secrets_in_value(&mut safe_parameters);
    json!({
        "provider": config.provider,
        "lang": config.lang,
        "model": config.model,
        "base_url": config.base_url,
        "parameters": safe_parameters,
        "credential_config": redact_provider_auth_config(&config.credential_config),
        "available_providers": ocr_available_providers_with_disabled(),
        "configured": config.provider != OCR_DISABLED_PROVIDER_NAME,
        "server_setup": {
            "local_model": {
                "provider": "local_json_ocr",
                "configured": runtime.local_json_configured,
                "bundled": runtime.bundled,
                "display_name": runtime.display_name,
                "model": runtime.model,
            }
        },
    })
}

fn build_ocr_config_response_payload(config: &OcrConfigContract) -> Value {
    build_ocr_config_response_payload_with_runtime(config, &OcrRuntimePresentation::from_env())
}

/// 构建 OCR 配置读取/保存成功响应，维持 success/result envelope。
fn build_ocr_config_success_response(config: &OcrConfigContract) -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": build_ocr_config_response_payload(config),
        }),
    }
}

/// 构建未知 OCR provider 的 400 响应。
fn build_unknown_ocr_provider_response() -> ImportV2RouteResponse {
    ImportV2RouteResponse {
        status_code: 400,
        body: json!({
            "success": false,
            "error": "Bad Request",
            "message": "Unknown OCR provider",
        }),
    }
}

/// 构建 OCR 错误响应，并按错误码映射 HTTP 状态与默认文案。
fn build_ocr_error_response(code: &str, message: Option<&str>) -> ImportV2RouteResponse {
    let message_text = message.unwrap_or_default().trim();
    let fallback = if code == OCR_ERROR_PROVIDER_UNCONFIGURED {
        "Receipt recognition not implemented"
    } else {
        code
    };
    let resolved_message = if message_text.is_empty() {
        fallback
    } else {
        message_text
    };

    ImportV2RouteResponse {
        status_code: ocr_error_http_status(code),
        body: json!({
            "success": false,
            "errorCode": code,
            "message": resolved_message,
        }),
    }
}

/// 将 OCR 文本结果解析为交易草稿响应，金额仍以元值停留在 OCR 外部边界。
fn build_ocr_recognition_success_response_with_context(
    provider_name: &str,
    provider_result: &OcrProviderTextResult,
    request_id: &str,
    draft_context: &ReceiptDraftContext,
) -> ImportV2RouteResponse {
    let parsed = parse_payment_screenshot_text(&provider_result.text);
    let draft = build_receipt_transaction_draft(&parsed, provider_result, draft_context);
    let confidence = provider_result
        .confidence
        .max(parsed.confidence)
        .clamp(0.0, 1.0);
    ImportV2RouteResponse {
        status_code: 200,
        body: json!({
            "success": true,
            "result": {
                "amount": parsed.amount,
                "trade_time": parsed.trade_time,
                "description": parsed.description,
                "payment_method": parsed.payment_method,
                "payment_platform": parsed.payment_platform,
                "provenance": {
                    "provider": provider_name,
                    "model": provider_result.model,
                    "request_id": request_id,
                },
                "confidence": confidence,
                "draft": draft,
                "raw_provider_response": provider_result.raw_provider_response,
            },
        }),
    }
}

/// 将 OCR 业务错误码映射为 HTTP 状态码，保持 provider/runtime 错误对前端稳定。
fn ocr_error_http_status(code: &str) -> u16 {
    match code {
        OCR_ERROR_PROVIDER_UNCONFIGURED => 501,
        OCR_ERROR_TIMEOUT => 504,
        OCR_ERROR_PARSE => 422,
        OCR_ERROR_CANCELLED => 499,
        OCR_ERROR_RATE_LIMITED => 429,
        OCR_ERROR_PROVIDER_RELOGIN_REQUIRED => 401,
        _ => 500,
    }
}
