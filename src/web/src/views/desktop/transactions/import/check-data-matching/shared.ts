import type { ImportMatchingSourcePayload } from '@/models/import_matching.ts';

import type { ImportCheckMatchingDedupTitleOptions, ImportCheckMatchingSourceContext, ImportPreviewSignalInfoLabels } from './types.ts';



// 共享格式化 helper 是信号展示和历史改写确认的公共底座，集中在这里避免视图模块重复散落。

export const MATCHING_DEDUP_LABEL_KEYS: Record<string, string> = {
    transfer: 'Transfer Match',
    platform_bank: 'Platform Duplicate',
    similar: 'Similar Duplicate',
    split: 'Split-Merge Duplicate',
    split_merge: 'Split-Merge Duplicate',
    transfer_cross_batch: 'Cross-Batch Transfer'
};

export const DEFAULT_SOURCE_ROLE_LABELS: Record<string, string> = {
    outgoing: 'Outgoing',
    incoming: 'Incoming',
    debit: 'Outgoing',
    credit: 'Incoming'
};

export const DEFAULT_SIGNAL_INFO_LABELS: ImportPreviewSignalInfoLabels = {
    sourceLabel: 'Source',
    duplicateSourcesLabel: 'Duplicate Sources',
    recommendedCategoryLabel: 'Recommended Category',
    accountRouteLabel: 'Account Route'
};
export const HISTORY_REWRITE_NOTICE = '将改写/合并历史账单';
export const HISTORY_REWRITE_OPERATION_LABELS: Record<string, string> = {
    update_history: 'History Rewrite',
    merge_transfer_history: 'Merge History Transfer'
};

export function normalizeDedupType(rawType: string | undefined): string {
    return (rawType || '').trim().toLowerCase();
}

export function isVisibleDedupType(rawType: string | undefined): boolean {
    const normalizedDedupType = normalizeDedupType(rawType);
    return !!MATCHING_DEDUP_LABEL_KEYS[normalizedDedupType];
}

export function normalizeTextValue(value: unknown): string {
    return typeof value === 'string' ? value.trim() : '';
}

export function normalizePositiveInteger(value: unknown): number {
    if (typeof value === 'number' && Number.isFinite(value)) {
        return Math.trunc(value);
    }

    if (typeof value === 'string') {
        const parsedValue = Number(value.trim());
        return Number.isFinite(parsedValue) ? Math.trunc(parsedValue) : 0;
    }

    return 0;
}

export function isTransferLikeDedupType(rawType: string | undefined): boolean {
    const normalizedDedupType = normalizeDedupType(rawType);
    return normalizedDedupType === 'transfer' || normalizedDedupType === 'transfer_cross_batch';
}

export function getParserDisplayLabel(parserId: string, parserLabels?: Record<string, string>): string {
    return parserLabels?.[parserId] || parserId;
}

export function getParserIdFromTag(tag: string): string {
    const normalizedTag = (tag || '').trim();
    const parserTagPrefix = 'parser:';
    if (!normalizedTag.toLowerCase().startsWith(parserTagPrefix)) {
        return '';
    }

    return normalizedTag.slice(parserTagPrefix.length).trim();
}

export function getSourceContextFromLookupValue(
    value: string | ImportCheckMatchingSourceContext | undefined
): ImportCheckMatchingSourceContext {
    if (typeof value === 'string') {
        return { parserId: value };
    }

    return value || {};
}

export function getSignalInfoLabels(
    options: ImportCheckMatchingDedupTitleOptions
): ImportPreviewSignalInfoLabels {
    return {
        ...DEFAULT_SIGNAL_INFO_LABELS,
        ...(options.infoLabels || {})
    };
}

export function buildSourceRowLookup(
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

export function getSourceDisplayLabel(
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

export function dedupeTextItems(items: string[]): string[] {
    const deduped: string[] = [];
    for (const item of items) {
        const normalizedItem = item.trim();
        if (normalizedItem && !deduped.includes(normalizedItem)) {
            deduped.push(normalizedItem);
        }
    }

    return deduped;
}

export function formatInfoLine(label: string, value: string): string {
    const normalizedLabel = label.trim();
    const separator = /[\u3400-\u9fff]/.test(normalizedLabel) ? '：' : ': ';
    return `${normalizedLabel}${separator}${value}`;
}

export function normalizeLearningRecommendationLabel(label: string): string {
    const normalizedLabel = label.trim();
    if (!normalizedLabel) {
        return '';
    }

    return normalizedLabel
        .replace(/\s+Category$/i, '')
        .replace(/分类$/, '');
}

export function normalizeLearningCategoryPath(categoryPath: string): string {
    return categoryPath.trim().replace(/\s*\/\s*/g, '-');
}

export function normalizeLLMCategoryPath(categoryPath: string): string {
    return normalizeLearningCategoryPath(categoryPath);
}

export function isPlaceholderLearningAccount(account: string): boolean {
    return account.trim() === '' || account.trim() === '-';
}

export function normalizeLearningAccountRoute(accountRoute: string): string {
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

export function formatConfidencePercent(confidence: number | undefined): string {
    const numericConfidence = typeof confidence === 'number' && Number.isFinite(confidence)
        ? confidence
        : 0;
    if (numericConfidence <= 0) {
        return '';
    }

    return `${Math.round(numericConfidence * 100)}%`;
}

export function sortSourceChain(
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

export function getSourceChainDisplayLabels(
    sources: ImportMatchingSourcePayload[] | undefined,
    options: ImportCheckMatchingDedupTitleOptions,
    pairOrder?: string
): string[] {
    return dedupeTextItems(
        sortSourceChain(sources, pairOrder).map(source => getSourceDisplayLabel(source, options.parserLabels))
    );
}

export function getParserContextDisplayLabels(
    context: ImportCheckMatchingSourceContext,
    options: ImportCheckMatchingDedupTitleOptions
): string[] {
    return dedupeTextItems([
        context.parserId || '',
        ...(context.parserTags || []).map(tag => getParserIdFromTag(tag))
    ].map(parserId => getParserDisplayLabel(parserId, options.parserLabels)));
}

export function buildSignalTitle(detailLines: string[], fallbackTitle: string | undefined = ''): string {
    return detailLines.length > 0 ? detailLines.join(' | ') : (fallbackTitle || '');
}
