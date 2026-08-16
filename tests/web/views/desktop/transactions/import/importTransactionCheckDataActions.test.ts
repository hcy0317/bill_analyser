import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockFetch = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockShowMessage = jest.fn();
const mockLoggerError = jest.fn();
const mockLoadAllCategories = jest.fn();
const mockLoadAllAccounts = jest.fn();
const mockLlmPreviewRecommend = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockAnalyzeLLMTransactions = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetImportLearningSuggestions = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockPromoteImportLearning = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockReviewImportTransferDecision = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockUpdateImportPreviewRecurringMatch = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetImportPreviewRowVersionConflict = jest.fn<(...args: Array<any>) => any>();
const mockPatchImportPreviewSelection = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetImportPreviewSelectionConflict = jest.fn<(...args: Array<any>) => any>();

const mockExpenseChild = {
    id: '8',
    parentId: '7',
    name: 'Cafe',
    type: 3,
    icon: 'food',
    color: '#ffaa00',
    hidden: false,
    subCategories: []
};
const mockExpenseParent = {
    id: '7',
    parentId: '0',
    name: 'Food',
    type: 3,
    icon: 'food',
    color: '#ffaa00',
    hidden: false,
    subCategories: [mockExpenseChild]
};
const mockHiddenCategory = {
    id: '9',
    parentId: '0',
    name: 'Hidden',
    type: 3,
    icon: 'food',
    color: '#999999',
    hidden: true,
    subCategories: []
};
const mockWallet = {
    id: 'wallet',
    name: 'Wallet',
    currency: 'CNY',
    category: 1,
    icon: 'wallet',
    color: '#0088ff',
    hidden: false
};
const mockSavings = {
    id: 'savings',
    name: 'Savings',
    currency: 'CNY',
    category: 2,
    icon: 'bank',
    color: '#00aa66',
    hidden: false
};
const mockHiddenAccount = {
    id: 'hidden',
    name: 'Hidden wallet',
    currency: 'CNY',
    category: 1,
    icon: 'wallet',
    color: '#999999',
    hidden: true
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null)
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => params ? `${key}:${JSON.stringify(params)}` : key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown) => String(value ?? ''),
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown, currency: string) => `${currency}:${value}`,
        getCategorizedAccountsWithDisplayBalance: (accounts: any[]) => [
            { category: 1, accounts: accounts.filter(account => account.category === 1) },
            { category: 2, accounts: accounts.filter(account => account.category === 2) }
        ]
    })
}));
jest.mock('@/stores/setting.ts', () => ({
    useSettingsStore: () => ({ appSettings: { showAccountBalance: false, timeZone: 'Asia/Shanghai' } })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        currentUserFirstDayOfWeek: 1,
        currentUserFiscalYearStart: 1,
        currentUserDefaultCurrency: 'CNY',
        currentUserCoordinateDisplayType: 0,
        currentUserCashTransferCategoryId: ''
    })
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allPlainAccounts: [mockWallet, mockSavings, mockHiddenAccount],
        allVisiblePlainAccounts: [mockWallet, mockSavings],
        allAccountsMap: { wallet: mockWallet, savings: mockSavings, hidden: mockHiddenAccount },
        allAccounts: [mockWallet, mockSavings, mockHiddenAccount],
        loadAllAccounts: mockLoadAllAccounts
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: { 3: [mockExpenseParent, mockHiddenCategory] },
        allTransactionCategoriesMap: { '7': mockExpenseParent, '8': mockExpenseChild, '9': mockHiddenCategory },
        loadAllCategories: mockLoadAllCategories
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({ allTransactionTags: [], allTransactionTagsMap: {} })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getLLMMemoryEvents: mockGetLLMMemoryEvents,
        llmPreviewRecommend: mockLlmPreviewRecommend,
        analyzeLLMTransactions: mockAnalyzeLLMTransactions,
        getImportLearningSuggestions: mockGetImportLearningSuggestions,
        promoteImportLearning: mockPromoteImportLearning,
        reviewImportTransferDecision: mockReviewImportTransferDecision,
        updateImportPreviewRecurringMatch: mockUpdateImportPreviewRecurringMatch,
        getImportPreviewRowVersionConflict: mockGetImportPreviewRowVersionConflict,
        patchImportPreviewSelection: mockPatchImportPreviewSelection,
        getImportPreviewSelectionConflict: mockGetImportPreviewSelectionConflict
    }
}));
jest.mock('@/lib/server_settings.ts', () => ({ isTransactionFromAIImageRecognitionEnabled: () => false }));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => 'action-token' }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: mockLoggerError }
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    showLoading: jest.fn(),
    hideLoading: jest.fn(),
    useI18nUIComponents: () => ({
        showAlert: jest.fn(),
        showConfirm: jest.fn(),
        showToast: jest.fn(),
        routeBackOnError: jest.fn()
    })
}));
jest.mock('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataMenus.ts', () => ({
    useImportCheckDataMenus: () => ({ filterMenus: [], toolMenus: [] })
}));
jest.mock('@/views/desktop/transactions/import/check-data-tab/useImportCheckDataBatchActions.ts', () => ({
    useImportCheckDataBatchActions: () => ({
        clearSelectedRecurringMatches: jest.fn(),
        convertTransactionType: jest.fn(),
        showBatchAddDialog: jest.fn(),
        showBatchCreateInvalidItemDialog: jest.fn(),
        showBatchReplaceDialog: jest.fn(),
        showReplaceAllTypesDialog: jest.fn(),
        showReplaceInvalidItemDialog: jest.fn()
    })
}));

