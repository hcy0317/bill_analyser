import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const accountType = {
    SingleAccount: { type: 1 },
    MultiAccount: { type: 2 }
} as const;

const transactionType = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4
} as const;

const dateRange = {
    ThisMonth: { type: 10 },
    Custom: { type: 99 },
    isBillingCycle: jest.fn<(type: number) => boolean>()
};

const chartDateAggregationType = {
    Day: { type: 1 }
} as const;

const showAlert = jest.fn<(...args: any[]) => void>();
const showToast = jest.fn<(...args: any[]) => void>();
const routeBackOnError = jest.fn<(...args: any[]) => void>();
const showLoading = jest.fn<(...args: any[]) => void>();
const hideLoading = jest.fn<(...args: any[]) => void>();
const onSwipeoutDeleted = jest.fn<(...args: any[]) => void>();

const getCurrentUnixTime = jest.fn<() => number>();
const getDateTypeByDateRange = jest.fn<(...args: any[]) => number | null>();
const getDateTypeByBillingCycleDateRange = jest.fn<(...args: any[]) => number | null>();
const getDateRangeByDateType = jest.fn<(...args: any[]) => any>();
const getDateRangeByBillingCycleDateType = jest.fn<(...args: any[]) => any>();

const accountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
    getAccountStatementDate: jest.fn<(accountId: string) => number | undefined>()
};

const categoriesStore = {
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
};

const transactionsStore = {
    transactionReconciliationStatementStateInvalid: false,
    getReconciliationStatements: jest.fn<(...args: any[]) => Promise<any>>(),
    deleteTransaction: jest.fn<(...args: any[]) => Promise<void>>()
};

let lastBase: ReturnType<typeof createBase>;

function createTransaction(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'transaction-1',
        type: transactionType.Expense,
        sourceAmountCents: 12_345,
        destinationAmountCents: 12_345,
        sourceAccountId: 'wallet',
        destinationAccountId: '',
        categoryId: 'food',
        time: 1_720_000_000,
        utcOffset: 480,
        accountClosingBalanceCents: 98_765,
        comment: 'Lunch',
        editable: true,
        ...overrides
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
        ...overrides
    };
}

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const accountId = ref('');
    const startTime = ref(0);
    const endTime = ref(0);
    const reconciliationStatements = ref(undefined as any);
    const currentAccount = ref({
        id: 'wallet',
        name: 'Wallet',
        type: accountType.SingleAccount.type,
        currency: 'CNY',
        isLiability: false
    });
    const isCurrentLiabilityAccount = ref(false);

    return {
        accountId,
        startTime,
        endTime,
        reconciliationStatements,
        firstDayOfWeek: ref(1),
        fiscalYearStart: ref(101),
        allDateAggregationTypes: ref([
            { type: 1, displayName: 'Day' },
            { type: 2, displayName: 'Week' }
        ]),
        currentTimezoneOffsetMinutes: ref(480),
        isCurrentLiabilityAccount,
        allCategoriesMap: ref({
            food: { id: 'food', name: 'Food', icon: 'fork', color: '#112233' }
        }),
        currentAccount,
        currentAccountCurrency: ref('CNY'),
        displayStartDateTime: computed(() => `start:${startTime.value}`),
        displayEndDateTime: computed(() => `end:${endTime.value}`),
        displayTotalInflows: computed(() => `CNY ${reconciliationStatements.value?.totalInflowsCents ?? 0}`),
        displayTotalOutflows: computed(() => `CNY ${reconciliationStatements.value?.totalOutflowsCents ?? 0}`),
        displayTotalBalance: computed(() => `CNY ${reconciliationStatements.value?.netFlowCents ?? 0}`),
        displayOpeningBalance: computed(() => `CNY ${reconciliationStatements.value?.openingBalanceCents ?? 0}`),
        displayClosingBalance: computed(() => `CNY ${reconciliationStatements.value?.closingBalanceCents ?? 0}`),
        getDisplayDate: jest.fn((transaction: any) => `date:${Math.floor(transaction.time / 86_400)}`),
        getDisplayTime: jest.fn((transaction: any) => `time:${transaction.time}`),
        getDisplayTimezone: jest.fn((transaction: any) => `UTC:${transaction.utcOffset}`),
        getDisplaySourceAmount: jest.fn((transaction: any) => `source:${transaction.sourceAmountCents}`),
        getDisplayDestinationAmount: jest.fn((transaction: any) => `destination:${transaction.destinationAmountCents}`),
        getDisplayAccountBalance: jest.fn((transaction: any) => `balance:${transaction.accountClosingBalanceCents}`)
    };
}

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentLanguageTextDirection: () => 1,
        getAllDateRanges: () => [
            { type: 10, displayName: 'This month' },
            { type: 20, displayName: 'Billing cycle' },
            { type: 99, displayName: 'Custom', isUserCustomRange: true }
        ],
        formatUnixTimeToLongDateTime: (value: number) => `long:${value}`,
        formatNumberToLocalizedNumerals: (value: number) => `localized:${value}`
    })
}));

jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showAlert, showToast, routeBackOnError }),
    showLoading,
    hideLoading,
    onSwipeoutDeleted: (...args: any[]) => onSwipeoutDeleted(...args)
}));

jest.mock('@/views/base/accounts/ReconciliationStatementPageBase.ts', () => ({
    useReconciliationStatementPageBase: () => {
        lastBase = createBase();
        return lastBase;
    }
}));

jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => accountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => categoriesStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => transactionsStore }));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 1, RTL: 2 } }));
jest.mock('@/core/datetime.ts', () => ({ DateRange: dateRange, DateRangeScene: { Normal: 1 } }));
jest.mock('@/core/account.ts', () => ({ AccountType: accountType }));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: transactionType }));
jest.mock('@/core/statistics.ts', () => ({ ChartDateAggregationType: chartDateAggregationType }));
jest.mock('@/consts/transaction.ts', () => ({
    TRANSACTION_MIN_AMOUNT: -99_999_999_999,
    TRANSACTION_MAX_AMOUNT: 99_999_999_999
}));
jest.mock('@/lib/common.ts', () => ({
    isDefined: (value: unknown) => value !== null && value !== undefined,
    isEquals: (left: unknown, right: unknown) => JSON.stringify(left) === JSON.stringify(right),
    findDisplayNameByType: (values: Array<{ type: number; displayName: string }>, type: number) => (
        values.find(value => value.type === type)?.displayName
    )
}));
jest.mock('@/lib/datetime.ts', () => ({
    getCurrentUnixTime: () => getCurrentUnixTime(),
    getDateTypeByDateRange: (...args: any[]) => getDateTypeByDateRange(...args),
    getDateTypeByBillingCycleDateRange: (...args: any[]) => getDateTypeByBillingCycleDateRange(...args),
    getDateRangeByDateType: (...args: any[]) => getDateRangeByDateType(...args),
    getDateRangeByBillingCycleDateType: (...args: any[]) => getDateRangeByBillingCycleDateType(...args)
}));

import ReconciliationStatementPage from '@/views/mobile/accounts/ReconciliationStatementPage.vue';

function setup(query: Record<string, string> = { accountId: 'wallet' }): { bindings: any; router: any } {
    const router = { navigate: jest.fn(), back: jest.fn() };
    const bindings = (ReconciliationStatementPage as any).setup(
        { f7route: { query }, f7router: router },
        { expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function render(bindings: any): unknown {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return (ReconciliationStatementPage as any).render(exposed, [], {}, exposed, {}, {});
}

function collectRenderCallbacks(
    value: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => any }>,
    seen = new Set<any>()
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) collectRenderCallbacks(item, callbacks, seen);
        return;
    }

    if (value.props && typeof value.props === 'object') {
        for (const [name, prop] of Object.entries(value.props)) {
            if (name.startsWith('on')) {
                if (typeof prop === 'function') {
                    callbacks.push({ name, callback: prop as (...args: any[]) => any });
                } else if (Array.isArray(prop)) {
                    for (const callback of prop) {
                        if (typeof callback === 'function') {
                            callbacks.push({ name, callback: callback as (...args: any[]) => any });
                        }
                    }
                }
            }
        }
    }

    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const slot of Object.values(value.children)) {
            if (typeof slot === 'function') {
                collectRenderCallbacks((slot as (...args: any[]) => any)({}), callbacks, seen);
            } else {
                collectRenderCallbacks(slot, callbacks, seen);
            }
        }
    } else {
        collectRenderCallbacks(value.children, callbacks, seen);
    }

    collectRenderCallbacks(value.dynamicChildren, callbacks, seen);
    collectRenderCallbacks(value.ssContent, callbacks, seen);
    collectRenderCallbacks(value.ssFallback, callbacks, seen);
}

