import type { ImportPreviewSignalStatus } from './checkDataMatching.ts';

export interface LLMMemoryEventItem {
    id?: number | null;
    preview_id?: number | null;
    event_type?: string | null;
    decision?: string | null;
    llm_response_raw?: string | null;
    suggested_type?: string | null;
    suggested_category_id?: number | null;
    suggested_main_category?: string | null;
    suggested_sub_category?: string | null;
    suggested_source_account?: string | null;
    suggested_destination_account?: string | null;
    confidence?: number | null;
}

export interface LLMSignalMemoryState {
    reviewStatus: ImportPreviewSignalStatus | '';
    suppressed: boolean;
    tombstone?: boolean;
    suggestedType?: string;
    suggestedCategoryId?: number;
    suggestedMainCategory: string;
    suggestedSubCategory: string;
    suggestedSourceAccount: string;
    suggestedDestinationAccount: string;
    confidence: number;
    reason: string;
}

/**
 * 将单条 LLM memory event 解析为导入预览信号状态，兼容 llm_response_raw 内的建议字段。
 */
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
    if (reviewStatus === 'clear') {
        return {
            reviewStatus: '',
            suppressed: false,
            tombstone: true,
            suggestedMainCategory: '',
            suggestedSubCategory: '',
            suggestedSourceAccount: '',
            suggestedDestinationAccount: '',
            confidence: 0,
            reason: ''
        };
    }
    const normalizedReviewStatus: ImportPreviewSignalStatus | '' = reviewStatus === 'accept'
        ? 'accepted'
        : reviewStatus === 'reject'
            ? 'rejected'
            : '';

    const suggestedType = String(event.suggested_type || rawSuggestion['suggested_type'] || '');
    const suggestedCategoryId = Number(event.suggested_category_id || rawSuggestion['suggested_category_id'] || 0);
    const signal: LLMSignalMemoryState = {
        reviewStatus: normalizedReviewStatus || 'pending',
        suppressed: normalizedReviewStatus === 'rejected',
        suggestedMainCategory: String(event.suggested_main_category || rawSuggestion['suggested_main_category'] || ''),
        suggestedSubCategory: String(event.suggested_sub_category || rawSuggestion['suggested_sub_category'] || ''),
        suggestedSourceAccount: String(event.suggested_source_account || rawSuggestion['suggested_source_account'] || ''),
        suggestedDestinationAccount: String(event.suggested_destination_account || rawSuggestion['suggested_destination_account'] || ''),
        confidence: Number(event.confidence || rawSuggestion['confidence'] || 0),
        reason: String(rawSuggestion['reason'] || '')
    };
    if (suggestedType) {
        signal.suggestedType = suggestedType;
    }
    if (suggestedCategoryId > 0) {
        signal.suggestedCategoryId = suggestedCategoryId;
    }
    return signal;
}

/**
 * 按 preview_id 构建 LLM 信号记忆表。API 固定按 created_at DESC, id DESC
 * 返回，因此每行首条事件是唯一权威最新状态，clear 也不得被更旧终态复活。
 */
export function buildLLMSignalMemoryMap(events: LLMMemoryEventItem[]): Map<number, LLMSignalMemoryState> {
    const nextMemoryMap = new Map<number, LLMSignalMemoryState>();

    for (const event of events) {
        const previewId = Number(event.preview_id || 0);
        if (!previewId) {
            continue;
        }

        if (nextMemoryMap.has(previewId)) {
            continue;
        }

        nextMemoryMap.set(previewId, parseLLMMemoryEventSignal(event)!);
    }

    return nextMemoryMap;
}
