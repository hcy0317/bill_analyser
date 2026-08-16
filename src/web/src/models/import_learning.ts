import type {
    ImportPreviewActionScope,
    ImportPreviewPatchPayload
} from './import_preview.ts';

export type ImportLearningSuggestionsRequest = {
    sessionId: string;
} & (
    | { actionScope?: undefined; previewUpdates?: undefined }
    | { actionScope: ImportPreviewActionScope; previewUpdates?: ImportPreviewPatchPayload[] }
);

export interface ImportLearningPromoteRequest {
    sessionId: string;
    actionScope: ImportPreviewActionScope;
    previewUpdates?: ImportPreviewPatchPayload[];
}

export interface ImportLearningActionPayload {
    action_scope: ImportPreviewActionScope;
    preview_updates?: ImportPreviewPatchPayload[];
}

export interface ImportLearningSuggestion {
    matchType: string;
    matchValue: string;
    matchFeatures: Record<string, string>;
    sampleCount: number;
    sourcePreviewIds: number[];
    learnedType: string;
    learnedCategoryId: number | null;
    learnedCategoryName: string;
    learnedSourceAccountId: number | null;
    learnedSourceAccountName: string;
    learnedDestinationAccountId: number | null;
    learnedDestinationAccountName: string;
    summary: string;
    recommendationKey?: string;
    recommendation_key?: string;
    signalState?: string;
    signal_state?: string;
    lifecycleStatus?: string;
    lifecycle_status?: string;
}

export interface ImportLearningSuggestionsResponse {
    sessionId: string;
    totalCount: number;
    suggestions: ImportLearningSuggestion[];
}

export interface ImportLearningPromoteResponse {
    success: boolean;
    session_id: string;
    selected_samples: number;
    rules_total: number;
    created: number;
    updated: number;
}

/**
 * 生成导入学习建议的稳定前端 key，优先使用后端 recommendation_key。
 */
export function getImportLearningSuggestionKey(suggestion: ImportLearningSuggestion): string {
    const stableKey = suggestion.recommendationKey || suggestion.recommendation_key || '';
    if (stableKey.trim()) {
        return stableKey.trim();
    }
    return [
        suggestion.matchType,
        suggestion.matchValue,
        suggestion.sourcePreviewIds.join(',')
    ].join('::');
}

/**
 * 汇总学习建议关联的 preview id，去重后用于批量选择和同步状态。
 */
export function collectImportLearningSuggestionPreviewIds(
    suggestions: ImportLearningSuggestion[]
): number[] {
    const previewIds = new Set<number>();

    for (const suggestion of suggestions) {
        for (const previewId of suggestion.sourcePreviewIds) {
            if (previewId > 0) {
                previewIds.add(previewId);
            }
        }
    }

    return [...previewIds];
}

/**
 * 将学习建议的特征字段转换为可读摘要，供导入预览页展示命中依据。
 */
export function getImportLearningSuggestionFeatureSummary(
    suggestion: ImportLearningSuggestion
): string {
    const labels: Record<string, string> = {
        parser_id: 'parser',
        counterparty: 'counterparty',
        description: 'description',
        payment_method: 'payment_method'
    };

    return Object.entries(suggestion.matchFeatures || {})
        .filter(([, value]) => typeof value === 'string' && value.trim().length > 0)
        .map(([key, value]) => `${labels[key] || key}: ${value}`)
        .join(' · ');
}