async function invokeRenderCallbacks(callbacks: Array<{ name: string; callback: (...args: any[]) => any }>): Promise<void> {
    for (const { name, callback } of callbacks) {
        if (name === 'onDateRange:change') {
            callback(9_000, 10_000);
        } else if (name === 'onUpdate:modelValue') {
            callback(20_000);
        } else if (name.startsWith('onUpdate:')) {
            callback(true);
        } else {
            callback();
        }
        await flush(2);
    }
}

function mountWithHostRenderer(): { app: any; router: any; root: any; state: any } {
    const { createRenderer, defineComponent, h } = jest.requireActual('vue') as any;
    const createNode = (type: string, text = ''): any => ({ type, text, children: [], parent: null, props: {} });
    const renderer = createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(type: string) {
            return createNode(type);
        },
        createText(text: string) {
            return createNode('#text', text);
        },
        createComment(text: string) {
            return createNode('#comment', text);
        },
        setText(node: any, text: string) {
            node.text = text;
        },
        setElementText(node: any, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: any) {
            return node.parent;
        },
        nextSibling(node: any) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: any, id: string) {
            node.props[id] = '';
        },
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = createNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        }
    });
    const SlotHost = defineComponent({
        name: 'SlotHost',
        setup(_props: unknown, { slots }: any) {
            return () => h('stub', null, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
        }
    });
    const router = { navigate: jest.fn(), back: jest.fn() };
    const app = renderer.createApp(ReconciliationStatementPage as any, {
        f7route: { query: { accountId: 'wallet' } },
        f7router: router
    });
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link', 'f7-icon',
        'f7-popover', 'f7-list', 'f7-list-item', 'f7-card', 'f7-card-header', 'f7-card-content',
        'f7-swipeout-actions', 'f7-swipeout-button', 'f7-actions', 'f7-actions-group', 'f7-actions-button',
        'f7-actions-label', 'account-balance-trends-bar-chart', 'date-range-selection-sheet',
        'number-pad-sheet', 'ItemIcon'
    ]) {
        app.component(name, SlotHost);
    }
    const root = createNode('root');
    const vm = app.mount(root) as any;
    return { app, router, root, state: vm.$.setupState };
}

beforeEach(() => {
    jest.clearAllMocks();
    transactionsStore.transactionReconciliationStatementStateInvalid = false;
    accountsStore.loadAllAccounts.mockResolvedValue(undefined);
    accountsStore.getAccountStatementDate.mockReturnValue(20);
    categoriesStore.loadAllCategories.mockResolvedValue(undefined);
    transactionsStore.getReconciliationStatements.mockResolvedValue(createStatement());
    transactionsStore.deleteTransaction.mockResolvedValue(undefined);
    dateRange.isBillingCycle.mockReturnValue(false);
    getCurrentUnixTime.mockReturnValue(1_800);
    getDateRangeByDateType.mockReturnValue({ dateType: 10, minTime: 1_000, maxTime: 2_000 });
    getDateRangeByBillingCycleDateType.mockReturnValue({ dateType: 20, minTime: 3_000, maxTime: 4_000 });
    getDateTypeByDateRange.mockReturnValue(99);
    getDateTypeByBillingCycleDateRange.mockReturnValue(null);
});

