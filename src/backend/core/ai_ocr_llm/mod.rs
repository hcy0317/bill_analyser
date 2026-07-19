// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

mod llm_config;
mod llm_prompts;
mod llm_provider;
mod llm_responses;
mod ocr_config;
mod ocr_parser;
mod provider_auth;
mod receipt_draft;
mod secret_redaction;
mod types;
mod value_helpers;

// LLM/OCR facade 只暴露脱敏配置、provider allowlist、prompt 渲染、
// response 截断和 receipt draft 合同；HTTP 层负责 rate limit 与用户上下文。
pub use llm_config::{
    build_llm_config_get_response, build_runtime_llm_config_from_saved_config,
    copy_runtime_llm_config, normalize_llm_advanced_settings, safe_llm_config_payload,
};
pub use llm_prompts::{
    build_llm_account_rule_induction_prompt, build_llm_category_rule_induction_prompt,
    build_llm_classification_prompt, build_llm_import_preview_recommendation_prompt,
    build_llm_rule_expression_synthesis_prompt, build_llm_rule_induction_prompt,
    parse_llm_json_array_response, render_llm_prompt_template,
};
pub use llm_provider::{
    build_llm_provider_config, llm_available_providers, normalize_llm_provider_name,
    validate_llm_vision_base_url,
};
pub use llm_responses::{
    build_llm_analysis_response, build_llm_candidate_list_response,
    build_llm_candidate_reject_response, build_llm_contract_error_response,
    build_llm_preview_recommend_response, llm_review_endpoint_requires_live_provider,
};
pub use ocr_config::{
    build_ocr_config_response_payload, build_ocr_config_success_response, build_ocr_error_response,
    build_ocr_recognition_success_response, build_ocr_recognition_success_response_with_context,
    build_unknown_ocr_provider_response, normalize_ocr_config, normalize_ocr_lang,
    normalize_ocr_provider_name, ocr_available_providers_with_disabled, ocr_error_http_status,
};
pub use ocr_parser::parse_payment_screenshot_text;
pub use provider_auth::{
    normalize_provider_auth_config, provider_auth_access_token,
    provider_auth_has_refresh_credential, provider_auth_is_expired, provider_auth_refresh_token,
    redact_provider_auth_config,
};
pub use receipt_draft::build_receipt_transaction_draft;
pub use types::{
    AiRouteResponse, LlmProviderConfigContract, OcrConfigContract, OcrProviderTextLine,
    OcrProviderTextResult, PaymentScreenshotParseContract, ReceiptDraftAccount,
    ReceiptDraftCategory, ReceiptDraftCategoryRule, ReceiptDraftContext, ReceiptDraftField,
    ReceiptDraftTag, ReceiptTransactionDraft, LLM_AVAILABLE_PROVIDERS, LLM_SYSTEM_PROMPT,
    NETWORK_OCR_PROVIDER_NAME, OCR_AVAILABLE_PROVIDERS, OCR_DEFAULT_LANG,
    OCR_DISABLED_PROVIDER_NAME,
};
