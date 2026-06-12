//! Shared Rust business contracts for the Bill Analyser backend.
//!
//! The HTTP runtime is Rust-only. This crate keeps request-independent
//! primitives, import/matching/budget/statistics/auth contracts, and response
//! helpers close to the business invariants they protect.

// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

pub mod account_rules;
pub mod adapters;
pub mod ai_ocr_llm;
pub mod auth;
pub mod budgets;
pub mod category_rules;
pub mod error;
pub mod import_learning;
pub mod import_learning_lifecycle;
pub mod import_pipeline;
pub mod import_pipeline_learning;
pub mod matching;
pub mod ops;
pub mod primitives;
pub mod response;
pub mod runtime;
pub mod runtime_governance;
pub mod smart_dedup;
pub mod statistics;
pub mod weaviate_derived;

pub use adapters::{api, category, transaction};
pub use ai_ocr_llm::{
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
    normalize_provider_auth_config, ocr_available_providers_with_disabled, ocr_error_http_status,
    parse_llm_json_array_response, parse_payment_screenshot_text, provider_auth_access_token,
    provider_auth_has_refresh_credential, provider_auth_is_expired, provider_auth_refresh_token,
    redact_provider_auth_config, render_llm_prompt_template, safe_llm_config_payload,
    validate_llm_vision_base_url, AiRouteResponse, LlmProviderConfigContract, OcrConfigContract,
    OcrProviderTextLine, OcrProviderTextResult, PaymentScreenshotParseContract,
    ReceiptDraftAccount, ReceiptDraftCategory, ReceiptDraftCategoryRule, ReceiptDraftContext,
    ReceiptDraftField, ReceiptDraftTag, ReceiptTransactionDraft, LLM_AVAILABLE_PROVIDERS,
    LLM_SYSTEM_PROMPT, NETWORK_OCR_PROVIDER_NAME, OCR_AVAILABLE_PROVIDERS, OCR_DEFAULT_LANG,
    OCR_DISABLED_PROVIDER_NAME,
};
pub use error::{ErrorCode, RuntimeError};
pub use import_learning::{
    amount_cents_bucket, build_composite_match_features, build_composite_match_hash,
    build_dataset_snapshot_payload, build_feature_payload, build_label_confirmation_counts,
    build_llm_preview_apply_plan, build_model_registry_payload, build_route_label,
    build_semantic_label, build_semantic_label_counts, composite_hash_from_features,
    evaluate_learning_policy, import_learning_model_version, iter_feature_tokens,
    learning_batch_accept_response, learning_center_page_response, learning_data_response,
    learning_error_response, learning_rules_page_response, llm_error_response,
    llm_memory_events_success, normalize_import_learning_suggestion_id,
    normalize_import_learning_text, normalize_learning_text, normalize_llm_preview_review_decision,
    parse_composite_match_value, parse_learning_suggestion_ids, parse_preview_ids,
    parse_route_label, parse_semantic_label, prepare_training_samples,
    should_revert_llm_preview_application, ImportLearningDatasetSnapshotPayload,
    ImportLearningModelRegistryPayload, ImportLearningPrediction, ImportLearningTrainingSample,
    LearningPolicyDecision, LearningRouteResponse, LlmMemoryEventContract, LlmPreviewApplyPlan,
    LlmPreviewSnapshot, LlmPreviewSuggestion, RouteLabelParts, SemanticLabelParts,
    BLUE_ACCEPT_CONFIRMATION_THRESHOLD, BLUE_CONFIDENCE_THRESHOLD, BLUE_MARGIN_THRESHOLD,
    DEFAULT_FEATURE_DIMENSION, FEATURE_SCHEMA_VERSION, GREEN_CONFIDENCE_THRESHOLD,
    GREEN_MARGIN_THRESHOLD, HIDDEN_DIMENSION, MIN_TRAINING_SAMPLES, MODEL_FAMILY, MODEL_KEY,
    POLICY_VERSION,
};
pub use import_learning_lifecycle::{
    build_import_learning_recommendation_key, learning_lifecycle_is_auto_eligible,
    learning_lifecycle_signal_state, normalize_learning_lifecycle_status,
    transition_import_learning_lifecycle, ImportLearningLifecycleState,
    ImportLearningLifecycleTransition, ImportLearningRecommendationKeyInput,
    LEARNING_LIFECYCLE_ACCEPTS_TO_GREEN, LEARNING_LIFECYCLE_GREEN_REJECTS_TO_DOWNGRADE,
    LEARNING_LIFECYCLE_STATUS_AUTO_APPLIED, LEARNING_LIFECYCLE_STATUS_DOWNGRADED,
    LEARNING_LIFECYCLE_STATUS_GREEN, LEARNING_LIFECYCLE_STATUS_SUPPRESSED,
    LEARNING_LIFECYCLE_STATUS_YELLOW, LEARNING_LIFECYCLE_YELLOW_REJECTS_TO_SUPPRESS,
    RECOMMENDATION_KEY_SCHEMA_VERSION,
};
pub use import_pipeline::{
    attach_import_preview_matching_payload, build_import_history_rewrite_ack_token,
    build_import_history_rewrite_operation_id, build_import_preview_filter_index_item,
    build_import_preview_matching_payload, coerce_preview_selected_value,
    expected_preview_state_from_value, expected_preview_state_is_valid,
    import_preview_index_success, import_preview_page_success,
    import_session_cancel_missing_response, import_session_cancel_success_response,
    import_session_not_found_response, import_session_success, import_stage_confirm_success,
    import_stage_dedup_success, import_stage_parse_success, import_v2_data_response,
    import_v2_error_response, import_v2_invalid_request_response, import_v2_message_response,
    import_v2_missing_session_id_response, map_import_preview_type_to_frontend_value,
    normalize_import_preview_page_query, normalize_import_preview_page_sort_direction,
    normalize_import_preview_page_sort_key, normalize_page, normalize_page_size,
    normalize_preview_ids, preview_state_conflict_response, preview_update_is_selected,
    resolve_import_preview_learning_signal_status, resolve_import_preview_transfer_signal_status,
    sort_import_preview_page_items, AccountLookup, AnnotationMatchingPayload, CategoryLookup,
    DedupMatchingPayload, ExpectedPreviewState, ImportPreviewFilterIndexItem,
    ImportPreviewIndexData, ImportPreviewMatchingPayload, ImportPreviewPageData,
    ImportPreviewPageQuery, ImportPreviewSortDirection, ImportSessionSummary,
    ImportStageConfirmData, ImportStageDedupData, ImportStageParseData, ImportV2RouteResponse,
    InvestmentMatchingPayload, LlmRecommendationPayload, ParserMatchingPayload,
    ReconciliationMatchingPayload, RecurringMatchingPayload, TransferMatchingPayload,
    BILLS_PREVIEW_CONTRACT_FIELDS, HISTORY_REWRITE_NOTICE, IMPORT_PREVIEW_SELECTION_KEYS,
    IMPORT_PREVIEW_SORT_KEYS, IMPORT_STAGING_TABLES, IMPORT_V2_PIPELINE_STEPS,
};
pub use import_pipeline_learning::LearningMatchingPayload;
pub use matching::{
    bill_pair_feedback_payload_is_related, build_bill_pair_feedback_payload,
    build_duplicate_bill_candidate, build_duplicate_bill_candidates,
    build_formal_duplicate_candidate_id, build_formal_investment_candidate_id,
    build_formal_learning_candidate_id, build_formal_transfer_candidate_id,
    build_investment_pair_candidate, build_investment_pair_candidates,
    build_learning_candidate_for_bill, build_learning_candidates_for_bill,
    build_learning_rule_result_summary, build_learning_rule_revision,
    build_matching_candidate_action_payload, build_matching_session_candidates,
    build_transfer_pair_candidate, build_transfer_pair_candidates,
    build_user_investment_keyword_settings, classify_investment_pnl_change,
    compute_recurring_pattern_hash, deserialize_learning_match_features,
    detect_recurring_frequency, detect_recurring_patterns, detect_recurring_patterns_with_today,
    estimate_next_recurring_date, extract_investment_profile, is_ordinary_bank_interest_income,
    normalize_keyword_list, normalize_learning_rule_revision, normalize_reconcile_history_families,
    normalize_transfer_pair_bill_ids, parse_manual_pair_request, parse_matching_candidate_id,
    parse_reconciliation_candidates_query, parse_recurring_date, score_investment_candidate,
    score_learning_rule_similarity, serialize_keyword_list, serialize_recurring_suggestion,
    serialize_recurring_suggestions, FrequencyDetection, InvestmentProfile, InvestmentSignal,
    ManualPairRequest, MatchingCandidateDescriptor, RecurringPattern, CANDIDATE_KIND_ORDER,
    DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS, DEFAULT_INVESTMENT_PLATFORM_KEYWORDS,
    DEFAULT_INVESTMENT_PRODUCT_KEYWORDS, EXPLICIT_INVESTMENT_TYPES, INVESTMENT_PAIR_LOOKBACK_DAYS,
    INVESTMENT_PAIR_TYPE, MANUAL_PAIR_SOURCE, MAX_RECURRING_GAP_RATIO,
    MAX_RECURRING_INTERVAL_VARIATION, MIN_RECURRING_OCCURRENCES, MIN_RECURRING_PATTERN_CONFIDENCE,
    SUMMARY_KIND_ORDER, TRANSFER_AMOUNT_TOLERANCE_CENTS, TRANSFER_PAIR_LOOKBACK_DAYS,
    TRANSFER_PAIR_TYPE,
};
pub use ops::{
    backup_archive_summary_from_entries, backup_encryption_secret_configured,
    build_backup_file_info, build_cloud_backup_object_key, build_sync_config_contract,
    build_user_data_audit_contract, derive_backup_fernet_key, invalid_backup_archive_summary,
    is_allowed_backup_filename, is_safe_backup_archive_member, normalize_backup_job_payload,
    normalize_report_export_format, normalize_sync_provider, normalize_user_data_statistics,
    parse_comma_separated_ints, parse_export_timestamp_millis, plan_backup_cleanup,
    resolve_backup_filename, resolve_sensitive_auth_mode, secure_backup_filename,
    secure_report_filename, user_data_statistics_response, BackupArchiveSummary,
    BackupCleanupDecision, BackupCleanupPlan, BackupFileCandidate, BackupFileInfoContract,
    BackupFileInfoInput, BackupFilenameResolution, BackupJobContract, BackupRecordContract,
    OpsContractError, ReportExportContract, SensitiveAuthMode, SyncConfigContract,
    UserDataAuditContract, UserDataClearKind, UserDataStatisticsContract,
    BACKUP_DEFAULT_RETENTION_COUNT, BACKUP_DEFAULT_RETENTION_DAYS, BACKUP_ENCRYPTED_SUFFIX,
    BACKUP_PREFIX, BACKUP_ZIP_SUFFIX, DEFAULT_BACKUP_SYNC_PREFIX, SUPPORTED_SYNC_PROVIDERS,
    VALID_REPORT_EXPORT_FORMATS,
};
pub use primitives::{
    normalize_bill_date_text, parse_bill_datetime, AuthContext, BillDateTime, CurrencyCode,
    EntityId, Money, Pagination, SortField, SortOrder, TransactionType, UnixTimestampSeconds,
    UserId, UtcOffsetMinutes,
};
pub use response::{ApiError, ApiResponse};
pub use runtime::{
    runtime_health, runtime_identity_json, RuntimeHealth, RuntimeIdentity, RuntimeStatus,
};
pub use runtime_governance::{
    bills_crud_db_writer_policy, budgets_crud_db_writer_policy, can_delete_retired_import_paths,
    database_schema_db_writer_policy, domain_governance_policies, endpoints_by_owner,
    expanded_route_manifest, find_domain_policy, find_endpoint_ownership,
    governance_manifest_snapshot, import_db_writer_policy, import_deletion_blocked_endpoints,
    import_deletion_gates, manifest_states, missing_import_deletion_gates,
    response_envelope_policies, response_envelope_policy, routes_by_state, runtime_state_machine,
    rust_http_shell_ownership_matrix, DbWriteInvariant, DbWriterMode, DbWriterPolicy,
    DecisionRequired, DomainGovernancePolicy, EndpointOwnership, ExpandedRouteManifestEntry,
    GovernanceManifestSnapshot, ImportDeletionEvidence, ImportDeletionGate, ResponseEnvelopeFamily,
    ResponseEnvelopePolicy, RouteHandlerId, RuntimeBlockedStatus, RuntimeState,
};
pub use smart_dedup::{
    build_transfer_source_snapshot, find_cross_batch_transfer_pairs, find_database_duplicates,
    find_import_reconciliation_candidates, CrossBatchTransferMatch, DedupBill, DeduplicationResult,
    DeduplicationType, DuplicateGroup, ImportReconciliationCandidate, MergedBillSource,
    ReconciliationCandidateType, SmartDeduplicationEngine, SplitGroup, TransferPair,
    TransferSourceSnapshot,
};
pub use weaviate_derived::{
    build_import_learning_vector_recall_filters, build_import_learning_vector_recall_queries,
    build_weaviate_batch_upsert_payload, build_weaviate_collection_names,
    build_weaviate_delete_path, build_weaviate_derived_object, build_weaviate_graphql_query,
    build_weaviate_required_metadata, build_weaviate_schema_classes,
    derive_weaviate_feature_vector, deterministic_weaviate_object_id,
    normalize_weaviate_transaction_type_scope, validate_weaviate_collection_prefix,
    WeaviateCollectionNames, WeaviateDerivedClass, WeaviateDerivedObject, WeaviateFilterValue,
    WeaviateImportLearningRecallQuery, WeaviateMetadataFilter, WEAVIATE_CLASS_COUNTERPARTY_FEATURE,
    WEAVIATE_CLASS_DESCRIPTION_FEATURE, WEAVIATE_CLASS_IMPORT_LEARNING_SAMPLE,
    WEAVIATE_CLASS_IMPORT_LEARNING_SUGGESTION_VECTOR, WEAVIATE_DEFAULT_COLLECTION_PREFIX,
    WEAVIATE_DEFAULT_VECTOR_DIMENSIONS, WEAVIATE_RECALL_DEFAULT_LIMIT, WEAVIATE_RECALL_MAX_LIMIT,
    WEAVIATE_RULE_STATE_POSTGRES_AUTHORITATIVE,
};
