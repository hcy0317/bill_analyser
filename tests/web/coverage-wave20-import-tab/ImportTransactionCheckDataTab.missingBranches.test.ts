import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockFetch = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockAcceptMatchingCandidate = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockRejectMatchingCandidate = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockClearMatchingCandidate = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockLlmPreviewRecommendAccept = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockLlmPreviewRecommendReject = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetMatchingSessionCandidates = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockUpdateImportPreviewItem = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockReviewImportTransferDecision = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockReclassifyImportPreview = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetImportPreviewRowVersionConflict = jest.fn<(...args: Array<any>) => any>();
const mockLlmPreviewRecommend = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockAnalyzeLLMTransactions = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockGetImportLearningSuggestions = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockPromoteImportLearning = jest.fn<(...args: Array<any>) => Promise<any>>();
const mockShowMessage = jest.fn();
const mockShowError = jest.fn();
const mockLoggerError = jest.fn();
const mockLoadAllCategories = jest.fn();
const mockLoadAllAccounts = jest.fn();
let mockCurrentToken = '';

const mockExpenseChild = {
    id: '8', parentId: '7', name: 'Cafe', type: 3, icon: 'food', color: '#ffaa00', hidden: false, subCategories: []
};
const mockExpenseParent = {
    id: '7', parentId: '0', name: 'Food', type: 3, icon: 'food', color: '#ffaa00', hidden: false,
    subCategories: [mockExpenseChild]
};
const mockHiddenCategory = {
    id: '9', parentId: '0', name: 'Hidden', type: 3, icon: 'food', color: '#999999', hidden: true, subCategories: []
};
const mockHiddenParentChild = {
    id: '11', parentId: '10', name: 'Hidden child', type: 3, icon: 'food', color: '#999999', hidden: false,
    subCategories: []
};
const mockHiddenParent = {
    id: '10', parentId: '0', name: 'Hidden parent', type: 3, icon: 'food', color: '#999999', hidden: true,
    subCategories: [mockHiddenParentChild]
};
const mockIncomeCategory = {
    id: '20', parentId: '0', name: 'Salary', type: 2, icon: 'salary', color: '#22aa66', hidden: false, subCategories: []
};
const mockTransferCategory = {
    id: '30', parentId: '0', name: 'Move', type: 4, icon: 'transfer', color: '#2288ff', hidden: false, subCategories: []
};
const mockInvestmentCategory = {
    id: '40', parentId: '0', name: 'Fund', type: 5, icon: 'fund', color: '#8855ff', hidden: false, subCategories: []
};
const mockRootlessCategory = {
    id: '12', parentId: '', name: 'Rootless', type: 3, icon: 'food', color: '#ffaa00', hidden: false, subCategories: []
};
const mockCategoryMap: Record<string, any> = {
    '7': mockExpenseParent,
    '8': mockExpenseChild,
    '9': mockHiddenCategory,
    '10': mockHiddenParent,
    '11': mockHiddenParentChild,
    '12': mockRootlessCategory,
    '20': mockIncomeCategory,
    '30': mockTransferCategory,
    '40': mockInvestmentCategory
};

