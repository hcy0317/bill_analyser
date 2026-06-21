import type { ImportCheckMatchingContextSummary, ImportCheckMatchingDedupTitleOptions, ImportPreviewSignalDecision, ImportPreviewSignalReviewView, ImportPreviewSignalState, ImportPreviewSignalStatus, ImportPreviewSignalViewModel, ImportPreviewSignalViewModelOptions, ImportPreviewVisibleSignalFilterValue } from './types.ts';

import { getImportCheckMatchingContextSummary, getImportCheckMatchingDedupLabel, getImportCheckMatchingDedupTitle, getImportCheckMatchingParserTagsText, hasImportCheckMatchingDedupContext } from './context.ts';

import { buildHistoryRewriteSignalView } from './historyRewrite.ts';

import { buildSignalTitle, buildSourceRowLookup, dedupeTextItems, DEFAULT_SOURCE_ROLE_LABELS, formatConfidencePercent, formatInfoLine, getParserContextDisplayLabels, getParserDisplayLabel, getParserIdFromTag, getSignalInfoLabels, getSourceChainDisplayLabels, getSourceContextFromLookupValue, getSourceDisplayLabel, isTransferLikeDedupType, isVisibleDedupType, normalizeDedupType, normalizeLearningAccountRoute, normalizeLearningCategoryPath, normalizeLearningRecommendationLabel, normalizeLLMCategoryPath, sortSourceChain } from './shared.ts';



// 信号视图模型只负责 UI 可见 chip/action/detail 的派生，不直接修改导入预览业务状态。

export function buildParserDetailLines(
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

export function buildTransferDetailLines(
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

export function buildDedupDetailLines(
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

export function buildLearningDetailLines(
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

export function buildLLMDetailLines(
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

export function buildDedupLabel(
    summary: ImportCheckMatchingContextSummary,
    options: ImportCheckMatchingDedupTitleOptions
): string {
    const labelKey = getImportCheckMatchingDedupLabel(summary);
    const dedupLabel = options.dedupLabels?.[labelKey] || labelKey;
    return dedupLabel;
}

export function buildReviewView(
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

// 汇总 parser、dedup、transfer、history、recurring、learning、LLM 和 annotation 状态，生成信号列唯一可见模型。
export function buildImportPreviewSignalViewModel(
    state: ImportPreviewSignalState,
    options: ImportPreviewSignalViewModelOptions = {}
): ImportPreviewSignalViewModel {
    const matchingSummary = getImportCheckMatchingContextSummary(state);
    const parserDetailLines = buildParserDetailLines(matchingSummary, state, options);
    const transferDetailLines = buildTransferDetailLines(state, options);
    const normalizedDedupType = normalizeDedupType(matchingSummary.dedupType);
    const hasTransferSignal = !!state.transferStatus;
    const shouldHideParser = normalizedDedupType === 'platform_bank'
        || hasTransferSignal;
    const parser = matchingSummary.parserId && !shouldHideParser
        ? {
            parserId: matchingSummary.parserId,
            label: getParserDisplayLabel(matchingSummary.parserId, options.parserLabels),
            color: options.parserColors?.[matchingSummary.parserId] || 'grey',
            title: buildSignalTitle(parserDetailLines, getImportCheckMatchingParserTagsText(matchingSummary)),
            detailLines: parserDetailLines
        }
        : null;
    const isTransferDedupWithoutSignal = isTransferLikeDedupType(normalizedDedupType) && !hasTransferSignal;
    const hasMeaningfulDedup = isVisibleDedupType(normalizedDedupType)
        && !isTransferDedupWithoutSignal;
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
            color: isTransferLikeDedupType(matchingSummary.dedupType) && hasTransferSignal ? 'primary' : 'secondary',
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
            { decision: 'accept', labelKey: 'Accept', color: 'warning' },
            { decision: 'reject', labelKey: 'Reject', color: 'error' }
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

// 信号筛选只消费可见模型，避免把普通交易类型、隐藏 dedup 文本或未授权转账误判成可筛选信号。
export function matchesImportPreviewSignalFilter(
    viewModel: ImportPreviewSignalViewModel,
    filter: ImportPreviewVisibleSignalFilterValue | null
): boolean {
    if (filter === null) {
        return true;
    }

    const normalizedDedupType = normalizeDedupType(viewModel.dedup?.dedupType);
    if (filter === 'parser') {
        return !!viewModel.parser
            && normalizedDedupType !== 'platform_bank'
            && !viewModel.transferSuggestion
            && !viewModel.historyRewrite
            && !viewModel.learning
            && !viewModel.llm;
    }

    if (filter === 'platform_duplicate') {
        return normalizedDedupType === 'platform_bank';
    }

    if (filter === 'transfer') {
        return !!viewModel.transferSuggestion;
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

    return false;
}
