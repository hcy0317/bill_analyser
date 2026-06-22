import { TransactionType } from '@/core/transaction.ts';

import type { ImportMatchingPayload } from '../import_matching.ts';

export function getFirstNonEmptyString(...values: Array<string | null | undefined>): string {
    for (const value of values) {
        if (typeof value === 'string' && value) {
            return value;
        }
    }

    return '';
}

export function getFirstDefinedIdString(...values: Array<number | string | null | undefined>): string {
    for (const value of values) {
        if (typeof value === 'number') {
            return String(value);
        }

        if (typeof value === 'string' && value) {
            return value;
        }
    }

    return '';
}

export function getSuggestedTypeFromMatchingCandidate(candidateType: string | undefined): number | undefined {
    const normalizedCandidateType = (candidateType || '').trim().toLowerCase();
    if (
        normalizedCandidateType === '转账'
        || normalizedCandidateType === '4'
        || normalizedCandidateType.includes('transfer')
    ) {
        return TransactionType.Transfer;
    }

    return undefined;
}

export function normalizeDedupSourceIds(rawValue: Array<number | string> | string | undefined): Array<number | string> {
    if (Array.isArray(rawValue)) {
        return rawValue.map(value => {
            if (typeof value === 'string') {
                const trimmedValue = value.trim();
                const parsedValue = Number(trimmedValue);
                return Number.isNaN(parsedValue) ? trimmedValue : parsedValue;
            }

            return value;
        }).filter(value => value !== '');
    }

    if (typeof rawValue === 'string' && rawValue) {
        return rawValue.split(',').map(value => value.trim()).filter(value => !!value).map(value => {
            const parsedValue = Number(value);
            return Number.isNaN(parsedValue) ? value : parsedValue;
        });
    }

    return [];
}

export function hasDedupSourceIds(rawValue: Array<number | string> | string | undefined): boolean {
    return normalizeDedupSourceIds(rawValue).length > 0;
}

export type SparseImportMatchingPayload = {
    transfer?: Partial<ImportMatchingPayload['transfer']>;
    investment?: Partial<ImportMatchingPayload['investment']>;
    learning?: Partial<ImportMatchingPayload['learning']>;
    recurring?: Partial<ImportMatchingPayload['recurring']>;
    dedup?: Partial<ImportMatchingPayload['dedup']>;
    parser?: Partial<ImportMatchingPayload['parser']>;
    annotation?: Partial<ImportMatchingPayload['annotation']>;
    reconciliation?: ImportMatchingPayload['reconciliation'];
    stage2_baseline?: ImportMatchingPayload['stage2_baseline'];
};

export function normalizeImportMatchingPayload(matching?: SparseImportMatchingPayload): ImportMatchingPayload | undefined {
    if (!matching) {
        return undefined;
    }

    const parser = matching.parser;
    const parserTags = Array.isArray(parser?.tags) ? parser.tags : [];
    const dedupSourceIds = normalizeDedupSourceIds(matching.dedup?.source_ids);

    return {
        transfer: {
            candidate_type: '',
            score: 0,
            level: '',
            reason: '',
            review_status: '',
            reviewed_type: '',
            suppressed: false,
            pair_order: '',
            source_chain: [],
            ...matching.transfer,
        },
        investment: {
            score: 0,
            level: '',
            reason: '',
            platform: '',
            product: '',
            review_status: '',
            suppressed: false,
            ...matching.investment,
        },
        learning: {
            rule_id: null,
            score: 0,
            level: '',
            reason: '',
            recommended_type: '',
            summary: '',
            review_status: '',
            suppressed: false,
            source: '',
            mode: '',
            auto_apply: false,
            model_version: '',
            recommendation_key: '',
            lifecycle_status: '',
            signal_state: '',
            accepted_count: 0,
            rejected_count: 0,
            auto_applied_count: 0,
            ...matching.learning,
        },
        recurring: {
            id: null,
            name: '',
            candidate_count: 0,
            match_score: 0,
            match_reasons: '',
            matched_date: '',
            ...matching.recurring,
        },
        dedup: {
            type: matching.dedup?.type || '',
            source_ids: dedupSourceIds,
            source_count: matching.dedup?.source_count ?? dedupSourceIds.length,
            source_labels: matching.dedup?.source_labels || [],
            sources: matching.dedup?.sources || [],
        },
        parser: {
            id: parser?.id || '',
            tags: parserTags,
            source_chain: parser?.source_chain || [],
        },
        annotation: {
            is_manually_annotated: !!matching.annotation?.is_manually_annotated,
        },
        reconciliation: matching.reconciliation || {},
        stage2_baseline: matching.stage2_baseline,
    };
}
