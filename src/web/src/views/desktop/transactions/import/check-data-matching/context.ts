import type { ImportCheckMatchingContextState, ImportCheckMatchingContextSummary, ImportCheckMatchingDedupTitleOptions, ImportPreviewTypeColumnViewModel } from './types.ts';

import { buildSourceRowLookup, getParserDisplayLabel, getParserIdFromTag, getSourceContextFromLookupValue, isTransferLikeDedupType, isVisibleDedupType, MATCHING_DEDUP_LABEL_KEYS, normalizeDedupType } from './shared.ts';



// 导入预览匹配上下文负责把 parser/dedup 原始状态归一成表格列和筛选可消费的稳定模型。

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
        || (isVisibleDedupType(summary.dedupType) && summary.dedupSourceIds.length > 0);
}

export function hasImportCheckMatchingDedupContext(summary: ImportCheckMatchingContextSummary): boolean {
    return isVisibleDedupType(summary.dedupType) && summary.dedupSourceIds.length > 0;
}

export function getImportCheckMatchingDedupLabel(summary: ImportCheckMatchingContextSummary): string {
    const normalizedDedupType = normalizeDedupType(summary.dedupType);

    if (!normalizedDedupType || normalizedDedupType === 'remaining') {
        return '';
    }

    return MATCHING_DEDUP_LABEL_KEYS[normalizedDedupType] || '';
}

export function shouldShowImportCheckMatchingDedupSourceCount(dedupType: string | undefined): boolean {
    void dedupType;
    return false;
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
