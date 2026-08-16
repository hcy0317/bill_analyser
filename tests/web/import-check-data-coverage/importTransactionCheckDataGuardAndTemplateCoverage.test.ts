import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockGetLLMMemoryEvents = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockApplyImportPreviewSelectionAction = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockPatchImportPreviewSelection = jest.fn<(...args: Array<unknown>) => Promise<any>>();
const mockGetImportPreviewSelectionConflict = jest.fn<(...args: Array<unknown>) => any>();
const mockGetImportPreviewRowVersionConflict = jest.fn<(...args: Array<unknown>) => any>();
const mockLoadAllCategories = jest.fn();
const mockLoadAllAccounts = jest.fn();
let mockCurrentToken = 'coverage-token';

const expenseChild = {
    id: 'expense-child',
    parentId: 'expense-parent',
    name: 'Cafe',
    type: 3,
    icon: 'coffee',
    color: '#ff8800',
    hidden: false,
    subCategories: [] as any[]
};
const categoryFixtures = [
    { id: 'expense-parent', parentId: '0', name: 'Food', type: 3, icon: 'food', color: '#ff8800', hidden: false, subCategories: [expenseChild] },
    { id: 'income-parent', parentId: '0', name: 'Salary', type: 2, icon: 'income', color: '#00aa66', hidden: false, subCategories: [] as any[] },
    { id: 'transfer-parent', parentId: '0', name: 'Transfer', type: 4, icon: 'transfer', color: '#0088ff', hidden: false, subCategories: [] as any[] },
    { id: 'investment-parent', parentId: '0', name: 'Investment', type: 5, icon: 'investment', color: '#8866ff', hidden: false, subCategories: [] as any[] }
];
const categoryMap = Object.fromEntries([...categoryFixtures, expenseChild].map(category => [category.id, category]));

const wallet = { id: 'wallet', name: 'Wallet', currency: 'CNY', category: 1, icon: 'wallet', color: '#0088ff', hidden: false };
const bank = { id: 'bank', name: 'Bank', currency: 'USD', category: 2, icon: 'bank', color: '#00aa66', hidden: false };
const hiddenAccount = { id: 'hidden', name: 'Hidden', currency: 'CNY', category: 1, icon: 'wallet', color: '#777777', hidden: true };
const knownTag = { id: 'known-tag', name: 'Known Tag', hidden: false };

type CapturedAction = {
    component: string;
    event: string;
    label: string;
    handler: (...args: any[]) => unknown;
};

const mockUiActions: CapturedAction[] = [];
const mockSignalActions: Record<string, (...args: any[]) => unknown> = {};

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
        allPlainAccounts: [wallet, bank, hiddenAccount],
        allVisiblePlainAccounts: [wallet, bank],
        allAccountsMap: { wallet, bank, hidden: hiddenAccount },
        allAccounts: [wallet, bank, hiddenAccount],
        loadAllAccounts: mockLoadAllAccounts
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => ({
        allTransactionCategories: {
            2: [categoryFixtures[1]],
            3: [categoryFixtures[0]],
            4: [categoryFixtures[2]],
            5: [categoryFixtures[3]]
        },
        allTransactionCategoriesMap: categoryMap,
        loadAllCategories: mockLoadAllCategories
    })
}));
jest.mock('@/stores/transactionTag.ts', () => ({
    useTransactionTagsStore: () => ({
        allTransactionTags: [knownTag],
        allTransactionTagsMap: { 'known-tag': knownTag }
    })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getLLMMemoryEvents: mockGetLLMMemoryEvents,
        applyImportPreviewSelectionAction: mockApplyImportPreviewSelectionAction,
        patchImportPreviewSelection: mockPatchImportPreviewSelection,
        getImportPreviewSelectionConflict: mockGetImportPreviewSelectionConflict,
        getImportPreviewRowVersionConflict: mockGetImportPreviewRowVersionConflict
    }
}));
jest.mock('@/lib/server_settings.ts', () => ({ isTransactionFromAIImageRecognitionEnabled: () => true }));
jest.mock('@/lib/userstate.ts', () => ({ getCurrentToken: () => mockCurrentToken }));
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

