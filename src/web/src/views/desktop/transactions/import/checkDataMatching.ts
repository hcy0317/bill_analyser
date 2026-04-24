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

export interface ImportCheckMatchingSourceRow {
    id: number | string;
    parserSource?: string;
}

export interface ImportCheckMatchingDedupTitleOptions {
    matchLabel?: string;
    currentParserSource?: string;
    parserLabels?: Record<string, string>;
    sourceRows?: ImportCheckMatchingSourceRow[];
    sourceRowLookup?: ReadonlyMap<string, string>;
}

export type ImportPreviewSignalStatus = 'pending' | 'accepted' | 'rejected';

export interface ImportPreviewSignalDecision {
    decision: 'accept' | 'reject' | 'clear';
    labelKey: string;
    color: string;
}

export interface ImportPreviewSignalParserView {
    parserId: string;
    label: string;
    color: string;
    title: string;
}

export interface ImportPreviewSignalDedupView {
    dedupType: string;
    labelKey: string;
    title: string;
    color: string;
    sourceCount: number;
}

export interface ImportPreviewSignalReviewView {
    status: ImportPreviewSignalStatus;
    labelKey: string;
    title: string;
    color: string;
    profileText?: string;
    summary?: string;
    actions: ImportPreviewSignalDecision[];
}

export interface ImportPreviewSignalRecurringView {
    hasMatch: boolean;
    title: string;
    candidateCount: number;
    primaryReason: string;
}

export interface ImportPreviewSignalViewModel {
    parser: ImportPreviewSignalParserView | null;
    dedup: ImportPreviewSignalDedupView | null;
    isManuallyAnnotated: boolean;
    transferSuggestion: ImportPreviewSignalReviewView | null;
    investment: ImportPreviewSignalReviewView | null;
    learning: ImportPreviewSignalReviewView | null;
    recurring: ImportPreviewSignalRecurringView | null;
    hasAnySignal: boolean;
}

export interface ImportPreviewSignalState extends ImportCheckMatchingContextState {
    transferStatus?: ImportPreviewSignalStatus | null;
    transferTitle?: string;
    investmentStatus?: ImportPreviewSignalStatus | null;
    investmentTitle?: string;
    investmentProfileText?: string;
    learningStatus?: ImportPreviewSignalStatus | null;
    learningTitle?: string;
    learningSummary?: string;
    hasRecurringMatch?: boolean;
    recurringTitle?: string;
    recurringCandidateCount?: number;
    recurringPrimaryReason?: string;
}

export interface ImportPreviewSignalViewModelOptions extends ImportCheckMatchingDedupTitleOptions {
    parserColors?: Record<string, string>;
}

export interface ImportPreviewTypeColumnViewModel {
    type: number;
    signalKeys: string[];
}

