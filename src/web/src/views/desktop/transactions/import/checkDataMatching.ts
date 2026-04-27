import type { ImportMatchingSourcePayload } from '@/models/import_matching.ts';

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
    parserTags?: string[];
}

export interface ImportCheckMatchingSourceContext {
    parserSource?: string;
    parserTags?: string[];
}

export interface ImportCheckMatchingDedupTitleOptions {
    matchLabel?: string;
    currentParserSource?: string;
    parserLabels?: Record<string, string>;
    sourceRows?: ImportCheckMatchingSourceRow[];
    sourceRowLookup?: ReadonlyMap<string, string | ImportCheckMatchingSourceContext>;
    dedupLabels?: Record<string, string>;
    sourceRoleLabels?: Record<string, string>;
    infoLabels?: Partial<ImportPreviewSignalInfoLabels>;
}

export type ImportPreviewSignalStatus = 'pending' | 'accepted' | 'rejected';

export interface ImportPreviewSignalDecision {
    decision: 'accept' | 'reject' | 'clear';
    labelKey: string;
    color: string;
}

export interface ImportPreviewInvestmentDecisionResponse {
    reviewStatus?: string;
    suppressed?: boolean;
}

export interface ImportPreviewInvestmentDecisionState {
    reviewStatus: ImportPreviewSignalStatus;
    suppressed: boolean;
}

export function resolveImportPreviewInvestmentDecisionState(
    decision: ImportPreviewSignalDecision['decision'],
    response: ImportPreviewInvestmentDecisionResponse = {}
): ImportPreviewInvestmentDecisionState {
    const normalizedReviewStatus = (response.reviewStatus || '').trim().toLowerCase();
    const fallbackReviewStatus: ImportPreviewSignalStatus = decision === 'accept'
        ? 'accepted'
        : decision === 'reject'
            ? 'rejected'
            : 'pending';
    const reviewStatus: ImportPreviewSignalStatus = normalizedReviewStatus === 'accepted'
        || normalizedReviewStatus === 'rejected'
        || normalizedReviewStatus === 'pending'
        ? normalizedReviewStatus
        : fallbackReviewStatus;

    return {
        reviewStatus,
        suppressed: typeof response.suppressed === 'boolean'
            ? response.suppressed
            : decision === 'reject'
    };
}

export interface ImportPreviewSignalParserView {
    parserId: string;
    label: string;
    color: string;
    title: string;
    detailLines: string[];
}

export interface ImportPreviewSignalDedupView {
    dedupType: string;
    labelKey: string;
    label: string;
    title: string;
    color: string;
    sourceCount: number;
    detailLines: string[];
}

export interface ImportPreviewSignalReviewView {
    status: ImportPreviewSignalStatus;
    labelKey: string;
    title: string;
    color: string;
    profileText?: string;
    summary?: string;
    actions: ImportPreviewSignalDecision[];
    detailLines: string[];
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
    transferPairOrder?: string;
    transferSourceChain?: ImportMatchingSourcePayload[];
    investmentStatus?: ImportPreviewSignalStatus | null;
    investmentTitle?: string;
    investmentProfileText?: string;
    learningStatus?: ImportPreviewSignalStatus | null;
    learningTitle?: string;
    learningSummary?: string;
    dedupSourceCount?: number;
    dedupSourceLabels?: string[];
    dedupSources?: ImportMatchingSourcePayload[];
    parserSourceChain?: ImportMatchingSourcePayload[];
    hasRecurringMatch?: boolean;
    recurringTitle?: string;
    recurringCandidateCount?: number;
    recurringPrimaryReason?: string;
}

export interface ImportPreviewSignalViewModelOptions extends ImportCheckMatchingDedupTitleOptions {
    parserColors?: Record<string, string>;
    investmentReasonLabels?: Record<string, string>;
}

export interface ImportPreviewSignalInfoLabels {
    sourceLabel: string;
    duplicateSourcesLabel: string;
    recommendedCategoryLabel: string;
    accountRouteLabel: string;
}