for (const componentPath of [
    '@/components/desktop/PaginationButtons.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/import/tabs/ImportPreviewSignalCell.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceAllTypesDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchCreateDialog.vue',
    '@/views/desktop/transactions/import/dialogs/ImportLearningSuggestionDialog.vue',
    '@/views/desktop/categories/list/dialogs/EditDialog.vue',
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue',
    '@/views/desktop/transactions/list/dialogs/EditDialog.vue'
]) {
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'ActionMatrixStub' } }));
}

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

function createTransaction(id: number, options: Record<string, unknown> = {}): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: 3,
        categoryId: '8',
        originalCategoryName: 'Cafe',
        time: 1_788_480_000 + id,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        originalSourceAccountName: 'Wallet',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        originalDestinationAccountName: '',
        originalDestinationAccountCurrency: 'CNY',
        sourceAmountCents: -1_000 - id,
        destinationAmountCents: 1_000 + id,
        tagIds: [],
        originalTagNames: [],
        comment: `row-${id}`,
        counterparty: `merchant-${id}`,
        paymentMethod: 'card',
        selected: true,
        matching: {
            parser: { id: 'fixture', tags: [] },
            transfer: { review_status: '', reviewed_type: '', suppressed: false },
            learning: {
                review_status: 'pending',
                rule_id: 'rule-1',
                score: 0.91,
                level: 'high',
                reason: 'merchant rule',
                recommended_type: 'expense',
                summary: 'Food/Cafe',
                source: 'model',
                model_version: 'v1'
            },
            llm: { review_status: 'pending', reason: 'evidence', confidence: 0.8 },
            annotation: {}
        },
        ...options
    } as never, id);
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    return transaction;
}

function createBindings(transactions: ImportTransaction[], sessionId = 'action-session'): any {
    const bindings = (ImportTransactionCheckDataTab as any).setup({
        importTransactions: transactions,
        sessionId,
        serverPaged: false,
        totalImportTransactionCount: transactions.length,
        previewMetadata: null
    }, { emit: jest.fn(), expose: jest.fn() });
    bindings.snackbar.value = { showMessage: mockShowMessage };
    return bindings;
}

function createServerPagedBindings(
    transactions: ImportTransaction[],
    metadata: Record<string, unknown>
): any {
    const bindings = (ImportTransactionCheckDataTab as any).setup({
        importTransactions: transactions,
        sessionId: 'action-session',
        serverPaged: true,
        totalImportTransactionCount: 3,
        previewMetadata: metadata
    }, { emit: jest.fn(), expose: jest.fn() });
    bindings.snackbar.value = { showMessage: mockShowMessage };
    return bindings;
}

async function flushPromises(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
}