jest.mock('@/views/desktop/transactions/import/tabs/ImportPreviewSignalCell.vue', () => {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return {
        __esModule: true,
        default: defineComponent({
            name: 'CoverageSignalCellStub',
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => () => {
                for (const [event, handler] of Object.entries(attrs)) {
                    if (event.startsWith('on') && typeof handler === 'function') {
                        mockSignalActions[event] = handler as (...args: any[]) => unknown;
                    }
                }
                return h('div', { 'data-testid': 'coverage-signal-cell' });
            }
        })
    };
});

for (const componentPath of [
    '@/components/desktop/PaginationButtons.vue',
    '@/components/desktop/SnackBar.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchReplaceAllTypesDialog.vue',
    '@/views/desktop/transactions/import/dialogs/BatchCreateDialog.vue',
    '@/views/desktop/transactions/import/dialogs/ImportLearningSuggestionDialog.vue',
    '@/views/desktop/categories/list/dialogs/EditDialog.vue',
    '@/views/desktop/accounts/list/dialogs/EditDialog.vue',
    '@/views/desktop/transactions/list/dialogs/EditDialog.vue'
]) {
    jest.mock(componentPath, () => {
        const { defineComponent, h } = jest.requireActual('vue') as any;
        return {
            __esModule: true,
            default: defineComponent({
                name: 'CoverageImportedStub',
                inheritAttrs: false,
                setup: (_props: unknown, { attrs, slots }: any) => () => {
                    captureActions('imported-stub', attrs, []);
                    return h('div', attrs, Object.values(slots).flatMap(slot => (
                        typeof slot === 'function' ? (slot as any)({}) : []
                    )));
                }
            })
        };
    });
}

const { createSSRApp, defineComponent, h, proxyRefs } = jest.requireActual('vue') as any;
const { renderToString } = jest.requireActual('vue/server-renderer') as any;

import { ImportTransaction } from '@/models/imported_transaction.ts';
import ImportTransactionCheckDataTab from '@/views/desktop/transactions/import/tabs/ImportTransactionCheckDataTab.vue';

function vnodeText(value: unknown): string {
    if (value === null || value === undefined || typeof value === 'boolean') return '';
    if (typeof value === 'string' || typeof value === 'number') return String(value);
    if (Array.isArray(value)) return value.map(vnodeText).join(' ');
    if (typeof value === 'object') {
        const vnode = value as { children?: unknown; props?: Record<string, unknown> };
        return [vnodeText(vnode.children), vnodeText(vnode.props?.['text']), vnodeText(vnode.props?.['title'])]
            .filter(Boolean)
            .join(' ');
    }
    return '';
}

function captureActions(component: string, attrs: Record<string, unknown>, children: unknown[]): void {
    const label = [vnodeText(children), vnodeText(attrs['title']), vnodeText(attrs['text'])]
        .filter(Boolean)
        .join(' ')
        .trim();
    for (const [event, value] of Object.entries(attrs)) {
        if (!event.startsWith('on')) continue;
        for (const handler of Array.isArray(value) ? value : [value]) {
            if (typeof handler === 'function') {
                mockUiActions.push({ component, event, label, handler: handler as (...args: any[]) => unknown });
            }
        }
    }
}

function createUiStub(component: string): any {
    return defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => {
            const payload = {
                props: {},
                index: 0,
                item: { value: 'known-tag', raw: { hidden: false }, title: 'Known Tag' }
            };
            const children = Object.values(slots).flatMap(slot => (
                typeof slot === 'function' ? (slot as any)(payload) : []
            ));
            captureActions(component, attrs, children);
            return h(component, attrs, children);
        }
    });
}

