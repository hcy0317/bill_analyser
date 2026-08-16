import type { ImportMatchingPayload, ImportMatchingRawFlag } from './import_matching.ts';
import type { ImportPreviewStateSnapshot } from './import_preview_state.ts';

export interface ImportPreviewLLMMatchingPayload {
    suggested_type?: string;
    suggested_category_id?: number | string;
    suggested_main_category?: string;
    suggested_sub_category?: string;
    suggested_source_account?: string;
    suggested_destination_account?: string;
    confidence?: number | string;
    reason?: string;
    review_status?: string;
    status?: string;
    lifecycle_status?: string;
    signal_state?: string;
    suppressed?: ImportMatchingRawFlag;
}

export type ImportPreviewMatchingPayload = ImportMatchingPayload & {
    llm?: ImportPreviewLLMMatchingPayload;
};

export interface ImportPreviewRecord {
    id: number;
    row_version?: number;
    category_id?: number | string | null;
    categoryId?: number | string | null;
    preview_type?: string;
    suggested_preview_type?: string;
    preview_date?: string;
    preview_amount_cents?: number;
    preview_destination_amount_cents?: number;
    preview_main_category?: string;
    preview_sub_category?: string;
    preview_source_account_id?: number | string | null;
    preview_destination_account_id?: number | string | null;
    preview_description?: string;
    preview_counterparty?: string;
    preview_payment_method?: string;
    transfer_suggestion_score?: number;
    transfer_suggestion_level?: string;
    transfer_suggestion_reason?: string;
    investment_signal_score?: number;
    investment_signal_level?: string;
    investment_signal_reason?: string;
    learning_recommendation_score?: number;
    learning_recommendation_level?: string;
    learning_recommendation_reason?: string;
    learning_recommendation_type?: string;
    learning_recommendation_summary?: string;
    investment_platform?: string;
    investment_product?: string;
    preview_recurring_id?: number;
    preview_recurring_name?: string;
    preview_recurring_candidate_count?: number;
    preview_recurring_match_score?: number;
    preview_recurring_match_reasons?: string;
    preview_recurring_matched_date?: string;
    dedup_type?: string;
    dedup_source_ids?: Array<number | string> | string;
    preview_parser_id?: string;
    preview_parser_tags?: string[];
    matching?: ImportPreviewMatchingPayload;
    preview_state?: ImportPreviewStateSnapshot;
    preview_is_manually_annotated?: boolean;
    preview_selected?: boolean;
    selected?: boolean;
}

export interface ImportPreviewPatchPayload {
    [field: string]: unknown;
    id?: number;
    expected_row_version?: number;
    preview_type?: string;
    preview_amount_cents?: number;
    preview_destination_amount_cents?: number;
    preview_source_account_id?: number | null;
    preview_destination_account_id?: number | null;
    preview_recurring_id?: number | null;
    preview_recurring_name?: string;
    preview_recurring_candidate_count?: number;
    preview_recurring_match_score?: number;
    preview_recurring_match_reasons?: string;
    preview_recurring_matched_date?: string;
    category_id?: number | null;
    preview_main_category?: string;
    preview_sub_category?: string;
    clear_transfer_decision?: boolean;
    is_manually_annotated?: boolean;
    selected?: boolean;
}

export interface ImportPreviewPageData {
    preview: ImportPreviewRecord[];
    total: number;
    page: number;
    page_size: number;
}

export interface ImportPreviewRowVersionConflict {
    expected_row_version: number;
    actual_row_version: number;
    previewItem: ImportPreviewRecord;
}

export interface ImportSessionSummary {
    session_id: string;
    session_version: number;
    status: string;
    created_at: string;
    parsed_count: number;
    preview_count: number;
    file_paths: unknown;
}

export interface ImportSessionVersionConflict {
    expected_session_version: number;
    actual_session_version: number;
    session: ImportSessionSummary;
}

export interface ImportConfirmResult {
    imported_count: number;
    skipped_count: number;
    errors: string[];
}

export interface ImportPreviewSelectionMetadata {
    selection_hash?: string;
    [field: string]: unknown;
}

export interface ImportPreviewSelectionPatchResponse {
    updated: number;
    selectionAction: 'patch';
    metadata: ImportPreviewSelectionMetadata;
}

export type ImportPreviewSelectionAction =
    | 'select_all'
    | 'select_valid'
    | 'select_invalid'
    | 'select_needs_annotation'
    | 'select_none'
    | 'invert';

export interface ImportPreviewSelectionActionResponse {
    updated: number;
    applied_preview_updates: number;
    selectionAction: ImportPreviewSelectionAction;
    metadata: ImportPreviewSelectionMetadata;
    previewItems: ImportPreviewRecord[];
}

export interface ImportPreviewSelectionConflict {
    expected_selection_hash: string;
    actual_selection_hash: string;
    metadata: ImportPreviewSelectionMetadata;
    previewItems: ImportPreviewRecord[];
}
