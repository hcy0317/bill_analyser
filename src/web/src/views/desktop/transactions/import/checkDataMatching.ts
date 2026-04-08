export interface ImportCheckMatchingContextState {
    parserSource?: string;
    parserTags?: string[];
    dedupType?: string;
    dedupSourceIds?: Array<number | string>;
    isManuallyAnnotated?: boolean;
}

export interface ImportCheckMatchingContextSummary {
    parserId: string;
    parserTags: string[];
    dedupType: string;
    dedupSourceIds: Array<number | string>;
    isManuallyAnnotated: boolean;
}

const MATCHING_DEDUP_LABEL_KEYS: Record<string, string> = {
    transfer: 'Transfer',
    platform_bank: 'Platform-Bank Duplicate',
    similar: 'Similar Duplicate',
    split: 'Split-Merge Duplicate',
    split_merge: 'Split-Merge Duplicate',
    transfer_cross_batch: 'Cross-Batch Transfer'
};

function humanizeDedupType(rawType: string): string {
    return rawType
        .split('_')
        .filter(part => !!part)
        .map(part => part.charAt(0).toUpperCase() + part.slice(1))
        .join(' ');
}

export function getImportCheckMatchingContextSummary(
    state: ImportCheckMatchingContextState
): ImportCheckMatchingContextSummary {
    return {
        parserId: state.parserSource || '',
        parserTags: state.parserTags || [],
        dedupType: state.dedupType || '',
        dedupSourceIds: state.dedupSourceIds || [],
        isManuallyAnnotated: !!state.isManuallyAnnotated
    };
}

export function hasImportCheckMatchingContext(summary: ImportCheckMatchingContextSummary): boolean {
    return !!summary.parserId
        || (summary.dedupType !== '' && summary.dedupType !== 'remaining' && summary.dedupSourceIds.length > 0)
        || summary.isManuallyAnnotated;
}

export function hasImportCheckMatchingDedupContext(summary: ImportCheckMatchingContextSummary): boolean {
    return summary.dedupType !== '' && summary.dedupType !== 'remaining' && summary.dedupSourceIds.length > 0;
}

export function getImportCheckMatchingDedupLabel(summary: ImportCheckMatchingContextSummary): string {
    const normalizedDedupType = (summary.dedupType || '').trim().toLowerCase();

    if (!normalizedDedupType || normalizedDedupType === 'remaining') {
        return '';
    }

    return MATCHING_DEDUP_LABEL_KEYS[normalizedDedupType] || humanizeDedupType(normalizedDedupType);
}

export function getImportCheckMatchingDedupTitle(summary: ImportCheckMatchingContextSummary): string {
    return [
        summary.dedupType,
        summary.dedupSourceIds.length > 0 ? summary.dedupSourceIds.join('|') : ''
    ].filter(text => !!text).join(' | ');
}

export function getImportCheckMatchingParserTagsText(summary: ImportCheckMatchingContextSummary): string {
    return summary.parserTags.filter(tag => !!tag).join(' · ');
}
