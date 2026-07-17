import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const transactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4,
} as const;

const chartType = {
    Default: { type: 1 },
} as const;

const aggregationType = {
    Day: { type: 10 },
} as const;

const mockCsvFileType = {
    name: 'CSV',
    formatFileName: jest.fn<(name: string) => string>(name => `${name}.csv`),
    createBlob: jest.fn<(data: unknown) => unknown>(data => ({ kind: 'csv', data })),
};
const mockTsvFileType = {
    name: 'TSV',
    formatFileName: jest.fn<(name: string) => string>(name => `${name}.tsv`),
    createBlob: jest.fn<(data: unknown) => unknown>(data => ({ kind: 'tsv', data })),
};
const knownFileType = { CSV: mockCsvFileType, TSV: mockTsvFileType } as const;

const mockAccountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
};
const mockCategoriesStore = {
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>(),
};
const mockTransactionsStore = {
    getReconciliationStatements: jest.fn<(...args: any[]) => Promise<any>>(),
};
const mockIsEquals = jest.fn<(left: unknown, right: unknown) => boolean>();
const mockGetCurrentUnixTime = jest.fn<() => number>();
const mockStartDownloadFile = jest.fn<(...args: any[]) => void>();
const mockTransactionOf = jest.fn<(value: unknown) => unknown>();

const mockTemplateRefs = new Map<string, any>();
const mockRuntimeEvents: CapturedCallback[] = [];
let mockLastBase: ReturnType<typeof createBase>;

function createTransaction(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'transaction-1',
        index: 0,
        type: transactionType.Expense,
        sourceAmountCents: 12_345,
        destinationAmountCents: 12_345,
        sourceAccountId: 'wallet',
        destinationAccountId: '',
        categoryId: 'food',
        category: undefined,
        time: 1_720_000_000,
        utcOffset: 480,
        accountClosingBalanceCents: 98_765,
        comment: 'Lunch',
        ...overrides,
    };
}

function createStatement(overrides: Record<string, unknown> = {}): any {
    return {
        totalInflowsCents: 30_000,
        totalOutflowsCents: 12_345,
        netFlowCents: 17_655,
        openingBalanceCents: 81_110,
        closingBalanceCents: 98_765,
        transactions: [createTransaction()],
        ...overrides,
    };
}

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const accountId = ref('');
    const startTime = ref(0);
    const endTime = ref(0);
    const reconciliationStatements = ref(undefined as any);
    const isCurrentLiabilityAccount = ref(false);

    return {
        accountId,
        startTime,
        endTime,
        reconciliationStatements,
        currentTimezoneOffsetMinutes: ref(480),
        fiscalYearStart: ref(101),
        allChartTypes: ref([
            { type: 1, displayName: 'Line' },
            { type: 2, displayName: 'Bar' },
        ]),
        allDateAggregationTypes: ref([
            { type: 10, displayName: 'Day' },
            { type: 20, displayName: 'Week' },
        ]),
        currentAccount: ref({ id: 'wallet', name: 'Wallet', currency: 'CNY', isLiability: false }),
        currentAccountCurrency: ref('CNY'),
        isCurrentLiabilityAccount,
        allAccountsMap: ref({
            wallet: { id: 'wallet', name: 'Wallet' },
            bank: { id: 'bank', name: 'Bank' },
        }),
        allCategoriesMap: ref({
            food: { id: 'food', name: 'Food', icon: 'fork', color: '#112233' },
            plain: { id: 'plain', name: 'Plain', icon: '', color: '' },
        }),
        exportFileName: ref('wallet-statement'),
        displayStartDateTime: computed(() => `start:${startTime.value}`),
        displayEndDateTime: computed(() => `end:${endTime.value}`),
        displayTotalInflows: computed(() => `in:${reconciliationStatements.value?.totalInflowsCents ?? 0}`),
        displayTotalOutflows: computed(() => `out:${reconciliationStatements.value?.totalOutflowsCents ?? 0}`),
        displayTotalBalance: computed(() => `net:${reconciliationStatements.value?.netFlowCents ?? 0}`),
        displayOpeningBalance: computed(() => `opening:${reconciliationStatements.value?.openingBalanceCents ?? 0}`),
        displayClosingBalance: computed(() => `closing:${reconciliationStatements.value?.closingBalanceCents ?? 0}`),
        getDisplayTransactionType: jest.fn((transaction: any) => `type:${transaction.type}`),
        getDisplayDateTime: jest.fn((transaction: any) => `time:${transaction.time}`),
        getDisplayTimezone: jest.fn((transaction: any) => `timezone:${transaction.utcOffset}`),
        getDisplaySourceAmount: jest.fn((transaction: any) => `source:${transaction.sourceAmountCents}`),
        getDisplayDestinationAmount: jest.fn((transaction: any) => `destination:${transaction.destinationAmountCents}`),
        getDisplayAccountBalance: jest.fn((transaction: any) => `balance:${transaction.accountClosingBalanceCents}`),
        getExportedData: jest.fn((fileType: any) => `export:${fileType.name}`),
    };
}

