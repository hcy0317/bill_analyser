import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockApplyImportPreviewSelectionAction = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetImportPreviewSelectionConflict = jest.fn<(...args: Array<unknown>) => any>();
const mockGetImportPreviewRowVersionConflict = jest.fn<(...args: Array<unknown>) => any>();
const mockShowError = jest.fn();
const mockShowMessage = jest.fn();
const mockFetch = jest.fn<(...args: Array<any>) => Promise<any>>();

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
const mockWallet = {
    id: 'wallet',
    name: 'Wallet',
    currency: 'CNY',
    category: 1,
    icon: 'wallet',
    color: '#0088ff',
    hidden: false
};
const mockHiddenAccount = {
    id: 'hidden',
    name: 'Hidden',
    currency: 'CNY',
    category: 1,
    icon: 'wallet',
    color: '#999999',
    hidden: true
};
const mockKnownTag = { id: 'tag-known', name: 'Known tag', hidden: false };

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => String(value) }),
        formatUnixTimeToLongDateTime: (value: unknown) => String(value ?? ''),
        formatAmountToLocalizedNumeralsWithCurrency: (value: unknown, currency: string) => `${currency}:${value}`,
        getCategorizedAccountsWithDisplayBalance: () => [{ category: 1, accounts: [mockWallet] }]
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
        currentUserCoordinateDisplayType: 0
    })
}));
jest.mock('@/stores/account.ts', () => ({
    useAccountsStore: () => ({
        allPlainAccounts: [mockWallet, mockHiddenAccount],
        allVisiblePlainAccounts: [mockWallet],
        allAccountsMap: { wallet: mockWallet, hidden: mockHiddenAccount },
        allAccounts: [mockWallet, mockHiddenAccount],
        loadAllAccounts: jest.fn()
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: { 3: [mockExpenseParent] },
        allTransactionCategoriesMap: {
            '7': mockExpenseParent,
            '8': mockExpenseChild
        },
        loadAllCategories: jest.fn()
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({
        allTransactionTags: [mockKnownTag],
        allTransactionTagsMap: { 'tag-known': mockKnownTag }
    })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getLLMMemoryEvents: mockGetLLMMemoryEvents,
        applyImportPreviewSelectionAction: mockApplyImportPreviewSelectionAction,
        getImportPreviewSelectionConflict: mockGetImportPreviewSelectionConflict,
        getImportPreviewRowVersionConflict: mockGetImportPreviewRowVersionConflict
    }
}));
jest.mock('@/lib/server_settings.ts', () => ({
    isTransactionFromAIImageRecognitionEnabled: () => false
}));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => 'selection-token' }));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
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
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: { name: 'SelectionPagingSfcStub' }
    }));
}

import { ImportTransaction } from '@/models/imported_transaction.ts';
import { PreviewPageRequestCoordinator } from '@/views/desktop/transactions/import/import-dialog/previewPageRequestCoordinator.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

function deferred<T>() {
    let resolve!: (value: T) => void;
    const promise = new Promise<T>(done => { resolve = done; });
    return { promise, resolve };
}

type TransactionOptions = {
    id: number;
    type?: number;
    categoryId?: string;
    categoryName?: string;
    sourceAccountId?: string;
    sourceAccountName?: string;
    destinationAccountId?: string;
    destinationAccountName?: string;
    selected?: boolean;
    tagIds?: string[];
    tagNames?: string[];
    comment?: string;
};

function createTransaction(options: TransactionOptions): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: options.type ?? 3,
        categoryId: options.categoryId ?? '8',
        originalCategoryName: options.categoryName ?? 'Cafe',
        time: 1_788_480_000 + options.id,
        utcOffset: 480,
        sourceAccountId: options.sourceAccountId ?? 'wallet',
        originalSourceAccountName: options.sourceAccountName ?? 'Wallet',
        originalSourceAccountCurrency: 'CNY',
        destinationAccountId: options.destinationAccountId ?? '',
        originalDestinationAccountName: options.destinationAccountName ?? '',
        originalDestinationAccountCurrency: 'CNY',
        sourceAmountCents: -1_000 - options.id,
        destinationAmountCents: 1_000 + options.id,
        tagIds: options.tagIds ?? ['tag-known'],
        originalTagNames: options.tagNames ?? ['Known tag'],
        comment: options.comment ?? `row-${options.id}`,
        counterparty: `merchant-${options.id}`,
        paymentMethod: 'card',
        selected: options.selected ?? false,
        matching: {
            parser: { id: 'test-parser', tags: ['fixture'] },
            annotation: {}
        }
    } as never, options.id);
    (transaction as ImportTransaction & { _previewId: number })._previewId = options.id;
    return transaction;
}