function recurringPreview(id: number, recurringId: number | null): Record<string, unknown> {
    return {
        id,
        preview_type: 'expense',
        preview_category_id: '8',
        preview_source_account_id: 'wallet',
        preview_destination_account_id: null,
        preview_amount_cents: -1000,
        preview_destination_amount_cents: 0,
        preview_recurring_id: recurringId,
        preview_recurring_name: recurringId ? 'Monthly coffee' : '',
        preview_recurring_candidate_count: recurringId ? 2 : 0,
        preview_recurring_match_score: recurringId ? 0.92 : 0,
        preview_recurring_match_reasons: recurringId ? 'amount|date' : '',
        preview_recurring_matched_date: recurringId ? '2026-07-10' : '',
        matching: { parser: { id: 'fixture', tags: [] }, annotation: {} }
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    mockLlmPreviewRecommend.mockResolvedValue({ data: { result: { suggestions: [] } } });
    mockAnalyzeLLMTransactions.mockResolvedValue({ data: { result: { candidates_created: 0 } } });
    mockGetImportLearningSuggestions.mockResolvedValue({ data: { result: { suggestions: [] } } });
    mockPromoteImportLearning.mockResolvedValue({ data: { result: { rules_total: 0 } } });
    mockUpdateImportPreviewRecurringMatch.mockReset();
    mockGetImportPreviewRowVersionConflict.mockReturnValue(null);
    mockPatchImportPreviewSelection.mockResolvedValue({
        data: {
            result: {
                updated: 0,
                metadata: {
                    counts: { total: 3, selected: 0, selected_total: 0, selected_invalid: 0 },
                    facets: {},
                    selection_hash: 'fnv1a32:811c9dc5'
                }
            }
        }
    });
    mockGetImportPreviewSelectionConflict.mockReturnValue(null);
    Object.defineProperty(globalThis, 'fetch', { configurable: true, writable: true, value: mockFetch });
});