function createCaptureStub(name: string): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => {
            for (const [eventName, handler] of Object.entries(attrs)) {
                const handlers = Array.isArray(handler) ? handler : [handler];
                for (const candidate of handlers) {
                    if (eventName.startsWith('on') && typeof candidate === 'function') {
                        mockRuntimeEvents.push({ name: eventName, callback: candidate as (...args: any[]) => any });
                    }
                }
            }
            return () => h(
                'div',
                { ...attrs, 'data-stub': name },
                Object.values(slots).flatMap((slot: any) => {
                    try {
                        return slot?.({ item: createTransaction(), props: {} }) ?? [];
                    } catch {
                        return [];
                    }
                }),
            );
        },
    });
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
    };
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentNumeralSystemType: () => ({ formatNumber: (value: number) => `number:${value}` }),
        formatNumberToLocalizedNumerals: (value: number) => `localized:${value}`,
    }),
}));

jest.mock('@/views/base/accounts/ReconciliationStatementPageBase.ts', () => ({
    useReconciliationStatementPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    },
}));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoriesStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: transactionType }));
jest.mock('@/core/statistics.ts', () => ({
    AccountBalanceTrendChartType: chartType,
    ChartDateAggregationType: aggregationType,
}));
jest.mock('@/core/file.ts', () => ({ KnownFileType: knownFileType }));
jest.mock('@/models/transaction.ts', () => ({
    Transaction: { of: (value: unknown) => mockTransactionOf(value) },
}));
jest.mock('@/lib/common.ts', () => ({ isEquals: (left: unknown, right: unknown) => mockIsEquals(left, right) }));
jest.mock('@/lib/datetime.ts', () => ({ getCurrentUnixTime: () => mockGetCurrentUnixTime() }));
jest.mock('@/lib/ui/common.ts', () => ({ startDownloadFile: (...args: any[]) => mockStartDownloadFile(...args) }));
jest.mock('@/views/base/transactions/TransactionEditPageBase.ts', () => ({
    TransactionEditPageType: { Transaction: 1 },
}));
jest.mock('@/views/desktop/accounts/list/dialogs/reconciliationStatementDialogOptions.ts', () => ({
    chartDataDateAggregationTypeIconMap: { 10: 'day-icon', 20: 'week-icon' },
    chartTypeIconMap: { 1: 'line-icon', 2: 'bar-icon' },
}));

for (const componentPath of [
    '@/components/desktop/PaginationButtons.vue',
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/AmountInputDialog.vue',
    '@/views/desktop/transactions/list/dialogs/EditDialog.vue',
]) {
    jest.mock(componentPath, () => ({
        __esModule: true,
        default: createCaptureStub('ReconciliationDialogCoverageChildStub'),
    }));
}

const ReconciliationStatementDialog = require(
    '@/views/desktop/accounts/list/dialogs/ReconciliationStatementDialog.vue'
).default as any;

function setupDialog(): { bindings: any; emit: jest.Mock; exposed: Record<string, unknown> } {
    mockTemplateRefs.clear();
    const emit = jest.fn();
    const exposed: Record<string, unknown> = {};
    const bindings = ReconciliationStatementDialog.setup({}, {
        emit,
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value),
    });
    expect(exposed).toEqual({ open: bindings.open });
    return { bindings, emit, exposed };
}

function installDialogRefs(bindings: any): {
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    amountInput: { open: jest.Mock<(...args: any[]) => Promise<any>> };
    edit: { open: jest.Mock<(...args: any[]) => Promise<any>> };
} {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const amountInput = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    const edit = { open: jest.fn<(...args: any[]) => Promise<any>>() };
    bindings.snackbar.value = snackbar;
    bindings.amountInputDialog.value = amountInput;
    bindings.editDialog.value = edit;
    return { snackbar, amountInput, edit };
}

