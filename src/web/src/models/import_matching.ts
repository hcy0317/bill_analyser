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
}

export interface ImportMatchingPayload {
    transfer: ImportMatchingTransferPayload;
    investment: ImportMatchingInvestmentPayload;
    learning: ImportMatchingLearningPayload;
    recurring: ImportMatchingRecurringPayload;
    dedup: ImportMatchingDedupPayload;
    parser: ImportMatchingParserPayload;
    annotation: ImportMatchingAnnotationPayload;
}