describe('mobile reconciliation statement production behavior', () => {
    test('initializes the selected account, default range, stores, and display contracts', async () => {
        const { bindings } = setup();
        await flush();

        expect(lastBase.accountId.value).toBe('wallet');
        expect(lastBase.startTime.value).toBe(1_000);
        expect(lastBase.endTime.value).toBe(2_000);
        expect(bindings.finishQuery.value).toBe(false);
        expect(bindings.loading.value).toBe(false);
        expect(bindings.validQuery.value).toBe(true);
        expect(bindings.textDirection.value).toBe(1);
        expect(bindings.allAvailableDateRanges.value).toHaveLength(3);
        expect(bindings.displayStartTime.value).toBe('long:1000');
        expect(bindings.displayEndTime.value).toBe('long:2000');
        expect(bindings.chartDataDateAggregationTypeDisplayName.value).toBe('Day');
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(categoriesStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(accountsStore.getAccountStatementDate).toHaveBeenCalledWith('wallet');

        lastBase.currentAccount.value = { type: accountType.MultiAccount.type };
        expect(bindings.validQuery.value).toBe(false);
        lastBase.currentAccount.value = undefined;
        expect(bindings.validQuery.value).toBeFalsy();
        lastBase.allDateAggregationTypes.value = [];
        expect(bindings.chartDataDateAggregationTypeDisplayName.value).toBe('Unknown');

        getDateRangeByDateType.mockReturnValueOnce(null);
        const fallback = setup();
        expect(lastBase.startTime.value).toBe(0);
        expect(lastBase.endTime.value).toBe(0);
        fallback.bindings.updateClosingBalance(undefined);
        expect(fallback.bindings.newClosingBalance.value).toBe(0);
    });

    test('retains initialization failures for route recovery and reports readable and raw errors', async () => {
        accountsStore.loadAllAccounts.mockRejectedValueOnce(new Error('accounts unavailable'));
        const first = setup({});
        await flush();
        expect(lastBase.accountId.value).toBe('');
        expect(first.bindings.loadingError.value).toEqual(new Error('accounts unavailable'));
        expect(showToast).toHaveBeenCalledWith('accounts unavailable');
        first.bindings.onPageAfterIn();
        expect(routeBackOnError).toHaveBeenCalledWith(first.router, first.bindings.loadingError);

        categoriesStore.loadAllCategories.mockRejectedValueOnce('category failure');
        setup();
        await flush();
        expect(showToast).toHaveBeenCalledWith('category failure');
    });

    test('groups statement rows by display date and exposes empty and DOM-id behavior', () => {
        const { bindings } = setup();
        expect(bindings.allReconciliationStatementVirtualListItems.value).toEqual([]);

        const first = createTransaction({ id: 'first', time: 86_400 });
        const second = createTransaction({ id: 'second', time: 86_401 });
        const third = createTransaction({ id: 'third', time: 172_800 });
        lastBase.reconciliationStatements.value = createStatement({ transactions: [first, second, third] });

        expect(bindings.allReconciliationStatementVirtualListItems.value).toEqual([
            { index: 0, type: 'date', displayDate: 'date:1' },
            { index: 1, type: 'transaction', transaction: first },
            { index: 2, type: 'transaction', transaction: second },
            { index: 3, type: 'date', displayDate: 'date:2' },
            { index: 4, type: 'transaction', transaction: third }
        ]);
        expect(bindings.getTransactionDomId(second)).toBe('transaction_second');

        lastBase.reconciliationStatements.value = createStatement({ transactions: [] });
        expect(bindings.allReconciliationStatementVirtualListItems.value).toEqual([]);
        lastBase.reconciliationStatements.value = { transactions: undefined };
        expect(bindings.allReconciliationStatementVirtualListItems.value).toEqual([]);
    });

    test('changes normal, billing-cycle, custom, and invalid date ranges', () => {
        const { bindings } = setup();

        bindings.changeDateFilter(dateRange.Custom.type);
        expect(bindings.showCustomDateRangeSheet.value).toBe(true);

        getDateRangeByDateType.mockReturnValueOnce(null);
        bindings.changeDateFilter(11);
        expect(bindings.queryDateRangeType.value).toBe(dateRange.ThisMonth.type);

        getDateRangeByDateType.mockReturnValueOnce({ dateType: 11, minTime: 11_000, maxTime: 12_000 });
        bindings.changeDateFilter(11);
        expect(bindings.queryDateRangeType.value).toBe(11);
        expect(lastBase.startTime.value).toBe(11_000);
        expect(lastBase.endTime.value).toBe(12_000);

        dateRange.isBillingCycle.mockReturnValueOnce(true);
        bindings.changeDateFilter(20);
        expect(getDateRangeByBillingCycleDateType).toHaveBeenCalledWith(20, 1, 101, 20);
        expect(bindings.queryDateRangeType.value).toBe(20);
        expect(lastBase.startTime.value).toBe(3_000);
        expect(lastBase.endTime.value).toBe(4_000);

        bindings.changeCustomDateFilter(0, 9_000);
        bindings.changeCustomDateFilter(8_000, 0);
        expect(bindings.queryDateRangeType.value).toBe(20);

        bindings.showCustomDateRangeSheet.value = true;
        getDateTypeByBillingCycleDateRange.mockReturnValueOnce(21);
        bindings.changeCustomDateFilter(5_000, 6_000);
        expect(bindings.queryDateRangeType.value).toBe(21);
        expect(getDateTypeByDateRange).not.toHaveBeenCalled();

        bindings.showCustomDateRangeSheet.value = true;
        getDateTypeByBillingCycleDateRange.mockReturnValueOnce(null);
        getDateTypeByDateRange.mockReturnValueOnce(99);
        bindings.changeCustomDateFilter(7_000, 8_000);
        expect(bindings.queryDateRangeType.value).toBe(99);
        expect(lastBase.startTime.value).toBe(7_000);
        expect(lastBase.endTime.value).toBe(8_000);
        expect(bindings.showCustomDateRangeSheet.value).toBe(false);
    });

    test('loads statements, distinguishes forced refresh outcomes, and handles processed errors', async () => {
        const initial = createStatement();
        transactionsStore.getReconciliationStatements.mockResolvedValueOnce(initial);
        const { bindings } = setup();

        bindings.reload(false);
        expect(bindings.finishQuery.value).toBe(true);
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(transactionsStore.getReconciliationStatements).toHaveBeenCalledWith({
            accountId: 'wallet', startTime: 1_000, endTime: 2_000
        });
        expect(lastBase.reconciliationStatements.value).toStrictEqual(initial);
        expect(bindings.loading.value).toBe(false);

        transactionsStore.getReconciliationStatements.mockResolvedValueOnce(createStatement());
        bindings.reload(true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('Data is up to date');

        transactionsStore.getReconciliationStatements.mockResolvedValueOnce(createStatement({ closingBalanceCents: 123_456 }));
        bindings.reload(true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('Data has been updated');

        transactionsStore.getReconciliationStatements.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reload(false);
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled');
        expect(bindings.loading.value).toBe(false);

        transactionsStore.getReconciliationStatements.mockRejectedValueOnce({ processed: false, message: 'network failure' });
        bindings.reload(false);
        await flush();
        expect(showToast).toHaveBeenCalledWith('network failure');

        transactionsStore.getReconciliationStatements.mockRejectedValueOnce('raw failure');
        bindings.reload(false);
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw failure');
    });

    test('builds add, duplicate, and edit navigation from the selected account and transaction', () => {
        const { bindings, router } = setup();
        const transaction = createTransaction({ id: 'tx / 1', type: transactionType.Transfer });

        bindings.addTransaction();
        bindings.duplicateTransaction(transaction);
        bindings.editTransaction(transaction);

        expect(router.navigate).toHaveBeenNthCalledWith(1, '/transaction/add?accountId=wallet');
        expect(router.navigate).toHaveBeenNthCalledWith(2, '/transaction/add?id=tx / 1&type=4');
        expect(router.navigate).toHaveBeenNthCalledWith(3, '/transaction/edit?id=tx / 1&type=4');
    });

    test('opens and routes closing-balance adjustments in cents without yuan conversion', () => {
        const { bindings, router } = setup();
        lastBase.reconciliationStatements.value = createStatement({ closingBalanceCents: 12_345 });

        bindings.updateClosingBalance(undefined);
        expect(bindings.newClosingBalance.value).toBe(12_345);
        expect(bindings.showNewClosingBalanceSheet.value).toBe(true);

        bindings.updateClosingBalance(23_456);
        expect(router.navigate).toHaveBeenLastCalledWith(
            '/transaction/add?type=2&sourceAmountCents=11111&accountId=wallet&noTransactionDraft=true'
        );

        bindings.updateClosingBalance(1_234);
        expect(router.navigate).toHaveBeenLastCalledWith(
            '/transaction/add?type=3&sourceAmountCents=11111&accountId=wallet&noTransactionDraft=true'
        );

        getCurrentUnixTime.mockReturnValue(3_000);
        bindings.updateClosingBalance(13_345);
        expect(router.navigate).toHaveBeenLastCalledWith(
            '/transaction/add?time=2000&type=2&sourceAmountCents=1000&accountId=wallet&noTransactionDraft=true'
        );

        getCurrentUnixTime.mockReturnValue(500);
        bindings.updateClosingBalance(13_345);
        expect(router.navigate).toHaveBeenLastCalledWith(
            '/transaction/add?time=1000&type=2&sourceAmountCents=1000&accountId=wallet&noTransactionDraft=true'
        );

        lastBase.isCurrentLiabilityAccount.value = true;
        getCurrentUnixTime.mockReturnValue(1_500);
        bindings.updateClosingBalance(undefined);
        expect(bindings.newClosingBalance.value).toBe(-12_345);
        bindings.updateClosingBalance(-2_345);
        expect(router.navigate).toHaveBeenLastCalledWith(
            '/transaction/add?type=3&sourceAmountCents=10000&accountId=wallet&noTransactionDraft=true'
        );
        bindings.updateClosingBalance(-22_345);
        expect(router.navigate).toHaveBeenLastCalledWith(
            '/transaction/add?type=2&sourceAmountCents=10000&accountId=wallet&noTransactionDraft=true'
        );
    });

    test('confirms deletion, waits for swipeout removal, reloads, and reports failures', async () => {
        const { bindings } = setup();
        const transaction = createTransaction();

        bindings.removeTransaction(null, false);
        expect(showAlert).toHaveBeenCalledWith('An error occurred');

        bindings.removeTransaction(transaction, false);
        expect(bindings.transactionToDelete.value).toStrictEqual(transaction);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        transactionsStore.deleteTransaction.mockImplementationOnce(async ({ beforeResolve }: any) => {
            beforeResolve('done-callback');
        });
        bindings.removeTransaction(transaction, true);
        expect(showLoading).toHaveBeenCalled();
        expect(bindings.showDeleteActionSheet.value).toBe(false);
        expect(bindings.transactionToDelete.value).toBeNull();
        await flush();
        expect(onSwipeoutDeleted).toHaveBeenCalledWith('transaction_transaction-1', 'done-callback');
        expect(hideLoading).toHaveBeenCalled();
        expect(transactionsStore.getReconciliationStatements).toHaveBeenCalled();

        transactionsStore.deleteTransaction.mockRejectedValueOnce({ processed: true, message: 'handled delete' });
        bindings.removeTransaction(transaction, true);
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled delete');

        transactionsStore.deleteTransaction.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.removeTransaction(transaction, true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('delete failed');

        transactionsStore.deleteTransaction.mockRejectedValueOnce('raw delete failure');
        bindings.removeTransaction(transaction, true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('raw delete failure');
    });

    test('updates chart and virtual-list state and refreshes only stale finished pages', async () => {
        const { bindings, router } = setup();
        bindings.setChartDataDateAggregationType(2);
        expect(bindings.chartDataDateAggregationType.value).toBe(2);
        expect(bindings.showChartDataDateAggregationTypePopover.value).toBe(false);

        const virtualData = { items: [{ index: 0, type: 'date', displayDate: 'date' }], topPosition: 42 };
        bindings.renderExternal({}, virtualData);
        expect(bindings.virtualDataItems.value).toStrictEqual(virtualData);

        transactionsStore.transactionReconciliationStatementStateInvalid = true;
        bindings.onPageAfterIn();
        expect(transactionsStore.getReconciliationStatements).not.toHaveBeenCalled();
        expect(routeBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);

        bindings.finishQuery.value = true;
        bindings.onPageAfterIn();
        await flush();
        expect(transactionsStore.getReconciliationStatements).toHaveBeenCalledTimes(1);
    });

    test('renders query, loading, empty, populated, liability, transfer, and chart template branches', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { bindings } = setup();
            await flush();
            expect(render(bindings)).toBeDefined();

            bindings.queryDateRangeType.value = dateRange.Custom.type;
            lastBase.startTime.value = 1_000;
            lastBase.endTime.value = 2_000;
            const customCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            const customTree = render(bindings);
            collectRenderCallbacks(customTree, customCallbacks);
            expect(customTree).toBeDefined();

            lastBase.startTime.value = 0;
            collectRenderCallbacks(render(bindings), []);
            lastBase.startTime.value = 1_000;
            lastBase.endTime.value = 0;
            collectRenderCallbacks(render(bindings), []);

            bindings.finishQuery.value = true;
            bindings.loading.value = true;
            expect(render(bindings)).toBeDefined();

            bindings.loading.value = false;
            lastBase.startTime.value = 0;
            lastBase.endTime.value = 0;
            lastBase.reconciliationStatements.value = createStatement({ transactions: [] });
            expect(render(bindings)).toBeDefined();

            const modify = createTransaction({
                id: 'modify', type: transactionType.ModifyBalance, comment: '', editable: true, utcOffset: 0
            });
            const incomingTransfer = createTransaction({
                id: 'incoming', type: transactionType.Transfer, sourceAccountId: 'bank', destinationAccountId: 'wallet'
            });
            const income = createTransaction({ id: 'income', type: transactionType.Income, categoryId: 'missing', editable: false });
            lastBase.startTime.value = 1_000;
            lastBase.endTime.value = 2_000;
            lastBase.isCurrentLiabilityAccount.value = true;
            lastBase.reconciliationStatements.value = createStatement({ transactions: [modify, incomingTransfer, income] });
            bindings.virtualDataItems.value = {
                items: bindings.allReconciliationStatementVirtualListItems.value,
                topPosition: 17
            };
            expect(bindings.finishQuery.value).toBe(true);
            expect(bindings.showAccountBalanceTrendsCharts.value).toBe(false);
            expect(bindings.loading.value).toBe(false);
            expect(bindings.allReconciliationStatementVirtualListItems.value.length).toBeGreaterThan(0);
            expect(bindings.virtualDataItems.value.items.length).toBeGreaterThan(0);
            const populatedCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            const populatedTree = render(bindings);
            collectRenderCallbacks(populatedTree, populatedCallbacks);
            expect(populatedTree).toBeDefined();

            bindings.showAccountBalanceTrendsCharts.value = true;
            expect(render(bindings)).toBeDefined();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('executes production template page, popover, filter, swipeout, sheet, and action handlers', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { bindings } = setup();
            await flush();

            const queryCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectRenderCallbacks(render(bindings), queryCallbacks);
            await invokeRenderCallbacks(queryCallbacks);

            bindings.finishQuery.value = true;
            bindings.loading.value = false;
            lastBase.startTime.value = 1_000;
            lastBase.endTime.value = 2_000;
            lastBase.reconciliationStatements.value = createStatement({
                transactions: [
                    createTransaction({ id: 'expense' }),
                    createTransaction({
                        id: 'incoming',
                        type: transactionType.Transfer,
                        sourceAccountId: 'bank',
                        destinationAccountId: 'wallet'
                    }),
                    createTransaction({ id: 'modify', type: transactionType.ModifyBalance })
                ]
            });
            bindings.virtualDataItems.value = {
                items: bindings.allReconciliationStatementVirtualListItems.value,
                topPosition: 9
            };

            const resultCallbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectRenderCallbacks(render(bindings), resultCallbacks);
            await invokeRenderCallbacks(resultCallbacks);

            expect(queryCallbacks.length).toBeGreaterThan(5);
            expect(resultCallbacks.length).toBeGreaterThan(10);
            expect(transactionsStore.getReconciliationStatements).toHaveBeenCalled();
        } finally {
            warnSpy.mockRestore();
        }
    });

    test('renders Framework7 named slots through a no-DOM production component runtime', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        const mounted = mountWithHostRenderer();
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            await flush();
            mounted.state.finishQuery = true;
            mounted.state.loading = false;
            lastBase.startTime.value = 1_000;
            lastBase.endTime.value = 2_000;
            lastBase.reconciliationStatements.value = createStatement({
                transactions: [
                    createTransaction({ id: 'date-and-expense', time: 86_400 }),
                    createTransaction({
                        id: 'incoming-transfer',
                        type: transactionType.Transfer,
                        time: 86_401,
                        sourceAccountId: 'bank',
                        destinationAccountId: 'wallet'
                    }),
                    createTransaction({
                        id: 'modify',
                        type: transactionType.ModifyBalance,
                        time: 172_800,
                        categoryId: 'missing',
                        comment: '',
                        utcOffset: 0
                    })
                ]
            });
            mounted.state.virtualDataItems = {
                items: mounted.state.allReconciliationStatementVirtualListItems,
                topPosition: 23
            };
            await nextTick();

            expect(mounted.root.children.length).toBeGreaterThan(0);
            expect(lastBase.getDisplayDestinationAmount).toHaveBeenCalled();
            expect(lastBase.getDisplaySourceAmount).toHaveBeenCalled();
            expect(lastBase.getDisplayAccountBalance).toHaveBeenCalled();

            lastBase.isCurrentLiabilityAccount.value = true;
            mounted.state.showAccountBalanceTrendsCharts = true;
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
            warnSpy.mockRestore();
        }
    });
});