async function flush(times = 8): Promise<void> {
    const { nextTick } = jest.requireActual('vue') as any;
    for (let index = 0; index < times; index++) await Promise.resolve();
    await nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

async function renderSsrDialog(mutator: (bindings: any) => void): Promise<{ html: string; bindings: any }> {
    const { createSSRApp } = require('vue') as any;
    const { renderToString } = require('vue/server-renderer') as any;
    let bindings: any;
    const RuntimeDialog = {
        ...ReconciliationStatementDialog,
        setup(props: any, context: any) {
            bindings = ReconciliationStatementDialog.setup(props, context);
            mutator(bindings);
            return bindings;
        },
    };
    const app = createSSRApp(RuntimeDialog);
    for (const componentName of [
        'v-dialog', 'v-card', 'v-card-text', 'v-btn', 'v-progress-circular', 'v-icon',
        'v-tooltip', 'v-menu', 'v-list', 'v-list-subheader', 'v-list-item',
        'v-list-item-title', 'v-divider', 'v-switch', 'v-skeleton-loader', 'v-spacer',
        'v-data-table', 'v-chip', 'v-select', 'pagination-buttons', 'item-icon',
        'account-balance-trends-chart', 'amount-input-dialog', 'edit-dialog', 'snack-bar',
    ]) {
        app.component(componentName, createCaptureStub(`ReconciliationSsr${componentName}`));
    }
    app.config.warnHandler = () => undefined;
    return { html: await renderToString(app), bindings };
}

function render(bindings: any): any {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return ReconciliationStatementDialog.render(exposed, [], {}, exposed, {}, {});
}

type CapturedCallback = { name: string; callback: (...args: any[]) => any };

function exerciseRenderTree(
    value: any,
    transactions: any[],
    callbacks: CapturedCallback[],
    seen = new Set<any>(),
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) exerciseRenderTree(item, transactions, callbacks, seen);
        return;
    }

    if (value.props && typeof value.props === 'object') {
        for (const [name, prop] of Object.entries(value.props)) {
            const values = Array.isArray(prop) ? prop : [prop];
            for (const candidate of values) {
                if (name.startsWith('on') && typeof candidate === 'function') {
                    callbacks.push({ name, callback: candidate as (...args: any[]) => any });
                }
            }
        }
    }

    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const [slotName, slot] of Object.entries(value.children)) {
            if (typeof slot !== 'function') {
                exerciseRenderTree(slot, transactions, callbacks, seen);
                continue;
            }

            const slotTransactions = slotName.startsWith('item.') ? transactions : [transactions[0]];
            for (const transaction of slotTransactions) {
                try {
                    const rendered = (slot as (...args: any[]) => any)({
                        item: transaction,
                        props: { role: 'button' },
                    });
                    exerciseRenderTree(rendered, transactions, callbacks, seen);
                } catch {
                    // Named slots intentionally receive different shapes; incompatible probes are ignored.
                }
            }
        }
    } else {
        exerciseRenderTree(value.children, transactions, callbacks, seen);
    }

    exerciseRenderTree(value.dynamicChildren, transactions, callbacks, seen);
    exerciseRenderTree(value.ssContent, transactions, callbacks, seen);
    exerciseRenderTree(value.ssFallback, transactions, callbacks, seen);
}

beforeEach(() => {
    jest.resetAllMocks();
    mockTemplateRefs.clear();
    mockRuntimeEvents.length = 0;
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoriesStore.loadAllCategories.mockResolvedValue(undefined);
    mockTransactionsStore.getReconciliationStatements.mockResolvedValue(createStatement());
    mockIsEquals.mockImplementation((left, right) => JSON.stringify(left) === JSON.stringify(right));
    mockGetCurrentUnixTime.mockReturnValue(2_000);
    mockTransactionOf.mockImplementation(value => ({ wrapped: value }));
    mockCsvFileType.formatFileName.mockImplementation(name => `${name}.csv`);
    mockCsvFileType.createBlob.mockImplementation(data => ({ kind: 'csv', data }));
    mockTsvFileType.formatFileName.mockImplementation(name => `${name}.tsv`);
    mockTsvFileType.createBlob.mockImplementation(data => ({ kind: 'tsv', data }));
});

