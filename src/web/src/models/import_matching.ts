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

export interface ImportMatchingTransferPayload {
    candidate_type: string;
    score: number;
    level: string;
    reason: string;
    pair_order?: string;
    source_chain?: ImportMatchingSourcePayload[];
    review_status?: string;
    reviewed_type?: string;
    suppressed?: boolean;
}

export interface ImportMatchingInvestmentPayload {
    score: number;
    level: string;
    reason: string;
    platform: string;
    product: string;
    review_status?: string;
    suppressed?: boolean;
}

export interface ImportMatchingLearningPayload {
    rule_id: number | null;
    score: number;
    level: string;
    reason: string;
    recommended_type: string;
    summary: string;
    review_status?: string;
    suppressed?: boolean;
    source?: string;
    mode?: string;
    auto_apply?: boolean;
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
    is_manually_annotated: boolean;
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
    reconciliation?: ImportMatchingReconciliationPayload;
    stage2_baseline?: ImportMatchingStage2BaselinePayload;
}
