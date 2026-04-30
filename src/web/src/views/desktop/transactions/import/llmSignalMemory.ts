import type { ImportPreviewSignalStatus } from './checkDataMatching.ts';

export interface LLMMemoryEventItem {
    id?: number | null;
    preview_id?: number | null;
    decision?: string | null;
    llm_response_raw?: string | null;
    suggested_main_category?: string | null;
    suggested_sub_category?: string | null;
    suggested_source_account?: string | null;
    suggested_destination_account?: string | null;
    confidence?: number | null;
}

export interface LLMSignalMemoryState {
    reviewStatus: ImportPreviewSignalStatus | '';
    suppressed: boolean;
    suggestedMainCategory: string;
    suggestedSubCategory: string;
    suggestedSourceAccount: string;
    suggestedDestinationAccount: string;
    confidence: number;
    reason: string;
}

function resolveLLMSignalMemoryPriority(signal: LLMSignalMemoryState): number {
    if (signal.reviewStatus === 'accepted' || signal.reviewStatus === 'rejected') {
        return 2;
    }

    if (signal.reviewStatus === 'pending') {
        return 1;
    }

    return 0;
}

export function parseLLMMemoryEventSignal(event: LLMMemoryEventItem): LLMSignalMemoryState | null {
    const previewId = Number(event.preview_id || 0);
    if (!previewId) {
        return null;
    }

    let rawSuggestion: Record<string, unknown> = {};
    try {
        rawSuggestion = event.llm_response_raw ? JSON.parse(event.llm_response_raw) as Record<string, unknown> : {};
    } catch {
        rawSuggestion = {};
    }

    const reviewStatus = String(event.decision || '').trim().toLowerCase();
    const normalizedReviewStatus: ImportPreviewSignalStatus | '' = reviewStatus === 'accept'
        ? 'accepted'
        : reviewStatus === 'reject'
            ? 'rejected'
            : '';

    return {
        reviewStatus: normalizedReviewStatus || 'pending',
        suppressed: normalizedReviewStatus === 'rejected',
        suggestedMainCategory: String(event.suggested_main_category || rawSuggestion['suggested_main_category'] || ''),
        suggestedSubCategory: String(event.suggested_sub_category || rawSuggestion['suggested_sub_category'] || ''),
        suggestedSourceAccount: String(event.suggested_source_account || rawSuggestion['suggested_source_account'] || ''),
        suggestedDestinationAccount: String(event.suggested_destination_account || rawSuggestion['suggested_destination_account'] || ''),
        confidence: Number(event.confidence || rawSuggestion['confidence'] || 0),
        reason: String(rawSuggestion['reason'] || '')
    };
}

export function shouldReplaceLLMSignalMemoryState(
    current: LLMSignalMemoryState,
    next: LLMSignalMemoryState,
): boolean {
    return resolveLLMSignalMemoryPriority(next) > resolveLLMSignalMemoryPriority(current);
}

export function buildLLMSignalMemoryMap(events: LLMMemoryEventItem[]): Map<number, LLMSignalMemoryState> {
    const nextMemoryMap = new Map<number, LLMSignalMemoryState>();

    for (const event of events) {
        const previewId = Number(event.preview_id || 0);
        if (!previewId) {
            continue;
        }

        const signalState = parseLLMMemoryEventSignal(event)!;
        const existing = nextMemoryMap.get(previewId);
        if (!existing || shouldReplaceLLMSignalMemoryState(existing, signalState)) {
            nextMemoryMap.set(previewId, signalState);
        }
    }

    return nextMemoryMap;
}