describe('desktop ReconciliationStatementDialog production-loaded state helpers', () => {
    test('builds localized pagination options, totals, headers, and transaction colors', () => {
        const { bindings } = setupDialog();

        expect(bindings.numeralSystem.value.formatNumber(5)).toBe('number:5');
        expect(bindings.getTablePageOptions()).toEqual([{ value: -1, name: 'All' }]);
        expect(bindings.getTablePageOptions(0)).toEqual([{ value: -1, name: 'All' }]);
        expect(bindings.getTablePageOptions(4)).toEqual([{ value: -1, name: 'All' }]);
        expect(bindings.getTablePageOptions(5)).toEqual([
            { value: 5, name: 'number:5' },
            { value: -1, name: 'All' },
        ]);
        expect(bindings.getTablePageOptions(26)).toEqual([
            { value: 5, name: 'number:5' },
            { value: 10, name: 'number:10' },
            { value: 15, name: 'number:15' },
            { value: 20, name: 'number:20' },
            { value: 25, name: 'number:25' },
            { value: -1, name: 'All' },
        ]);

        expect(bindings.totalPageCount.value).toBe(1);
        mockLastBase.reconciliationStatements.value = { transactions: undefined };
        expect(bindings.totalPageCount.value).toBe(1);
        mockLastBase.reconciliationStatements.value = { transactions: [] };
        expect(bindings.totalPageCount.value).toBe(1);
        mockLastBase.reconciliationStatements.value = createStatement({
            transactions: Array.from({ length: 21 }, (_, index) => createTransaction({ index })),
        });
        bindings.countPerPage.value = 10;
        expect(bindings.totalPageCount.value).toBe(3);
        expect(bindings.reconciliationStatementsTablePageOptions.value).toHaveLength(5);

        expect(bindings.dataTableHeaders.value.map((header: any) => header.title)).toContain('Account Balance');
        mockLastBase.isCurrentLiabilityAccount.value = true;
        expect(bindings.dataTableHeaders.value.map((header: any) => header.title)).toContain('Account Outstanding Balance');

        expect(bindings.getTransactionTypeColor(createTransaction({ type: transactionType.ModifyBalance }))).toBe('secondary');
        expect(bindings.getTransactionTypeColor(createTransaction({ type: transactionType.Income }))).toBeUndefined();
        expect(bindings.getTransactionTypeColor(createTransaction({ type: transactionType.Expense }))).toBeUndefined();
        expect(bindings.getTransactionTypeColor(createTransaction({ type: transactionType.Transfer }))).toBe('primary');
        expect(bindings.getTransactionTypeColor(createTransaction({ type: 99 }))).toBe('default');
    });
});

