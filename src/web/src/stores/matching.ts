import { ref, computed } from 'vue';
import { defineStore } from 'pinia';

import type {
    BillMatchingPairDetail
} from '@/models/bill_matching.ts';
import { normalizeMatchingPairsResponse } from '@/models/bill_matching.ts';

import services from '@/lib/services.ts';
import logger from '@/lib/logger.ts';

export const useMatchingStore = defineStore('matching', () => {
    const pairs = ref<BillMatchingPairDetail[]>([]);
    const loading = ref(false);
    const error = ref<string | null>(null);
    const pairTypeFilter = ref<string | undefined>(undefined);

    const pairCount = computed(() => pairs.value.length);

    const pairsByType = computed(() => {
        const grouped: Record<string, BillMatchingPairDetail[]> = {};
        for (const pair of pairs.value) {
            const key = pair.pairType || 'unknown';
            if (!grouped[key]) {
                grouped[key] = [];
            }
            grouped[key].push(pair);
        }
        return grouped;
    });

    async function loadPairs(pairType?: string): Promise<void> {
        loading.value = true;
        error.value = null;
        try {
            const response = await services.getMatchingPairs({ pairType });
            const apiData = response.data;
            if (apiData?.success && apiData.result) {
                const normalized = normalizeMatchingPairsResponse(apiData.result);
                pairs.value = normalized.pairs;
                pairTypeFilter.value = pairType;
            } else {
                error.value = 'Failed to load pairs';
            }
        } catch (err) {
            logger.error('Failed to load matching pairs', err);
            error.value = String(err);
        } finally {
            loading.value = false;
        }
    }

    async function deletePair(pairId: number): Promise<boolean> {
        try {
            const response = await services.deleteMatchingPair({ pairId });
            if (response.data?.success) {
                pairs.value = pairs.value.filter(p => p.id !== pairId);
                return true;
            }
            error.value = 'Failed to delete pair';
            return false;
        } catch (err) {
            logger.error('Failed to delete matching pair', err);
            error.value = String(err);
            return false;
        }
    }

    async function createManualPair(
        billId: number,
        candidateBillId: number,
        pairType?: string
    ): Promise<boolean> {
        try {
            const response = await services.createManualPair({ billId, candidateBillId, pairType });
            if (response.data?.success) {
                await loadPairs(pairTypeFilter.value);
                return true;
            }
            error.value = 'Failed to create pair';
            return false;
        } catch (err) {
            logger.error('Failed to create manual pair', err);
            error.value = String(err);
            return false;
        }
    }

    function $reset(): void {
        pairs.value = [];
        loading.value = false;
        error.value = null;
        pairTypeFilter.value = undefined;
    }

    return {
        pairs,
        loading,
        error,
        pairTypeFilter,
        pairCount,
        pairsByType,
        loadPairs,
        deletePair,
        createManualPair,
        $reset
    };
});
