import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import type {
    BatchAcceptResponse,
    GenerateSuggestionsResponse,
    LearningRule,
    LearningSuggestion
} from '@/models/learning_center.ts';

type ServiceMock = jest.Mock<(...args: any[]) => Promise<any>>;

const mockServices = {
    getLearningSuggestions: jest.fn(),
    generateLearningSuggestions: jest.fn(),
    acceptLearningSuggestion: jest.fn(),
    rejectLearningSuggestion: jest.fn(),
    batchAcceptLearningSuggestions: jest.fn(),
    getLearningRules: jest.fn(),
    toggleLearningRule: jest.fn(),
    deleteLearningRule: jest.fn()
} satisfies Record<string, ServiceMock>;

const mockLogger = { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() };

jest.mock('@/lib/services.ts', () => ({ __esModule: true, default: mockServices }));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: mockLogger }));

import { useLearningStore } from '@/stores/learning.ts';

function suggestion(id: number, status = 'pending'): LearningSuggestion {
    return {
        id,
        status,
        matchType: 'merchant',
        matchValue: `merchant-${id}`,
        matchFeaturesJson: '{}',
        suggestedType: 'expense',
        suggestedCategoryId: null,
        suggestedSourceAccountId: null,
        suggestedDestinationAccountId: null,
        sampleCount: 1,
        summary: '',
        createdAt: '',
        updatedAt: ''
    };
}

function rule(id: number, enabled = true): LearningRule {
    return {
        id,
        enabled,
        matchType: 'merchant',
        matchValue: `merchant-${id}`,
        learnedType: 'expense',
        learnedCategoryId: null,
        learnedSourceAccountId: null,
        learnedDestinationAccountId: null,
        appliedCount: 0,
        lastAppliedAt: '',
        matchFeaturesJson: '{}',
        createdAt: '',
        updatedAt: ''
    };
}

function response<T>(result: T, success = true): Promise<{ data: { success: boolean; result: T } }> {
    return Promise.resolve({ data: { success, result } });
}

function invalid(): Promise<{ data: { success: false } }> {
    return Promise.resolve({ data: { success: false } });
}

beforeEach(() => {
    setActivePinia(createPinia());
    jest.clearAllMocks();
});

describe('learning suggestion actions', () => {
    test('loads suggestions and derives pending state', async () => {
        mockServices.getLearningSuggestions.mockReturnValue(response({
            items: [suggestion(1), suggestion(2, 'accepted')],
            total: 7
        }));
        const store = useLearningStore();

        expect(store.pendingCount).toBe(0);
        await store.loadSuggestions('pending');

        expect(mockServices.getLearningSuggestions).toHaveBeenCalledWith({ status: 'pending', limit: 500 });
        expect(store.suggestions.map(item => item.id)).toEqual([1, 2]);
        expect(store.suggestionsTotal).toBe(7);
        expect(store.pendingSuggestions.map(item => item.id)).toEqual([1]);
        expect(store.pendingCount).toBe(1);
        expect(store.suggestionsLoading).toBe(false);
        expect(store.error).toBeNull();
    });

    test('maps invalid and rejected suggestion loads to stable errors', async () => {
        const store = useLearningStore();
        mockServices.getLearningSuggestions.mockReturnValueOnce(invalid());
        await store.loadSuggestions();
        expect(store.error).toBe('Failed to load suggestions');
        expect(store.suggestionsLoading).toBe(false);

        mockServices.getLearningSuggestions.mockRejectedValueOnce(new Error('network down'));
        await store.loadSuggestions();
        expect(store.error).toBe('Error: network down');
        expect(mockLogger.error).toHaveBeenCalledWith('Failed to load learning suggestions', expect.any(Error));
    });

    test('generates suggestions, reloads the list and handles failure paths', async () => {
        const generated: GenerateSuggestionsResponse = { mined: 3, created: 2, updated: 1, skippedExisting: 0 };
        mockServices.generateLearningSuggestions.mockReturnValueOnce(response(generated));
        mockServices.getLearningSuggestions.mockReturnValueOnce(response({ items: [suggestion(1)], total: 1 }));
        const store = useLearningStore();

        await expect(store.generateSuggestions()).resolves.toEqual(generated);
        expect(mockServices.getLearningSuggestions).toHaveBeenCalledWith({ status: undefined, limit: 500 });
        expect(store.suggestionsLoading).toBe(false);

        mockServices.generateLearningSuggestions.mockReturnValueOnce(invalid());
        await expect(store.generateSuggestions()).resolves.toBeNull();
        expect(store.error).toBe('Failed to generate suggestions');

        mockServices.generateLearningSuggestions.mockRejectedValueOnce('offline');
        await expect(store.generateSuggestions()).resolves.toBeNull();
        expect(store.error).toBe('offline');
        expect(mockLogger.error).toHaveBeenCalledWith('Failed to generate learning suggestions', 'offline');
    });

    test('accepts and rejects individual suggestions with local immutable updates', async () => {
        mockServices.getLearningSuggestions.mockReturnValue(response({ items: [suggestion(1), suggestion(2)], total: 2 }));
        const store = useLearningStore();
        await store.loadSuggestions();

        mockServices.acceptLearningSuggestion.mockReturnValueOnce(response(true));
        await expect(store.acceptSuggestion(1)).resolves.toBe(true);
        expect(mockServices.acceptLearningSuggestion).toHaveBeenCalledWith({ suggestionId: 1 });
        expect(store.suggestions.find(item => item.id === 1)?.status).toBe('accepted');

        mockServices.rejectLearningSuggestion.mockReturnValueOnce(response(true));
        await expect(store.rejectSuggestion(2)).resolves.toBe(true);
        expect(mockServices.rejectLearningSuggestion).toHaveBeenCalledWith({ suggestionId: 2 });
        expect(store.suggestions.find(item => item.id === 2)?.status).toBe('rejected');
    });

    test.each([
        ['accept', 'acceptSuggestion', 'acceptLearningSuggestion', 'Failed to accept suggestion'],
        ['reject', 'rejectSuggestion', 'rejectLearningSuggestion', 'Failed to reject suggestion']
    ] as const)('handles invalid and rejected %s responses', async (_name, action, service, message) => {
        const store = useLearningStore();
        mockServices[service].mockReturnValueOnce(invalid());
        await expect(store[action](9)).resolves.toBe(false);
        expect(store.error).toBe(message);

        mockServices[service].mockRejectedValueOnce(new Error(`${_name} failed`));
        await expect(store[action](9)).resolves.toBe(false);
        expect(store.error).toBe(`Error: ${_name} failed`);
    });

    test('batch accepts only acknowledged ids and handles invalid or rejected responses', async () => {
        mockServices.getLearningSuggestions.mockReturnValue(response({ items: [suggestion(1), suggestion(2)], total: 2 }));
        const store = useLearningStore();
        await store.loadSuggestions();
        const result: BatchAcceptResponse = {
            accepted: [{ id: 2, ruleId: 20 }],
            failed: [{ id: 1, error: 'conflict' }],
            acceptedCount: 1,
            failedCount: 1
        };

        mockServices.batchAcceptLearningSuggestions.mockReturnValueOnce(response(result));
        await expect(store.batchAcceptSuggestions([1, 2])).resolves.toEqual(result);
        expect(mockServices.batchAcceptLearningSuggestions).toHaveBeenCalledWith({ suggestionIds: [1, 2] });
        expect(store.suggestions.map(item => item.status)).toEqual(['pending', 'accepted']);
        expect(store.suggestionsLoading).toBe(false);

        mockServices.batchAcceptLearningSuggestions.mockReturnValueOnce(invalid());
        await expect(store.batchAcceptSuggestions([1])).resolves.toBeNull();
        expect(store.error).toBe('Failed to batch accept');

        mockServices.batchAcceptLearningSuggestions.mockRejectedValueOnce('batch offline');
        await expect(store.batchAcceptSuggestions([1])).resolves.toBeNull();
        expect(store.error).toBe('batch offline');
        expect(store.suggestionsLoading).toBe(false);
    });
});