describe('desktop ReconciliationStatementDialog production-loaded loading and refresh', () => {
    test('opens after loading account/category metadata and statement cents, then closes cleanly', async () => {
        const { bindings } = setupDialog();
        installDialogRefs(bindings);
        bindings.currentPage.value = 8;
        bindings.countPerPage.value = 50;
        bindings.showAccountBalanceTrendsCharts.value = true;
        bindings.chartType.value = 2;
        bindings.chartDataDateAggregationType.value = 20;
        const statement = createStatement({ closingBalanceCents: 123_456 });
        mockTransactionsStore.getReconciliationStatements.mockResolvedValueOnce(statement);

        const pending = bindings.open({ accountId: 'wallet', startTime: 1_000, endTime: 1_500 });
        expect(bindings.showState.value).toBe(true);
        expect(bindings.loading.value).toBe(true);
        expect(bindings.currentPage.value).toBe(1);
        expect(bindings.countPerPage.value).toBe(10);
        expect(bindings.showAccountBalanceTrendsCharts.value).toBe(false);
        expect(bindings.chartType.value).toBe(1);
        expect(bindings.chartDataDateAggregationType.value).toBe(10);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(mockCategoriesStore.loadAllCategories).toHaveBeenCalledWith({ force: false });

        await flush();
        expect(mockTransactionsStore.getReconciliationStatements).toHaveBeenCalledWith({
            accountId: 'wallet', startTime: 1_000, endTime: 1_500,
        });
        expect(mockLastBase.reconciliationStatements.value).toStrictEqual(statement);
        expect(mockLastBase.reconciliationStatements.value.closingBalanceCents).toBe(123_456);
        expect(bindings.loading.value).toBe(false);
        bindings.close();
        await expect(pending).resolves.toBeUndefined();
        expect(bindings.showState.value).toBe(false);
        expect(bindings.resolveFunc).toBeNull();
    });

    test('emits and settles unprocessed open failures while leaving processed failures owned upstream', async () => {
        const failed = setupDialog();
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'accounts failed' });
        const failedPending = failed.bindings.open({ accountId: 'wallet', startTime: 1, endTime: 2 });
        await flush();
        await expect(failedPending).resolves.toBeUndefined();
        expect(failed.emit).toHaveBeenCalledWith('error', expect.objectContaining({ message: 'accounts failed' }));
        expect(failed.bindings.showState.value).toBe(false);
        expect(failed.bindings.loading.value).toBe(false);

        const processed = setupDialog();
        mockCategoriesStore.loadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const processedPending = processed.bindings.open({ accountId: 'bank', startTime: 3, endTime: 4 });
        await flush();
        expect(processed.emit).not.toHaveBeenCalled();
        expect(processed.bindings.showState.value).toBe(true);
        expect(processed.bindings.loading.value).toBe(false);
        processed.bindings.close();
        await expect(processedPending).resolves.toBeUndefined();
    });

    test('reloads normal/equal/changed results and reports only unprocessed errors', async () => {
        const { bindings } = setupDialog();
        const { snackbar } = installDialogRefs(bindings);
        mockLastBase.accountId.value = 'wallet';
        mockLastBase.startTime.value = 10;
        mockLastBase.endTime.value = 20;
        const original = createStatement({ closingBalanceCents: 10_000 });
        mockLastBase.reconciliationStatements.value = original;

        const normal = createStatement({ closingBalanceCents: 11_000 });
        mockTransactionsStore.getReconciliationStatements.mockResolvedValueOnce(normal);
        bindings.reload(false);
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(snackbar.showMessage).not.toHaveBeenCalled();
        expect(mockLastBase.reconciliationStatements.value).toStrictEqual(normal);

        mockIsEquals.mockReturnValueOnce(true);
        mockTransactionsStore.getReconciliationStatements.mockResolvedValueOnce(normal);
        bindings.reload(true);
        await flush();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Data is up to date');

        const changed = createStatement({ closingBalanceCents: 12_000 });
        mockIsEquals.mockReturnValueOnce(false);
        mockTransactionsStore.getReconciliationStatements.mockResolvedValueOnce(changed);
        bindings.reload(true);
        await flush();
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Data has been updated');

        mockTransactionsStore.getReconciliationStatements.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reload(false);
        await flush();
        expect(snackbar.showError).not.toHaveBeenCalled();
        expect(bindings.loading.value).toBe(false);

        mockTransactionsStore.getReconciliationStatements.mockRejectedValueOnce({ processed: false, message: 'reload failed' });
        bindings.reload(false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'reload failed' }));
    });
});

