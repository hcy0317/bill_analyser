import type { ImportMatchingSourcePayload } from '@/models/import_matching.ts';

export interface ImportCheckMatchingContextState {
    parserId?: string;
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
    parserId?: string;
    parserTags?: string[];
}

export interface ImportCheckMatchingSourceContext {
    parserId?: string;
    parserTags?: string[];
}

export interface ImportCheckMatchingDedupTitleOptions {
    matchLabel?: string;
    currentParserId?: string;
    parserLabels?: Record<string, string>;
    sourceRows?: ImportCheckMatchingSourceRow[];
    sourceRowLookup?: ReadonlyMap<string, string | ImportCheckMatchingSourceContext>;
    dedupLabels?: Record<string, string>;
    sourceRoleLabels?: Record<string, string>;
    infoLabels?: Partial<ImportPreviewSignalInfoLabels>;
}

export type ImportPreviewSignalStatus = 'pending' | 'accepted' | 'rejected' | 'skipped';
export type ImportPreviewLearningMode = 'green' | 'blue' | '';

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
    historyRewrite: ImportPreviewSignalReviewView | null;
    transferSuggestion: ImportPreviewSignalReviewView | null;
    investment: ImportPreviewSignalReviewView | null;
    learning: ImportPreviewSignalReviewView | null;
    llm: ImportPreviewSignalReviewView | null;
    recurring: ImportPreviewSignalRecurringView | null;
    hasAnySignal: boolean;
}

export type ImportPreviewVisibleSignalFilterValue =
    | 'parser'
    | 'platform_duplicate'
    | 'transfer'
    | 'history'
    | 'learning'
    | 'llm';

export interface ImportPreviewHistoryRewriteAcknowledgementOperation {
    preview_id: number;
    operation_id: string;
    planned_operation: string;
    history_bill_id: number;
    history_bill_version: number;
    acknowledgement_token: string;
}

export interface ImportPreviewHistoryRewriteAcknowledgement {
    acknowledged: true;
    selected_preview_ids: number[];
    operations: ImportPreviewHistoryRewriteAcknowledgementOperation[];
    selection_scope: Record<string, unknown>;
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
    learningMode?: ImportPreviewLearningMode | string;
    learningSignalState?: string;
    learningAutoApplied?: boolean;
    llmStatus?: ImportPreviewSignalStatus | null;
    llmTitle?: string;
    llmSummary?: string;
    llmConfidence?: number;
    llmCategoryPath?: string;
    llmSourceAccount?: string;
    llmDestinationAccount?: string;
    dedupSourceCount?: number;
    dedupSourceLabels?: string[];
    dedupSources?: ImportMatchingSourcePayload[];
    parserIdChain?: ImportMatchingSourcePayload[];
    reconciliationType?: string;
    reconciliationStatus?: string;
    reconciliationTitle?: string;
    reconciliationSourceChain?: ImportMatchingSourcePayload[];
    reconciliationPlannedOperation?: string;
    reconciliationHistoryBillId?: number | string | null;
    reconciliationHistoryBillVersion?: number | string | null;
    reconciliationHistoryRole?: string;
    reconciliationGroupKey?: string;
    reconciliationOperationId?: string;
    reconciliationAcknowledgementToken?: string;
    reconciliationDestructiveAckRequired?: boolean;
    reconciliationNotice?: string;
    hasRecurringMatch?: boolean;
    recurringTitle?: string;
    recurringCandidateCount?: number;
    recurringPrimaryReason?: string;
}

export interface ImportPreviewSignalViewModelOptions extends ImportCheckMatchingDedupTitleOptions {
    parserColors?: Record<string, string>;
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
const HISTORY_REWRITE_NOTICE = '将改写/合并历史账单';
const HISTORY_REWRITE_OPERATION_LABELS: Record<string, string> = {
    update_history: 'History Rewrite',
    merge_transfer_history: 'Merge History Transfer'
};

function humanizeDedupType(rawType: string): string {
    return rawType
        .split('_')
        .filter(part => !!part)
        .map(part => part.charAt(0).toUpperCase() + part.slice(1))
        .join(' ');
}

function normalizeDedupType(rawType: string | undefined): string {
    return (rawType || '').trim().toLowerCase();
}

function normalizeTextValue(value: unknown): string {
    return typeof value === 'string' ? value.trim() : '';
}

function normalizePositiveInteger(value: unknown): number {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return Math.trunc(value);
    }