describe('desktop import category, account, and decision baselines', () => {
    test('category/account helpers preserve valid identities and reject unavailable choices', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(101);
            const bindings = createBindings([transaction]);

            expect(bindings.getCategoriesForType(3)).toEqual([mockExpenseParent, mockHiddenCategory]);
            expect(bindings.getCategoriesForType(999)).toEqual([]);
            expect(bindings.hasAvailableCategoriesForType(3)).toBe(true);
            expect(bindings.hasAvailableCategoriesForType(2)).toBe(false);
            expect(bindings.getCategoryPrimaryText(transaction)).toBe('Food');
            expect(bindings.getCategorySecondaryText(transaction)).toBe('Cafe');
            expect(bindings.requiresDestinationAccount(transaction)).toBe(false);
            transaction.type = 5 as never;
            expect(bindings.requiresDestinationAccount(transaction)).toBe(true);
            expect(bindings.getDestinationAccountTitle(transaction)).toBe('Investment Account');
            transaction.type = 4 as never;
            expect(bindings.getDestinationAccountTitle(transaction)).toBe('Destination Account');

            bindings.setTransactionCategoryFromId(transaction, '9');
            expect(transaction).toMatchObject({ categoryId: '', actualCategoryName: '', originalCategoryName: '' });
            bindings.setTransactionCategoryFromId(transaction, '8');
            expect(transaction).toMatchObject({ categoryId: '8', actualCategoryName: 'Cafe' });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('material draft changes clear recurring state while a no-op baseline remains stable', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(102, {
                recurringTemplateId: '17',
                recurringTemplateName: 'Monthly coffee',
                recurringCandidateCount: 2,
                recurringMatchScore: 0.92,
                recurringMatchReasons: 'amount|date',
                recurringMatchedDate: '2026-07-10'
            });
            const bindings = createBindings([transaction]);

            bindings.syncLearningDecisionBaseline(transaction);
            expect(bindings.hasLearningDecisionTextDraftChanges(transaction)).toBe(false);
            expect(bindings.shouldBlockLearningDecisionOnSync(transaction)).toBe(false);

            bindings.onTransactionDataDraftChange(transaction);
            expect((transaction.matching as any).learning.review_status).toBe('pending');

            transaction.categoryId = '';
            expect(bindings.shouldBlockLearningDecisionOnSync(transaction)).toBe(true);
            bindings.onTransactionTypeChange(transaction);
            expect(transaction).toMatchObject({ categoryId: '', recurringTemplateId: '', recurringTemplateName: '' });

            (transaction.matching as any).learning.review_status = 'accepted';
            bindings.clearLearningRecommendationState(transaction);
            expect((transaction.matching as any).learning).toMatchObject({
                review_status: '',
                lifecycle_status: '',
                signal_state: '',
                suppressed: false,
                score: 0
            });
            expect(bindings.shouldBlockLearningDecisionOnSync(transaction)).toBe(false);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('shows the transfer decision server error once without nested fallback prefixes', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(150);
            const bindings = createBindings([transaction]);
            bindings.syncTransferDecisionBaseline(transaction);
            mockReviewImportTransferDecision.mockRejectedValueOnce(
                new Error('Rust import route runtime DB error')
            );

            await bindings.reviewTransferSuggestion(transaction, 'accept');

            expect(mockShowMessage).toHaveBeenLastCalledWith('Rust import route runtime DB error');
            const visibleMessage = String(mockShowMessage.mock.calls.at(-1)?.[0] ?? '');
            expect(visibleMessage.match(/Transfer decision failed/g) ?? []).toHaveLength(0);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('normalizes Error, string, and unknown decision failures', () => {
        const bindings = createBindings([createTransaction(154)]);

        expect(bindings.getImportDecisionErrorMessage(new Error(' runtime failure '), 'fallback'))
            .toBe('runtime failure');
        expect(bindings.getImportDecisionErrorMessage(' string failure ', 'fallback'))
            .toBe('string failure');
        expect(bindings.getImportDecisionErrorMessage({ failure: true }, 'fallback')).toBe('fallback');
    });

    test('flushes partial selection once and sends the same server selection hash to all three actions', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const selected = createTransaction(151, { selected: false });
            const bindings = createServerPagedBindings([selected], {
                counts: { total: 3, selected: 0, selected_total: 0, selected_invalid: 0 },
                facets: {},
                selection_hash: 'fnv1a32:811c9dc5'
            });
            selected.selected = true;
            mockPatchImportPreviewSelection.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 1,
                        metadata: {
                            counts: { total: 3, selected: 1, selected_total: 1, selected_invalid: 0 },
                            facets: {},
                            selection_hash: 'fnv1a32:resolved-selection'
                        }
                    }
                }
            });

            await bindings.applyLLMPreviewRecommendations();
            await bindings.analyzeSelectedPreviewWithLLM();
            await bindings.promoteSelectedToLongTermLearning();

            expect(mockFetch).not.toHaveBeenCalled();
            expect(mockPatchImportPreviewSelection).toHaveBeenCalledTimes(1);
            expect(mockPatchImportPreviewSelection).toHaveBeenCalledWith({
                sessionId: 'action-session',
                expectedSelectionHash: 'fnv1a32:811c9dc5',
                selectedIds: [151],
                deselectedIds: []
            });
            const expectedScope = {
                kind: 'selected',
                selection_hash: 'fnv1a32:resolved-selection'
            };
            for (const service of [
                mockLlmPreviewRecommend,
                mockAnalyzeLLMTransactions,
                mockGetImportLearningSuggestions
            ]) {
                expect(service).toHaveBeenCalledTimes(1);
                expect(service).toHaveBeenCalledWith(expect.objectContaining({
                    sessionId: 'action-session',
                    actionScope: expectedScope,
                    previewUpdates: [expect.objectContaining({ id: 151 })]
                }));
            }
            expect(mockPatchImportPreviewSelection.mock.invocationCallOrder[0]).toBeLessThan(
                mockLlmPreviewRecommend.mock.invocationCallOrder[0]!
            );
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('rebases a typed selection conflict and does not retry the stale patch', async () => {
        const selected = createTransaction(155, { selected: false });
        const bindings = createServerPagedBindings([selected], {
            counts: { total: 1, selected: 0, selected_total: 0, selected_invalid: 0 },
            facets: {},
            selection_hash: 'fnv1a32:11111111'
        });
        selected.selected = true;
        const conflictError = new Error('selection conflict');
        mockPatchImportPreviewSelection.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewSelectionConflict.mockReturnValueOnce({
            expected_selection_hash: 'fnv1a32:11111111',
            actual_selection_hash: 'fnv1a32:22222222',
            metadata: {
                counts: { total: 1, selected: 0, selected_total: 0, selected_invalid: 0 },
                facets: {},
                selection_hash: 'fnv1a32:22222222'
            },
            previewItems: [{ id: 155, preview_selected: false }]
        });

        await expect(bindings.flushPreviewSelectionAndBuildActionScope())
            .rejects.toThrow('Preview selection changed, please review before retrying');

        expect(selected.selected).toBe(false);
        expect(bindings.previewMetadata.value.selection_hash).toBe('fnv1a32:22222222');
        expect(mockPatchImportPreviewSelection).toHaveBeenCalledTimes(1);
        await expect(bindings.flushPreviewSelectionAndBuildActionScope()).resolves.toEqual(expect.any(Object));
        expect(mockPatchImportPreviewSelection).toHaveBeenCalledTimes(1);
    });

    test('keeps selected scope when the active filter hides every selected row', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const visibleUnselected = createTransaction(152, { selected: false });
            const bindings = createServerPagedBindings([visibleUnselected], {
                counts: { total: 1, selected: 0, selected_total: 1, selected_invalid: 0 },
                facets: {},
                selection_hash: 'fnv1a32:hidden-selected-row'
            });

            await bindings.applyLLMPreviewRecommendations();

            expect(mockFetch).not.toHaveBeenCalled();
            expect(mockLlmPreviewRecommend).toHaveBeenCalledWith({
                sessionId: 'action-session',
                previewUpdates: [],
                actionScope: {
                    kind: 'selected',
                    selection_hash: 'fnv1a32:hidden-selected-row'
                }
            });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('does not guess a selected scope from a nonempty hash when selected_total is absent', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const visibleUnselected = createTransaction(153, { selected: false });
            const bindings = createServerPagedBindings([visibleUnselected], {
                counts: { total: 1, selected: 0, selected_invalid: 0 } as never,
                facets: {},
                selection_hash: 'fnv1a32:untrusted-without-count'
            });

            await bindings.applyLLMPreviewRecommendations();

            expect(mockFetch).not.toHaveBeenCalled();
            expect(mockLlmPreviewRecommend).toHaveBeenCalledWith(expect.objectContaining({
                sessionId: 'action-session',
                previewUpdates: [],
                actionScope: expect.objectContaining({ kind: 'all_matching' })
            }));
        } finally {
            warnSpy.mockRestore();
        }
    });
});

