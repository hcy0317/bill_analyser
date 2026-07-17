/* eslint-disable @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

const mockServices = {
    getRecurringSuggestions: jest.fn<(...args: any[]) => Promise<any>>(),
    detectRecurringPatterns: jest.fn<(...args: any[]) => Promise<any>>(),
    acceptRecurringSuggestion: jest.fn<(...args: any[]) => Promise<any>>(),
    rejectRecurringSuggestion: jest.fn<(...args: any[]) => Promise<any>>(),
    getMatchingPairs: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteMatchingPair: jest.fn<(...args: any[]) => Promise<any>>(),
    createManualPair: jest.fn<(...args: any[]) => Promise<any>>(),
};
const mockLoggerError = jest.fn();
const mockNormalizePairs = jest.fn((result: any) => result.normalized);

jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: { error: mockLoggerError } }));
jest.mock('@/models/bill_matching.ts', () => ({
    normalizeMatchingPairsResponse: (result: any) => mockNormalizePairs(result),
}));

import { useRecurringStore } from '@/stores/recurring.ts';
import { useMatchingStore } from '@/stores/matching.ts';

const recurringItems = [
    { id: 1, status: 'pending', name: 'Rent' },
    { id: 2, status: 'accepted', name: 'Salary' },
];
const matchingPairs = [
    { id: 10, pairType: 'transfer' },
    { id: 11, pairType: 'transfer' },
    { id: 12, pairType: '' },
];

beforeEach(() => {
    setActivePinia(createPinia());
    jest.clearAllMocks();
    mockServices.getRecurringSuggestions.mockResolvedValue({
        data: { success: true, result: { items: recurringItems, total: 2 } },
    });
    mockServices.detectRecurringPatterns.mockResolvedValue({
        data: { success: true, result: { created: 2, updated: 1 } },
    });
    mockServices.acceptRecurringSuggestion.mockResolvedValue({ data: { success: true } });
    mockServices.rejectRecurringSuggestion.mockResolvedValue({ data: { success: true } });
    mockServices.getMatchingPairs.mockResolvedValue({
        data: { success: true, result: { normalized: { pairs: matchingPairs } } },
    });
    mockServices.deleteMatchingPair.mockResolvedValue({ data: { success: true } });
    mockServices.createManualPair.mockResolvedValue({ data: { success: true } });
});

describe('recurring store production behavior', () => {
    test('loads and projects pending suggestions with optional status', async () => {
        const store = useRecurringStore();
        await store.loadSuggestions('pending');
        expect(mockServices.getRecurringSuggestions).toHaveBeenCalledWith({ status: 'pending', limit: 500 });
        expect(store.suggestions).toEqual(recurringItems);
        expect(store.suggestionsTotal).toBe(2);
        expect(store.pendingSuggestions).toEqual([recurringItems[0]]);
        expect(store.pendingCount).toBe(1);
        expect(store.loading).toBe(false);
        expect(store.error).toBeNull();
    });

    test('maps unsuccessful and rejected suggestion loads and always releases loading', async () => {
        const store = useRecurringStore();
        mockServices.getRecurringSuggestions.mockResolvedValueOnce({ data: { success: false } });
        await store.loadSuggestions();
        expect(store.error).toBe('Failed to load recurring suggestions');
        expect(store.loading).toBe(false);

        mockServices.getRecurringSuggestions.mockRejectedValueOnce(new Error('load recurring failed'));
        await store.loadSuggestions();
        expect(store.error).toContain('load recurring failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to load recurring suggestions', expect.any(Error));
        expect(store.loading).toBe(false);
    });

    test('detects patterns, refreshes suggestions, and returns the result', async () => {
        const store = useRecurringStore();
        const result = await store.detectPatterns();
        expect(result).toEqual({ created: 2, updated: 1 });
        expect(mockServices.getRecurringSuggestions).toHaveBeenCalledWith({ status: undefined, limit: 500 });
        expect(store.loading).toBe(false);
    });

    test('returns null for unsuccessful and rejected pattern detection', async () => {
        const store = useRecurringStore();
        mockServices.detectRecurringPatterns.mockResolvedValueOnce({ data: { success: false } });
        expect(await store.detectPatterns()).toBeNull();
        expect(store.error).toBe('Failed to detect patterns');

        mockServices.detectRecurringPatterns.mockRejectedValueOnce('detect failed');
        expect(await store.detectPatterns()).toBeNull();
        expect(store.error).toBe('detect failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to detect recurring patterns', 'detect failed');
        expect(store.loading).toBe(false);
    });

    test('accepts and rejects matching suggestions while preserving unrelated rows', async () => {
        const store = useRecurringStore();
        store.suggestions = recurringItems as any;
        expect(await store.acceptSuggestion(1)).toBe(true);
        expect(store.suggestions).toEqual([
            expect.objectContaining({ id: 1, status: 'accepted' }), recurringItems[1],
        ]);
        expect(await store.rejectSuggestion(2)).toBe(true);
        expect(store.suggestions[1]).toEqual(expect.objectContaining({ id: 2, status: 'rejected' }));
    });

    test('maps unsuccessful and rejected accept/reject actions', async () => {
        const store = useRecurringStore();
        mockServices.acceptRecurringSuggestion.mockResolvedValueOnce({ data: { success: false } });
        expect(await store.acceptSuggestion(1)).toBe(false);
        expect(store.error).toBe('Failed to accept suggestion');
        mockServices.acceptRecurringSuggestion.mockRejectedValueOnce('accept failed');
        expect(await store.acceptSuggestion(1)).toBe(false);
        expect(store.error).toBe('accept failed');

        mockServices.rejectRecurringSuggestion.mockResolvedValueOnce({ data: { success: false } });
        expect(await store.rejectSuggestion(2)).toBe(false);
        expect(store.error).toBe('Failed to reject suggestion');
        mockServices.rejectRecurringSuggestion.mockRejectedValueOnce('reject failed');
        expect(await store.rejectSuggestion(2)).toBe(false);
        expect(store.error).toBe('reject failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to accept recurring suggestion', 'accept failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to reject recurring suggestion', 'reject failed');
    });

    test('resets all recurring state', () => {
        const store = useRecurringStore();
        store.suggestions = recurringItems as any;
        store.suggestionsTotal = 2;
        store.loading = true;
        store.error = 'old';
        store.$reset();
        expect(store.$state).toEqual({ suggestions: [], suggestionsTotal: 0, loading: false, error: null });
    });
});

describe('matching store production behavior', () => {
    test('loads, normalizes, counts, and groups pairs including unknown types', async () => {
        const store = useMatchingStore();
        await store.loadPairs('transfer');
        expect(mockServices.getMatchingPairs).toHaveBeenCalledWith({ pairType: 'transfer' });
        expect(mockNormalizePairs).toHaveBeenCalled();
        expect(store.pairs).toEqual(matchingPairs);
        expect(store.pairTypeFilter).toBe('transfer');
        expect(store.pairCount).toBe(3);
        expect(store.pairsByType).toEqual({
            transfer: [matchingPairs[0], matchingPairs[1]],
            unknown: [matchingPairs[2]],
        });
        expect(store.loading).toBe(false);
    });

    test('maps unsuccessful and rejected pair loads', async () => {
        const store = useMatchingStore();
        mockServices.getMatchingPairs.mockResolvedValueOnce({ data: { success: false } });
        await store.loadPairs();
        expect(store.error).toBe('Failed to load pairs');
        mockServices.getMatchingPairs.mockRejectedValueOnce(new Error('pair load failed'));
        await store.loadPairs();
        expect(store.error).toContain('pair load failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to load matching pairs', expect.any(Error));
        expect(store.loading).toBe(false);
    });

    test('deletes a pair on success and maps response and request failures', async () => {
        const store = useMatchingStore();
        store.pairs = matchingPairs as any;
        expect(await store.deletePair(11)).toBe(true);
        expect(mockServices.deleteMatchingPair).toHaveBeenCalledWith({ pairId: 11 });
        expect(store.pairs.map(pair => pair.id)).toEqual([10, 12]);

        mockServices.deleteMatchingPair.mockResolvedValueOnce({ data: { success: false } });
        expect(await store.deletePair(10)).toBe(false);
        expect(store.error).toBe('Failed to delete pair');
        mockServices.deleteMatchingPair.mockRejectedValueOnce('delete failed');
        expect(await store.deletePair(10)).toBe(false);
        expect(store.error).toBe('delete failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to delete matching pair', 'delete failed');
    });

    test('creates a manual pair and refreshes the active pair type', async () => {
        const store = useMatchingStore();
        store.pairTypeFilter = 'transfer';
        expect(await store.createManualPair(1, 2, 'duplicate')).toBe(true);
        expect(mockServices.createManualPair).toHaveBeenCalledWith({
            billId: 1, candidateBillId: 2, pairType: 'duplicate',
        });
        expect(mockServices.getMatchingPairs).toHaveBeenCalledWith({ pairType: 'transfer' });
    });

    test('maps unsuccessful and rejected manual-pair creation', async () => {
        const store = useMatchingStore();
        mockServices.createManualPair.mockResolvedValueOnce({ data: { success: false } });
        expect(await store.createManualPair(1, 2)).toBe(false);
        expect(store.error).toBe('Failed to create pair');
        mockServices.createManualPair.mockRejectedValueOnce('create failed');
        expect(await store.createManualPair(1, 2)).toBe(false);
        expect(store.error).toBe('create failed');
        expect(mockLoggerError).toHaveBeenCalledWith('Failed to create manual pair', 'create failed');
    });

    test('resets pairs, filters, loading, and errors', () => {
        const store = useMatchingStore();
        store.pairs = matchingPairs as any;
        store.pairTypeFilter = 'transfer';
        store.loading = true;
        store.error = 'old';
        store.$reset();
        expect(store.$state).toEqual({ pairs: [], loading: false, error: null, pairTypeFilter: undefined });
    });
});
