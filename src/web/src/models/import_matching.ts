export interface ImportMatchingTransferPayload {
    candidate_type: string;
    score: number;
    level: string;
    reason: string;
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
}

export interface ImportMatchingLearningPayload {
    rule_id: number | null;
    score: number;
    level: string;
    reason: string;
    recommended_type: string;
    summary: string;
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
}

export interface ImportMatchingParserPayload {
    id: string;
    tags: string[];
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