describe('desktop recurring decision behavior', () => {
    test('guards missing session, editing, and busy decisions without issuing requests', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(201);
            const withoutSession = createBindings([transaction], '');
            await expect(withoutSession.updatePreviewRecurringMatch(transaction, '17')).resolves.toBe(false);
            expect(mockShowMessage).toHaveBeenCalledWith('No session ID available');

            const bindings = createBindings([transaction]);
            bindings.editingTransaction.value = transaction;
            await expect(bindings.updatePreviewRecurringMatch(transaction, '17')).resolves.toBe(false);
            expect(mockShowMessage).toHaveBeenCalledWith('Please sync manual preview edits before updating scheduled matches');

            bindings.editingTransaction.value = null;
            bindings.recurringDecisionLoadingIds.value = [201];
            await expect(bindings.updatePreviewRecurringMatch(transaction, '17')).resolves.toBe(false);
            expect(mockFetch).not.toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('PUT and DELETE recurring decisions sync the authoritative preview and release busy state', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(202);
            const bindings = createBindings([transaction]);
            mockUpdateImportPreviewRecurringMatch.mockResolvedValueOnce({
                data: { result: { sessionId: 'action-session', previewItem: recurringPreview(202, 17) } }
            });

            await expect(bindings.updatePreviewRecurringMatch(transaction, '17')).resolves.toBe(true);
            expect(mockUpdateImportPreviewRecurringMatch).toHaveBeenLastCalledWith({
                previewId: 202,
                recurringId: 17,
                expectedState: expect.any(Object)
            });
            expect(transaction).toMatchObject({
                recurringTemplateId: '17',
                recurringTemplateName: 'Monthly coffee',
                recurringCandidateCount: 2,
                recurringMatchScore: 0.92
            });
            expect(bindings.recurringDecisionLoadingIds.value).toEqual([]);
            expect(mockShowMessage).toHaveBeenCalledWith('Scheduled Match');

            mockUpdateImportPreviewRecurringMatch.mockResolvedValueOnce({
                data: { result: { sessionId: 'action-session', preview: [recurringPreview(202, null)] } }
            });
            await bindings.clearRecurringMatch(transaction);
            expect(mockUpdateImportPreviewRecurringMatch).toHaveBeenLastCalledWith({
                previewId: 202,
                recurringId: null,
                expectedState: expect.any(Object)
            });
            expect(transaction).toMatchObject({ recurringTemplateId: '', recurringCandidateCount: 0, recurringMatchScore: 0 });
            expect(mockShowMessage).toHaveBeenLastCalledWith('Clear Scheduled Match');
        } finally {
            warnSpy.mockRestore();
        }
    });

    test.each([
        [{ reject: { response: { status: 409, data: { error: 'stale recurring state' } } } }, 'stale recurring state'],
        [{ reject: { response: { status: 500, data: {} } } }, 'Internal Server Error'],
        [{ result: { sessionId: 'other-session' } }, 'Recurring match response is out of date'],
        [{ result: { sessionId: 'action-session' } }, 'Recurring match response missing preview item']
    ])('reports safe recurring failures and always clears loading state', async (outcome, message) => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(203);
            const bindings = createBindings([transaction]);
            if ('reject' in outcome) {
                mockUpdateImportPreviewRecurringMatch.mockRejectedValueOnce(outcome.reject);
            } else {
                mockUpdateImportPreviewRecurringMatch.mockResolvedValueOnce({ data: { result: outcome.result } });
            }

            await expect(bindings.updatePreviewRecurringMatch(transaction, '17')).resolves.toBe(false);
            expect(mockShowMessage).toHaveBeenLastCalledWith(message);
            expect(bindings.recurringDecisionLoadingIds.value).toEqual([]);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('rebases a typed recurring row conflict without retrying the mutation', async () => {
        const transaction = createTransaction(207);
        (transaction as ImportTransaction & { _rowVersion?: number })._rowVersion = 8;
        const bindings = createBindings([transaction]);
        const conflictError = new Error('Preview row changed, please refresh');
        const latestPreview = {
            ...recurringPreview(207, 23),
            row_version: 9,
            preview_description: 'newer server description'
        };
        mockUpdateImportPreviewRecurringMatch.mockRejectedValueOnce(conflictError);
        mockGetImportPreviewRowVersionConflict.mockReturnValueOnce({
            expected_row_version: 8,
            actual_row_version: 9,
            previewItem: latestPreview
        });

        await expect(bindings.updatePreviewRecurringMatch(transaction, 17)).resolves.toBe(false);

        expect(mockUpdateImportPreviewRecurringMatch).toHaveBeenCalledTimes(1);
        expect(mockUpdateImportPreviewRecurringMatch).toHaveBeenCalledWith({
            previewId: 207,
            recurringId: 17,
            expectedState: expect.objectContaining({ rowVersion: 8 })
        });
        expect((transaction as ImportTransaction & { _rowVersion?: number })._rowVersion).toBe(9);
        expect(transaction.recurringTemplateId).toBe('23');
        expect(transaction.comment).toBe('newer server description');
    });

    test('candidate dialog selects the linked/best candidate and supports apply and clear', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(204);
            const bindings = createBindings([transaction]);
            const candidates = [
                { id: 17, name: 'Best', matchReasons: ['amount', 'date'], matchedOccurrenceDate: '2026-07-10' },
                { id: 18, name: 'Other', matchReasons: [], matchedOccurrenceDate: '' }
            ];
            mockFetch.mockResolvedValueOnce({
                ok: true,
                json: async () => ({ success: true, result: { candidates, linkedRecurringId: 18 } })
            });

            await bindings.openRecurringCandidateDialog(transaction);
            expect(bindings.showRecurringCandidateDialog.value).toBe(true);
            expect(bindings.selectedRecurringCandidateId.value).toBe('18');
            expect(bindings.isBestRecurringCandidate(candidates[0])).toBe(true);
            expect(bindings.isBestRecurringCandidate(candidates[1])).toBe(false);
            expect(bindings.getRecurringCandidatePrimaryReason(candidates[0])).toBe('amount');
            expect(bindings.getRecurringCandidatePrimaryReason(candidates[1])).toBe('');
            expect(bindings.formatRecurringCandidateSubtitle(candidates[0])).toContain('Matched Date: 2026-07-10');

            bindings.selectedRecurringCandidateId.value = '17';
            mockUpdateImportPreviewRecurringMatch.mockResolvedValueOnce({
                data: { result: { sessionId: 'action-session', previewItem: recurringPreview(204, 17) } }
            });
            await bindings.applySelectedRecurringCandidate();
            expect(bindings.showRecurringCandidateDialog.value).toBe(false);
            expect(transaction.recurringTemplateId).toBe('17');

            bindings.recurringCandidateTarget.value = transaction;
            bindings.showRecurringCandidateDialog.value = true;
            mockUpdateImportPreviewRecurringMatch.mockResolvedValueOnce({
                data: { result: { sessionId: 'action-session', previewItem: recurringPreview(204, null) } }
            });
            await bindings.clearRecurringMatchFromDialog();
            expect(bindings.showRecurringCandidateDialog.value).toBe(false);
            expect(transaction.recurringTemplateId).toBe('');
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('candidate dialog guards unavailable rows and closes on empty/failing responses', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const noPreview = createTransaction(205);
            delete (noPreview as any)._previewId;
            const bindings = createBindings([noPreview]);
            await bindings.openRecurringCandidateDialog(noPreview);
            expect(mockShowMessage).toHaveBeenLastCalledWith('No preview ID available');

            const transaction = createTransaction(206);
            const withPreview = createBindings([transaction]);
            mockFetch.mockResolvedValueOnce({ ok: true, json: async () => ({ success: true, result: { candidates: [] } }) });
            await withPreview.openRecurringCandidateDialog(transaction);
            expect(withPreview.recurringCandidates.value).toEqual([]);
            expect(withPreview.selectedRecurringCandidateId.value).toBe('');
            expect(withPreview.isBestRecurringCandidate({ id: 1 })).toBe(false);
            await withPreview.applySelectedRecurringCandidate();

            mockFetch.mockResolvedValueOnce({ ok: false, status: 503, text: async () => 'offline' });
            await withPreview.openRecurringCandidateDialog(transaction);
            expect(withPreview.showRecurringCandidateDialog.value).toBe(false);
            expect(mockShowMessage).toHaveBeenLastCalledWith('Load Scheduled Candidates Failed');
        } finally {
            warnSpy.mockRestore();
        }
    });
});