let forcedNamedSlotItem: ImportTransaction | null = null;
const DataTableStub = defineComponent({
    props: { items: { type: Array, default: () => [] } },
    setup: (props: any, { slots }: any) => () => {
        const children: unknown[] = [slots.default?.(), slots['header.data-table-select']?.(), slots.bottom?.()];
        for (const item of forcedNamedSlotItem ? [forcedNamedSlotItem] : props.items) {
            for (const [name, slot] of Object.entries(slots)) {
                if (name.startsWith('item.') && typeof slot === 'function') {
                    children.push((slot as any)({ item }));
                }
            }
        }
        return h('div', { 'data-testid': 'coverage-data-table' }, children);
    }
});

function registerUiStubs(app: any): void {
    app.component('v-data-table', DataTableStub);
    for (const name of [
        'v-dialog', 'v-card', 'v-card-title', 'v-card-text', 'v-card-actions', 'v-btn', 'v-btn-group',
        'v-chip', 'v-list', 'v-list-item', 'v-list-item-title', 'v-list-item-subtitle', 'v-checkbox',
        'v-menu', 'v-divider', 'v-select', 'v-autocomplete', 'v-text-field', 'v-tabs', 'v-tab',
        'v-spacer', 'v-icon', 'v-alert', 'v-progress-circular', 'two-column-select', 'icon-select',
        'date-range-selection-dialog', 'amount-input', 'item-icon'
    ]) {
        app.component(name, createUiStub(name));
    }
    app.config.warnHandler = () => undefined;
}

type TransactionOptions = {
    type?: number;
    selected?: boolean;
    valid?: boolean;
    categoryId?: string;
    categoryName?: string;
    sourceAccountId?: string;
    sourceAccountName?: string;
    destinationAccountId?: string;
    destinationAccountName?: string;
    sourceCurrency?: string;
    destinationCurrency?: string;
    tagIds?: string[];
    tagNames?: string[];
    historyRewrite?: boolean;
};

function createTransaction(id: number, options: TransactionOptions = {}): ImportTransaction {
    const transaction = ImportTransaction.of({
        type: options.type ?? 3,
        categoryId: options.categoryId ?? 'expense-child',
        originalCategoryName: options.categoryName ?? 'Cafe',
        time: 1_788_480_000 + id,
        utcOffset: 480,
        sourceAccountId: options.sourceAccountId ?? 'wallet',
        originalSourceAccountName: options.sourceAccountName ?? 'Wallet',
        originalSourceAccountCurrency: options.sourceCurrency ?? 'CNY',
        destinationAccountId: options.destinationAccountId ?? '',
        originalDestinationAccountName: options.destinationAccountName ?? '',
        originalDestinationAccountCurrency: options.destinationCurrency ?? 'USD',
        sourceAmountCents: -1_000 - id,
        destinationAmountCents: 2_000 + id,
        tagIds: options.tagIds ?? ['known-tag'],
        originalTagNames: options.tagNames ?? ['Known Tag'],
        counterparty: `Merchant ${id}`,
        paymentMethod: 'Card',
        comment: `Memo ${id}`,
        selected: options.selected ?? true,
        matching: {
            parser: { id: 'coverage', tags: ['guard-template'] },
            transfer: { review_status: '', reviewed_type: '', suppressed: false },
            learning: { review_status: '', source: 'model', model_version: 'coverage' },
            llm: { review_status: '' },
            reconciliation: options.historyRewrite ? {
                planned_operation: 'update_history',
                history_bill_id: id,
                history_bill_version: 2,
                operation_id: `operation-${id}`,
                acknowledgement_token: `ack-${id}`,
                destructive_ack_required: true,
                notice: 'History rewrite notice'
            } : undefined,
            annotation: options.historyRewrite ? { history_rewrite_notice: 'History rewrite notice' } : {}
        }
    } as never, id);
    Object.assign(transaction, {
        index: id,
        valid: options.valid ?? true,
        selected: options.selected ?? true
    });
    (transaction as ImportTransaction & { _previewId: number })._previewId = id;
    return transaction;
}

type RenderOptions = {
    transactions: ImportTransaction[];
    serverPaged?: boolean;
    total?: number;
    sessionId?: string;
    configure?: (bindings: any) => void;
};

