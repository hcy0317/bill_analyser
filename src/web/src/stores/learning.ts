import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import type {
    LearningSuggestion,
    LearningRule,
    BatchAcceptResponse,
    GenerateSuggestionsResponse
} from '@/models/learning_center.ts';

import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';

/**
 * 学习中心 Pinia store，集中管理学习建议、学习规则、加载状态和后端动作。
 */
export const useLearningStore = defineStore('learning', () => {
    // ── Suggestions ─────────────────────────
    const suggestions = ref<LearningSuggestion[]>([]);
    const suggestionsTotal = ref(0);
    const suggestionsLoading = ref(false);

    // ── Rules ───────────────────────────────
    const rules = ref<LearningRule[]>([]);
    const rulesTotal = ref(0);
    const rulesLoading = ref(false);

    const error = ref<string | null>(null);

    // ── Computed ─────────────────────────────
    const pendingSuggestions = computed(() =>
        suggestions.value.filter(s => s.status === 'pending')
    );
    const pendingCount = computed(() => pendingSuggestions.value.length);

    // ── Suggestions actions ─────────────────

    async function loadSuggestions(status?: string): Promise<void> {
        suggestionsLoading.value = true;
        error.value = null;
        try {
            const response = await services.getLearningSuggestions({ status, limit: 500 });
            const data = response.data;
            if (data?.success && data.result) {
                suggestions.value = data.result.items;
                suggestionsTotal.value = data.result.total;
            } else {
                error.value = 'Failed to load suggestions';
            }
        } catch (err) {
            logger.error('Failed to load learning suggestions', err);
            error.value = String(err);
        } finally {
            suggestionsLoading.value = false;
        }
    }

    async function generateSuggestions(): Promise<GenerateSuggestionsResponse | null> {
        suggestionsLoading.value = true;
        error.value = null;
        try {
            const response = await services.generateLearningSuggestions();
            const data = response.data;
            if (data?.success && data.result) {
                await loadSuggestions();
                return data.result;
            }
            error.value = 'Failed to generate suggestions';
            return null;
        } catch (err) {
            logger.error('Failed to generate learning suggestions', err);
            error.value = String(err);
            return null;
        } finally {
            suggestionsLoading.value = false;
        }
    }

    async function acceptSuggestion(id: number): Promise<boolean> {
        try {
            const response = await services.acceptLearningSuggestion({ suggestionId: id });
            if (response.data?.success) {
                suggestions.value = suggestions.value.map(s =>
                    s.id === id ? { ...s, status: 'accepted' } : s
                );
                return true;
            }
            error.value = 'Failed to accept suggestion';
            return false;
        } catch (err) {
            logger.error('Failed to accept suggestion', err);
            error.value = String(err);
            return false;
        }
    }

    async function rejectSuggestion(id: number): Promise<boolean> {
        try {
            const response = await services.rejectLearningSuggestion({ suggestionId: id });
            if (response.data?.success) {
                suggestions.value = suggestions.value.map(s =>
                    s.id === id ? { ...s, status: 'rejected' } : s
                );
                return true;
            }
            error.value = 'Failed to reject suggestion';
            return false;
        } catch (err) {
            logger.error('Failed to reject suggestion', err);
            error.value = String(err);
            return false;
        }
    }

    async function batchAcceptSuggestions(ids: number[]): Promise<BatchAcceptResponse | null> {
        suggestionsLoading.value = true;
        error.value = null;
        try {
            const response = await services.batchAcceptLearningSuggestions({ suggestionIds: ids });
            const data = response.data;
            if (data?.success && data.result) {
                const acceptedIds = new Set(data.result.accepted.map(a => a.id));
                suggestions.value = suggestions.value.map(s =>
                    acceptedIds.has(s.id) ? { ...s, status: 'accepted' } : s
                );
                return data.result;
            }
            error.value = 'Failed to batch accept';
            return null;
        } catch (err) {
            logger.error('Failed to batch accept suggestions', err);
            error.value = String(err);
            return null;
        } finally {
            suggestionsLoading.value = false;
        }
    }

    // ── Rules actions ───────────────────────

    async function loadRules(enabledOnly?: boolean): Promise<void> {
        rulesLoading.value = true;
        error.value = null;
        try {
            const response = await services.getLearningRules({ enabledOnly, limit: 500 });
            const data = response.data;
            if (data?.success && data.result) {
                rules.value = data.result.items;
                rulesTotal.value = data.result.total;
            } else {
                error.value = 'Failed to load rules';
            }
        } catch (err) {
            logger.error('Failed to load learning rules', err);
            error.value = String(err);
        } finally {
            rulesLoading.value = false;
        }
    }

    async function toggleRule(ruleId: number, enabled: boolean): Promise<boolean> {
        try {
            const response = await services.toggleLearningRule({ ruleId, enabled });
            if (response.data?.success) {
                rules.value = rules.value.map(r =>
                    r.id === ruleId ? { ...r, enabled } : r
                );
                return true;
            }
            error.value = 'Failed to toggle rule';
            return false;
        } catch (err) {
            logger.error('Failed to toggle learning rule', err);
            error.value = String(err);
            return false;
        }
    }

    async function deleteRule(ruleId: number): Promise<boolean> {
        try {
            const response = await services.deleteLearningRule({ ruleId });
            if (response.data?.success) {
                rules.value = rules.value.filter(r => r.id !== ruleId);
                rulesTotal.value = Math.max(0, rulesTotal.value - 1);
                return true;
            }
            error.value = 'Failed to delete rule';
            return false;
        } catch (err) {
            logger.error('Failed to delete learning rule', err);
            error.value = String(err);
            return false;
        }
    }

    // ── Reset ───────────────────────────────

    function $reset(): void {
        suggestions.value = [];
        suggestionsTotal.value = 0;
        suggestionsLoading.value = false;
        rules.value = [];
        rulesTotal.value = 0;
        rulesLoading.value = false;
        error.value = null;
    }

    return {
        suggestions,
        suggestionsTotal,
        suggestionsLoading,
        rules,
        rulesTotal,
        rulesLoading,
        error,
        pendingSuggestions,
        pendingCount,
        loadSuggestions,
        generateSuggestions,
        acceptSuggestion,
        rejectSuggestion,
        batchAcceptSuggestions,
        loadRules,
        toggleRule,
        deleteRule,
        $reset
    };
});