export interface ImportPreviewTypeColumnViewModel {
    type: number;
    signalKeys: string[];
}

const MATCHING_DEDUP_LABEL_KEYS: Record<string, string> = {
    transfer: 'Transfer Match',
    platform_bank: 'Platform Duplicate',
    similar: 'Similar Duplicate',
    split: 'Split-Merge Duplicate',
    split_merge: 'Split-Merge Duplicate',
    transfer_cross_batch: 'Cross-Batch Transfer'
};

const DEFAULT_INVESTMENT_REASON_LABELS: Record<string, string> = {
    platform: 'Platform',
    product: 'Product',
    exclude: 'Exclude',
    negative: 'Negative',
    type: 'Type'
};

const DEFAULT_SOURCE_ROLE_LABELS: Record<string, string> = {
    outgoing: 'Outgoing',
    incoming: 'Incoming',
    debit: 'Outgoing',
    credit: 'Incoming'
};

const DEFAULT_SIGNAL_INFO_LABELS: ImportPreviewSignalInfoLabels = {
    sourceLabel: 'Source',
    duplicateSourcesLabel: 'Duplicate Sources',
    recommendedCategoryLabel: 'Recommended Category',
    accountRouteLabel: 'Account Route'
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

export function shouldShowImportCheckMatchingDedupSourceCount(dedupType: string | undefined): boolean {
    return (dedupType || '').trim().toLowerCase() !== 'platform_bank';
}

function getParserDisplayLabel(parserId: string, parserLabels?: Record<string, string>): string {
    return parserLabels?.[parserId] || parserId;
}

function getParserSourceFromTag(tag: string): string {
    const normalizedTag = (tag || '').trim();
    const parserTagPrefix = 'parser:';
    if (!normalizedTag.toLowerCase().startsWith(parserTagPrefix)) {
        return '';
    }

    return normalizedTag.slice(parserTagPrefix.length).trim();
}

function getSourceContextFromLookupValue(
    value: string | ImportCheckMatchingSourceContext | undefined
): ImportCheckMatchingSourceContext {
    if (typeof value === 'string') {
        return { parserSource: value };
    }

    return value || {};
}

function getSignalInfoLabels(
    options: ImportCheckMatchingDedupTitleOptions
): ImportPreviewSignalInfoLabels {
    return {
        ...DEFAULT_SIGNAL_INFO_LABELS,
        ...(options.infoLabels || {})
    };
}

function getSourceDisplayLabel(
    source: ImportMatchingSourcePayload,
    parserLabels?: Record<string, string>
): string {
    const preferredLabel = (source.label || '').trim();
    if (preferredLabel) {
        return preferredLabel;
    }

    const parserLabel = (source.parser_label || '').trim();
    if (parserLabel) {
        return parserLabel;
    }

    return getParserDisplayLabel((source.parser_id || '').trim(), parserLabels);
}

function dedupeTextItems(items: string[]): string[] {
    const deduped: string[] = [];
    for (const item of items) {
        const normalizedItem = item.trim();
        if (normalizedItem && !deduped.includes(normalizedItem)) {
            deduped.push(normalizedItem);
        }
    }

    return deduped;
}

function formatInfoLine(label: string, value: string): string {
    const normalizedLabel = label.trim();
    const separator = /[\u3400-\u9fff]/.test(normalizedLabel) ? '：' : ': ';
    return `${normalizedLabel}${separator}${value}`;
}

function sortSourceChain(
    sources: ImportMatchingSourcePayload[] | undefined,
    pairOrder: string | undefined
): ImportMatchingSourcePayload[] {
    const normalizedPairOrder = (pairOrder || '').trim().toLowerCase();
    const orderedSources = [...(sources || [])];
    const roleOrder = normalizedPairOrder === 'outgoing_first'
        ? ['outgoing', 'debit', 'incoming', 'credit']
        : [];

    return orderedSources.sort((left, right) => {
        const leftRole = (left.role || '').trim().toLowerCase();
        const rightRole = (right.role || '').trim().toLowerCase();
        const leftRoleIndex = roleOrder.indexOf(leftRole);
        const rightRoleIndex = roleOrder.indexOf(rightRole);
        if (leftRoleIndex !== rightRoleIndex) {
            if (leftRoleIndex >= 0 && rightRoleIndex >= 0) {
                return leftRoleIndex - rightRoleIndex;
            }
            if (leftRoleIndex >= 0) {
                return -1;
            }
            if (rightRoleIndex >= 0) {
                return 1;
            }
        }

        return Number(left.position || 0) - Number(right.position || 0);
    });
}

function getSourceChainDisplayLabels(
    sources: ImportMatchingSourcePayload[] | undefined,
    options: ImportCheckMatchingDedupTitleOptions,
    pairOrder?: string
): string[] {
    return dedupeTextItems(
        sortSourceChain(sources, pairOrder).map(source => getSourceDisplayLabel(source, options.parserLabels))
    );
}

function buildParserDetailLines(
    summary: ImportCheckMatchingContextSummary,
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const infoLabels = getSignalInfoLabels(options);
    const structuredLabels = getSourceChainDisplayLabels(state.parserSourceChain, options);
    if (structuredLabels.length > 0) {
        return [formatInfoLine(infoLabels.sourceLabel, structuredLabels.join(' · '))];
    }

    const parserSources = dedupeTextItems([
        summary.parserId,
        ...summary.parserTags.map(tag => getParserSourceFromTag(tag))
    ]).map(parserSource => getParserDisplayLabel(parserSource, options.parserLabels));

    if (parserSources.length > 0) {
        return [formatInfoLine(infoLabels.sourceLabel, parserSources.join(' · '))];
    }

    return [];
}

function buildTransferDetailLines(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const sourceChain = sortSourceChain(state.transferSourceChain, state.transferPairOrder);
    if (sourceChain.length > 0) {
        const roleLabels = {
            ...DEFAULT_SOURCE_ROLE_LABELS,
            ...(options.sourceRoleLabels || {})
        };
        return sourceChain.map(source => {
            const role = (source.role || '').trim().toLowerCase();
            const roleLabel = roleLabels[role];
            const sourceLabel = getSourceDisplayLabel(source, options.parserLabels);
            return roleLabel
                ? formatInfoLine(roleLabel, sourceLabel)
                : sourceLabel;
        }).filter(line => !!line);
    }

    return state.transferTitle ? [state.transferTitle] : [];
}

function buildDedupDetailLines(
    summary: ImportCheckMatchingContextSummary,
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const normalizedDedupType = (summary.dedupType || '').trim().toLowerCase();
    if (normalizedDedupType === 'platform_bank') {
        const infoLabels = getSignalInfoLabels(options);
        const sourceLabels = dedupeTextItems([
            ...(state.dedupSourceLabels || []),
            ...getSourceChainDisplayLabels(state.dedupSources, options)
        ]);

        if (sourceLabels.length > 0) {
            return [formatInfoLine(infoLabels.duplicateSourcesLabel, sourceLabels.join(' · '))];
        }
    }

    const title = getImportCheckMatchingDedupTitle(summary, options);
    return title ? [title] : [];
}

function buildLearningDetailLines(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const infoLabels = getSignalInfoLabels(options);
    const summaryParts = (state.learningSummary || '')
        .split('|')
        .map(part => part.trim())
        .filter(part => !!part);

    if (summaryParts.length >= 2) {
        const recommendedType = summaryParts[0] || '';
        const recommendedCategory = summaryParts[1] || '';
        const detailLines = [formatInfoLine(infoLabels.recommendedCategoryLabel, `${recommendedType} / ${recommendedCategory}`)];
        if (summaryParts.length > 2) {
            detailLines.push(formatInfoLine(infoLabels.accountRouteLabel, summaryParts.slice(2).join(' / ')));
        }
        return detailLines;
    }

    if (summaryParts.length === 1) {
        return [formatInfoLine(infoLabels.recommendedCategoryLabel, summaryParts[0] || '')];
    }

    return state.learningTitle ? [state.learningTitle] : [];
}

function buildSignalTitle(detailLines: string[], fallbackTitle: string | undefined = ''): string {
    return detailLines.length > 0 ? detailLines.join(' | ') : (fallbackTitle || '');
}

function buildDedupLabel(
    summary: ImportCheckMatchingContextSummary,
    sourceCount: number,
    options: ImportCheckMatchingDedupTitleOptions
): string {
    const labelKey = getImportCheckMatchingDedupLabel(summary);
    const dedupLabel = options.dedupLabels?.[labelKey] || labelKey;
    if ((summary.dedupType || '').trim().toLowerCase() === 'platform_bank' && sourceCount > 0) {
        return `${dedupLabel}·${sourceCount}`;
    }

    return dedupLabel;
}

export function resolveImportCheckMatchingTransferParserSources(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions = {}
): string[] {
    const sourceRows = options.sourceRows || [];
    const sourceRowLookup = options.sourceRowLookup || (sourceRows.length > 0
        ? new Map(
            sourceRows
                .map(sourceRow => [
                    String(sourceRow.id),
                    {
                        parserSource: (sourceRow.parserSource || '').trim(),
                        parserTags: sourceRow.parserTags || []
                    }
                ] as const)
                .filter(([, sourceContext]) => !!sourceContext.parserSource || sourceContext.parserTags.length > 0)
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
    const addParserSourcesFromTags = (parserTags: string[] | undefined): void => {
        for (const tag of parserTags || []) {
            addParserSource(getParserSourceFromTag(tag));
        }
    };

    addParserSource(options.currentParserSource || summary.parserId);
    addParserSourcesFromTags(summary.parserTags);

    for (const sourceId of summary.dedupSourceIds) {
        const sourceContext = getSourceContextFromLookupValue(sourceRowLookup?.get(String(sourceId)));
        addParserSource(sourceContext.parserSource);
        addParserSourcesFromTags(sourceContext.parserTags);
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

    const labelKey = getImportCheckMatchingDedupLabel(summary);
    const dedupLabel = options.dedupLabels?.[labelKey] || labelKey;
    if (!dedupLabel) {
        return '';
    }

    if (!shouldShowImportCheckMatchingDedupSourceCount(summary.dedupType)) {
        return dedupLabel;
    }

    return summary.dedupSourceIds.length > 0
        ? `${dedupLabel} · ${summary.dedupSourceIds.length}`
        : dedupLabel;
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

export function formatInvestmentSignalReason(
    reason: string | undefined,
    reasonLabels: Record<string, string> = DEFAULT_INVESTMENT_REASON_LABELS
): string {
    return (reason || '').split(',').map(part => {
        const trimmedPart = part.trim();
        if (!trimmedPart) {
            return '';
        }

        const separatorIndex = trimmedPart.indexOf(':');
        if (separatorIndex <= 0) {
            return trimmedPart;
        }

        const rawKey = trimmedPart.slice(0, separatorIndex).trim();
        const value = trimmedPart.slice(separatorIndex + 1).trim();
        if (!value) {
            return reasonLabels[rawKey] || rawKey;
        }

        return `${reasonLabels[rawKey] || rawKey}: ${value}`;
    }).filter(part => !!part).join(', ');
}

function buildReviewView(
    status: ImportPreviewSignalStatus | null | undefined,
    title: string | undefined,
    pendingLabelKey: string,
    acceptedLabelKey: string,
    rejectedLabelKey: string,
    pendingActions: ImportPreviewSignalDecision[],
    profileText?: string,
    summary?: string,
    reviewedActions: ImportPreviewSignalDecision[] = [{ decision: 'clear', labelKey: 'Clear', color: 'warning' }],
    detailLines: string[] = []
): ImportPreviewSignalReviewView | null {
    if (!status) {
        return null;
    }

    const titleParts = [title || '', profileText || ''].filter((part, index, parts) => {
        const normalizedPart = part.trim();
        return !!normalizedPart && parts.findIndex(candidate => candidate.trim() === normalizedPart) === index;
    });
    const resolvedTitle = buildSignalTitle(detailLines, titleParts.join(' | '));

    if (status === 'pending') {
        return {
            status,
            labelKey: pendingLabelKey,
            title: resolvedTitle,
            color: 'info',
            profileText,
            summary,
            actions: pendingActions,
            detailLines
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
            actions: reviewedActions,
            detailLines
        };
    }

    return {
        status,
        labelKey: rejectedLabelKey,
        title: resolvedTitle,
        color: 'error',
        profileText,
        summary,
        actions: reviewedActions,
        detailLines
    };
}

export function buildImportPreviewSignalViewModel(
    state: ImportPreviewSignalState,
    options: ImportPreviewSignalViewModelOptions = {}
): ImportPreviewSignalViewModel {
    const matchingSummary = getImportCheckMatchingContextSummary(state);
    const parserDetailLines = buildParserDetailLines(matchingSummary, state, options);
    const transferDetailLines = buildTransferDetailLines(state, options);
    const transferSourceLabelCount = getSourceChainDisplayLabels(state.transferSourceChain, options, state.transferPairOrder).length;
    const shouldHideParser = !!state.transferStatus && transferSourceLabelCount >= 2;
    const parser = matchingSummary.parserId && !shouldHideParser
        ? {
            parserId: matchingSummary.parserId,
            label: getParserDisplayLabel(matchingSummary.parserId, options.parserLabels),
            color: options.parserColors?.[matchingSummary.parserId] || 'grey',
            title: buildSignalTitle(parserDetailLines, getImportCheckMatchingParserTagsText(matchingSummary)),
            detailLines: parserDetailLines
        }
        : null;
    const dedupSourceCount = state.dedupSourceCount || matchingSummary.dedupSourceIds.length;
    const dedupVisible = hasImportCheckMatchingDedupContext(matchingSummary)
        || ((state.dedupSourceLabels || []).length > 0)
        || ((state.dedupSources || []).length > 0)
        || dedupSourceCount > 0;
    const dedupDetailLines = buildDedupDetailLines(matchingSummary, state, options);
    const dedup = dedupVisible
        ? {
            dedupType: matchingSummary.dedupType,
            labelKey: getImportCheckMatchingDedupLabel(matchingSummary),
            label: buildDedupLabel(matchingSummary, dedupSourceCount, options),
            title: buildSignalTitle(dedupDetailLines, getImportCheckMatchingDedupTitle(matchingSummary, options)),
            color: (matchingSummary.dedupType || '').trim().toLowerCase() === 'transfer' ? 'primary' : 'secondary',
            sourceCount: dedupSourceCount,
            detailLines: dedupDetailLines
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
        ],
        undefined,
        undefined,
        [{ decision: 'clear', labelKey: 'Clear', color: 'warning' }],
        transferDetailLines
    );
    const investmentTitle = formatInvestmentSignalReason(state.investmentTitle, options.investmentReasonLabels);
    const investmentDetailLines = dedupeTextItems([
        investmentTitle,
        state.investmentProfileText || ''
    ]);
    const investment = buildReviewView(
        state.investmentStatus,
        investmentTitle,
        'Investment Signal',
        'Accepted',
        'Rejected',
        [],
        state.investmentProfileText,
        undefined,
        [],
        investmentDetailLines
    );
    const learningDetailLines = buildLearningDetailLines(state, options);
    const learning = buildReviewView(
        state.learningStatus,
        buildSignalTitle(learningDetailLines, state.learningTitle),
        'Learning Suggestion',
        'Learning Suggestion Accepted',
        'Learning Suggestion Rejected',
        [
            { decision: 'accept', labelKey: 'Apply Suggestion', color: 'secondary' },
            { decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' }
        ],
        undefined,
        state.learningSummary,
        [],
        learningDetailLines
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