describe('learning rule actions and reset', () => {
    test('loads, toggles and deletes rules while keeping totals non-negative', async () => {
        mockServices.getLearningRules.mockReturnValue(response({ items: [rule(1), rule(2, false)], total: 2 }));
        const store = useLearningStore();
        await store.loadRules(true);

        expect(mockServices.getLearningRules).toHaveBeenCalledWith({ enabledOnly: true, limit: 500 });
        expect(store.rulesTotal).toBe(2);
        expect(store.rulesLoading).toBe(false);

        mockServices.toggleLearningRule.mockReturnValueOnce(response(true));
        await expect(store.toggleRule(2, true)).resolves.toBe(true);
        expect(mockServices.toggleLearningRule).toHaveBeenCalledWith({ ruleId: 2, enabled: true });
        expect(store.rules.find(item => item.id === 2)?.enabled).toBe(true);

        mockServices.deleteLearningRule.mockReturnValueOnce(response(true));
        await expect(store.deleteRule(1)).resolves.toBe(true);
        expect(store.rules.map(item => item.id)).toEqual([2]);
        expect(store.rulesTotal).toBe(1);

        store.rulesTotal = 0;
        mockServices.deleteLearningRule.mockReturnValueOnce(response(true));
        await store.deleteRule(2);
        expect(store.rulesTotal).toBe(0);
    });

    test('maps invalid and rejected rule operations', async () => {
        const store = useLearningStore();
        mockServices.getLearningRules.mockReturnValueOnce(invalid());
        await store.loadRules();
        expect(store.error).toBe('Failed to load rules');

        mockServices.getLearningRules.mockRejectedValueOnce(new Error('rules offline'));
        await store.loadRules();
        expect(store.error).toBe('Error: rules offline');
        expect(store.rulesLoading).toBe(false);

        mockServices.toggleLearningRule.mockReturnValueOnce(invalid());
        await expect(store.toggleRule(1, false)).resolves.toBe(false);
        expect(store.error).toBe('Failed to toggle rule');
        mockServices.toggleLearningRule.mockRejectedValueOnce('toggle offline');
        await expect(store.toggleRule(1, false)).resolves.toBe(false);
        expect(store.error).toBe('toggle offline');

        mockServices.deleteLearningRule.mockReturnValueOnce(invalid());
        await expect(store.deleteRule(1)).resolves.toBe(false);
        expect(store.error).toBe('Failed to delete rule');
        mockServices.deleteLearningRule.mockRejectedValueOnce('delete offline');
        await expect(store.deleteRule(1)).resolves.toBe(false);
        expect(store.error).toBe('delete offline');
    });

    test('resets every state field', async () => {
        mockServices.getLearningSuggestions.mockReturnValue(response({ items: [suggestion(1)], total: 1 }));
        mockServices.getLearningRules.mockReturnValue(response({ items: [rule(1)], total: 1 }));
        const store = useLearningStore();
        await store.loadSuggestions();
        await store.loadRules();
        store.error = 'stale';
        store.$reset();

        expect(store.$state).toMatchObject({
            suggestions: [],
            suggestionsTotal: 0,
            suggestionsLoading: false,
            rules: [],
            rulesTotal: 0,
            rulesLoading: false,
            error: null
        });
    });
});