const mockWallet = {
    id: 'wallet', name: 'Wallet', currency: 'CNY', category: 1, icon: 'wallet', color: '#0088ff', hidden: false
};
const mockSavings = {
    id: 'savings', name: 'Savings', currency: 'USD', category: 2, icon: 'bank', color: '#00aa66', hidden: false
};
const mockHiddenAccount = {
    id: 'hidden', name: 'Hidden wallet', currency: 'CNY', category: 1, icon: 'wallet', color: '#999999', hidden: true
};
const mockAccountsMap: Record<string, any> = {
    wallet: mockWallet,
    savings: mockSavings,
    hidden: mockHiddenAccount
};
const mockKnownTag = { id: 'known-tag', name: 'Known Tag', hidden: false };

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return { ...actual, useTemplateRef: () => actual.ref(null) };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, params?: Record<string, unknown>) => params ? `${key}:${JSON.stringify(params)}` : key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown, offset?: unknown) => `DATE:${String(value)}:${String(offset ?? '')}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown, currency: string) => `${currency}:${String(value)}`,
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
        allAccountsMap: mockAccountsMap,
        allAccounts: [mockWallet, mockSavings, mockHiddenAccount],
        loadAllAccounts: mockLoadAllAccounts
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: {
            2: [mockIncomeCategory],
            3: [mockExpenseParent, mockHiddenCategory, mockHiddenParent],
            4: [mockTransferCategory],
            5: [mockInvestmentCategory]
        },
        allTransactionCategoriesMap: mockCategoryMap,
        loadAllCategories: mockLoadAllCategories
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({
        allTransactionTags: [mockKnownTag],
        allTransactionTagsMap: { 'known-tag': mockKnownTag }
    })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getLLMMemoryEvents: mockGetLLMMemoryEvents,
        acceptMatchingCandidate: mockAcceptMatchingCandidate,
        rejectMatchingCandidate: mockRejectMatchingCandidate,
        clearMatchingCandidate: mockClearMatchingCandidate,
        llmPreviewRecommendAccept: mockLlmPreviewRecommendAccept,
        llmPreviewRecommendReject: mockLlmPreviewRecommendReject,
        getMatchingSessionCandidates: mockGetMatchingSessionCandidates,
        updateImportPreviewItem: mockUpdateImportPreviewItem,
        reviewImportTransferDecision: mockReviewImportTransferDecision,
        reclassifyImportPreview: mockReclassifyImportPreview,
        getImportPreviewRowVersionConflict: mockGetImportPreviewRowVersionConflict,
        llmPreviewRecommend: mockLlmPreviewRecommend,
        analyzeLLMTransactions: mockAnalyzeLLMTransactions,
        getImportLearningSuggestions: mockGetImportLearningSuggestions,
        promoteImportLearning: mockPromoteImportLearning
    }
}));
jest.mock('@/views/desktop/transactions/import/importPreview.ts', () => {
    const actual = jest.requireActual('@/views/desktop/transactions/import/importPreview.ts') as any;
    return {
        ...actual,
        resolveImportPreviewCategoryPath: (categoryId: unknown, categoriesById: Record<string, any>) => {
            if (!categoryId || categoryId === '9' || categoryId === '11' || categoryId === '12') {
                return {
                    id: String(categoryId ?? ''),
                    mainCategory: 'Synthetic',
                    subCategory: '',
                    displayCategory: 'Synthetic',
                    type: 3
                };
            }
            return actual.resolveImportPreviewCategoryPath(categoryId, categoriesById);
        }
    };
});
jest.mock('@/lib/server_settings.ts', () => ({ isTransactionFromAIImageRecognitionEnabled: () => true }));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => mockCurrentToken }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: mockLoggerError }
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    showLoading: jest.fn(),
    hideLoading: jest.fn(),
    useI18nUIComponents: () => ({
        showAlert: jest.fn(), showConfirm: jest.fn(), showToast: jest.fn(), routeBackOnError: jest.fn()
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
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'MissingBranchStub' } }));
}

const { createRenderer, createSSRApp, defineComponent, h, nextTick, proxyRefs, reactive } = jest.requireActual('vue') as any;
const { renderToString } = jest.requireActual('vue/server-renderer') as any;

import { TransactionType } from '@/core/transaction.ts';
import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

type BindingOptions = {
    sessionId?: string;
    serverPaged?: boolean;
    total?: number;
    metadata?: Record<string, unknown> | null;
    reactiveProps?: boolean;
};

function createTransaction(id: number, options: Record<string, unknown> = {}): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: TransactionType.Expense,
        categoryId: '8',
        originalCategoryName: 'Cafe',
        time: 1_788_480_000 + id,
        utcOffset: 480,
        sourceAccountId: 'wallet',
        originalSourceAccountName: 'Wallet',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: '',
        originalDestinationAccountName: '',
        originalDestinationAccountCurrency: 'USD',
        sourceAmountCents: -1_000 - id,
        destinationAmountCents: 1_000 + id,
        tagIds: ['known-tag'],
        originalTagNames: ['Known Tag'],
        comment: `row-${id}`,
        counterparty: `merchant-${id}`,
        paymentMethod: 'card',
        selected: true,
        valid: true,
        matching: {
            parser: { id: 'fixture', tags: [] },
            transfer: { review_status: '', reviewed_type: '', suppressed: false },
            learning: {
                review_status: 'pending', rule_id: 1, score: 0.91, level: 'high', reason: 'merchant rule',
                recommended_type: 'expense', summary: 'Food/Cafe', source: 'model', model_version: 'v1'
            },
            llm: { review_status: 'pending', reason: 'evidence', confidence: 0.8 },
            annotation: {}
        },
        ...options
    } as never, id);
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    Object.assign(transaction, options);
    return transaction;
}

function previewFor(transaction: ImportTransaction, overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        id: (transaction as ImportTransaction & { _previewId?: number })._previewId,
        preview_type: 'expense',
        preview_category_id: '8',
        preview_source_account_id: 'wallet',
        preview_destination_account_id: null,
        preview_amount_cents: transaction.sourceAmountCents,
        preview_destination_amount_cents: transaction.destinationAmountCents,
        preview_recurring_id: null,
        preview_recurring_name: '',
        preview_recurring_candidate_count: 0,
        preview_recurring_match_score: 0,
        preview_recurring_match_reasons: '',
        preview_recurring_matched_date: '',
        preview_parser_id: '',
        preview_parser_tags: [],
        matching: { parser: { id: 'fixture', tags: [] }, annotation: {} },
        ...overrides
    };
}

function createBindings(transactions: ImportTransaction[], options: BindingOptions = {}) {
    const emit = jest.fn();
    const rawProps = {
        importTransactions: transactions,
        disabled: false,
        sessionId: options.sessionId ?? 'branch-session',
        serverPaged: options.serverPaged ?? false,
        totalImportTransactionCount: options.total ?? transactions.length,
        previewMetadata: options.metadata ?? null
    };
    const props = options.reactiveProps ? reactive(rawProps) : rawProps;
    const bindings = (ImportTransactionCheckDataTab as any).setup(props, { emit, expose: jest.fn() });
    bindings.snackbar.value = { showMessage: mockShowMessage, showError: mockShowError };
    return { bindings, emit, props };
}

async function flushPromises(): Promise<void> {
    await Promise.resolve();
    await Promise.resolve();
    await nextTick();
}

function response(options: {
    ok?: boolean;
    status?: number;
    json?: unknown;
    text?: string;
    jsonReject?: unknown;
} = {}): any {
    return {
        ok: options.ok ?? true,
        status: options.status ?? 200,
        json: options.jsonReject === undefined
            ? jest.fn<(...args: Array<any>) => Promise<any>>().mockResolvedValue(options.json ?? {})
            : jest.fn<(...args: Array<any>) => Promise<any>>().mockRejectedValue(options.jsonReject),
        text: jest.fn<(...args: Array<any>) => Promise<any>>().mockResolvedValue(options.text ?? '')
    };
}

function resolvedOpen(value: unknown): (...args: Array<any>) => Promise<any> {
    return jest.fn<(...args: Array<any>) => Promise<any>>().mockResolvedValue(value);
}

function rejectedOpen(value: unknown): (...args: Array<any>) => Promise<any> {
    return jest.fn<(...args: Array<any>) => Promise<any>>().mockRejectedValue(value);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCurrentToken = '';
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    mockGetMatchingSessionCandidates.mockResolvedValue({ data: { result: { candidates: [] } } });
    mockLlmPreviewRecommend.mockResolvedValue({ data: { result: { suggestions: [] } } });
    mockAnalyzeLLMTransactions.mockResolvedValue({ data: { result: { candidates_created: 0 } } });
    mockGetImportLearningSuggestions.mockResolvedValue({ data: { result: { suggestions: [] } } });
    mockPromoteImportLearning.mockResolvedValue({ data: { result: { rules_total: 0 } } });
    mockGetImportPreviewRowVersionConflict.mockReturnValue(null);
    Object.defineProperty(globalThis, 'fetch', { configurable: true, writable: true, value: mockFetch });
});

describe('ImportTransactionCheckDataTab uncovered category and recurring branches', () => {
    test('rejects hidden categories and hidden parents, then normalizes absent identities', () => {
        const transaction = createTransaction(1);
        const { bindings } = createBindings([transaction]);

        transaction.categoryId = '9';
        expect(bindings.getAcceptedCategoryPathForTransaction(transaction)).toBeNull();
        transaction.categoryId = '11';
        expect(bindings.getAcceptedCategoryPathForTransaction(transaction)).toBeNull();
        transaction.categoryId = '12';
        expect(bindings.getAcceptedCategoryPathForTransaction(transaction)).not.toBeNull();
        transaction.categoryId = null as never;
        expect(bindings.getAcceptedCategoryPathForTransaction(transaction)).toBeNull();
        expect(bindings.assignCategoryIdIfKnown(transaction, null)).toBe(false);
        expect(bindings.assignSourceAccountIdIfKnown(transaction, null)).toBe(false);
        expect(bindings.assignDestinationAccountIdIfKnown(transaction, undefined)).toBe(false);
    });

    test('recurring mutation handles unauthenticated success, protocol failures, stale data, and non-Error failures', async () => {
        const transaction = createTransaction(2);
        const { bindings } = createBindings([transaction]);

        mockFetch.mockResolvedValueOnce(response({
            json: { success: true, data: { sessionId: 'branch-session', previewItem: previewFor(transaction) } }
        }));
        await expect(bindings.updatePreviewRecurringMatch(transaction, 17)).resolves.toBe(true);
        expect(mockFetch.mock.calls[0]?.[1]?.headers).not.toHaveProperty('Authorization');

        mockFetch.mockResolvedValueOnce(response({ ok: false, status: 400, json: { error: '  recurring rejected  ' } }));
        await expect(bindings.updatePreviewRecurringMatch(transaction, null)).resolves.toBe(false);
        expect(mockShowMessage).toHaveBeenLastCalledWith('recurring rejected');

        mockFetch.mockResolvedValueOnce(response({ ok: false, status: 400, json: { error: '   ' } }));
        await bindings.updatePreviewRecurringMatch(transaction, null);
        expect(mockShowMessage).toHaveBeenLastCalledWith('Recurring match request failed (400)');

        mockFetch.mockResolvedValueOnce(response({ json: { success: false, error: 'protocol rejected' } }));
        await bindings.updatePreviewRecurringMatch(transaction, null);
        expect(mockShowMessage).toHaveBeenLastCalledWith('protocol rejected');

        mockFetch.mockResolvedValueOnce(response({ json: { success: false, error: '' } }));
        await bindings.updatePreviewRecurringMatch(transaction, null);
        expect(mockShowMessage).toHaveBeenLastCalledWith('Unknown error');

        mockFetch.mockResolvedValueOnce(response({ json: { success: true, data: { sessionId: '', previewItem: previewFor(transaction) } } }));
        await bindings.updatePreviewRecurringMatch(transaction, null);
        expect(mockShowMessage).toHaveBeenLastCalledWith('Recurring match response is out of date');

        mockFetch.mockRejectedValueOnce('plain recurring failure');
        await bindings.updatePreviewRecurringMatch(transaction, null);
        expect(mockShowMessage).toHaveBeenLastCalledWith('Recurring match failed');
    });

    test('recurring candidate loader covers empty token, failed payloads, missing arrays, and first-candidate fallback', async () => {
        const transaction = createTransaction(3, { recurringTemplateId: '' });
        const { bindings } = createBindings([transaction]);

        mockFetch.mockResolvedValueOnce(response({ json: { success: false, error: 'candidate rejected' } }));
        await bindings.openRecurringCandidateDialog(transaction);
        expect(mockFetch.mock.calls[0]?.[1]?.headers).not.toHaveProperty('Authorization');
        expect(mockShowMessage).toHaveBeenLastCalledWith('Load Scheduled Candidates Failed');

        mockFetch.mockResolvedValueOnce(response({ json: { success: false, error: '' } }));
        await bindings.openRecurringCandidateDialog(transaction);
        expect(mockShowMessage).toHaveBeenLastCalledWith('Load Scheduled Candidates Failed');

        mockFetch.mockResolvedValueOnce(response({ json: { success: true, result: {} } }));
        await bindings.openRecurringCandidateDialog(transaction);
        expect(bindings.recurringCandidates.value).toEqual([]);

        mockFetch.mockResolvedValueOnce(response({
            json: { success: true, result: { candidates: [{ id: 91, matchReasons: [] }], linkedRecurringId: '' } }
        }));
        await bindings.openRecurringCandidateDialog(transaction);
        expect(bindings.selectedRecurringCandidateId.value).toBe('91');

        transaction.recurringMatchReasons = ' | ';
        expect(bindings.getPrimaryRecurringReason(transaction)).toBe('');
    });
});

describe('ImportTransactionCheckDataTab uncovered decision and refresh branches', () => {
    test('clears optional learning and LLM state even when preview identity or payload is absent', () => {
        const transaction = createTransaction(10);
        const { bindings } = createBindings([transaction]);
        delete (transaction as ImportTransaction & { _previewId?: number })._previewId;
        transaction.matching = { parser: { id: 'fixture', tags: [] } } as never;

        bindings.clearLearningRecommendationState(transaction);
        bindings.clearLLMRecommendationState(transaction);

        expect(transaction.learningRecommendationScore).toBe(0);
        expect((transaction.matching as any).llm.review_status).toBe('');
    });

    test('syncs preview decisions without an id and covers parser/category/account fallbacks', () => {
        const transaction = createTransaction(11);
        delete (transaction as ImportTransaction & { _previewId?: number })._previewId;
        const { bindings } = createBindings([transaction]);

        bindings.syncTransactionFromLLMPreviewPayload(transaction, {
            preview: {
                preview_type: 'income',
                preview_category_id: '20',
                preview_source_account_id: null,
                preview_destination_account_id: null,
                preview_description: '',
                preview_counterparty: '',
                preview_payment_method: '',
                preview_parser_id: '',
                preview_parser_tags: null
            },
            matching: null
        });
        expect(transaction.type).toBe(TransactionType.Income);

        bindings.syncTransactionFromPreviewDecision(transaction, {
            preview_type: 'expense',
            preview_category_id: '8',
            preview_source_account_id: null,
            preview_destination_account_id: null,
            preview_amount_cents: null,
            preview_destination_amount_cents: null,
            preview_parser_tags: null,
            matching: null
        });
        expect(transaction.sourceAccountId).toBe('');
    });

    test('transfer review uses the atomic replacement path without a token', async () => {
        const transaction = createTransaction(12);
        const { bindings, emit } = createBindings([transaction]);
        mockReviewImportTransferDecision.mockResolvedValueOnce({
            data: {
                result: {
                    removedPreviewIds: [12],
                    upsertedPreviewItems: [previewFor(transaction)]
                }
            }
        });

        await bindings.reviewTransferSuggestion(transaction, 'accept');
        expect(mockReviewImportTransferDecision).toHaveBeenCalledWith(expect.objectContaining({
            previewId: 12,
            decision: 'accept',
            payload: expect.objectContaining({ responseMode: 'preview-item' })
        }));
        const request = mockReviewImportTransferDecision.mock.calls[0]?.[0];
        expect(request.payload.expectedState).not.toHaveProperty('rowVersion');
        expect(emit).toHaveBeenCalledWith('reclassified', expect.any(Array), [12]);
    });

    test('LLM review reports both accept and reject outcomes', async () => {
        const accepted = createTransaction(13);
        const rejected = createTransaction(14);
        const { bindings } = createBindings([accepted, rejected]);
        mockLlmPreviewRecommendAccept.mockResolvedValueOnce({
            data: { result: { session_id: 'branch-session', preview: previewFor(accepted), matching: { llm: {} } } }
        });
        mockLlmPreviewRecommendReject.mockResolvedValueOnce({
            data: { result: { sessionId: 'branch-session', preview: previewFor(rejected), matching: { llm: {} } } }
        });

        await bindings.reviewLLMRecommendation(accepted, 'accept');
        await bindings.reviewLLMRecommendation(rejected, 'reject');

        expect(mockShowMessage).toHaveBeenCalledWith('LLM Suggestion Accepted');
        expect(mockShowMessage).toHaveBeenCalledWith('LLM Suggestion Rejected');
    });

    test('learning review rejects a truthy response whose session id falls back to empty', async () => {
        const transaction = createTransaction(15);
        const { bindings } = createBindings([transaction]);
        mockAcceptMatchingCandidate.mockResolvedValueOnce({ data: { result: { sessionId: '' } } });

        await bindings.reviewLearningSuggestion(transaction, 'accept');

        expect(mockShowMessage).toHaveBeenLastCalledWith('Learning decision response is out of date');
    });
});

describe('ImportTransactionCheckDataTab uncovered server, LLM, and management branches', () => {
    test('reclassify commits an active edit and reports domain and unknown service errors', async () => {
        const transaction = createTransaction(20);
        const { bindings } = createBindings([transaction]);
        bindings.editingTransaction.value = transaction;
        bindings.editingTags.value = [];
        mockReclassifyImportPreview.mockRejectedValueOnce(new Error('reclassify rejected'));
        await bindings.reclassifySelected();
        expect(mockShowMessage).toHaveBeenLastCalledWith(expect.stringContaining('reclassify rejected'));

        mockReclassifyImportPreview.mockRejectedValueOnce({});
        await bindings.reclassifySelected();
        expect(mockShowMessage).toHaveBeenLastCalledWith('Reclassify failed');
    });

    test('tracked lookup, selection flush, and LLM recommendation cover absent identities and payloads', async () => {
        const tracked = createTransaction(21, { selected: false });
        const noId = createTransaction(22);
        delete (noId as ImportTransaction & { _previewId?: number })._previewId;
        const { bindings } = createBindings([tracked, noId], {
            serverPaged: true,
            total: 2,
            metadata: { counts: { total: 2, selected_total: 0 } }
        });

        bindings.serverPagedDrafts.value.set(99, createTransaction(99));
        expect(bindings.getTrackedTransactionByPreviewId(99)).not.toBeNull();
        expect(bindings.getTrackedTransactionByPreviewId(1000)).toBeNull();
        await expect(bindings.flushPreviewSelectionAndBuildActionScope()).resolves.toEqual(expect.any(Object));

        mockLlmPreviewRecommend.mockResolvedValueOnce({ data: { result: { suggestions: null } } });
        await bindings.applyLLMPreviewRecommendations();
        expect(mockShowMessage).toHaveBeenLastCalledWith('LLM preview recommendation completed, but no suggestions were generated');

        mockLlmPreviewRecommend.mockResolvedValueOnce({
            data: { result: { suggestions: [{ preview_id: 0 }, { preview_id: 888 }, { preview_id: 21, preview: previewFor(tracked) }] } }
        });
        await bindings.applyLLMPreviewRecommendations();
        expect(mockShowMessage).toHaveBeenLastCalledWith(expect.stringContaining('updated'));
    });

    test('selection flush skips a truly identity-less row and normalizes absent response metadata', async () => {
        const selected = createTransaction(221, { selected: true });
        const { bindings, props } = createBindings([selected], {
            serverPaged: true,
            total: 1,
            metadata: { counts: { selected_total: 1 } }
        });
        const noId = { ...selected, _previewId: undefined, index: undefined } as unknown as ImportTransaction;
        bindings.serverPagedDrafts.value = new Map([[999, noId]]);
        props.importTransactions[0] = noId;
        await expect(bindings.flushPreviewSelectionAndBuildActionScope()).resolves.toEqual(expect.any(Object));
        expect(bindings.getServerPagedSelectionDelta()).toEqual({ selected: 0, selectedInvalid: 0 });
        expect(bindings.getServerPagedAnnotationDelta()).toBe(0);

        props.importTransactions[0] = selected;
        selected.selected = false;
        mockFetch.mockResolvedValueOnce(response({ json: { success: true, data: {} } }));
        await bindings.flushPreviewSelectionAndBuildActionScope();
        expect(bindings.serverPagedSelectionMetadataOverride.value).toBeNull();
    });

    test('LLM analysis uses unknown error fallbacks and learning promotion handles absent optional results', async () => {
        const transaction = createTransaction(23);
        const { bindings } = createBindings([transaction]);

        mockAnalyzeLLMTransactions.mockRejectedValueOnce(new Error('analysis failed'));
        await bindings.analyzeSelectedPreviewWithLLM();
        expect(mockLoggerError).toHaveBeenCalledWith(expect.stringContaining('code=unknown status=unknown'));

        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: {} } });
        await bindings.promoteSelectedToLongTermLearning();
        expect(mockShowMessage).toHaveBeenLastCalledWith('No learning suggestions available for the selected preview rows');

        bindings.importLearningSuggestionDialog.value = { open: resolvedOpen({}) };
        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: { suggestions: [{ preview_id: 23 }] } } });
        await bindings.promoteSelectedToLongTermLearning();
        expect(mockPromoteImportLearning).not.toHaveBeenCalled();

        bindings.importLearningSuggestionDialog.value = { open: resolvedOpen({ previewIds: [23] }) };
        mockGetImportLearningSuggestions.mockResolvedValueOnce({ data: { result: { suggestions: [{ preview_id: 23 }] } } });
        mockPromoteImportLearning.mockResolvedValueOnce({ data: { result: {} } });
        await bindings.promoteSelectedToLongTermLearning();
        expect(mockShowMessage).toHaveBeenLastCalledWith(expect.stringContaining('0'));
    });

    test('quick-create and managed dialogs cover missing results, rejected operations, and absent selections', async () => {
        const transaction = createTransaction(24);
        const { bindings } = createBindings([transaction]);
        const unprocessed = { processed: false };

        bindings.categoryEditDialog.value = { open: resolvedOpen({}) };
        await bindings.quickCreatePrimaryCategory(transaction);
        bindings.categoryEditDialog.value = { open: rejectedOpen(unprocessed) };
        await bindings.quickCreatePrimaryCategory(transaction);
        bindings.categoryEditDialog.value = { open: rejectedOpen({ processed: true }) };
        await bindings.quickCreatePrimaryCategory(transaction);
        await bindings.quickCreateSecondaryCategory(transaction, null);
        bindings.categoryEditDialog.value = { open: rejectedOpen(unprocessed) };
        await bindings.quickCreateSecondaryCategory(transaction, mockExpenseParent);

        bindings.accountEditDialog.value = { open: resolvedOpen({}) };
        await bindings.quickCreateAccount(transaction, 'destination', {});
        bindings.accountEditDialog.value = { open: rejectedOpen(unprocessed) };
        await bindings.quickCreateAccount(transaction, 'source', { category: 1 });
        bindings.accountEditDialog.value = { open: rejectedOpen({ processed: true }) };
        await bindings.quickCreateAccount(transaction, 'source', { category: 1 });
        bindings.applyCreatedAccountToTransaction(transaction, 'destination', { id: 'missing', name: 'Missing' });

        bindings.manageCategoryType.value = 999;
        expect(bindings.getManageCategoryItems()).toEqual([]);
        bindings.manageCategoryId.value = 'missing';
        expect(bindings.getSelectedManagePrimaryCategory()).toBeUndefined();

        bindings.categoryEditDialog.value = { open: resolvedOpen({}) };
        bindings.openManagedPrimaryCategoryCreateDialog();
        await flushPromises();
        bindings.categoryEditDialog.value = { open: rejectedOpen(unprocessed) };
        bindings.openManagedPrimaryCategoryCreateDialog();
        await flushPromises();

        bindings.manageCategoryId.value = '8';
        bindings.categoryEditDialog.value = { open: resolvedOpen({}) };
        bindings.openManagedSecondaryCategoryCreateDialog();
        await flushPromises();
        bindings.categoryEditDialog.value = { open: rejectedOpen(unprocessed) };
        bindings.openManagedSecondaryCategoryCreateDialog();
        await flushPromises();

        bindings.manageAccountId.value = 'missing';
        bindings.accountEditDialog.value = { open: resolvedOpen({}) };
        bindings.openManagedAccountCreateDialog();
        await flushPromises();
        bindings.accountEditDialog.value = { open: rejectedOpen(unprocessed) };
        bindings.openManagedAccountCreateDialog();
        await flushPromises();

        bindings.manageCategoryId.value = 'missing';
        bindings.openSelectedCategoryEditDialog();
        bindings.manageAccountId.value = 'missing';
        bindings.openSelectedAccountEditDialog();

        bindings.manageCategoryId.value = '8';
        bindings.categoryEditDialog.value = { open: rejectedOpen(unprocessed) };
        bindings.openSelectedCategoryEditDialog();
        await flushPromises();
        bindings.manageAccountId.value = 'wallet';
        bindings.accountEditDialog.value = { open: rejectedOpen(unprocessed) };
        bindings.openSelectedAccountEditDialog();
        await flushPromises();
        expect(mockLoggerError).toHaveBeenCalled();
    });

    test('batch application and facet helpers expose no-op and fallback labels', () => {
        const transaction = createTransaction(25, {
            categoryId: '', sourceAccountId: '', tagIds: null, originalTagNames: null,
            counterparty: null, paymentMethod: null, comment: null
        });
        const { bindings } = createBindings([transaction]);

        bindings.applyBatchCategory();
        bindings.batchCategoryId.value = '8';
        bindings.applyBatchCategory();
        bindings.applyBatchAccount();
        bindings.batchAccountId.value = 'wallet';
        bindings.applyBatchAccount();

        expect(bindings.metadataAccountFacetLabels([
            { value: '', label: '' },
            { value: 'missing', label: '' },
            { value: 'wallet', label: '' }
        ])).toContain('Wallet');
        expect(bindings.buildAccountFacetValueByLabel([
            { value: '', label: '' },
            { value: 'missing', label: 'Fallback' },
            { value: 'wallet', label: '' }
        ])).toMatchObject({ Fallback: 'missing', Wallet: 'wallet' });
        expect(bindings.captureImportPreviewEditableDraftState(transaction)).toMatchObject({
            tagIds: [], counterparty: '', paymentMethod: '', comment: ''
        });
        expect(bindings.canImport.value).toBe(true);
    });

    test('batch updates tolerate identities disappearing between validation and display lookup', () => {
        const transaction = createTransaction(26, { selected: true });
        const { bindings } = createBindings([transaction]);
        const volatileCategory = { ...mockExpenseChild, id: 'volatile-category' };
        let categoryReads = 0;
        Object.defineProperty(mockCategoryMap, 'volatile-category', {
            configurable: true,
            get: () => (++categoryReads === 1 ? volatileCategory : undefined)
        });
        bindings.batchCategoryId.value = 'volatile-category';
        bindings.applyBatchCategory();
        delete mockCategoryMap['volatile-category'];

        const volatileAccount = { ...mockWallet, id: 'volatile-account' };
        let accountReads = 0;
        Object.defineProperty(mockAccountsMap, 'volatile-account', {
            configurable: true,
            get: () => (++accountReads === 1 ? volatileAccount : undefined)
        });
        bindings.batchAccountId.value = 'volatile-account';
        bindings.applyBatchAccount();
        delete mockAccountsMap['volatile-account'];

        expect(transaction.isManuallyAnnotated).toBe(true);
    });
});

describe('ImportTransactionCheckDataTab uncovered paging, computed, and display branches', () => {
    test('server draft and selection state skip rows without identities and preserve existing baselines', () => {
        const identified = createTransaction(30);
        const noId = createTransaction(31);
        delete (noId as ImportTransaction & { _previewId?: number })._previewId;
        const { bindings } = createBindings([identified, noId], {
            serverPaged: true,
            total: 20,
            metadata: { counts: {} }
        });

        bindings.recordServerPagedDraftBaselines([identified, identified, noId]);
        bindings.cacheCurrentPageDrafts();
        bindings.recordServerPagedSelectionBaselines([identified, identified, noId]);
        bindings.getServerPagedSelectionDelta();
        bindings.getServerPagedAnnotationDelta();
        expect(bindings.getUniqueTrackedServerPagedTransactions()).toEqual(expect.any(Array));

        bindings.countPerPage.value = 0;
        bindings.updatePreviewTableSort([{ key: 'time', order: 'asc' }]);
        expect(bindings.getCurrentServerPagedSortRequest()).toMatchObject({ sortBy: 'time', sortDirection: 'asc' });
        bindings.updatePreviewTableSort([]);
        expect(bindings.getCurrentServerPagedSortRequest()).toMatchObject({ sortBy: null, sortDirection: null });
    });

    test('reactive server rows cover replacement watcher tombstones and rehydrated rows', async () => {
        const first = createTransaction(32);
        const { bindings, props } = createBindings([first], {
            serverPaged: true,
            total: 1,
            reactiveProps: true,
            metadata: null
        });
        first.comment = 'local draft';
        bindings.cacheCurrentPageDrafts();

        const replacement = createTransaction(32, {
            comment: 'server',
            matching: {
                parser: { id: 'fixture', tags: [] },
                llm: { review_status: 'pending', reason: 'evidence', confidence: 0.8, suggested_main_category: 'Food' }
            }
        });
        props.importTransactions = [replacement];
        await flushPromises();

        expect(replacement.comment).toBe('local draft');
        expect(bindings.getTrackedTransactionByPreviewId(32)).toMatchObject({ comment: 'local draft' });
    });

    test('rehydration checks both LLM draft-drift operands', async () => {
        const first = createTransaction(33);
        const { bindings, props } = createBindings([first], {
            serverPaged: true,
            total: 1,
            reactiveProps: true
        });
        first.categoryId = '20';
        bindings.cacheCurrentPageDrafts();
        const replacement = createTransaction(33, {
            matching: {
                parser: { id: 'fixture', tags: [] },
                llm: { review_status: 'pending', reason: 'evidence', confidence: 0.8, suggested_main_category: 'Food' }
            }
        });
        props.importTransactions = [replacement];
        await flushPromises();
        expect(replacement.categoryId).toBe('20');
    });

    test('component-scoped watcher distinguishes same rows, tombstoned replacements, and authoritative LLM replacements', async () => {
        const first = createTransaction(34);
        const state = reactive({
            importTransactions: [first],
            disabled: false,
            sessionId: 'watch-session',
            serverPaged: false,
            totalImportTransactionCount: 1,
            previewMetadata: null
        });
        let bindings: any;
        const Harness = defineComponent({
            setup: () => {
                bindings = (ImportTransactionCheckDataTab as any).setup(state, { emit: jest.fn(), expose: jest.fn() });
                return () => h('div');
            }
        });
        const renderer = createRenderer({
            createElement: (type: string) => ({ type, children: [] as any[], parent: null as any }),
            createText: (text: string) => ({ text, parent: null as any }),
            createComment: (text: string) => ({ comment: text, parent: null as any }),
            setText: (node: any, text: string) => { node.text = text; },
            setElementText: (node: any, text: string) => { node.text = text; },
            parentNode: (node: any) => node.parent,
            nextSibling: () => null,
            insert: (child: any, parent: any) => { child.parent = parent; parent.children.push(child); },
            remove: (child: any) => {
                const index = child.parent?.children.indexOf(child) ?? -1;
                if (index >= 0) child.parent.children.splice(index, 1);
            },
            patchProp: (node: any, key: string, _previous: unknown, value: unknown) => { node[key] = value; }
        });
        const app = renderer.createApp(Harness);
        app.config.warnHandler = () => undefined;
        app.mount({ children: [] });
        await nextTick();

        state.importTransactions = [first];
        await nextTick();
        const tombstoned = createTransaction(34, { matching: { parser: { id: 'fixture', tags: [] } } });
        state.importTransactions = [tombstoned];
        await nextTick();
        const authoritative = createTransaction(34, {
            matching: { parser: { id: 'fixture', tags: [] }, llm: { review_status: 'accepted' } }
        });
        state.importTransactions = [authoritative];
        await nextTick();

        expect(bindings.getTrackedTransactionByPreviewId(34)).toMatchObject({
            matching: { llm: { review_status: 'accepted' } }
        });
        app.unmount();
    });

    test('server mode activation and filter changes use the ten-row fallback when page size is non-positive', async () => {
        const transaction = createTransaction(35);
        const { bindings, props, emit } = createBindings([transaction], {
            serverPaged: false,
            total: 1,
            reactiveProps: true
        });
        bindings.countPerPage.value = 0;
        props.serverPaged = true;
        await flushPromises();
        bindings.countPerPage.value = 0;
        bindings.filters.value.description = 'row';
        await flushPromises();
        expect(emit).toHaveBeenCalledWith('requestPage', 1, 10, expect.any(Object));
    });

    test('table height, server counts, facets, and current invalid labels use every fallback source', () => {
        const rows = Array.from({ length: 12 }, (_, index) => createTransaction(40 + index));
        rows[0]!.actualCategoryName = 'Cafe';
        rows[0]!.actualSourceAccountName = 'Wallet';
        rows[0]!.actualDestinationAccountName = 'Savings';
        rows[1]!.tagIds = ['0'];
        rows[1]!.originalTagNames = ['Imported'];
        const { bindings } = createBindings(rows, {
            serverPaged: true,
            total: 12,
            metadata: {
                counts: { selected: 1, selected_invalid: 0, annotations: { 'needs-review': 0 } },
                facets: {
                    categories: [{ value: '8', label: '' }, { value: '', label: '' }],
                    accounts: [{ value: 'wallet', label: '' }, { value: 'missing', label: 'Missing' }],
                    tags: [{ value: 'known-tag', label: '' }]
                }
            }
        });

        bindings.countPerPage.value = 11;
        expect(bindings.importTransactionsTableHeight.value).toBe(400);
        expect(bindings.selectedImportTransactionCount.value).toBeGreaterThanOrEqual(0);
        expect(bindings.annotationTransactionCount.value).toBeGreaterThanOrEqual(0);
        expect(bindings.selectedVisibleHistoryRewriteOperationCount.value).toBeGreaterThanOrEqual(0);
        expect(bindings.allUsedCategoryNames.value).toEqual(expect.any(Array));
        expect(bindings.allUsedAccountNames.value).toEqual(expect.any(Array));
        expect(bindings.allUsedTagNames.value).toEqual(expect.any(Array));

        const local = createTransaction(60, {
            categoryId: '', originalCategoryName: '', sourceAccountId: '', originalSourceAccountName: '',
            destinationAccountId: '', originalDestinationAccountName: '', tagIds: null, originalTagNames: null
        });
        const localEmptyNames = createTransaction(62, {
            actualCategoryName: '', actualSourceAccountName: '', actualDestinationAccountName: '',
            tagIds: ['missing'], originalTagNames: ['']
        });
        const localBindings = createBindings([local, localEmptyNames]).bindings;
        local.actualCategoryName = 'Visible Category';
        local.actualSourceAccountName = 'Visible Account';
        local.actualDestinationAccountName = '';
        expect(localBindings.allUsedCategoryNames.value).toContain('Visible Category');
        expect(localBindings.allUsedAccountNames.value).toContain('Visible Account');
        expect(createBindings([]).bindings.allUsedTagNames.value).toEqual([]);
        expect(localBindings.allUsedTagNames.value).toEqual([]);
        expect(localBindings.allInvalidExpenseCategoryNames.value).toContainEqual({ name: '(Empty)', value: '' });
        expect(localBindings.allInvalidAccountNames.value).toContainEqual({ name: '(Empty)', value: '' });
        expect(localBindings.allInvalidTransactionTagNames.value).toEqual([]);
        expect(localBindings.allOriginalTransactionTagNames.value).toContainEqual({ name: '(Empty)', value: '' });

        const importedTag = createTransaction(61, { tagIds: ['0', 'missing'], originalTagNames: ['Imported', ''] });
        const importedTagBindings = createBindings([importedTag]).bindings;
        expect(importedTagBindings.allUsedTagNames.value).toContain('Imported');
        const emptyNameObject = new String('');
        importedTag.originalTagNames = [emptyNameObject as unknown as string];
        expect(importedTagBindings.allInvalidTransactionTagNames.value).toContainEqual({ name: '(Empty)', value: '' });
    });

    test('selection respects hidden rows while display and identity helpers fall back safely', async () => {
        const visible = createTransaction(70, { selected: false });
        const hidden = createTransaction(71, { selected: false });
        const { bindings } = createBindings([visible, hidden]);
        bindings.filters.value.description = 'row-70';

        await bindings.selectAll();
        expect(visible.selected).toBe(true);
        expect(hidden.selected).toBe(false);
        await bindings.selectNone();
        expect(visible.selected).toBe(false);
        await bindings.selectInvert();
        expect(visible.selected).toBe(true);

        visible.sourceAccountId = 'missing';
        visible.destinationAccountId = 'missing';
        visible.originalSourceAccountCurrency = '';
        visible.originalDestinationAccountCurrency = '';
        visible.type = TransactionType.Transfer;
        expect(bindings.getSourceAccountDisplayName(visible)).toBe('');
        expect(bindings.getTransactionDisplayDestinationAmount(visible)).toContain('CNY');
    });

    test('selected updates choose tracked server drafts and assignment helpers normalize nulls', () => {
        const transaction = createTransaction(72);
        const { bindings } = createBindings([transaction], { serverPaged: true, total: 1 });
        expect(bindings.getSelectedPreviewUpdates()).toEqual(expect.any(Array));
        expect(bindings.assignCategoryIdIfKnown(transaction, undefined)).toBe(false);
        expect(bindings.assignSourceAccountIdIfKnown(transaction, undefined)).toBe(false);
        expect(bindings.assignDestinationAccountIdIfKnown(transaction, undefined)).toBe(false);
    });

    test('local selection action predicates cover both valid and invalid rows', () => {
        const valid = createTransaction(73, { valid: true, selected: false });
        const invalid = createTransaction(74, { valid: false, categoryId: '', selected: false });
        const { bindings } = createBindings([valid, invalid]);

        bindings.applySelectionActionToLocalTransactions('select_valid', [valid, invalid]);
        bindings.applySelectionActionToLocalTransactions('select_invalid', [valid, invalid]);

        expect(valid.selected).toBe(true);
        expect(invalid.selected).toBe(true);
    });

    test('server selection omits Authorization when no token is available', async () => {
        const transaction = createTransaction(75, { selected: false });
        const { bindings } = createBindings([transaction], { serverPaged: true, total: 1 });
        mockFetch.mockResolvedValueOnce(response({ json: { success: true, data: {} } }));

        await bindings.selectAll();

        expect(mockFetch.mock.calls[0]?.[1]?.headers).not.toHaveProperty('Authorization');
    });
});

describe('ImportTransactionCheckDataTab uncovered template fallbacks', () => {
    test('renders missing transfer destinations and a candidate without a score', async () => {
        const zeroDestination = createTransaction(80, {
            type: TransactionType.Transfer,
            categoryId: '30',
            destinationAccountId: '0',
            originalDestinationAccountName: 'Zero destination'
        });
        const absentDestination = createTransaction(81, {
            type: TransactionType.Transfer,
            categoryId: '30',
            destinationAccountId: 'missing',
            originalDestinationAccountName: 'Missing destination'
        });
        const props = {
            importTransactions: [zeroDestination, absentDestination],
            disabled: false,
            sessionId: 'template-session',
            serverPaged: false,
            totalImportTransactionCount: 2,
            previewMetadata: null
        };
        let bindings: any;
        const DataTableStub = defineComponent({
            props: { items: { type: Array, default: () => [] } },
            setup: (tableProps: any, { slots }: any) => () => {
                const children: unknown[] = [slots.default?.(), slots['header.data-table-select']?.(), slots.bottom?.()];
                for (const item of tableProps.items) {
                    for (const [name, slot] of Object.entries(slots)) {
                        if (name.startsWith('item.') && typeof slot === 'function') {
                            children.push((slot as any)({ item }));
                        }
                    }
                }
                return h('div', children);
            }
        });
        const GenericStub = defineComponent({
            inheritAttrs: false,
            setup: (_props: unknown, { attrs, slots }: any) => () => h(
                'div',
                attrs,
                Object.values(slots).flatMap(slot => typeof slot === 'function' ? (slot as any)({}) : [])
            )
        });
        const Harness = defineComponent({
            setup: () => {
                bindings = (ImportTransactionCheckDataTab as any).setup(props, { emit: jest.fn(), expose: jest.fn() });
                bindings.showRecurringCandidateDialog.value = true;
                bindings.recurringCandidateTarget.value = zeroDestination;
                bindings.recurringCandidates.value = [{ id: 1, name: '', matchScore: undefined, matchReasons: [] }];
                const renderBindings = proxyRefs(bindings);
                return () => (ImportTransactionCheckDataTab as any).render(
                    renderBindings,
                    [],
                    props,
                    renderBindings,
                    {},
                    {}
                );
            }
        });
        const app = createSSRApp(Harness);
        app.component('v-data-table', DataTableStub);
        for (const name of [
            'v-dialog', 'v-card', 'v-card-title', 'v-card-text', 'v-card-actions', 'v-btn', 'v-btn-group',
            'v-chip', 'v-list', 'v-list-item', 'v-list-item-title', 'v-list-item-subtitle', 'v-checkbox',
            'v-menu', 'v-divider', 'v-select', 'v-autocomplete', 'v-text-field', 'v-tabs', 'v-tab',
            'v-spacer', 'v-icon', 'v-alert', 'v-progress-circular', 'two-column-select', 'icon-select',
            'date-range-selection-dialog', 'amount-input', 'item-icon'
        ]) {
            app.component(name, GenericStub);
        }
        app.config.warnHandler = () => undefined;

        const html = await renderToString(app);

        expect(html).toContain('Zero destination');
        expect(html).toContain('Missing destination');
        expect(html).toContain('Match Score');
        expect(html).toContain('0');
    });
});
