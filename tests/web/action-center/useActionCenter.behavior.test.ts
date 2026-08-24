import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const getAnomalies = jest.fn<(...args: any[]) => Promise<any>>();
const getRecurringSuggestions = jest.fn<(...args: any[]) => Promise<any>>();
const detectRecurringPatterns = jest.fn<() => Promise<any>>();
const acceptRecurringSuggestion = jest.fn<(...args: any[]) => Promise<any>>();
const rejectRecurringSuggestion = jest.fn<(...args: any[]) => Promise<any>>();

jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getAnomalies,
        getRecurringSuggestions,
        detectRecurringPatterns,
        acceptRecurringSuggestion,
        rejectRecurringSuggestion
    }
}));

import { useActionCenter } from '@/views/base/action-center/useActionCenter.ts';

function response(result: unknown, success = true): any {
    return { data: { success, result } };
}

function recurring(id: number, status = 'pending'): any {
    return {
        id,
        status,
        name: `Suggestion ${id}`,
        counterparty: `Counterparty ${id}`,
        amountCents: 1_000,
        confidenceScore: 0.8
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    getAnomalies.mockResolvedValue(response({
        items: [{ key: 'large_transaction:1', type: 'large_transaction' }],
        totalCount: 1,
        analyzedBills: 10,
        analyzedMonths: 6,
        startDate: '2026-03-01',
        endDate: '2026-08-12'
    }));
    getRecurringSuggestions.mockResolvedValue(response({
        items: [recurring(1), recurring(2, 'accepted')],
        total: 2
    }));
    detectRecurringPatterns.mockResolvedValue(response({ detected: 2, created: 1, updated: 1, skipped: 0 }));
    acceptRecurringSuggestion.mockResolvedValue(response({ suggestionId: 1, recurringId: 9, status: 'accepted' }));
    rejectRecurringSuggestion.mockResolvedValue(response({ suggestionId: 1, status: 'rejected' }));
});

describe('action center shared controller', () => {
    test('loads anomaly and pending recurring work together and updates the summary', async () => {
        const state = useActionCenter();

        expect(await state.load()).toBe(true);
        expect(getAnomalies).toHaveBeenCalledWith({ months: 6 });
        expect(getRecurringSuggestions).toHaveBeenCalledWith({ status: 'pending', limit: 500 });
        expect(state.recurringSuggestions.value.map(item => item.id)).toEqual([1]);
        expect(state.summary.value).toEqual({
            total: 2,
            anomalyCount: 1,
            recurringCount: 1,
            hasWork: true
        });
        expect(state.loading.value).toBe(false);
    });

    test('reports transport and unsuccessful-envelope failures without leaving busy state', async () => {
        const state = useActionCenter();
        getAnomalies.mockRejectedValueOnce(new Error('network down'));
        expect(await state.load()).toBe(false);
        expect(state.error.value).toBe('network down');
        expect(state.loading.value).toBe(false);

        getAnomalies.mockResolvedValueOnce(response(null, false));
        expect(await state.load()).toBe(false);
        expect(state.error.value).toBe('Failed to load action center');
    });

    test('discovers recurring patterns and refreshes both work queues', async () => {
        const state = useActionCenter();

        expect(await state.detectRecurring()).toBe(true);
        expect(detectRecurringPatterns).toHaveBeenCalledTimes(1);
        expect(getAnomalies).toHaveBeenCalledTimes(1);
        expect(state.detectResult.value).toEqual({ detected: 2, created: 1, updated: 1, skipped: 0 });
        expect(state.detecting.value).toBe(false);

        detectRecurringPatterns.mockRejectedValueOnce(new Error('detect unavailable'));
        expect(await state.detectRecurring()).toBe(false);
        expect(state.error.value).toBe('detect unavailable');
    });

    test('accepts or rejects pending suggestions locally and preserves failed items', async () => {
        const state = useActionCenter();
        state.recurringSuggestions.value = [recurring(1), recurring(2)];

        expect(await state.acceptRecurring(1)).toBe(true);
        expect(acceptRecurringSuggestion).toHaveBeenCalledWith({ suggestionId: 1 });
        expect(state.recurringSuggestions.value.map(item => item.id)).toEqual([2]);

        rejectRecurringSuggestion.mockRejectedValueOnce(new Error('mutation failed'));
        expect(await state.rejectRecurring(2)).toBe(false);
        expect(state.recurringSuggestions.value.map(item => item.id)).toEqual([2]);
        expect(state.error.value).toBe('mutation failed');
        expect(state.mutatingSuggestionId.value).toBeNull();

        rejectRecurringSuggestion.mockResolvedValueOnce(response({ status: 'rejected' }));
        expect(await state.rejectRecurring(2)).toBe(true);
        expect(state.recurringSuggestions.value).toEqual([]);
        await actualVue.nextTick();
        expect(state.summary.value.hasWork).toBe(false);
    });
});
