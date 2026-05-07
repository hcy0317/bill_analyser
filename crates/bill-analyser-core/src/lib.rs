//! Internal Rust foundations for the Bill Analyser backend migration.
//!
//! Flask remains the REST shell. The crate currently contains migration
//! foundations and shared primitives only; no business API is migrated here.

pub mod adapters;
pub mod ai_ocr_llm;
pub mod auth;
pub mod budgets;
pub mod category_rules;
pub mod error;
pub mod import_learning;
pub mod import_pipeline;
pub mod matching;
pub mod parsers;
pub mod primitives;
pub mod response;
pub mod runtime;
pub mod smart_dedup;
pub mod statistics;

pub use adapters::{account, api, category, transaction};
pub use error::{ErrorCode, RuntimeError};
pub use import_learning::{
    amount_bucket, build_composite_match_features, build_composite_match_hash,
    build_dataset_snapshot_payload, build_feature_payload, build_label_confirmation_counts,
    build_llm_preview_apply_plan, build_model_registry_payload, build_route_label,
    build_semantic_label, build_semantic_label_counts, composite_hash_from_features,
    evaluate_learning_policy, import_learning_model_version, iter_feature_tokens,
    learning_batch_accept_response, learning_center_page_response, learning_data_response,
    learning_error_response, legacy_learning_rules_page_response, llm_error_response,
    llm_memory_events_success, normalize_import_learning_suggestion_id,
    normalize_import_learning_text, normalize_learning_text, normalize_llm_preview_review_decision,
    parse_composite_match_value, parse_learning_suggestion_ids, parse_preview_ids,
    parse_route_label, parse_semantic_label, prepare_training_samples,
    should_restore_llm_previous_preview, ImportLearningDatasetSnapshotPayload,
    ImportLearningModelRegistryPayload, ImportLearningPrediction, ImportLearningTrainingSample,
    LearningPolicyDecision, LearningRouteResponse, LlmMemoryEventContract, LlmPreviewApplyPlan,
    LlmPreviewSnapshot, LlmPreviewSuggestion, RouteLabelParts, SemanticLabelParts,
    BLUE_ACCEPT_CONFIRMATION_THRESHOLD, BLUE_CONFIDENCE_THRESHOLD, BLUE_MARGIN_THRESHOLD,
    DEFAULT_FEATURE_DIMENSION, FEATURE_SCHEMA_VERSION, GREEN_CONFIDENCE_THRESHOLD,
    GREEN_MARGIN_THRESHOLD, HIDDEN_DIMENSION, MIN_TRAINING_SAMPLES, MODEL_FAMILY, MODEL_KEY,
    POLICY_VERSION,
};
pub use import_pipeline::{
    build_import_preview_filter_index_item, coerce_preview_selected_value,
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
    InvestmentMatchingPayload, LearningMatchingPayload, LlmRecommendationPayload,
    ParserMatchingPayload, ReconciliationMatchingPayload, RecurringMatchingPayload,
    TransferMatchingPayload, BILLS_PREVIEW_CONTRACT_FIELDS, IMPORT_PREVIEW_SELECTION_KEYS,
    IMPORT_PREVIEW_SORT_KEYS, IMPORT_STAGING_TABLES, IMPORT_V2_PIPELINE_STEPS,
};
pub use matching::{
    bill_pair_feedback_payload_is_related, build_bill_pair_feedback_payload,
    build_formal_investment_candidate_id, build_formal_learning_candidate_id,
    build_formal_transfer_candidate_id, build_investment_pair_candidate,
    build_investment_pair_candidates, build_learning_candidate_for_bill,
    build_learning_candidates_for_bill, build_learning_rule_result_summary,
    build_learning_rule_revision, build_matching_candidate_action_payload,
    build_matching_session_candidates, build_transfer_pair_candidate,
    build_transfer_pair_candidates, build_user_investment_keyword_settings,
    classify_investment_pnl_change, compute_recurring_pattern_hash,
    deserialize_learning_match_features, detect_recurring_frequency, detect_recurring_patterns,
    detect_recurring_patterns_with_today, estimate_next_recurring_date, extract_investment_profile,
    is_ordinary_bank_interest_income, normalize_keyword_list, normalize_learning_rule_revision,
    normalize_reconcile_history_families, normalize_transfer_pair_bill_ids,
    parse_manual_pair_request, parse_matching_candidate_id, parse_reconciliation_candidates_query,
    parse_recurring_date, score_investment_candidate, score_learning_rule_similarity,
    serialize_keyword_list, serialize_recurring_suggestion, serialize_recurring_suggestions,
    FrequencyDetection, InvestmentProfile, InvestmentSignal, ManualPairRequest,
    MatchingCandidateDescriptor, RecurringPattern, CANDIDATE_KIND_ORDER,
    DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS, DEFAULT_INVESTMENT_PLATFORM_KEYWORDS,
    DEFAULT_INVESTMENT_PRODUCT_KEYWORDS, EXPLICIT_INVESTMENT_TYPES, INVESTMENT_PAIR_LOOKBACK_DAYS,
    INVESTMENT_PAIR_TYPE, LEGACY_SUMMARY_KIND_ORDER, MANUAL_PAIR_SOURCE, MAX_RECURRING_GAP_RATIO,
    MAX_RECURRING_INTERVAL_VARIATION, MIN_RECURRING_OCCURRENCES, MIN_RECURRING_PATTERN_CONFIDENCE,
    TRANSFER_AMOUNT_TOLERANCE, TRANSFER_PAIR_LOOKBACK_DAYS, TRANSFER_PAIR_TYPE,
};
pub use parsers::{
    aggregate_description, build_parser_tags, normalize_amount_text, normalize_parser_tags,
    normalize_transaction_type, parser_registry, parser_source_label, post_process_raw_bills,
    resolve_parser_tags, serialize_parser_tags, ParserInfo, RawBill, StandardBill,
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
pub use smart_dedup::{
    find_cross_batch_transfer_pairs, find_database_duplicates,
    find_import_reconciliation_candidates, CrossBatchTransferMatch, DedupBill, DeduplicationResult,
    DeduplicationType, DuplicateGroup, ImportReconciliationCandidate, MergedBillSource,
    ReconciliationCandidateType, SmartDeduplicationEngine, SplitGroup, TransferPair,
    TransferSourceSnapshot,
};
