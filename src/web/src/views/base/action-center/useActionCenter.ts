import { computed, ref } from 'vue';

import { getApiErrorMessageOrDefault } from '@/lib/api_error.ts';
import services from '@/lib/services.ts';
import type { RecurringDetectResponse, RecurringSuggestion } from '@/models/recurring_suggestion.ts';
import {
    normalizeActionCenterAnomalyResponse,
    summarizeActionCenter
} from './actionCenterModel.ts';

const RECURRING_PAGE_LIMIT = 500;
type RecurringListStatus = 'pending' | 'accepted' | 'rejected';

async function loadRecurringSuggestionsByStatus(
    status: RecurringListStatus
): Promise<RecurringSuggestion[]> {
    const suggestions: RecurringSuggestion[] = [];
    let offset = 0;

    while (true) {
        const response = await services.getRecurringSuggestions({
            status,
            limit: RECURRING_PAGE_LIMIT,
            offset
        });
        if (!response.data.success) {
            throw new Error('Failed to load action center');
        }

        const page = response.data.result?.items ?? [];
        suggestions.push(...page.filter(item => item.status === status));
        const total = response.data.result?.total ?? 0;
        offset += page.length;
        if (page.length === 0 || page.length < RECURRING_PAGE_LIMIT || offset >= total) {
            break;
        }
    }

    return suggestions;
}

function compareRecurringSuggestions(
    left: RecurringSuggestion,
    right: RecurringSuggestion
): number {
    return right.confidenceScore - left.confidenceScore
        || String(right.lastOccurrence ?? '').localeCompare(String(left.lastOccurrence ?? ''))
        || right.id - left.id;
}

export function useActionCenter() {
    const months = ref(6);
    const loading = ref(false);
    const detecting = ref(false);
    const mutatingSuggestionId = ref<number | null>(null);
    const error = ref<string | null>(null);
    const detectResult = ref<RecurringDetectResponse | null>(null);
    const anomalyData = ref(normalizeActionCenterAnomalyResponse(null));
    const recurringSuggestions = ref<RecurringSuggestion[]>([]);
    const recurringHistory = ref<RecurringSuggestion[]>([]);

    const summary = computed(() => summarizeActionCenter({
        anomalyCount: anomalyData.value.items.length,
        recurringCount: recurringSuggestions.value.length
    }));

    async function load(): Promise<boolean> {
        loading.value = true;
        error.value = null;
        try {
            const [anomalyResponse, pending, accepted, rejected] = await Promise.all([
                services.getAnomalies({ months: months.value }),
                loadRecurringSuggestionsByStatus('pending'),
                loadRecurringSuggestionsByStatus('accepted'),
                loadRecurringSuggestionsByStatus('rejected')
            ]);
            if (!anomalyResponse.data.success) {
                throw new Error('Failed to load action center');
            }
            anomalyData.value = anomalyResponse.data.result
                ?? normalizeActionCenterAnomalyResponse(null);
            recurringSuggestions.value = pending;
            recurringHistory.value = [...accepted, ...rejected].sort(compareRecurringSuggestions);
            return true;
        } catch (loadError) {
            error.value = getApiErrorMessageOrDefault(loadError, 'Failed to load action center');
            return false;
        } finally {
            loading.value = false;
        }
    }

    async function detectRecurring(): Promise<boolean> {
        detecting.value = true;
        error.value = null;
        try {
            const response = await services.detectRecurringPatterns();
            if (!response.data.success || !response.data.result) {
                throw new Error('Failed to discover recurring patterns');
            }
            detectResult.value = response.data.result;
            await load();
            return true;
        } catch (detectError) {
            error.value = getApiErrorMessageOrDefault(detectError, 'Failed to discover recurring patterns');
            return false;
        } finally {
            detecting.value = false;
        }
    }

    async function updateRecurring(id: number, action: 'accept' | 'reject'): Promise<boolean> {
        mutatingSuggestionId.value = id;
        error.value = null;
        try {
            const response = action === 'accept'
                ? await services.acceptRecurringSuggestion({ suggestionId: id })
                : await services.rejectRecurringSuggestion({ suggestionId: id });
            if (!response.data.success) {
                throw new Error('Failed to update recurring suggestion');
            }
            const reviewedSuggestion = recurringSuggestions.value.find(item => item.id === id);
            recurringSuggestions.value = recurringSuggestions.value.filter(item => item.id !== id);
            if (reviewedSuggestion) {
                recurringHistory.value = [
                    { ...reviewedSuggestion, status: action === 'accept' ? 'accepted' : 'rejected' },
                    ...recurringHistory.value.filter(item => item.id !== id)
                ];
            }
            return true;
        } catch (mutationError) {
            error.value = getApiErrorMessageOrDefault(mutationError, 'Failed to update recurring suggestion');
            return false;
        } finally {
            mutatingSuggestionId.value = null;
        }
    }

    return {
        months,
        loading,
        detecting,
        mutatingSuggestionId,
        error,
        detectResult,
        anomalyData,
        recurringSuggestions,
        recurringHistory,
        summary,
        load,
        detectRecurring,
        acceptRecurring: (id: number) => updateRecurring(id, 'accept'),
        rejectRecurring: (id: number) => updateRecurring(id, 'reject')
    };
}
