import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import type {
    RecurringSuggestion,
    RecurringDetectResponse,
} from '@/models/recurring_suggestion.ts';

import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';

export const useRecurringStore = defineStore('recurring', () => {
    const suggestions = ref<RecurringSuggestion[]>([]);
    const suggestionsTotal = ref(0);
    const loading = ref(false);
    const error = ref<string | null>(null);

    const pendingSuggestions = computed(() =>
        suggestions.value.filter(s => s.status === 'pending')
    );
    const pendingCount = computed(() => pendingSuggestions.value.length);

    async function loadSuggestions(status?: string): Promise<void> {
        loading.value = true;
        error.value = null;
        try {
            const response = await services.getRecurringSuggestions({ status, limit: 500 });
            const data = response.data;
            if (data?.success && data.result) {
                suggestions.value = data.result.items;
                suggestionsTotal.value = data.result.total;
            } else {
                error.value = 'Failed to load recurring suggestions';
            }
        } catch (err) {
            logger.error('Failed to load recurring suggestions', err);
            error.value = String(err);
        } finally {
            loading.value = false;
        }
    }

    async function detectPatterns(): Promise<RecurringDetectResponse | null> {
        loading.value = true;
        error.value = null;
        try {
            const response = await services.detectRecurringPatterns();
            const data = response.data;
            if (data?.success && data.result) {
                await loadSuggestions();
                return data.result;
            }
            error.value = 'Failed to detect patterns';
            return null;
        } catch (err) {
            logger.error('Failed to detect recurring patterns', err);
            error.value = String(err);
            return null;
        } finally {
            loading.value = false;
        }
    }

    async function acceptSuggestion(id: number): Promise<boolean> {
        try {
            const response = await services.acceptRecurringSuggestion({ suggestionId: id });
            if (response.data?.success) {
                suggestions.value = suggestions.value.map(s =>
                    s.id === id ? { ...s, status: 'accepted' } : s
                );
                return true;
            }
            error.value = 'Failed to accept suggestion';
            return false;
        } catch (err) {
            logger.error('Failed to accept recurring suggestion', err);
            error.value = String(err);
            return false;
        }
    }

    async function rejectSuggestion(id: number): Promise<boolean> {
        try {
            const response = await services.rejectRecurringSuggestion({ suggestionId: id });
            if (response.data?.success) {
                suggestions.value = suggestions.value.map(s =>
                    s.id === id ? { ...s, status: 'rejected' } : s
                );
                return true;
            }
            error.value = 'Failed to reject suggestion';
            return false;
        } catch (err) {
            logger.error('Failed to reject recurring suggestion', err);
            error.value = String(err);
            return false;
        }
    }

    function $reset(): void {
        suggestions.value = [];
        suggestionsTotal.value = 0;
        loading.value = false;
        error.value = null;
    }

    return {
        suggestions,
        suggestionsTotal,
        loading,
        error,
        pendingSuggestions,
        pendingCount,
        loadSuggestions,
        detectPatterns,
        acceptSuggestion,
        rejectSuggestion,
        $reset
    };
});