describe('desktop ReconciliationStatementDialog production-loaded actions and cents', () => {
    test('adds transactions and handles success, empty result, errors, and missing dialog refs', async () => {
        const { bindings } = setupDialog();
        const { edit, snackbar } = installDialogRefs(bindings);
        mockLastBase.accountId.value = 'wallet';
        edit.open.mockResolvedValueOnce({ message: 'Added' });
        bindings.addTransaction();
        await flush();
        expect(edit.open).toHaveBeenCalledWith({ accountId: 'wallet' });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Added');
        expect(mockTransactionsStore.getReconciliationStatements).toHaveBeenCalled();

        edit.open.mockResolvedValueOnce(undefined);
        bindings.addTransaction();
        await flush();
        edit.open.mockResolvedValueOnce({});
        bindings.addTransaction();
        await flush();

        edit.open.mockRejectedValueOnce(new Error('add failed'));
        bindings.addTransaction();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'add failed' }));
        edit.open.mockRejectedValueOnce(undefined);
        bindings.addTransaction();
        await flush();

        const noRef = setupDialog().bindings;
        noRef.addTransaction();
    });

    test('creates asset closing-balance adjustments with exact cents and bounded transaction times', async () => {
        const { bindings } = setupDialog();
        const { amountInput, edit, snackbar } = installDialogRefs(bindings);
        mockLastBase.accountId.value = 'wallet';
        mockLastBase.currentAccountCurrency.value = 'CNY';
        mockLastBase.reconciliationStatements.value = createStatement({ closingBalanceCents: 12_345 });

        amountInput.open.mockResolvedValueOnce(23_456);
        edit.open.mockResolvedValueOnce({ message: 'Balance increased' });
        mockLastBase.startTime.value = 1_000;
        mockLastBase.endTime.value = 1_500;
        mockGetCurrentUnixTime.mockReturnValueOnce(2_000);
        bindings.updateClosingBalance();
        await flush();
        expect(amountInput.open).toHaveBeenCalledWith(expect.objectContaining({
            currency: 'CNY',
            initAmount: 12_345,
        }));
        expect(edit.open).toHaveBeenCalledWith({
            time: 1_500,
            type: transactionType.Income,
            sourceAmountCents: 11_111,
            accountId: 'wallet',
            noTransactionDraft: true,
        });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Balance increased');

        mockLastBase.reconciliationStatements.value = createStatement({ closingBalanceCents: 12_345 });
        amountInput.open.mockResolvedValueOnce(1_234);
        edit.open.mockResolvedValueOnce(undefined);
        mockLastBase.startTime.value = 3_000;
        mockLastBase.endTime.value = 4_000;
        mockGetCurrentUnixTime.mockReturnValueOnce(2_000);
        bindings.updateClosingBalance();
        await flush();
        expect(edit.open).toHaveBeenLastCalledWith(expect.objectContaining({
            time: 3_000,
            type: transactionType.Expense,
            sourceAmountCents: 11_111,
        }));

        mockLastBase.reconciliationStatements.value = createStatement({ closingBalanceCents: 12_345 });
        amountInput.open.mockResolvedValueOnce(13_345);
        edit.open.mockResolvedValueOnce({});
        mockLastBase.startTime.value = 1_000;
        mockLastBase.endTime.value = 4_000;
        mockGetCurrentUnixTime.mockReturnValueOnce(2_000);
        bindings.updateClosingBalance();
        await flush();
        expect(edit.open).toHaveBeenLastCalledWith(expect.objectContaining({
            time: undefined,
            type: transactionType.Income,
            sourceAmountCents: 1_000,
        }));

        amountInput.open.mockResolvedValueOnce(0);
        bindings.updateClosingBalance();
        await flush();
        amountInput.open.mockResolvedValueOnce(undefined);
        bindings.updateClosingBalance();
        await flush();
    });

    test('creates liability adjustments in cents and handles nested edit errors', async () => {
        const { bindings } = setupDialog();
        const { amountInput, edit, snackbar } = installDialogRefs(bindings);
        mockLastBase.accountId.value = 'credit';
        mockLastBase.currentAccountCurrency.value = 'USD';
        mockLastBase.isCurrentLiabilityAccount.value = true;
        mockLastBase.reconciliationStatements.value = createStatement({ closingBalanceCents: 12_345 });

        amountInput.open.mockResolvedValueOnce(-2_345);
        edit.open.mockRejectedValueOnce(new Error('adjust failed'));
        bindings.updateClosingBalance();
        await flush();
        expect(amountInput.open).toHaveBeenCalledWith(expect.objectContaining({ initAmount: -12_345 }));
        expect(edit.open).toHaveBeenCalledWith(expect.objectContaining({
            type: transactionType.Expense,
            sourceAmountCents: 10_000,
            accountId: 'credit',
        }));
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'adjust failed' }));

        amountInput.open.mockResolvedValueOnce(-22_345);
        edit.open.mockRejectedValueOnce(undefined);
        bindings.updateClosingBalance();
        await flush();
        expect(edit.open).toHaveBeenLastCalledWith(expect.objectContaining({
            type: transactionType.Income,
            sourceAmountCents: 10_000,
        }));

        const noAmountRef = setupDialog().bindings;
        mockLastBase.reconciliationStatements.value = undefined;
        noAmountRef.updateClosingBalance();
    });

    test('exports populated statements and ignores empty or absent transaction collections', () => {
        const { bindings } = setupDialog();
        bindings.exportReconciliationStatements(mockCsvFileType);
        mockLastBase.reconciliationStatements.value = { transactions: undefined };
        bindings.exportReconciliationStatements(mockCsvFileType);
        mockLastBase.reconciliationStatements.value = { transactions: [] };
        bindings.exportReconciliationStatements(mockCsvFileType);
        expect(mockStartDownloadFile).not.toHaveBeenCalled();

        mockLastBase.reconciliationStatements.value = createStatement();
        bindings.exportReconciliationStatements(mockCsvFileType);
        expect(mockLastBase.getExportedData).toHaveBeenCalledWith(mockCsvFileType);
        expect(mockCsvFileType.formatFileName).toHaveBeenCalledWith('wallet-statement');
        expect(mockCsvFileType.createBlob).toHaveBeenCalledWith('export:CSV');
        expect(mockStartDownloadFile).toHaveBeenCalledWith('wallet-statement.csv', {
            kind: 'csv', data: 'export:CSV',
        });

        bindings.exportReconciliationStatements(mockTsvFileType);
        expect(mockStartDownloadFile).toHaveBeenLastCalledWith('wallet-statement.tsv', {
            kind: 'tsv', data: 'export:TSV',
        });
    });

    test('views editable transactions, skips balance rows, and handles dialog outcomes', async () => {
        const { bindings } = setupDialog();
        const { edit, snackbar } = installDialogRefs(bindings);
        const modify = createTransaction({ id: 'modify', type: transactionType.ModifyBalance });
        bindings.showTransaction(modify);
        expect(edit.open).not.toHaveBeenCalled();

        const expense = createTransaction({ id: 'expense', sourceAmountCents: 12_345 });
        const wrapped = { wrapped: expense };
        mockTransactionOf.mockReturnValueOnce(wrapped);
        edit.open.mockResolvedValueOnce({ message: 'Saved' });
        bindings.showTransaction(expense);
        await flush();
        expect(mockTransactionOf).toHaveBeenCalledWith(expense);
        expect(edit.open).toHaveBeenCalledWith({ id: 'expense', currentTransaction: wrapped });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Saved');

        edit.open.mockResolvedValueOnce(undefined);
        bindings.showTransaction(expense);
        await flush();
        edit.open.mockResolvedValueOnce({});
        bindings.showTransaction(expense);
        await flush();
        edit.open.mockRejectedValueOnce(new Error('view failed'));
        bindings.showTransaction(expense);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'view failed' }));
        edit.open.mockRejectedValueOnce(undefined);
        bindings.showTransaction(expense);
        await flush();

        const noRef = setupDialog().bindings;
        noRef.showTransaction(expense);
    });
});

