mod llm_config;
mod llm_prompts;
mod llm_provider;
mod llm_responses;
mod ocr_config;
mod ocr_parser;
mod receipt_draft;
mod secret_redaction;
mod types;
mod value_helpers;

pub use llm_config::{
    build_llm_config_get_response, build_runtime_llm_config_from_saved_config,
    copy_runtime_llm_config, normalize_llm_advanced_settings, safe_llm_config_payload,
};
pub use llm_prompts::{
    build_llm_classification_prompt, build_llm_import_preview_recommendation_prompt,
    build_llm_rule_expression_synthesis_prompt, build_llm_rule_induction_prompt,
    parse_llm_json_array_response, render_llm_prompt_template,
};
pub use llm_provider::{
    build_llm_provider_config, llm_available_providers, normalize_llm_provider_name,
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
pub use receipt_draft::build_receipt_transaction_draft;
pub use types::{
    AiRouteResponse, LlmProviderConfigContract, OcrConfigContract, OcrProviderTextLine,
    OcrProviderTextResult, PaymentScreenshotParseContract, ReceiptDraftAccount,
    ReceiptDraftCategory, ReceiptDraftCategoryRule, ReceiptDraftContext, ReceiptDraftField,
    ReceiptDraftTag, ReceiptTransactionDraft, LLM_AVAILABLE_PROVIDERS, LLM_SYSTEM_PROMPT,
    OCR_AVAILABLE_PROVIDERS, OCR_DEFAULT_LANG, OCR_DISABLED_PROVIDER_NAME,
};
