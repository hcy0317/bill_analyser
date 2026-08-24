import { computed, ref } from 'vue';

import { getApiErrorMessageOrDefault } from '@/lib/api_error.ts';
import services from '@/lib/services.ts';
import type { RecurringDetectResponse, RecurringSuggestion } from '@/models/recurring_suggestion.ts';
import {
    normalizeActionCenterAnomalyResponse,
    summarizeActionCenter
} from './actionCenterModel.ts';

export function useActionCenter() {
    const months = ref(6);
    const loading = ref(false);
    const detecting = ref(false);
    const mutatingSuggestionId = ref<number | null>(null);
    const error = ref<string | null>(null);
    const detectResult = ref<RecurringDetectResponse | null>(null);
    const anomalyData = ref(normalizeActionCenterAnomalyResponse(null));
    const recurringSuggestions = ref<RecurringSuggestion[]>([]);

    const summary = computed(() => summarizeActionCenter({
        anomalyCount: anomalyData.value.items.length,
        recurringCount: recurringSuggestions.value.length
    }));

    async function load(): Promise<boolean> {
        loading.value = true;
        error.value = null;
        try {
            const [anomalyResponse, recurringResponse] = await Promise.all([
                services.getAnomalies({ months: months.value }),
                services.getRecurringSuggestions({ status: 'pending', limit: 500 })
            ]);
            if (!anomalyResponse.data.success || !recurringResponse.data.success) {
                throw new Error('Failed to load action center');
            }
            anomalyData.value = anomalyResponse.data.result
                ?? normalizeActionCenterAnomalyResponse(null);
            recurringSuggestions.value = (recurringResponse.data.result?.items ?? [])
                .filter(item => item.status === 'pending');
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
            recurringSuggestions.value = recurringSuggestions.value.filter(item => item.id !== id);
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
        summary,
        load,
        detectRecurring,
        acceptRecurring: (id: number) => updateRecurring(id, 'accept'),
        rejectRecurring: (id: number) => updateRecurring(id, 'reject')
    };
}