function propsFor(options: RenderOptions): Record<string, unknown> {
    return {
        importTransactions: options.transactions,
        disabled: false,
        sessionId: options.sessionId ?? 'coverage-session',
        serverPaged: options.serverPaged ?? false,
        totalImportTransactionCount: options.total ?? options.transactions.length,
        previewMetadata: null
    };
}

function createBindings(options: RenderOptions, emit = jest.fn()): any {
    return (ImportTransactionCheckDataTab as any).setup(propsFor(options), { emit, expose: jest.fn() });
}

async function renderDesktop(options: RenderOptions): Promise<{ bindings: any; html: string }> {
    mockUiActions.length = 0;
    for (const key of Object.keys(mockSignalActions)) delete mockSignalActions[key];
    mockCurrentToken = 'coverage-token';
    forcedNamedSlotItem = null;
    let bindings: any;
    const props = propsFor(options);
    const Harness = defineComponent({
        setup: () => {
            bindings = (ImportTransactionCheckDataTab as any).setup(props, { emit: jest.fn(), expose: jest.fn() });
            options.configure?.(bindings);
            const renderBindings = proxyRefs(bindings);
            return () => (ImportTransactionCheckDataTab as any).render(renderBindings, [], props, renderBindings, {}, {});
        }
    });
    const app = createSSRApp(Harness);
    registerUiStubs(app);
    const html = await renderToString(app);
    return { bindings, html };
}

function actionByHandlerText(text: string, event?: string): CapturedAction {
    const action = mockUiActions.find(candidate => (
        (!event || candidate.event === event) && String(candidate.handler).includes(text)
    ));
    expect(action).toBeDefined();
    return action!;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockUiActions.length = 0;
    for (const key of Object.keys(mockSignalActions)) delete mockSignalActions[key];
    mockGetLLMMemoryEvents.mockResolvedValue({ data: { result: { events: [] } } });
    mockApplyImportPreviewSelectionAction.mockResolvedValue({
        data: {
            result: {
                updated: 0,
                applied_preview_updates: 0,
                selectionAction: 'select_all',
                metadata: {},
                previewItems: []
            }
        }
    });
    mockPatchImportPreviewSelection.mockResolvedValue({
        data: { result: { updated: 0, metadata: null } }
    });
    mockGetImportPreviewSelectionConflict.mockReturnValue(null);
    mockGetImportPreviewRowVersionConflict.mockReturnValue(null);
    Object.defineProperty(globalThis, 'fetch', { configurable: true, writable: true, value: jest.fn() });
});