const MATCHING_DEDUP_LABEL_KEYS: Record<string, string> = {
    transfer: 'Transfer Match',
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
        || (summary.dedupType !== '' && summary.dedupType !== 'remaining' && summary.dedupSourceIds.length > 0);
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

function getParserDisplayLabel(parserId: string, parserLabels?: Record<string, string>): string {
    return parserLabels?.[parserId] || parserId;
}

export function resolveImportCheckMatchingTransferParserSources(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions = {}
): string[] {
    const sourceRows = options.sourceRows || [];
    const sourceRowLookup = options.sourceRowLookup || (sourceRows.length > 0
        ? new Map(
            sourceRows
                .map(sourceRow => [String(sourceRow.id), (sourceRow.parserSource || '').trim()] as const)
                .filter(([, parserSource]) => !!parserSource)
        )
        : null);
    const normalizedDedupType = (summary.dedupType || '').trim().toLowerCase();

    if (normalizedDedupType !== 'transfer') {
        return [];
    }

    const parserSources: string[] = [];
    const addParserSource = (parserSource: string | undefined): void => {
        const normalizedParserSource = (parserSource || '').trim();
        if (normalizedParserSource && !parserSources.includes(normalizedParserSource)) {
            parserSources.push(normalizedParserSource);
        }
    };

    addParserSource(options.currentParserSource || summary.parserId);

    for (const sourceId of summary.dedupSourceIds) {
        addParserSource(sourceRowLookup?.get(String(sourceId)));
    }

    return parserSources;
}

export function getImportCheckMatchingDedupTitle(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions = {}
): string {
    const parserSources = resolveImportCheckMatchingTransferParserSources(summary, options);
    if (parserSources.length > 0) {
        return [
            options.matchLabel || 'Matching',
            ...parserSources.map(parserSource => getParserDisplayLabel(parserSource, options.parserLabels))
        ].filter(text => !!text).join(' | ');
    }

    return [
        summary.dedupType,
        summary.dedupSourceIds.length > 0 ? summary.dedupSourceIds.join('|') : ''
    ].filter(text => !!text).join(' | ');
}

export function getImportCheckMatchingParserTagsText(summary: ImportCheckMatchingContextSummary): string {
    return summary.parserTags.filter(tag => !!tag).join(' · ');
}

export function buildImportPreviewTypeColumnViewModel(type: number): ImportPreviewTypeColumnViewModel {
    return {
        type,
        signalKeys: []
    };
}

function buildReviewView(
    status: ImportPreviewSignalStatus | null | undefined,
    title: string | undefined,
    pendingLabelKey: string,
    acceptedLabelKey: string,
    rejectedLabelKey: string,
    pendingActions: ImportPreviewSignalDecision[],
    profileText?: string,
    summary?: string
): ImportPreviewSignalReviewView | null {
    if (!status) {
        return null;
    }

    const titleParts = [title || '', profileText || ''].filter((part, index, parts) => {
        const normalizedPart = part.trim();
        return !!normalizedPart && parts.findIndex(candidate => candidate.trim() === normalizedPart) === index;
    });
    const resolvedTitle = titleParts.join(' | ');

    if (status === 'pending') {
        return {
            status,
            labelKey: pendingLabelKey,
            title: resolvedTitle,
            color: 'info',
            profileText,
            summary,
            actions: pendingActions
        };
    }

    if (status === 'accepted') {
        return {
            status,
            labelKey: acceptedLabelKey,
            title: resolvedTitle,
            color: 'success',
            profileText,
            summary,
            actions: [{ decision: 'clear', labelKey: 'Clear', color: 'warning' }]
        };
    }

    return {
        status,
        labelKey: rejectedLabelKey,
        title: resolvedTitle,
        color: 'error',
        profileText,
        summary,
        actions: [{ decision: 'clear', labelKey: 'Clear', color: 'warning' }]
    };
}

export function buildImportPreviewSignalViewModel(
    state: ImportPreviewSignalState,
    options: ImportPreviewSignalViewModelOptions = {}
): ImportPreviewSignalViewModel {
    const matchingSummary = getImportCheckMatchingContextSummary(state);
    const parser = matchingSummary.parserId
        ? {
            parserId: matchingSummary.parserId,
            label: getParserDisplayLabel(matchingSummary.parserId, options.parserLabels),
            color: options.parserColors?.[matchingSummary.parserId] || 'grey',
            title: getImportCheckMatchingParserTagsText(matchingSummary)
        }
        : null;
    const dedup = hasImportCheckMatchingDedupContext(matchingSummary)
        ? {
            dedupType: matchingSummary.dedupType,
            labelKey: getImportCheckMatchingDedupLabel(matchingSummary),
            title: getImportCheckMatchingDedupTitle(matchingSummary, options),
            color: (matchingSummary.dedupType || '').trim().toLowerCase() === 'transfer' ? 'primary' : 'secondary',
            sourceCount: matchingSummary.dedupSourceIds.length
        }
        : null;
    const transferSuggestion = buildReviewView(
        state.transferStatus,
        state.transferTitle,
        'Likely Transfer',
        'Transfer Suggestion Accepted',
        'Transfer Suggestion Rejected',
        [
            { decision: 'accept', labelKey: 'Apply Suggestion', color: 'warning' },
            { decision: 'reject', labelKey: 'Reject Transfer Suggestion', color: 'error' }
        ]
    );
    const investment = buildReviewView(
        state.investmentStatus,
        state.investmentTitle,
        'Investment Signal',
        'Accepted',
        'Rejected',
        [
            { decision: 'accept', labelKey: 'Accept', color: 'info' },
            { decision: 'reject', labelKey: 'Reject', color: 'error' }
        ],
        state.investmentProfileText
    );
    const learning = buildReviewView(
        state.learningStatus,
        state.learningTitle,
        'Learning Suggestion',
        'Learning Suggestion Accepted',
        'Learning Suggestion Rejected',
        [
            { decision: 'accept', labelKey: 'Apply Suggestion', color: 'secondary' },
            { decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' }
        ],
        undefined,
        state.learningSummary
    );
    const recurring = state.hasRecurringMatch || (state.recurringCandidateCount || 0) > 0
        ? {
            hasMatch: !!state.hasRecurringMatch,
            title: state.recurringTitle || '',
            candidateCount: state.recurringCandidateCount || 0,
            primaryReason: state.recurringPrimaryReason || ''
        }
        : null;

    return {
        parser,
        dedup,
        isManuallyAnnotated: matchingSummary.isManuallyAnnotated,
        transferSuggestion,
        investment,
        learning,
        recurring,
        hasAnySignal: !!parser
            || !!dedup
            || !!transferSuggestion
            || !!investment
            || !!learning
            || !!recurring
    };
}