    if (typeof value === 'string') {
        const parsedValue = Number(value.trim());
        return Number.isFinite(parsedValue) ? Math.trunc(parsedValue) : 0;
    }

    return 0;
}

function isTransferLikeDedupType(rawType: string | undefined): boolean {
    const normalizedDedupType = normalizeDedupType(rawType);
    return normalizedDedupType === 'transfer' || normalizedDedupType === 'transfer_cross_batch';
}

export function isImportPreviewHistoryRewriteOperation(rawOperation: string | undefined): boolean {
    const normalizedOperation = normalizeTextValue(rawOperation).toLowerCase();
    return normalizedOperation === 'update_history' || normalizedOperation === 'merge_transfer_history';
}

export function getImportPreviewHistoryRewriteLabelKey(rawOperation: string | undefined): string {
    const normalizedOperation = normalizeTextValue(rawOperation).toLowerCase();
    return HISTORY_REWRITE_OPERATION_LABELS[normalizedOperation] || 'History Rewrite';
}

export function getImportCheckMatchingContextSummary(
    state: ImportCheckMatchingContextState
): ImportCheckMatchingContextSummary {
    return {
        parserId: state.parserId || '',
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
    const normalizedDedupType = normalizeDedupType(summary.dedupType);

    if (!normalizedDedupType || normalizedDedupType === 'remaining') {
        return '';
    }

    return MATCHING_DEDUP_LABEL_KEYS[normalizedDedupType] || humanizeDedupType(normalizedDedupType);
}

export function shouldShowImportCheckMatchingDedupSourceCount(dedupType: string | undefined): boolean {
    void dedupType;
    return false;
}

function getParserDisplayLabel(parserId: string, parserLabels?: Record<string, string>): string {
    return parserLabels?.[parserId] || parserId;
}

function getParserIdFromTag(tag: string): string {
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
        return { parserId: value };
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

function buildSourceRowLookup(
    options: ImportCheckMatchingDedupTitleOptions
): ReadonlyMap<string, string | ImportCheckMatchingSourceContext> | null {
    const sourceRows = options.sourceRows || [];
    return options.sourceRowLookup || (sourceRows.length > 0
        ? new Map(
            sourceRows
                .map(sourceRow => [
                    String(sourceRow.id),
                    {
                        parserId: (sourceRow.parserId || '').trim(),
                        parserTags: sourceRow.parserTags || []
                    }
                ] as const)
                .filter(([, sourceContext]) => !!sourceContext.parserId || sourceContext.parserTags.length > 0)
        )
        : null);
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

function normalizeLearningRecommendationLabel(label: string): string {
    const normalizedLabel = label.trim();
    if (!normalizedLabel) {
        return '';
    }

    return normalizedLabel
        .replace(/\s+Category$/i, '')
        .replace(/分类$/, '');
}

function normalizeLearningCategoryPath(categoryPath: string): string {
    return categoryPath.trim().replace(/\s*\/\s*/g, '-');
}

function normalizeLLMCategoryPath(categoryPath: string): string {
    return normalizeLearningCategoryPath(categoryPath);
}

function isPlaceholderLearningAccount(account: string): boolean {
    return account.trim() === '' || account.trim() === '-';
}

function normalizeLearningAccountRoute(accountRoute: string): string {
    const normalizedRoute = accountRoute.trim().replace(/\s*→\s*/g, '→');
    if (!normalizedRoute.includes('→')) {
        return normalizedRoute;
    }

    const routeParts = normalizedRoute.split('→').map(part => part.trim());
    const visibleRouteParts = routeParts.filter(part => !isPlaceholderLearningAccount(part));
    if (visibleRouteParts.length === 1) {
        return visibleRouteParts[0] || '';
    }

    return routeParts.map(part => (isPlaceholderLearningAccount(part) ? '-' : part)).join('→');
}

function formatConfidencePercent(confidence: number | undefined): string {
    const numericConfidence = typeof confidence === 'number' && Number.isFinite(confidence)
        ? confidence
        : 0;
    if (numericConfidence <= 0) {
        return '';
    }

    return `${Math.round(numericConfidence * 100)}%`;
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

function getParserContextDisplayLabels(
    context: ImportCheckMatchingSourceContext,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    return dedupeTextItems([
        context.parserId || '',
        ...(context.parserTags || []).map(tag => getParserIdFromTag(tag))
    ].map(parserId => getParserDisplayLabel(parserId, options.parserLabels)));
}

function buildParserDetailLines(
    summary: ImportCheckMatchingContextSummary,
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const infoLabels = getSignalInfoLabels(options);
    const structuredLabels = getSourceChainDisplayLabels(state.parserIdChain, options);
    if (structuredLabels.length > 0) {
        return [formatInfoLine(infoLabels.sourceLabel, structuredLabels.join(' · '))];
    }

    const parserIds = dedupeTextItems([
        summary.parserId,
        ...summary.parserTags.map(tag => getParserIdFromTag(tag))
    ]).map(parserId => getParserDisplayLabel(parserId, options.parserLabels));

    if (parserIds.length > 0) {
        return [formatInfoLine(infoLabels.sourceLabel, parserIds.join(' · '))];
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
        const sourceRowLookup = buildSourceRowLookup(options);
        const sourceLabels = dedupeTextItems([
            ...getSourceChainDisplayLabels(state.parserIdChain, options),
            ...getParserContextDisplayLabels({ parserId: summary.parserId, parserTags: summary.parserTags }, options),
            ...(state.dedupSourceLabels || []),
            ...getSourceChainDisplayLabels(state.dedupSources, options),
            ...summary.dedupSourceIds.flatMap(sourceId => getParserContextDisplayLabels(
                getSourceContextFromLookupValue(sourceRowLookup?.get(String(sourceId))),
                options
            ))
        ]);

        if (sourceLabels.length > 0) {
            return [formatInfoLine(infoLabels.duplicateSourcesLabel, sourceLabels.join('|'))];
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

    if (summaryParts.length > 0) {
        const normalizedSummaryParts = summaryParts.map((part, index) => {
            if (index === 1) {
                return normalizeLearningCategoryPath(part);
            }
            if (index >= 2) {
                return normalizeLearningAccountRoute(part);
            }
            return part;
        }).filter(part => !!part);
        const recommendationLabel = normalizeLearningRecommendationLabel(infoLabels.recommendedCategoryLabel)
            || infoLabels.recommendedCategoryLabel;
        return [formatInfoLine(recommendationLabel, normalizedSummaryParts.join('|'))];
    }

    return state.learningTitle ? [state.learningTitle] : [];
}

function buildLLMDetailLines(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const infoLabels = getSignalInfoLabels(options);
    const detailLines: string[] = [];
    const llmCategoryPath = normalizeLLMCategoryPath(state.llmCategoryPath || '');
    if (llmCategoryPath) {
        const recommendationLabel = normalizeLearningRecommendationLabel(infoLabels.recommendedCategoryLabel)
            || infoLabels.recommendedCategoryLabel;
        detailLines.push(formatInfoLine(recommendationLabel, llmCategoryPath));
    }

    const llmAccountRoute = normalizeLearningAccountRoute(
        [state.llmSourceAccount || '', state.llmDestinationAccount || ''].filter(part => part !== '').join('→')
    );
    if (llmAccountRoute) {
        detailLines.push(formatInfoLine(infoLabels.accountRouteLabel, llmAccountRoute));
    }

    const llmConfidence = formatConfidencePercent(state.llmConfidence);
    if (llmConfidence) {
        detailLines.push(formatInfoLine('Confidence', llmConfidence));
    }

    if (detailLines.length > 0) {
        return detailLines;
    }

    return state.llmTitle ? [state.llmTitle] : [];
}

function buildHistoryRewriteDetailLines(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    const lines: string[] = [];
    const notice = normalizeTextValue(state.reconciliationNotice) || HISTORY_REWRITE_NOTICE;
    const title = normalizeTextValue(state.reconciliationTitle);
    const plannedOperation = normalizeTextValue(state.reconciliationPlannedOperation);
    const historyBillId = normalizePositiveInteger(state.reconciliationHistoryBillId);
    const historyBillVersion = normalizePositiveInteger(state.reconciliationHistoryBillVersion);
    const sourceLabels = getSourceChainDisplayLabels(state.reconciliationSourceChain, options);

    lines.push(title || notice);
    if (plannedOperation) {
        lines.push(formatInfoLine('Operation', plannedOperation));
    }
    if (historyBillId > 0) {
        lines.push(formatInfoLine(
            'History Bill',
            historyBillVersion > 0 ? `#${historyBillId} v${historyBillVersion}` : `#${historyBillId}`
        ));
    }
    if (sourceLabels.length > 0) {
        lines.push(formatInfoLine(getSignalInfoLabels(options).sourceLabel, sourceLabels.join('|')));
    }
    if (state.reconciliationDestructiveAckRequired) {
        lines.push(notice);
    }

    return dedupeTextItems(lines);
}

function buildHistoryRewriteSignalView(
    state: ImportPreviewSignalState,
    options: ImportCheckMatchingDedupTitleOptions
): ImportPreviewSignalReviewView | null {
    const plannedOperation = normalizeTextValue(state.reconciliationPlannedOperation);
    const hasHistoryRewriteMarker = isImportPreviewHistoryRewriteOperation(plannedOperation)
        || !!state.reconciliationDestructiveAckRequired;
    if (!hasHistoryRewriteMarker) {
        return null;
    }

    const detailLines = buildHistoryRewriteDetailLines(state, options);
    const labelKey = getImportPreviewHistoryRewriteLabelKey(plannedOperation);
    return {
        status: 'pending',
        labelKey,
        title: buildSignalTitle(detailLines, state.reconciliationTitle || HISTORY_REWRITE_NOTICE),
        color: 'warning',
        actions: [],
        detailLines
    };
}

export function buildImportPreviewHistoryRewriteOperationAcknowledgement(
    previewId: number | string | null | undefined,
    state: ImportPreviewSignalState
): ImportPreviewHistoryRewriteAcknowledgementOperation | null {
    const normalizedPreviewId = normalizePositiveInteger(previewId);
    const plannedOperation = normalizeTextValue(state.reconciliationPlannedOperation);
    const operationId = normalizeTextValue(state.reconciliationOperationId);
    const acknowledgementToken = normalizeTextValue(state.reconciliationAcknowledgementToken);
    const historyBillId = normalizePositiveInteger(state.reconciliationHistoryBillId);
    const historyBillVersion = normalizePositiveInteger(state.reconciliationHistoryBillVersion) || 1;

    if (
        normalizedPreviewId <= 0
        || !isImportPreviewHistoryRewriteOperation(plannedOperation)
        || !operationId
        || !acknowledgementToken
        || historyBillId <= 0
    ) {
        return null;
    }

    return {
        preview_id: normalizedPreviewId,
        operation_id: operationId,
        planned_operation: plannedOperation,
        history_bill_id: historyBillId,
        history_bill_version: historyBillVersion,
        acknowledgement_token: acknowledgementToken
    };
}

export function buildImportPreviewHistoryRewriteAcknowledgement({
    selectedPreviewIds,
    operations,
    selectionScope
}: {
    selectedPreviewIds: Array<number | string | null | undefined>;
    operations: ImportPreviewHistoryRewriteAcknowledgementOperation[];
    selectionScope: Record<string, unknown>;
}): ImportPreviewHistoryRewriteAcknowledgement | null {
    const normalizedSelectedIds = selectedPreviewIds
        .map(previewId => normalizePositiveInteger(previewId))
        .filter(previewId => previewId > 0)
        .sort((left, right) => left - right)
        .filter((previewId, index, ids) => index === 0 || ids[index - 1] !== previewId);
    const uniqueOperations = new Map<number, ImportPreviewHistoryRewriteAcknowledgementOperation>();
    for (const operation of operations) {
        if (operation.preview_id > 0) {
            uniqueOperations.set(operation.preview_id, operation);
        }
    }

    if (uniqueOperations.size < 1) {
        return null;
    }

    return {
        acknowledged: true,
        selected_preview_ids: normalizedSelectedIds,
        operations: Array.from(uniqueOperations.values()).sort((left, right) => left.preview_id - right.preview_id),
        selection_scope: {
            ...selectionScope,
            selected_count: normalizedSelectedIds.length,
            history_rewrite_count: uniqueOperations.size
        }
    };
}

function buildSignalTitle(detailLines: string[], fallbackTitle: string | undefined = ''): string {
    return detailLines.length > 0 ? detailLines.join(' | ') : (fallbackTitle || '');
}

function buildDedupLabel(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions
): string {
    const labelKey = getImportCheckMatchingDedupLabel(summary);
    const dedupLabel = options.dedupLabels?.[labelKey] || labelKey;
    return dedupLabel;
}

export function resolveImportCheckMatchingTransferParserIds(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions = {}
): string[] {
    const sourceRowLookup = buildSourceRowLookup(options);
    if (!isTransferLikeDedupType(summary.dedupType)) {
        return [];
    }

    const parserIds: string[] = [];
    const addParserId = (parserId: string | undefined): void => {
        const normalizedParserId = (parserId || '').trim();
        if (normalizedParserId && !parserIds.includes(normalizedParserId)) {
            parserIds.push(normalizedParserId);
        }
    };
    const addParserIdsFromTags = (parserTags: string[] | undefined): void => {
        for (const tag of parserTags || []) {
            addParserId(getParserIdFromTag(tag));
        }
    };

    addParserId(options.currentParserId || summary.parserId);
    addParserIdsFromTags(summary.parserTags);

    for (const sourceId of summary.dedupSourceIds) {
        const sourceContext = getSourceContextFromLookupValue(sourceRowLookup?.get(String(sourceId)));
        addParserId(sourceContext.parserId);
        addParserIdsFromTags(sourceContext.parserTags);
    }

    return parserIds;
}

export function getImportCheckMatchingDedupTitle(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions = {}
): string {
    const parserIds = resolveImportCheckMatchingTransferParserIds(summary, options);
    if (parserIds.length > 0) {
        return [
            options.matchLabel || 'Matching',
            ...parserIds.map(parserId => getParserDisplayLabel(parserId, options.parserLabels))
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
    detailLines: string[] = [],
    pendingColor: string = 'info',
    skippedLabelKey: string = 'Suggestion Skipped'
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
            color: pendingColor,
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

    if (status === 'skipped') {
        return {
            status,
            labelKey: skippedLabelKey,
            title: resolvedTitle,
            color: 'warning',
            profileText,
            summary,
            actions: [],
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
    const normalizedDedupType = normalizeDedupType(matchingSummary.dedupType);
    const shouldHideParser = normalizedDedupType === 'platform_bank'
        || isTransferLikeDedupType(normalizedDedupType)
        || !!state.transferStatus;
    const parser = matchingSummary.parserId && !shouldHideParser
        ? {
            parserId: matchingSummary.parserId,
            label: getParserDisplayLabel(matchingSummary.parserId, options.parserLabels),
            color: options.parserColors?.[matchingSummary.parserId] || 'grey',
            title: buildSignalTitle(parserDetailLines, getImportCheckMatchingParserTagsText(matchingSummary)),
            detailLines: parserDetailLines
        }
        : null;
    const hasMeaningfulDedup = normalizedDedupType !== '' && normalizedDedupType !== 'remaining';
    const dedupSourceCount = hasMeaningfulDedup
        ? (state.dedupSourceCount || matchingSummary.dedupSourceIds.length)
        : 0;
    const dedupVisible = hasMeaningfulDedup && (
        hasImportCheckMatchingDedupContext(matchingSummary)
        || ((state.dedupSourceLabels || []).length > 0)
        || ((state.dedupSources || []).length > 0)
        || dedupSourceCount > 0
    );
    const dedupDetailLines = buildDedupDetailLines(matchingSummary, state, options);
    const reconciliationTitle = (state.reconciliationTitle || '').trim();
    const historyRewrite = buildHistoryRewriteSignalView(state, options);
    const reconciliationDedup = reconciliationTitle && !historyRewrite
        ? {
            dedupType: state.reconciliationType || 'reconciliation',
            labelKey: state.reconciliationType || 'reconciliation',
            label: reconciliationTitle,
            title: reconciliationTitle,
            color: state.reconciliationStatus === 'rejected' ? 'error' : 'secondary',
            sourceCount: (state.reconciliationSourceChain || []).length,
            detailLines: [reconciliationTitle]
        }
        : null;
    const dedup = dedupVisible
        ? {
            dedupType: matchingSummary.dedupType,
            labelKey: getImportCheckMatchingDedupLabel(matchingSummary),
            label: buildDedupLabel(matchingSummary, options),
            title: buildSignalTitle(dedupDetailLines, getImportCheckMatchingDedupTitle(matchingSummary, options)),
            color: isTransferLikeDedupType(matchingSummary.dedupType) ? 'primary' : 'secondary',
            sourceCount: dedupSourceCount,
            detailLines: dedupDetailLines
        }
        : reconciliationDedup;
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
    const learningDetailLines = buildLearningDetailLines(state, options);
    const normalizedLearningMode = (state.learningMode || '').trim().toLowerCase();
    const normalizedLearningSignalState = (state.learningSignalState || '').trim().toLowerCase();
    const isGreenLearning = normalizedLearningSignalState === 'green'
        || normalizedLearningSignalState === 'auto_applied'
        || normalizedLearningMode === 'green'
        || !!state.learningAutoApplied;
    const learningReviewedActions = isGreenLearning
        ? [{ decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' } as ImportPreviewSignalDecision]
        : [];
    const learning = buildReviewView(
        state.learningStatus,
        buildSignalTitle(learningDetailLines, state.learningTitle),
        isGreenLearning ? 'Learning Auto Apply' : 'Learning Suggestion',
        isGreenLearning ? 'Learning Applied' : 'Learning Suggestion Accepted',
        'Learning Suggestion Rejected',
        [
            { decision: 'accept', labelKey: 'Apply Suggestion', color: isGreenLearning ? 'success' : 'secondary' },
            { decision: 'reject', labelKey: 'Reject Learning Suggestion', color: 'error' }
        ],
        undefined,
        state.learningSummary,
        learningReviewedActions,
        learningDetailLines,
        'warning',
        'Learning Suggestion Skipped'
    );
    if (learning && isGreenLearning) {
        learning.color = 'success';
    }
    const llmDetailLines = buildLLMDetailLines(state, options);
    const llm = buildReviewView(
        state.llmStatus,
        buildSignalTitle(llmDetailLines, state.llmTitle),
        'LLM Suggestion',
        'LLM Suggestion Accepted',
        'LLM Suggestion Rejected',
        [
            { decision: 'accept', labelKey: 'Apply Suggestion', color: 'warning' },
            { decision: 'reject', labelKey: 'Reject LLM Suggestion', color: 'error' }
        ],
        undefined,
        state.llmSummary,
        [],
        llmDetailLines,
        'warning'
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
        historyRewrite,
        transferSuggestion,
        investment: null,
        learning,
        llm,
        recurring,
        hasAnySignal: !!parser
            || !!dedup
            || !!historyRewrite
            || !!transferSuggestion
            || !!learning
            || !!llm
            || !!recurring
    };
}

export function matchesImportPreviewSignalFilter(
    viewModel: ImportPreviewSignalViewModel,
    filter: ImportPreviewVisibleSignalFilterValue | null
): boolean {
    if (filter === null) {
        return true;
    }

    if (filter === 'parser') {
        return !!viewModel.parser;
    }

    const normalizedDedupType = normalizeDedupType(viewModel.dedup?.dedupType);
    if (filter === 'platform_duplicate') {
        return normalizedDedupType === 'platform_bank';
    }

    if (filter === 'transfer') {
        return isTransferLikeDedupType(normalizedDedupType) || !!viewModel.transferSuggestion;
    }

    if (filter === 'history') {
        return !!viewModel.historyRewrite;
    }

    if (filter === 'learning') {
        return !!viewModel.learning;
    }

    if (filter === 'llm') {
        return !!viewModel.llm;
    }

    return true;
}
