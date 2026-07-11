import type {
    ImportCheckVisibleTransactionContext,
    ImportCheckVisibleTransactionLike,
} from '../checkDataFilters.ts';
import type { ImportPreviewRawFlag, ImportPreviewRawSignalStatus, ImportPreviewSignalStatus } from '../checkDataMatching.ts';

export interface ImportPreviewIndexItem extends ImportCheckVisibleTransactionLike {
    id: number;
    selected: boolean;
    sourceAmountCents: number;
    counterparty: string;
    paymentMethod: string;
    parserId: string;
    parserTags: string[];
    dedupType: string;
    dedupSourceIds: Array<number | string>;
    transferStatus?: ImportPreviewSignalStatus | null;
    transferTitle?: string;
    learningStatus?: ImportPreviewRawSignalStatus;
    learningStatusAbsent?: boolean;
    learningLifecycleStatus?: string;
    learningSignalState?: string;
    learningTitle?: string;
    learningSummary?: string;
    learningMode?: string;
    learningAutoApplied?: ImportPreviewRawFlag;
    learningSuppressed?: ImportPreviewRawFlag;
    learningRuleId?: number | string | null;
    learningScore?: number | string;
    learningConfidence?: number | string;
    learningMargin?: number | string;
    learningAcceptedCount?: number | string;
    learningRejectedCount?: number | string;
    learningAutoAppliedCount?: number | string;
    llmStatus?: ImportPreviewRawSignalStatus;
    llmStatusAbsent?: boolean;
    llmLifecycleStatus?: string;
    llmSignalState?: string;
    llmTitle?: string;
    llmConfidence?: number | string;
    llmSuggestedCategoryId?: number | string;
    llmCategoryPath?: string;
    llmSourceAccount?: string;
    llmDestinationAccount?: string;
    llmSuppressed?: ImportPreviewRawFlag;
    historyStatus?: ImportPreviewSignalStatus | null | undefined;
    historyTitle?: string;
    historyPlannedOperation?: string;
    historyBillId?: number | string | null;
    historyBillVersion?: number | string | null;
    historyOperationId?: string;
    historyAcknowledgementToken?: string;
    historyDestructiveAckRequired?: boolean;
    recurringTemplateId?: string;
    recurringCandidateCount?: number;
    recurringMatchReasons?: string;
    recurringMatchedDate?: string;
}

export type ImportPreviewIndexFilterContext =
    Pick<ImportCheckVisibleTransactionContext<ImportPreviewIndexItem>, 'tagNameById'>;

export interface ImportPreviewIndexPageResult {
    page: number;
    totalCount: number;
    totalPages: number;
    previewIds: number[];
}

export type PreviewTableSortDirection = 'asc' | 'desc';

export const PREVIEW_FILTER_NONE_VALUE = '__none__';
export const PREVIEW_FILTER_INVALID_VALUE = '__invalid__';

export interface ImportPreviewFacetEntry {
    value: string;
    label?: string | null;
    count: number;
}

export interface ImportPreviewFilterGroup {
    title: string;
    labels: string[];
}

export interface ImportPreviewFilterCategoryLike {
    name: string;
    subCategories?: ImportPreviewFilterCategoryLike[];
}

export interface ImportPreviewFilterAccountLike {
    name: string;
    category?: number | string;
    subAccounts?: ImportPreviewFilterAccountLike[];
}

export interface ImportPreviewFilterAccountCategoryLike {
    type: number;
    name: string;
}

export interface ImportPreviewMetadata {
    facets?: {
        categories?: ImportPreviewFacetEntry[];
        accounts?: ImportPreviewFacetEntry[];
        tags?: ImportPreviewFacetEntry[];
    };
    counts?: {
        annotations?: Record<string, number>;
        signals?: Record<string, number>;
        selected?: number;
        selected_invalid?: number;
        total?: number;
    };
}

export interface ImportPreviewServerQueryFilters {
    minDatetime?: string;
    maxDatetime?: string;
    transactionType?: string;
    category?: string;
    account?: string;
    tag?: string;
    signal?: string;
    annotation?: string;
    description?: string;
}

export interface PreviewPageRequestOptions {
    sortBy?: string | null;
    sortDirection?: PreviewTableSortDirection | null;
    filters?: ImportPreviewServerQueryFilters;
}

export interface PreviewTableSortInputItem {
    key?: string;
    value?: string;
    order?: PreviewTableSortDirection | boolean | string | null;
}

export interface PreviewTableSortItem {
    key: string;
    order?: PreviewTableSortDirection | boolean;
}

export interface ImportPreviewIndexResponseItem {
    id: number;
    preview_date?: string;
    type?: number;
    source_amount_cents?: number;
    category_id?: string;
    actual_category_name?: string;
    source_account_id?: string;
    destination_account_id?: string;
    actual_source_account_name?: string;
    actual_destination_account_name?: string;
    comment?: string;
    counterparty?: string;
    payment_method?: string;
    selected?: boolean;
    is_manually_annotated?: boolean;
    parser_source?: string;
    parser_tags?: string[];
    dedup_type?: string;
    dedup_source_ids?: Array<number | string>;
    transfer_status?: ImportPreviewSignalStatus | null;
    transfer_title?: string;
    learning_status?: ImportPreviewSignalStatus | string | null;
    learning_lifecycle_status?: string;
    learning_signal_state?: string;
    learning_title?: string;
    learning_summary?: string;
    learning_mode?: string;
    learning_auto_applied?: ImportPreviewRawFlag;
    learning_suppressed?: ImportPreviewRawFlag;
    learning_rule_id?: number | string | null;
    learning_score?: number | string;
    learning_confidence?: number | string;
    learning_margin?: number | string;
    learning_accepted_count?: number | string;
    learning_rejected_count?: number | string;
    learning_auto_applied_count?: number | string;
    llm_status?: ImportPreviewSignalStatus | string | null;
    llm_lifecycle_status?: string;
    llm_signal_state?: string;
    llm_title?: string;
    llm_confidence?: number | string;
    llm_suggested_category_id?: number | string;
    llm_category_path?: string;
    llm_source_account?: string;
    llm_destination_account?: string;
    llm_suppressed?: ImportPreviewRawFlag;
    history_status?: ImportPreviewSignalStatus | null;
    history_title?: string;
    history_planned_operation?: string;
    history_bill_id?: number;
    history_bill_version?: number;
    history_operation_id?: string;
    history_acknowledgement_token?: string;
    history_destructive_ack_required?: boolean;
    recurring_template_id?: string;
    recurring_candidate_count?: number;
    recurring_match_reasons?: string;
    recurring_matched_date?: string;
}

export const SERVER_PAGED_SORTABLE_COLUMNS = new Set<string>([
    'time',
    'type',
    'sourceAmountCents',
    'counterparty',
    'paymentMethod',
    'comment'
]);