describe('desktop ReconciliationStatementDialog production template behavior', () => {
    test('renders loading, empty, populated, paginated, transfer, category, and chart branches', () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { bindings } = setupDialog();
            installDialogRefs(bindings);
            const callbacks: CapturedCallback[] = [];

            bindings.showState.value = true;
            bindings.loading.value = true;
            mockLastBase.startTime.value = 0;
            mockLastBase.endTime.value = 0;
            exerciseRenderTree(render(bindings), [createTransaction()], callbacks);

            bindings.loading.value = false;
            mockLastBase.startTime.value = 1_000;
            mockLastBase.endTime.value = 2_000;
            mockLastBase.reconciliationStatements.value = createStatement({ transactions: [] });
            exerciseRenderTree(render(bindings), [createTransaction()], callbacks);

            const transactions = [
                createTransaction({
                    id: 'expense',
                    index: 0,
                    type: transactionType.Expense,
                    utcOffset: 480,
                    category: { name: 'Dining', icon: 'dining', color: '#abcdef' },
                }),
                createTransaction({
                    id: 'income', index: 1, type: transactionType.Income,
                    categoryId: 'food', utcOffset: 0,
                }),
                createTransaction({
                    id: 'transfer', index: 2, type: transactionType.Transfer,
                    sourceAccountId: 'wallet', destinationAccountId: 'bank',
                    sourceAmountCents: 12_345, destinationAmountCents: 67_890,
                    categoryId: 'plain',
                }),
                createTransaction({
                    id: 'same-transfer', index: 3, type: transactionType.Transfer,
                    sourceAccountId: 'wallet', destinationAccountId: 'wallet',
                    sourceAmountCents: 50_000, destinationAmountCents: 50_000,
                    categoryId: 'missing',
                }),
                createTransaction({
                    id: 'modify', index: 4, type: transactionType.ModifyBalance,
                    categoryId: 'missing',
                }),
                createTransaction({ id: 'unknown', index: 5, type: 99, categoryId: 'missing' }),
            ];
            while (transactions.length < 12) {
                transactions.push(createTransaction({ id: `extra-${transactions.length}`, index: transactions.length }));
            }
            mockLastBase.reconciliationStatements.value = createStatement({ transactions });
            mockLastBase.isCurrentLiabilityAccount.value = true;
            exerciseRenderTree(render(bindings), transactions, callbacks);

            bindings.showAccountBalanceTrendsCharts.value = true;
            bindings.loading.value = true;
            exerciseRenderTree(render(bindings), transactions, callbacks);
            bindings.loading.value = false;
            exerciseRenderTree(render(bindings), transactions, callbacks);

            expect(callbacks.length).toBeGreaterThan(15);
            expect(mockLastBase.getDisplaySourceAmount).toHaveBeenCalledWith(expect.objectContaining({ id: 'transfer' }));
            expect(mockLastBase.getDisplayDestinationAmount).toHaveBeenCalledWith(expect.objectContaining({ id: 'transfer' }));
            expect(mockLastBase.getDisplayAccountBalance).toHaveBeenCalled();
            expect(mockLastBase.displayClosingBalance.value).toBe('closing:98765');
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('executes generated model, refresh, add, balance, export, view, and close handlers', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { bindings } = setupDialog();
            const { amountInput, edit } = installDialogRefs(bindings);
            bindings.showState.value = true;
            bindings.loading.value = false;
            mockLastBase.accountId.value = 'wallet';
            mockLastBase.reconciliationStatements.value = createStatement({
                transactions: Array.from({ length: 12 }, (_, index) => createTransaction({ index, id: `row-${index}` })),
            });
            amountInput.open.mockResolvedValue(99_999);
            edit.open.mockResolvedValue({});

            const callbacks: CapturedCallback[] = [];
            exerciseRenderTree(render(bindings), mockLastBase.reconciliationStatements.value.transactions, callbacks);
            for (const { name, callback } of callbacks) {
                try {
                    if (name === 'onUpdate:modelValue' || name.startsWith('onUpdate:')) {
                        callback(2);
                    } else {
                        callback();
                    }
                } catch {
                    // Generated handlers have heterogeneous slot payload contracts.
                }
            }
            await flush();

            expect(callbacks.some(item => item.name === 'onClick')).toBe(true);
            expect(bindings.currentPage.value).toBe(2);
            expect(mockTransactionsStore.getReconciliationStatements).toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('SSR executes title/menu slots and their generated action handlers', async () => {
        const rendered = await renderSsrDialog(bindings => {
            installDialogRefs(bindings);
            bindings.showState.value = true;
            bindings.loading.value = false;
            bindings.showAccountBalanceTrendsCharts.value = true;
            bindings.chartType.value = 1;
            bindings.chartDataDateAggregationType.value = 10;
            mockLastBase.accountId.value = 'wallet';
            mockLastBase.reconciliationStatements.value = createStatement({
                transactions: Array.from({ length: 12 }, (_, index) => createTransaction({ index })),
            });
            bindings.amountInputDialog.value.open.mockResolvedValue(99_999);
            bindings.editDialog.value.open.mockResolvedValue({});
        });
        expect(rendered.html).toContain('Reconciliation Statement');
        const clickSources = mockRuntimeEvents
            .filter(item => item.name === 'onClick')
            .map(item => item.callback.toString())
            .join('\n');
        expect(clickSources).toContain('reload');
        expect(clickSources).toContain('addTransaction');
        expect(clickSources).toContain('updateClosingBalance');
        expect(clickSources).toContain('exportReconciliationStatements');

        for (const { name, callback } of mockRuntimeEvents) {
            try {
                if (name === 'onUpdate:modelValue' || name.startsWith('onUpdate:')) {
                    callback(2);
                } else {
                    callback();
                }
            } catch {
                // Vuetify model and menu handlers expose heterogeneous argument contracts.
            }
        }
        await flush();
        expect(mockStartDownloadFile).toHaveBeenCalled();
        expect(mockTransactionsStore.getReconciliationStatements).toHaveBeenCalled();
    });

    test('settles the exposed promise when v-model hides the dialog', async () => {
        const { bindings } = setupDialog();
        const pending = bindings.open({ accountId: 'wallet', startTime: 1, endTime: 2 });
        await flush();
        bindings.showState.value = false;
        await flush();
        await expect(pending).resolves.toBeUndefined();
        expect(bindings.resolveFunc).toBeNull();

        bindings.settleOpenPromise();
        expect(bindings.resolveFunc).toBeNull();
    });
});