describe('ImportTransactionCheckDataTab guard and template coverage', () => {
    test('empty data evaluates derived filters and preserves every local guard', async () => {
        const bindings = createBindings({ transactions: [], sessionId: '' });

        expect(bindings.selectedExpenseTransactionCount.value).toBe(0);
        expect(bindings.selectedIncomeTransactionCount.value).toBe(0);
        expect(bindings.selectedTransferTransactionCount.value).toBe(0);
        expect(bindings.selectedRecurringMatchCount.value).toBe(0);
        expect(bindings.allUsedCategoryFilterGroups.value).toEqual(expect.any(Array));
        expect(bindings.allUsedAccountFilterGroups.value).toEqual(expect.any(Array));
        expect(bindings.allInvalidExpenseCategoryNames.value).toEqual([]);
        expect(bindings.allInvalidIncomeCategoryNames.value).toEqual([]);
        expect(bindings.allInvalidTransferCategoryNames.value).toEqual([]);
        expect(bindings.allInvalidAccountNames.value).toEqual([]);
        expect(bindings.allInvalidTransactionTagNames.value).toEqual([]);
        expect(bindings.allOriginalTransactionTagNames.value).toEqual([]);
        expect(bindings.currentDateFilterType.value).toEqual(expect.any(Number));

        await bindings.selectAllValid();
        await bindings.selectAllInvalid();
        await bindings.selectAllNeedsAnnotation();
        await bindings.selectAll();
        await bindings.selectNone();
        await bindings.selectInvert();
        bindings.openAnnotationDialog();
        bindings.editFirstAnnotationTransaction();

        expect(bindings.isKnownAccountId(null)).toBe(false);
        expect(bindings.isKnownAccountId('0')).toBe(false);
        expect(bindings.isKnownAccountId('missing')).toBe(false);
        expect(bindings.isKnownAccountId('hidden')).toBe(false);
        expect(bindings.isKnownAccountId('wallet')).toBe(true);
        expect(bindings.isKnownCategoryIdForType(null, 3)).toBe(false);
        expect(bindings.isKnownCategoryIdForType('0', 3)).toBe(false);
        expect(bindings.isKnownCategoryIdForType('missing', 3)).toBe(false);
        expect(bindings.isKnownCategoryIdForType('expense-child', 3)).toBe(true);

        const row = createTransaction(1);
        expect(bindings.assignCategoryIdIfKnown(row, 'missing')).toBe(false);
        expect(bindings.assignCategoryIdIfKnown(row, 'income-parent')).toBe(true);
        expect(bindings.assignSourceAccountIdIfKnown(row, 'hidden')).toBe(false);
        expect(bindings.assignSourceAccountIdIfKnown(row, 'bank')).toBe(true);
        expect(bindings.assignDestinationAccountIdIfKnown(row, 'missing')).toBe(false);
        expect(bindings.assignDestinationAccountIdIfKnown(row, 'wallet')).toBe(true);
    });

    test('recurring and history guards cover absent, editing, and candidate identities', async () => {
        const row = createTransaction(10, { historyRewrite: true });
        const bindings = createBindings({ transactions: [row] });
        const open = jest.fn();
        bindings.historyBillDetailDialog.value = { open };

        bindings.openHistoryBillDetail(0);
        bindings.openHistoryBillDetail(-1);
        bindings.openHistoryBillDetail(42);
        expect(open).toHaveBeenCalledTimes(1);
        expect(open).toHaveBeenCalledWith({ id: '42' });

        const missingPreview = createTransaction(11);
        delete (missingPreview as ImportTransaction & { _previewId?: number })._previewId;
        await bindings.openRecurringCandidateDialog(missingPreview);

        bindings.editingTransaction.value = row;
        await bindings.openRecurringCandidateDialog(row);
        await bindings.clearRecurringMatch(row);

        bindings.recurringCandidateTarget.value = null;
        bindings.selectedRecurringCandidateId.value = '';
        await bindings.applySelectedRecurringCandidate();
        await bindings.clearRecurringMatchFromDialog();

        bindings.recurringCandidateTarget.value = row;
        bindings.selectedRecurringCandidateId.value = 'missing-candidate';
        bindings.recurringCandidates.value = [{ id: 'known-candidate', matchReasons: [] }];
        await bindings.applySelectedRecurringCandidate();

        bindings.selectedRecurringCandidateId.value = 'known-candidate';
        await bindings.applySelectedRecurringCandidate();
        await bindings.clearRecurringMatchFromDialog();

        expect(bindings.formatRecurringCandidateSubtitle({ id: 1, matchReasons: null })).toBe('');
        expect(bindings.formatRecurringCandidateSubtitle({
            id: 2,
            matchedOccurrenceDate: '2026-07-15',
            matchReasons: ['date', 'amount']
        })).toContain('date | amount');
        bindings.recurringCandidates.value = [];
        expect(bindings.isBestRecurringCandidate({ id: 1 })).toBe(false);
        bindings.recurringCandidates.value = [{ id: 1 }];
        expect(bindings.isBestRecurringCandidate({ id: '1' })).toBe(true);
        expect(bindings.getRecurringCandidatePrimaryReason({ id: 1, matchReasons: null })).toBe('');
        expect(bindings.getRecurringCandidatePrimaryReason({ id: 1, matchReasons: [] })).toBe('');
        expect(bindings.getRecurringCandidatePrimaryReason({ id: 1, matchReasons: [''] })).toBe('');
        expect(bindings.getRecurringCandidatePrimaryReason({ id: 1, matchReasons: ['date'] })).toBe('date');
        expect(bindings.getPrimaryRecurringReason(row)).toBe('');
        row.recurringMatchReasons = ' | amount | ';
        expect(bindings.getPrimaryRecurringReason(row)).toBe('amount');
        expect(bindings.getRecurringMatchSummary(row)).toContain('amount');
    });

    test('server paging takes the authoritative selection path and server-only computed branches', async () => {
        const row = createTransaction(20, { selected: false });
        const bindings = createBindings({ transactions: [row], serverPaged: true, total: 0 });
        bindings.countPerPage.value = 0;
        expect(bindings.totalImportTransactionCount.value).toBe(0);
        expect(bindings.tablePage.value).toBe(1);
        expect(bindings.tableItemsPerPage.value).toBe(1);
        expect(bindings.totalPageCount.value).toBe(1);
        expect(bindings.filteredImportTransactions.value).toEqual([row]);
        expect(bindings.tableTransactions.value).toEqual([row]);

        await bindings.selectAllValid();
        await bindings.selectAllInvalid();
        await bindings.selectAllNeedsAnnotation();
        await bindings.selectAll();
        await bindings.selectNone();
        await bindings.selectInvert();
        expect(mockApplyImportPreviewSelectionAction).toHaveBeenCalledTimes(6);

        mockApplyImportPreviewSelectionAction.mockRejectedValueOnce(new Error('selection failed'));
        const selectionBeforeFailedRequest = row.selected;
        await bindings.selectAll();
        expect(row.selected).toBe(selectionBeforeFailedRequest);
    });

    test('server sort and action-scope flushing cover patch identities and response failures', async () => {
        const selected = createTransaction(21, { selected: false });
        const deselected = createTransaction(22, { selected: true });
        const unchanged = createTransaction(23, { selected: true });
        const bindings = createBindings({
            transactions: [selected, deselected, unchanged],
            serverPaged: true,
            total: 3
        });
        const fetchMock = globalThis.fetch as jest.MockedFunction<typeof fetch>;

        bindings.countPerPage.value = 0;
        bindings.updatePreviewTableSort();
        bindings.updatePreviewTableSort([{ key: 'time', order: 'desc' }]);
        bindings.updatePreviewTablePageSize(25);
        expect(bindings.tableSortBy.value).toEqual(expect.any(Array));

        selected.selected = true;
        deselected.selected = false;
        mockPatchImportPreviewSelection.mockResolvedValueOnce({
            data: { result: { updated: 2, metadata: { counts: { total: 3, selected: 2 } } } }
        });
        await expect(bindings.flushPreviewSelectionAndBuildActionScope()).resolves.toEqual(expect.any(Object));
        expect(mockPatchImportPreviewSelection).toHaveBeenCalledWith(expect.objectContaining({
            selectedIds: [21],
            deselectedIds: [22]
        }));

        selected.selected = false;
        mockCurrentToken = '';
        mockPatchImportPreviewSelection.mockRejectedValueOnce(new Error('Failed to flush preview selection'));
        await expect(bindings.flushPreviewSelectionAndBuildActionScope()).rejects.toThrow('Failed to flush preview selection');

        mockPatchImportPreviewSelection.mockRejectedValueOnce(new Error('Failed to flush preview selection'));
        await expect(bindings.flushPreviewSelectionAndBuildActionScope()).rejects.toThrow('Failed to flush preview selection');
        expect(fetchMock).not.toHaveBeenCalled();

        const missingTransactions = (ImportTransactionCheckDataTab as any).setup({
            ...propsFor({ transactions: [] }),
            importTransactions: null
        }, { emit: jest.fn(), expose: jest.fn() });
        expect(missingTransactions.importTransactions.value).toEqual([]);
        expect(missingTransactions.getSelectedPreviewUpdates()).toEqual([]);
    });

    test('template wrappers call signal decisions, draft handlers, and explicit dialog closers', async () => {
        const row = createTransaction(30, {
            type: 4,
            categoryId: 'transfer-parent',
            sourceAccountId: '',
            destinationAccountId: '0',
            sourceCurrency: '',
            destinationCurrency: '',
            tagIds: ['missing-tag'],
            tagNames: ['Imported Tag']
        });
        const { bindings } = await renderDesktop({
            transactions: [row],
            total: 12,
            configure: current => {
                current.editingTransaction.value = current.tableTransactions.value[0];
                current.editingTags.value = ['missing-tag'];
                forcedNamedSlotItem = current.editingTransaction.value;
                current.showBatchCategoryDialog.value = true;
                current.showBatchAccountDialog.value = true;
                current.showCustomDescriptionDialog.value = true;
                current.showCategorySelectDialog.value = true;
                current.showAccountSelectDialog.value = true;
                current.showAnnotationDialog.value = true;
            }
        });

        expect(Object.keys(mockSignalActions)).toEqual(expect.arrayContaining([
            'onReviewTransfer',
            'onReviewLearning',
            'onReviewLlm',
            'onOpenHistoryDetail',
            'onOpenRecurring',
            'onClearRecurring'
        ]));
        await mockSignalActions['onReviewTransfer']!('accept');
        await mockSignalActions['onReviewLearning']!('accept');
        await mockSignalActions['onReviewLlm']!('accept');
        await mockSignalActions['onOpenRecurring']!();
        await mockSignalActions['onClearRecurring']!();

        const open = jest.fn();
        bindings.historyBillDetailDialog.value = { open };
        mockSignalActions['onOpenHistoryDetail']!(0);
        mockSignalActions['onOpenHistoryDetail']!(30);
        expect(open).toHaveBeenCalledWith({ id: '30' });

        row.type = 5;
        await actionByHandlerText('onTransactionTypeChange', 'onUpdate:modelValue').handler(5);
        row.categoryId = 'investment-parent';
        const draftActions = mockUiActions.filter(candidate => (
            candidate.event === 'onUpdate:modelValue'
            && [
                'cacheServerPagedDraft',
                'onTransactionDataDraftChange',
                'cacheEditingTagsDraft'
            ].some(handlerName => String(candidate.handler).includes(handlerName))
        ));
        expect(draftActions).toHaveLength(10);
        for (const action of draftActions) {
            await action.handler('investment-parent');
        }
        const textModelAssignments = mockUiActions.filter(candidate => (
            candidate.component === 'v-text-field'
            && candidate.event === 'onUpdate:modelValue'
            && !draftActions.includes(candidate)
        ));
        expect(textModelAssignments.length).toBeGreaterThanOrEqual(3);
        for (const action of textModelAssignments) {
            await action.handler('edited text');
        }
        await actionByHandlerText('quickCreatePrimaryCategory', 'onPrimaryAction').handler();
        await actionByHandlerText('quickCreateSecondaryCategory', 'onSecondaryAction').handler('expense-parent');
        await actionByHandlerText("quickCreateAccount(item, 'source'", 'onSecondaryAction').handler('wallet');
        await actionByHandlerText("quickCreateAccount(item, 'destination'", 'onSecondaryAction').handler('bank');

        for (const action of mockUiActions.filter(candidate => (
            candidate.event === 'onClick' && (candidate.label.includes('Cancel') || candidate.label.includes('Close'))
        ))) {
            await action.handler();
        }
        const showModelAction = actionByHandlerText('showCustomDateRangeDialog', 'onUpdate:show');
        await showModelAction.handler(true);

        expect(bindings.showBatchCategoryDialog.value).toBe(false);
        expect(bindings.showBatchAccountDialog.value).toBe(false);
        expect(bindings.showCustomDescriptionDialog.value).toBe(false);
        expect(bindings.showCategorySelectDialog.value).toBe(false);
        expect(bindings.showAccountSelectDialog.value).toBe(false);
        expect(bindings.showAnnotationDialog.value).toBe(false);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);
        expect(row.sourceAmountCents).toBe(-1_030);
        expect(row.destinationAmountCents).toBe(2_030);
    });
});