function createBindings(options: {
    transactions: ImportTransaction[];
    emit?: ReturnType<typeof jest.fn>;
    serverPaged?: boolean;
    sessionId?: string;
    total?: number;
    metadata?: Record<string, unknown>;
}): any {
    return (ImportTransactionCheckDataTab as any).setup({
        importTransactions: options.transactions,
        sessionId: options.sessionId ?? '',
        serverPaged: options.serverPaged ?? false,
        totalImportTransactionCount: options.total ?? options.transactions.length,
        previewMetadata: options.metadata ?? null
    }, {
        emit: options.emit ?? jest.fn(),
        expose: jest.fn()
    });
}

beforeEach(() => {
    jest.clearAllMocks();
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    mockGetImportPreviewSelectionConflict.mockReturnValue(null);
    mockGetImportPreviewRowVersionConflict.mockReturnValue(null);
    Object.defineProperty(globalThis, 'fetch', {
        configurable: true,
        writable: true,
        value: mockFetch
    });
});

describe('desktop import selection, paging, and edit contracts', () => {
    test('server paging caches edited drafts and emits normalized request parameters', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const emit = jest.fn();
        const transaction = createTransaction({ id: 11, selected: true, comment: 'original' });
        try {
            const bindings = createBindings({
                transactions: [transaction],
                emit,
                serverPaged: true,
                sessionId: 'server-session',
                total: 42,
                metadata: {
                    facets: {
                        categories: [{ value: '8', label: 'Cafe', count: 12 }],
                        accounts: [{ value: 'wallet', label: 'Wallet fallback', count: 12 }]
                    }
                }
            });

            expect(emit).toHaveBeenCalledWith('requestPage', 1, 10, {
                sortBy: null,
                sortDirection: null,
                filters: {}
            });

            bindings.filters.value.category = 'Cafe';
            bindings.filters.value.account = 'Wallet';
            bindings.filters.value.tag = '';
            bindings.filters.value.signal = 'learning';
            bindings.filters.value.annotation = 'needs-review';
            bindings.filters.value.description = 'coffee';
            transaction.comment = 'edited draft';
            emit.mockClear();

            bindings.updatePreviewTablePage(2);

            expect(emit).toHaveBeenCalledTimes(1);
            expect(emit).toHaveBeenCalledWith('requestPage', 2, 10, {
                sortBy: null,
                sortDirection: null,
                filters: {
                    category: '8',
                    account: 'wallet',
                    tag: '__none__',
                    signal: 'learning',
                    annotation: 'needs-review',
                    description: 'coffee'
                }
            });
            expect(bindings.serverPagedDrafts.value.get(11).comment).toBe('edited draft');

            transaction.comment = 'fresh server payload';
            bindings.rehydrateCurrentPageDrafts();
            expect(transaction.comment).toBe('edited draft');

            emit.mockClear();
            bindings.emitServerPagedRequest(2, 10);
            expect(emit).not.toHaveBeenCalled();

            bindings.filters.value.signal = ' learning ';
            bindings.emitServerPagedRequest(2, 10);
            expect(emit).not.toHaveBeenCalled();

            bindings.emitServerPagedRequest(2, 10, { force: true });
            expect(emit).toHaveBeenCalledTimes(1);
            bindings.filters.value.signal = 'learning';

            bindings.updatePreviewTableSort([
                { key: 'parserId', order: 'asc' },
                { key: 'sourceAmountCents', order: 'desc' }
            ]);
            expect(emit).toHaveBeenLastCalledWith('requestPage', 1, 10, {
                sortBy: 'sourceAmountCents',
                sortDirection: 'desc',
                filters: expect.objectContaining({ category: '8', account: 'wallet' })
            });

            emit.mockClear();
            bindings.updatePreviewTablePageSize(0);
            expect(emit).toHaveBeenCalledWith('requestPage', 1, 1, {
                sortBy: 'sourceAmountCents',
                sortDirection: 'desc',
                filters: expect.objectContaining({ signal: 'learning' })
            });
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('an edit made after a page request remains authoritative over its stale response', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const transaction = createTransaction({
            id: 12,
            categoryId: '',
            categoryName: 'Unrecognized category'
        });
        try {
            const bindings = createBindings({
                transactions: [transaction],
                serverPaged: true,
                sessionId: 'stale-page-session',
                total: 1
            });

            transaction.categoryId = '8';
            bindings.onTransactionDataDraftChange(transaction);

            transaction.categoryId = '';
            transaction.actualCategoryName = '';
            transaction.originalCategoryName = 'Stale missing category';
            bindings.rehydrateCurrentPageDrafts();

            expect(transaction.categoryId).toBe('8');
            expect(transaction.actualCategoryName).toBe('Cafe');
            expect(bindings.hasMissingCategoryIssue(transaction)).toBe(false);

            const withoutPreviewId = createTransaction({ id: 14 });
            delete (withoutPreviewId as ImportTransaction & { _previewId?: number })._previewId;
            const draftCount = bindings.serverPagedDrafts.value.size;
            bindings.cacheServerPagedDraft(withoutPreviewId);
            expect(bindings.serverPagedDrafts.value.size).toBe(draftCount);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('a stale page replacement keeps the active editor bound to the current row object', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const original = createTransaction({ id: 13, comment: 'draft' });
        const replacement = createTransaction({ id: 13, comment: 'server' });
        try {
            const bindings = createBindings({
                transactions: [original],
                serverPaged: true,
                sessionId: 'editor-rebind-session',
                total: 1
            });

            bindings.editTransaction(original);
            bindings.rebindEditingTransactionToCurrentPage([replacement]);

            expect(bindings.editingTransaction.value).not.toBe(original);
            expect(bindings.editingTransaction.value._previewId).toBe(13);
            expect(bindings.editingTransaction.value.comment).toBe('server');
            expect(bindings.editingTags.value).toEqual(replacement.tagIds);

            bindings.editingTransaction.value.comment = 'committed draft';
            bindings.commitEditingTransactionDraft();
            expect(bindings.editingTransaction.value).toBeNull();
            expect(bindings.serverPagedDrafts.value.get(13).comment).toBe('committed draft');
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('server metadata drives facets, counts, headers, and page boundaries', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const selected = createTransaction({ id: 21, selected: true });
        const unselected = createTransaction({ id: 22, selected: false });
        try {
            const bindings = createBindings({
                transactions: [selected, unselected],
                serverPaged: true,
                sessionId: 'metadata-session',
                total: 42,
                metadata: {
                    counts: {
                        total: 42,
                        selected: 20,
                        selected_invalid: 3,
                        annotations: { 'needs-review': 5 }
                    },
                    facets: {
                        categories: [
                            { value: '8', label: 'Cafe', count: 10 },
                            { value: '9', label: 'Cafe', count: 2 },
                            { value: '7', label: 'Food', count: 12 },
                            { value: '', label: '', count: 1 }
                        ],
                        accounts: [
                            { value: 'wallet', label: 'Fallback wallet', count: 9 },
                            { value: 'missing', label: 'External account', count: 4 }
                        ],
                        tags: [
                            { value: 'tag-known', label: 'Known tag', count: 8 },
                            { value: 'tag-known', label: 'Known tag', count: 2 }
                        ]
                    }
                }
            });

            expect(bindings.metadataFacetLabels(bindings.previewMetadata.value.facets.categories)).toEqual([
                'Cafe',
                'Food'
            ]);
            expect(bindings.buildFacetValueByLabel(bindings.previewMetadata.value.facets.categories)).toEqual({
                Food: '7'
            });
            expect(bindings.metadataAccountFacetLabels(bindings.previewMetadata.value.facets.accounts)).toEqual([
                'Wallet',
                'External account'
            ]);
            expect(bindings.buildAccountFacetValueByLabel(bindings.previewMetadata.value.facets.accounts)).toEqual({
                Wallet: 'wallet',
                'External account': 'missing'
            });
            expect(bindings.allUsedCategoryNames.value).toEqual(['Cafe', 'Food']);
            expect(bindings.allUsedAccountNames.value).toEqual(['Wallet', 'External account']);
            expect(bindings.allUsedTagNames.value).toEqual(['Known tag']);
            expect(bindings.totalPageCount.value).toBe(5);
            expect(bindings.importTransactionsTablePageOptions.value).toEqual([
                { value: 10, name: '10' },
                { value: 20, name: '20' }
            ]);
            expect(bindings.tableTransactions.value).toEqual([selected, unselected]);
            expect(bindings.selectedImportTransactionCount.value).toBe(20);
            expect(bindings.selectedInvalidTransactionCount.value).toBe(3);
            expect(bindings.annotationTransactionCount.value).toBe(5);
            expect(bindings.anyButNotAllTransactionSelected.value).toBe(true);
            expect(bindings.allTransactionSelected.value).toBe(false);

            const sortableByKey = Object.fromEntries(bindings.importTransactionHeaders.value.map((header: any) => [header.value, header.sortable]));
            expect(sortableByKey).toMatchObject({
                time: true,
                type: true,
                sourceAmountCents: true,
                parserId: false,
                actualCategoryName: false
            });

            unselected.selected = true;
            bindings.serverPagedDrafts.value = new Map(bindings.serverPagedDrafts.value);
            expect(bindings.selectedImportTransactionCount.value).toBe(21);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('local invalid getters and selection actions preserve exact row semantics', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const valid = createTransaction({ id: 31, selected: false });
        const missingExpense = createTransaction({
            id: 32,
            categoryId: '',
            categoryName: 'Mystery category',
            sourceAccountId: '',
            sourceAccountName: 'Ghost wallet',
            selected: false,
            tagIds: ['', 'tag-known'],
            tagNames: ['Imported tag', 'Known tag']
        });
        const missingIncome = createTransaction({
            id: 33,
            type: 2,
            categoryId: '',
            categoryName: '',
            sourceAccountId: 'hidden',
            sourceAccountName: 'Hidden',
            selected: false,
            tagIds: [''],
            tagNames: ['']
        });
        const missingTransfer = createTransaction({
            id: 34,
            type: 4,
            categoryId: '8',
            destinationAccountId: '',
            destinationAccountName: 'Ghost destination',
            selected: false
        });
        try {
            const bindings = createBindings({ transactions: [valid, missingExpense, missingIncome, missingTransfer] });

            expect(bindings.getCurrentInvalidCategoryNames(3)).toEqual([
                { name: 'Mystery category', value: 'Mystery category' }
            ]);
            expect(bindings.getCurrentInvalidCategoryNames(2)).toEqual([
                { name: '(Empty)', value: '' }
            ]);
            expect(bindings.getCurrentInvalidAccountNames()).toEqual([
                { name: 'Ghost wallet', value: 'Ghost wallet' },
                { name: 'Hidden', value: 'Hidden' },
                { name: 'Ghost destination', value: 'Ghost destination' }
            ]);
            expect(bindings.getCurrentInvalidTagNames()).toEqual([
                { name: 'Imported tag', value: 'Imported tag' }
            ]);
            expect(bindings.getAllOriginalTagNames()).toEqual([
                { name: 'Known tag', value: 'Known tag' },
                { name: 'Imported tag', value: 'Imported tag' },
                { name: '(Empty)', value: '' }
            ]);
            expect(bindings.isTagValid(valid.tagIds, 0)).toBe(true);
            expect(bindings.isTagValid(missingExpense.tagIds, 0)).toBe(false);
            expect(bindings.isKnownAccountId('wallet')).toBe(true);
            expect(bindings.isKnownAccountId('hidden')).toBe(false);
            expect(bindings.isKnownCategoryIdForType('8', 3)).toBe(true);
            expect(bindings.isKnownCategoryIdForType('missing', 3)).toBe(false);

            await bindings.selectAllValid();
            expect([valid, missingExpense, missingIncome, missingTransfer].map(row => row.selected)).toEqual([
                true,
                false,
                false,
                false
            ]);
            await bindings.selectAllInvalid();
            expect([valid, missingExpense, missingIncome, missingTransfer].map(row => row.selected)).toEqual([
                true,
                true,
                true,
                true
            ]);
            await bindings.selectNone();
            expect(bindings.getSelectedPreviewCount()).toBe(0);
            await bindings.selectAll();
            expect(bindings.getSelectedPreviewIds()).toEqual([31, 32, 33, 34]);
            await bindings.selectInvert();
            expect(bindings.getSelectedPreviewIds()).toEqual([]);

            valid.selected = true;
            missingExpense.selected = true;
            expect(bindings.getSelectedPreviewUpdates().map((update: any) => update.id)).toEqual([31, 32]);

            bindings.editTransaction(valid);
            expect(bindings.editingTransaction.value._previewId).toBe(31);
            bindings.editingTags.value = ['tag-known'];
            bindings.editTransaction(missingExpense);
            expect(valid.tagIds).toEqual(['tag-known']);
            expect(valid.isManuallyAnnotated).toBe(true);
            expect(bindings.editingTransaction.value._previewId).toBe(32);
            bindings.editTransaction(bindings.editingTransaction.value);
            expect(bindings.editingTransaction.value).toBeNull();

            bindings.filters.value.category = 'Cafe';
            bindings.filters.value.signal = 'learning';
            bindings.changeCustomDateFilter(100, 200);
            bindings.setCountPerPage(25);
            expect(bindings.getCurrentPreviewPageSize()).toBe(25);
            bindings.reset();
            expect(bindings.filters.value).toEqual({
                minDatetime: null,
                maxDatetime: null,
                transactionType: null,
                category: null,
                account: null,
                tag: null,
                signal: null,
                annotation: null,
                description: null
            });
            expect(bindings.getCurrentPreviewPage()).toBe(1);
            expect(bindings.getCurrentPreviewPageSize()).toBe(10);
            expect(bindings.tableSortBy.value).toEqual([]);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('server selection sends scoped filters and rolls local state back on failure', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const emit = jest.fn();
        const first = createTransaction({ id: 41, selected: false });
        const second = createTransaction({ id: 42, selected: false });
        try {
            const bindings = createBindings({
                transactions: [first, second],
                emit,
                serverPaged: true,
                sessionId: 'selection/session',
                total: 8,
                metadata: {
                    counts: { total: 8, selected: 0 },
                    selection_hash: 'fnv1a32:10000000',
                    facets: { categories: [{ value: '8', label: 'Cafe', count: 8 }] }
                }
            });
            bindings.snackbar.value = { showError: mockShowError, showMessage: mockShowMessage };
            bindings.filters.value.category = 'Cafe';
            await Promise.resolve();
            emit.mockClear();
            mockApplyImportPreviewSelectionAction.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 8,
                        applied_preview_updates: 0,
                        selectionAction: 'select_all',
                        metadata: {
                            counts: { total: 8, selected: 8 },
                            selection_hash: 'fnv1a32:20000000',
                            facets: { categories: [{ value: '8', label: 'Cafe', count: 8 }] }
                        },
                        previewItems: []
                    }
                }
            });

            await bindings.selectAll();

            expect([first.selected, second.selected]).toEqual([true, true]);
            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledWith({
                sessionId: 'selection/session',
                selectionAction: 'select_all',
                filters: { category: '8' },
                expectedSelectionHash: 'fnv1a32:10000000',
                previewUpdates: undefined
            });
            expect(bindings.previewMetadata.value.counts.selected).toBe(8);
            expect(emit).not.toHaveBeenCalledWith(
                'requestPage',
                expect.anything(),
                expect.anything(),
                expect.anything()
            );

            mockApplyImportPreviewSelectionAction.mockRejectedValueOnce(new Error('selection conflict'));
            await bindings.selectNone();

            expect([first.selected, second.selected]).toEqual([true, true]);
            expect(mockApplyImportPreviewSelectionAction).toHaveBeenLastCalledWith({
                sessionId: 'selection/session',
                selectionAction: 'select_none',
                filters: { category: '8' },
                expectedSelectionHash: 'fnv1a32:20000000',
                previewUpdates: undefined
            });
            expect(bindings.serverPagedSelectionBusy.value).toBe(false);

            bindings.serverPagedSelectionBusy.value = true;
            const callsBeforeBusyAction = mockApplyImportPreviewSelectionAction.mock.calls.length;
            await bindings.selectInvert();
            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledTimes(callsBeforeBusyAction);
            expect([first.selected, second.selected]).toEqual([true, true]);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('selection invalidates an older page response before the PUT completes without issuing another GET', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const coordinator = new PreviewPageRequestCoordinator();
        const staleResponse = deferred<string>();
        let renderedPage = 'current';
        const staleHandle = coordinator.begin('selection-race?page=1')!;
        const staleRun = staleResponse.promise.then(value => {
            if (coordinator.isCurrent(staleHandle)) {
                renderedPage = value;
            }
            coordinator.finish(staleHandle);
        });
        const emit = jest.fn((event: string) => {
            if (event === 'invalidatePageRequest') {
                coordinator.abort();
            }
        });
        const row = createTransaction({ id: 49, selected: false });
        try {
            const bindings = createBindings({
                transactions: [row],
                emit,
                serverPaged: true,
                sessionId: 'selection-race',
                total: 1,
                metadata: { counts: { total: 1, selected: 0 } }
            });
            const selectionResponse = deferred<any>();
            mockApplyImportPreviewSelectionAction.mockImplementationOnce(() => selectionResponse.promise);

            const selectionRun = bindings.selectAll();
            staleResponse.resolve('stale');
            await staleRun;

            expect(staleHandle.controller.signal.aborted).toBe(true);
            expect(renderedPage).toBe('current');
            expect(row.selected).toBe(true);

            selectionResponse.resolve({
                data: {
                    result: {
                        updated: 1,
                        applied_preview_updates: 0,
                        selectionAction: 'select_all',
                        metadata: { counts: { total: 1, selected: 1 } },
                        previewItems: []
                    }
                }
            });
            await selectionRun;

            expect(row.selected).toBe(true);
            expect(bindings.previewMetadata.value.counts.selected).toBe(1);
            expect(emit).toHaveBeenCalledWith('invalidatePageRequest');
            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledTimes(1);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('conditional server selection submits validity drafts in the same request', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const edited = createTransaction({
            id: 43,
            categoryId: '',
            sourceAccountId: 'wallet',
            selected: false
        });
        try {
            (edited as ImportTransaction & { _rowVersion: number })._rowVersion = 3;
            const bindings = createBindings({
                transactions: [edited],
                serverPaged: true,
                sessionId: 'conditional/selection',
                total: 1,
                metadata: {
                    counts: { total: 1, selected: 0 },
                    selection_hash: 'fnv1a32:11111111'
                }
            });
            edited.categoryId = '8';
            edited.isManuallyAnnotated = true;
            bindings.onTransactionDataDraftChange(edited);
            mockFetch.mockResolvedValueOnce({
                ok: true,
                json: async () => ({
                    success: true,
                    data: { metadata: { counts: { total: 1, selected: 1 } } }
                })
            });
            mockApplyImportPreviewSelectionAction.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 1,
                        applied_preview_updates: 1,
                        selectionAction: 'select_valid',
                        metadata: {
                            counts: { total: 1, selected: 1 },
                            selection_hash: 'fnv1a32:22222222'
                        },
                        previewItems: [{
                            id: 43,
                            row_version: 4,
                            preview_type: '支出',
                            preview_amount_cents: edited.sourceAmountCents,
                            preview_destination_amount_cents: edited.destinationAmountCents,
                            category_id: 8,
                            preview_selected: true
                        }]
                    }
                }
            });

            await bindings.selectAllValid();

            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledWith({
                sessionId: 'conditional/selection',
                selectionAction: 'select_valid',
                filters: {},
                expectedSelectionHash: 'fnv1a32:11111111',
                previewUpdates: [expect.objectContaining({
                    id: 43,
                    expected_row_version: 3,
                    category_id: 8,
                    is_manually_annotated: true
                })]
            });
            expect((edited as ImportTransaction & { _rowVersion: number })._rowVersion).toBe(4);
            expect(edited.selected).toBe(true);
            expect(bindings.previewMetadata.value.counts.selected).toBe(1);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('conditional server selection also submits validity drafts cached from another page', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const currentPage = createTransaction({ id: 44, selected: false });
        const previousPage = createTransaction({
            id: 45,
            categoryId: '',
            sourceAccountId: 'wallet',
            selected: false
        });
        try {
            (previousPage as ImportTransaction & { _rowVersion: number })._rowVersion = 5;
            const bindings = createBindings({
                transactions: [currentPage],
                serverPaged: true,
                sessionId: 'cross-page/selection',
                total: 2,
                metadata: {
                    counts: { total: 2, selected: 0 },
                    selection_hash: 'fnv1a32:30000000'
                }
            });
            bindings.serverPagedDraftBaselines.value = new Map([[
                45,
                bindings.captureImportPreviewEditableDraftState(previousPage)
            ]]);
            previousPage.categoryId = '8';
            previousPage.isManuallyAnnotated = true;
            bindings.serverPagedDrafts.value = new Map([[45, previousPage]]);
            mockApplyImportPreviewSelectionAction.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 2,
                        applied_preview_updates: 1,
                        selectionAction: 'select_valid',
                        metadata: { counts: { total: 2, selected: 2 } },
                        previewItems: [{
                            id: 45,
                            row_version: 6,
                            preview_type: '支出',
                            preview_amount_cents: previousPage.sourceAmountCents,
                            preview_destination_amount_cents: previousPage.destinationAmountCents,
                            category_id: 8,
                            preview_selected: true
                        }]
                    }
                }
            });

            await bindings.selectAllValid();

            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledWith(expect.objectContaining({
                expectedSelectionHash: 'fnv1a32:30000000',
                previewUpdates: [
                expect.objectContaining({
                    id: 45,
                    expected_row_version: 5,
                    category_id: 8,
                    is_manually_annotated: true
                })
                ]
            }));
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('server selection keeps non-selection drafts cached from another page', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const currentPage = createTransaction({ id: 46, selected: false });
        const previousPage = createTransaction({ id: 47, selected: false, comment: 'before edit' });
        try {
            const bindings = createBindings({
                transactions: [currentPage],
                serverPaged: true,
                sessionId: 'draft-preserving/selection',
                total: 2,
                metadata: { counts: { total: 2, selected: 0 } }
            });
            bindings.serverPagedDraftBaselines.value = new Map([[
                47,
                bindings.captureImportPreviewEditableDraftState(previousPage)
            ]]);
            previousPage.comment = 'edited on previous page';
            previousPage.selected = true;
            bindings.serverPagedDrafts.value = new Map([[47, previousPage]]);
            mockApplyImportPreviewSelectionAction.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 2,
                        applied_preview_updates: 0,
                        selectionAction: 'select_all',
                        metadata: { counts: { total: 2, selected: 2 } },
                        previewItems: []
                    }
                }
            });

            await bindings.selectAll();

            expect(bindings.serverPagedDrafts.value.get(47)?.comment).toBe('edited on previous page');
            expect(bindings.serverPagedDrafts.value.get(47)?.selected).toBe(true);
            expect(bindings.serverPagedDraftBaselines.value.has(47)).toBe(true);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('a second conditional selection includes edits made after the first selection succeeds', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const edited = createTransaction({ id: 48, categoryId: '', sourceAccountId: 'wallet' });
        try {
            (edited as ImportTransaction & { _rowVersion: number })._rowVersion = 1;
            const bindings = createBindings({
                transactions: [edited],
                serverPaged: true,
                sessionId: 'repeat/conditional-selection',
                total: 1,
                metadata: {
                    counts: { total: 1, selected: 0 },
                    selection_hash: 'fnv1a32:40000000'
                }
            });
            mockApplyImportPreviewSelectionAction.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 1,
                        applied_preview_updates: 0,
                        selectionAction: 'select_all',
                        metadata: {
                            counts: { total: 1, selected: 1 },
                            selection_hash: 'fnv1a32:50000000'
                        },
                        previewItems: []
                    }
                }
            });
            await bindings.selectAll();

            edited.categoryId = '8';
            edited.isManuallyAnnotated = true;
            bindings.onTransactionDataDraftChange(edited);
            mockApplyImportPreviewSelectionAction.mockResolvedValueOnce({
                data: {
                    result: {
                        updated: 1,
                        applied_preview_updates: 1,
                        selectionAction: 'select_valid',
                        metadata: { counts: { total: 1, selected: 1 } },
                        previewItems: [{
                            id: 48,
                            row_version: 2,
                            preview_type: '支出',
                            preview_amount_cents: edited.sourceAmountCents,
                            preview_destination_amount_cents: edited.destinationAmountCents,
                            category_id: 8,
                            preview_selected: true
                        }]
                    }
                }
            });
            await bindings.selectAllValid();

            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledTimes(2);
            expect(mockApplyImportPreviewSelectionAction).toHaveBeenLastCalledWith(expect.objectContaining({
                expectedSelectionHash: 'fnv1a32:50000000',
                previewUpdates: [expect.objectContaining({
                    id: 48,
                    expected_row_version: 1,
                    category_id: 8,
                    is_manually_annotated: true
                })]
            }));
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('selection conflict rebases metadata and authoritative rows without retrying', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const edited = createTransaction({
            id: 53,
            categoryId: '',
            sourceAccountId: 'wallet',
            selected: false
        });
        const conflictError = new Error('selection changed');
        try {
            (edited as ImportTransaction & { _rowVersion: number })._rowVersion = 2;
            const bindings = createBindings({
                transactions: [edited],
                serverPaged: true,
                sessionId: 'selection-conflict',
                total: 1,
                metadata: {
                    counts: { total: 1, selected: 0 },
                    selection_hash: 'fnv1a32:60000000'
                }
            });
            bindings.snackbar.value = { showError: mockShowError, showMessage: mockShowMessage };
            edited.categoryId = '8';
            edited.isManuallyAnnotated = true;
            bindings.onTransactionDataDraftChange(edited);
            mockApplyImportPreviewSelectionAction.mockRejectedValueOnce(conflictError);
            mockGetImportPreviewSelectionConflict.mockImplementationOnce(error => (
                error === conflictError
                    ? {
                        expected_selection_hash: 'fnv1a32:60000000',
                        actual_selection_hash: 'fnv1a32:70000000',
                        metadata: {
                            counts: { total: 1, selected: 0 },
                            selection_hash: 'fnv1a32:70000000'
                        },
                        previewItems: [{
                            id: 53,
                            row_version: 2,
                            preview_type: '支出',
                            preview_amount_cents: edited.sourceAmountCents,
                            preview_destination_amount_cents: edited.destinationAmountCents,
                            preview_selected: false
                        }]
                    }
                    : null
            ));

            await bindings.selectAllValid();

            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledTimes(1);
            expect(edited.selected).toBe(false);
            expect(edited.categoryId).toBe('');
            expect((edited as ImportTransaction & { _rowVersion: number })._rowVersion).toBe(2);
            expect(bindings.previewMetadata.value.selection_hash).toBe('fnv1a32:70000000');
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('row conflict rebases the authoritative row without retrying selection', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const edited = createTransaction({
            id: 54,
            categoryId: '',
            sourceAccountId: 'wallet',
            selected: false
        });
        const pending = createTransaction({
            id: 55,
            categoryId: '',
            sourceAccountId: 'wallet',
            selected: false
        });
        const conflictError = new Error('row changed');
        try {
            (edited as ImportTransaction & { _rowVersion: number })._rowVersion = 2;
            (pending as ImportTransaction & { _rowVersion: number })._rowVersion = 2;
            const bindings = createBindings({
                transactions: [edited, pending],
                serverPaged: true,
                sessionId: 'row-conflict',
                total: 2,
                metadata: {
                    counts: { total: 2, selected: 0 },
                    selection_hash: 'fnv1a32:80000000'
                }
            });
            bindings.snackbar.value = { showError: mockShowError, showMessage: mockShowMessage };
            edited.categoryId = '8';
            edited.isManuallyAnnotated = true;
            bindings.onTransactionDataDraftChange(edited);
            pending.categoryId = '8';
            pending.isManuallyAnnotated = true;
            bindings.onTransactionDataDraftChange(pending);
            mockApplyImportPreviewSelectionAction.mockRejectedValueOnce(conflictError);
            mockGetImportPreviewRowVersionConflict.mockImplementationOnce(error => (
                error === conflictError
                    ? {
                        expected_row_version: 2,
                        actual_row_version: 3,
                        previewItem: {
                            id: 54,
                            row_version: 3,
                            preview_type: '支出',
                            preview_amount_cents: edited.sourceAmountCents,
                            preview_destination_amount_cents: edited.destinationAmountCents,
                            preview_selected: false
                        }
                    }
                    : null
            ));

            await bindings.selectAllValid();

            expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledTimes(1);
            expect(edited.selected).toBe(false);
            expect(edited.categoryId).toBe('');
            expect((edited as ImportTransaction & { _rowVersion: number })._rowVersion).toBe(3);
            expect(bindings.previewMetadata.value.selection_hash).toBe('fnv1a32:80000000');
            expect(bindings.buildConditionalSelectionPreviewUpdates('select_valid'))
                .toEqual([expect.objectContaining({
                    id: 55,
                    expected_row_version: 2,
                    category_id: 8
                })]);
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('known-id guards and batch edits update only eligible selected rows', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const eligible = createTransaction({ id: 51, categoryId: '', sourceAccountId: '', selected: true });
        const untouched = createTransaction({ id: 52, categoryId: '', sourceAccountId: '', selected: false });
        try {
            const bindings = createBindings({ transactions: [eligible, untouched] });

            expect(bindings.assignCategoryIdIfKnown(eligible, 'missing')).toBe(false);
            expect(bindings.assignSourceAccountIdIfKnown(eligible, 'hidden')).toBe(false);
            expect(bindings.assignDestinationAccountIdIfKnown(eligible, 'missing')).toBe(false);
            expect(eligible.categoryId).toBe('');
            expect(eligible.sourceAccountId).toBe('');

            bindings.batchCategoryType.value = 3;
            bindings.batchCategoryId.value = '8';
            bindings.applyBatchCategory();
            expect(eligible).toMatchObject({
                type: 3,
                categoryId: '8',
                actualCategoryName: 'Cafe',
                isManuallyAnnotated: true
            });
            expect(untouched.categoryId).toBe('');
            expect(bindings.batchCategoryId.value).toBe('');

            bindings.batchAccountId.value = 'wallet';
            bindings.applyBatchAccount();
            expect(eligible).toMatchObject({
                sourceAccountId: 'wallet',
                actualSourceAccountName: 'Wallet',
                originalSourceAccountName: 'Wallet'
            });
            expect(untouched.sourceAccountId).toBe('');
            expect(bindings.batchAccountId.value).toBe('');

            bindings.batchCategoryId.value = 'missing';
            bindings.applyBatchCategory();
            expect(eligible.type).toBe(3);
            expect(eligible.categoryId).toBe('8');
        } finally {
            warnSpy.mockRestore();
        }
    });
});
