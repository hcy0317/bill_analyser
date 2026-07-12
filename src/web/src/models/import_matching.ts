export interface ImportMatchingSourcePayload {
    position?: number;
    role?: string;
    parser_id?: string;
    parser_label?: string;
    label?: string;
    channel?: string;
    tags?: string[];
    account_id?: number | null;
}

export type ImportMatchingRawFlag = boolean | number | string | null | undefined;

export interface ImportMatchingTransferPayload {
    candidate_type: string;
    score: number;
    level: string;
    learning_level?: string;
    reason: string;
    pair_order?: string;
    source_chain?: ImportMatchingSourcePayload[];
    review_status?: string;
    reviewed_type?: string;
    suppressed?: ImportMatchingRawFlag;
}

export interface ImportMatchingInvestmentPayload {
    score: number;
    level: string;
    reason: string;
    platform: string;
    product: string;
    review_status?: string;
    suppressed?: ImportMatchingRawFlag;
}

export interface ImportMatchingLearningPayload {
    rule_id: number | null;
    score: number;
    level: string;
    reason: string;
    recommended_type: string;
    summary: string;
    review_status?: string;
    status?: string;
    suppressed?: ImportMatchingRawFlag;
    source?: string;
    mode?: string;
    auto_apply?: ImportMatchingRawFlag;
    model_version?: string;
    confidence?: number;
    margin?: number;
    confirmations?: number;
    recommendation_key?: string;
    lifecycle_status?: string;
    signal_state?: string;
    accepted_count?: number;
    rejected_count?: number;
    auto_applied_count?: number;
}

export interface ImportMatchingLlmPayload {
    suggested_type?: string;
    suggested_category_id?: number;
    suggested_main_category?: string;
    suggested_sub_category?: string;
    suggested_source_account?: string;
    suggested_destination_account?: string;
    confidence?: number;
    reason?: string;
    review_status?: string;
    status?: string;
    lifecycle_status?: string;
    signal_state?: string;
    suppressed?: ImportMatchingRawFlag;
}

export interface ImportMatchingIdentityValidationPayload {
    status?: string;
    issues?: Array<Record<string, unknown>>;
}

export interface ImportMatchingRecurringPayload {
    id: number | null;
    name: string;
    candidate_count: number;
    match_score: number;
    match_reasons: string;
    matched_date: string;
}

export interface ImportMatchingDedupPayload {
    type: string;
    source_ids: Array<number | string>;
    source_count?: number;
    source_labels?: string[];
    sources?: ImportMatchingSourcePayload[];
}

export interface ImportMatchingParserPayload {
    id: string;
    tags: string[];
    source_chain?: ImportMatchingSourcePayload[];
}

export interface ImportMatchingAnnotationPayload {
    is_manually_annotated: ImportMatchingRawFlag;
    type?: string;
    history_rewrite_notice?: string;
}

export interface ImportMatchingReconciliationPayload {
    candidate_id?: string;
    candidate_type?: string;
    status?: string;
    existing_bill_id?: number | null;
    group_id?: number | null;
    score?: number;
    level?: string;
    reason?: string;
    signal_label?: string;
    source_chain?: ImportMatchingSourcePayload[];
    planned_operation?: string;
    history_bill_id?: number | null;
    history_bill_version?: number | null;
    history_role?: string;
    group_key?: string;
    operation_id?: string;
    acknowledgement_token?: string;
    destructive_ack_required?: boolean;
    notice?: string;
    history_summary?: ImportHistoryBillSummaryPayload;
}

export interface ImportHistoryBillSummaryPayload {
    bill_id: number;
    date_time: string;
    amount_cents: number;
    currency: string;
    category_name: string;
    category_status?: 'known' | 'deleted' | 'unknown';
    source_account_name: string;
    source_account_status?: 'known' | 'deleted' | 'unknown';
    destination_account_name?: string | null;
    destination_account_status?: 'known' | 'deleted' | 'unknown';
    identity_source?: 'staging_snapshot' | 'runtime_current';
    counterparty?: string | null;
    description?: string | null;
}

export interface ImportMatchingStage2BaselinePayload {
    preview_type?: string;
    preview_main_category?: string;
    preview_sub_category?: string;
    preview_source_account_id?: number | null;
    preview_destination_account_id?: number | null;
}

export interface ImportMatchingPayload {
    transfer: ImportMatchingTransferPayload;
    investment: ImportMatchingInvestmentPayload;
    learning: ImportMatchingLearningPayload;
    recurring: ImportMatchingRecurringPayload;
    dedup: ImportMatchingDedupPayload;
    parser: ImportMatchingParserPayload;
    annotation: ImportMatchingAnnotationPayload;
    llm?: ImportMatchingLlmPayload;
    reconciliation?: ImportMatchingReconciliationPayload;
    identity_validation?: ImportMatchingIdentityValidationPayload;
    stage2_baseline?: ImportMatchingStage2BaselinePayload;
}