describe('desktop created identities, managed dialogs, and batch actions', () => {
    test('created categories/accounts apply only known active identities to the requested target', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(301, { categoryId: '', sourceAccountId: '', destinationAccountId: '' });
            const bindings = createBindings([transaction]);

            bindings.applyCreatedCategoryToTransaction(transaction, undefined);
            bindings.applyCreatedCategoryToTransaction(transaction, { ...mockExpenseChild, id: 'missing' });
            expect(transaction.categoryId).toBe('');
            bindings.applyCreatedCategoryToTransaction(transaction, mockExpenseChild);
            expect(transaction).toMatchObject({ categoryId: '8', actualCategoryName: 'Cafe', isManuallyAnnotated: true });

            bindings.applyCreatedAccountToTransaction(transaction, 'source', undefined);
            bindings.applyCreatedAccountToTransaction(transaction, 'source', mockHiddenAccount);
            expect(transaction.sourceAccountId).toBe('');
            bindings.applyCreatedAccountToTransaction(transaction, 'source', mockWallet);
            bindings.applyCreatedAccountToTransaction(transaction, 'destination', mockSavings);
            expect(transaction).toMatchObject({
                sourceAccountId: 'wallet',
                actualSourceAccountName: 'Wallet',
                destinationAccountId: 'savings',
                actualDestinationAccountName: 'Savings'
            });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('quick-create dialogs enforce type/parent guards and forward category/account defaults', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(302, { categoryId: '' });
            const bindings = createBindings([transaction]);
            const categoryOpen = jest.fn<(...args: any[]) => Promise<any>>()
                .mockResolvedValueOnce({ category: mockExpenseChild })
                .mockResolvedValueOnce({ category: mockExpenseChild });
            const accountOpen = jest.fn<(...args: any[]) => Promise<any>>()
                .mockResolvedValueOnce({ account: mockWallet })
                .mockResolvedValueOnce({ account: mockSavings });
            bindings.categoryEditDialog.value = { open: categoryOpen };
            bindings.accountEditDialog.value = { open: accountOpen };

            await bindings.quickCreatePrimaryCategory(transaction);
            expect(categoryOpen).toHaveBeenCalledWith({ parentId: '0', type: 3 });
            expect(transaction.categoryId).toBe('8');

            await bindings.quickCreateSecondaryCategory(transaction, undefined);
            expect(categoryOpen).toHaveBeenCalledTimes(1);
            await bindings.quickCreateSecondaryCategory(transaction, mockExpenseParent);
            expect(categoryOpen).toHaveBeenLastCalledWith({ parentId: '7', type: 3, color: '#ffaa00', icon: 'food' });

            await bindings.quickCreateAccount(transaction, 'source', { category: '1' });
            expect(accountOpen).toHaveBeenLastCalledWith({ category: 1 });
            await bindings.quickCreateAccount(transaction, 'destination', { category: 'invalid' });
            expect(accountOpen).toHaveBeenLastCalledWith(undefined);
            expect(transaction).toMatchObject({ sourceAccountId: 'wallet', destinationAccountId: 'savings' });

            const unsupported = createTransaction(303, { type: 1, categoryId: '' });
            await bindings.quickCreatePrimaryCategory(unsupported);
            await bindings.quickCreateSecondaryCategory(unsupported, mockExpenseParent);
            expect(categoryOpen).toHaveBeenCalledTimes(2);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('quick-create failures log only unprocessed dialog errors', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const transaction = createTransaction(304);
            const bindings = createBindings([transaction]);
            bindings.categoryEditDialog.value = {
                open: jest.fn<(...args: any[]) => Promise<any>>()
                    .mockRejectedValueOnce({ processed: false })
                    .mockRejectedValueOnce({ processed: true })
            };
            bindings.accountEditDialog.value = {
                open: jest.fn<(...args: any[]) => Promise<any>>().mockRejectedValueOnce({ processed: false })
            };

            await bindings.quickCreatePrimaryCategory(transaction);
            await bindings.quickCreateSecondaryCategory(transaction, mockExpenseParent);
            await bindings.quickCreateAccount(transaction, 'source', { category: 1 });
            expect(mockLoggerError).toHaveBeenCalledTimes(2);
            expect(mockLoggerError.mock.calls.map(call => call[0])).toEqual(expect.arrayContaining([
                expect.stringContaining('[快捷新建分类]'),
                expect.stringContaining('[快捷新建账户]')
            ]));
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('managed create/edit dialogs update selection and refresh the owning store', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const bindings = createBindings([createTransaction(305)]);
            const categoryOpen = jest.fn<(...args: any[]) => Promise<any>>()
                .mockResolvedValueOnce({ category: { id: '7' } })
                .mockResolvedValueOnce({ category: { id: '8' } })
                .mockResolvedValueOnce({})
                .mockRejectedValueOnce({ processed: false });
            const accountOpen = jest.fn<(...args: any[]) => Promise<any>>()
                .mockResolvedValueOnce({ account: { id: 'savings' } })
                .mockResolvedValueOnce({})
                .mockRejectedValueOnce({ processed: false });
            bindings.categoryEditDialog.value = { open: categoryOpen };
            bindings.accountEditDialog.value = { open: accountOpen };

            bindings.openCategoryManagement();
            expect(bindings.showCategorySelectDialog.value).toBe(true);
            bindings.openManagedPrimaryCategoryCreateDialog();
            await flushPromises();
            expect(categoryOpen).toHaveBeenCalledWith({ parentId: '0', type: 3 });
            expect(bindings.manageCategoryId.value).toBe('7');

            bindings.openManagedSecondaryCategoryCreateDialog();
            await flushPromises();
            expect(categoryOpen).toHaveBeenLastCalledWith({ parentId: '7', type: 3, color: '#ffaa00', icon: 'food' });
            expect(bindings.manageCategoryId.value).toBe('8');
            expect(mockLoadAllCategories).toHaveBeenCalledTimes(2);

            bindings.openSelectedCategoryEditDialog();
            await flushPromises();
            expect(bindings.showCategorySelectDialog.value).toBe(false);
            expect(categoryOpen).toHaveBeenLastCalledWith({ id: '8', type: 3, currentCategory: mockExpenseChild });

            bindings.openAccountManagement();
            bindings.manageAccountId.value = 'wallet';
            bindings.openManagedAccountCreateDialog();
            await flushPromises();
            expect(accountOpen).toHaveBeenCalledWith({ category: 1 });
            expect(bindings.manageAccountId.value).toBe('savings');
            bindings.openSelectedAccountEditDialog();
            await flushPromises();
            expect(bindings.showAccountSelectDialog.value).toBe(false);
            expect(accountOpen).toHaveBeenLastCalledWith({ id: 'savings', currentAccount: mockSavings });
            expect(mockLoadAllAccounts).toHaveBeenCalledTimes(2);

            bindings.manageCategoryId.value = '';
            bindings.openManagedSecondaryCategoryCreateDialog();
            bindings.openSelectedCategoryEditDialog();
            bindings.manageAccountId.value = '';
            bindings.openSelectedAccountEditDialog();
            expect(categoryOpen).toHaveBeenCalledTimes(3);
            expect(accountOpen).toHaveBeenCalledTimes(2);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('batch actions reject empty/unknown combinations and mutate only selected rows', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const selected = createTransaction(306, { categoryId: '', sourceAccountId: '', selected: true });
            const unselected = createTransaction(307, { categoryId: '', sourceAccountId: '', selected: false });
            const bindings = createBindings([selected, unselected]);

            bindings.applyBatchCategory();
            bindings.batchCategoryId.value = 'missing';
            bindings.applyBatchCategory();
            expect(selected.categoryId).toBe('');
            expect(mockShowMessage).not.toHaveBeenCalled();

            bindings.batchCategoryId.value = '8';
            bindings.batchCategoryType.value = 3;
            bindings.applyBatchCategory();
            expect(selected).toMatchObject({ categoryId: '8', type: 3, isManuallyAnnotated: true });
            expect(unselected.categoryId).toBe('');
            expect(mockShowMessage).toHaveBeenLastCalledWith('format.misc.youHaveUpdatedTransactions', { count: '1' });

            mockShowMessage.mockClear();
            bindings.batchAccountId.value = 'hidden';
            bindings.applyBatchAccount();
            expect(selected.sourceAccountId).toBe('');
            expect(mockShowMessage).not.toHaveBeenCalled();

            bindings.batchAccountId.value = 'wallet';
            bindings.applyBatchAccount();
            expect(selected).toMatchObject({ sourceAccountId: 'wallet', actualSourceAccountName: 'Wallet' });
            expect(unselected.sourceAccountId).toBe('');
            expect(mockShowMessage).toHaveBeenCalledWith('format.misc.youHaveUpdatedTransactions', { count: '1' });
        } finally {
            warnSpy.mockRestore();
        }
    });
});
