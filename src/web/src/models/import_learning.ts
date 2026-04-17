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

export function getImportLearningSuggestionKey(suggestion: ImportLearningSuggestion): string {
    return [
        suggestion.matchType,
        suggestion.matchValue,
        suggestion.sourcePreviewIds.join(',')
    ].join('::');
}

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
