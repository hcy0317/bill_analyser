import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const pageTypes = {
    List: { type: 1, name: 'List' },
    Calendar: { type: 2, name: 'Calendar' },
    values: () => [
        { type: 1, name: 'List' },
        { type: 2, name: 'Calendar' }
    ]
};

const transactionTypes = {
    ModifyBalance: 1,
    Income: 2,
    Expense: 3,
    Transfer: 4,
    Investment: 5
};

const dateRanges = {
    All: { type: 0 },
    ThisMonth: { type: 10 },
    LastMonth: { type: 11 },
    Custom: { type: 99 },
    isBillingCycle: jest.fn<(value: number) => boolean>()
};

const showAlert = jest.fn<(...args: any[]) => void>();
const showToast = jest.fn<(...args: any[]) => void>();
const routeBackOnError = jest.fn<(...args: any[]) => void>();
const showLoading = jest.fn<() => void>();
const hideLoading = jest.fn<() => void>();
const onSwipeoutDeleted = jest.fn<(...args: any[]) => void>();
const scrollToSelectedItem = jest.fn<(...args: any[]) => void>();
const infiniteCallbacks: Array<() => void> = [];
const mountedCallbacks: Array<() => void> = [];
const unmountedCallbacks: Array<() => void> = [];
const watchCallbacks: Array<(newValue: string, oldValue: string) => void> = [];

const getDateRangeByDateType = jest.fn<(...args: any[]) => any>();
const getDateRangeByBillingCycleDateType = jest.fn<(...args: any[]) => any>();
const getDateTypeByDateRange = jest.fn<(...args: any[]) => number | null>();
const getDateTypeByBillingCycleDateRange = jest.fn<(...args: any[]) => number | null>();
const getFullMonthDateRange = jest.fn<(...args: any[]) => any>();
const getShiftedDateRange = jest.fn<(...args: any[]) => any>();
const getShiftedBillingRange = jest.fn<(...args: any[]) => any>();
const getCurrentUnixTime = jest.fn<() => number>();
const getActualUnixTimeForStore = jest.fn<(...args: any[]) => number>();
const getBrowserTimezoneOffsetMinutes = jest.fn<() => number>();
const getDayFirstUnixTime = jest.fn<(value: number) => number>();
const getYearMonthFirstUnixTime = jest.fn<(...args: any[]) => number>();
const getYearMonthLastUnixTime = jest.fn<(...args: any[]) => number>();
const getValidMonthDay = jest.fn<(...args: any[]) => string>();
const parseDateTime = jest.fn<(...args: any[]) => any>();

const resetMonthState = jest.fn<() => void>();
const setMonthHeights = jest.fn<(...args: any[]) => Promise<void>>();
const setInvisibleMonths = jest.fn<() => void>();
const logger = { debug: jest.fn(), error: jest.fn(), info: jest.fn(), warn: jest.fn() };

let lastBase: any;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const query = ref({
        dateType: 0,
        minTime: 100,
        maxTime: 200,
        type: transactionTypes.Expense,
        categoryIds: '',
        accountIds: '',
        tagIds: '',
        tagFilterType: 0,
        amountFilterCents: '',
        keyword: ''
    });
    const categoryMap = ref({} as Record<string, boolean>);
    const accountMap = ref({} as Record<string, boolean>);
    const tagMap = ref({} as Record<string, boolean>);

    return {
        pageType: ref(pageTypes.List.type),
        loading: ref(false),
        customMinDatetime: ref(0),
        customMaxDatetime: ref(0),
        currentCalendarDate: ref('2026-07-15'),
        currentTimezoneOffsetMinutes: ref(480),
        firstDayOfWeek: ref(1),
        fiscalYearStart: ref(1),
        defaultCurrency: ref('CNY'),
        showTotalAmountInTransactionListPage: ref(true),
        showTagInTransactionListPage: ref(true),
        allDateRanges: ref([
            { type: 0, displayName: 'All' },
            { type: 10, displayName: 'This month' },
            { type: 99, displayName: 'Custom', isUserCustomRange: true }
        ]),
        allAccounts: ref([{ id: 'wallet', name: 'Wallet', icon: 'wallet', color: '#fff', hidden: false }]),
        allAccountsMap: ref({}),
        allAvailableAccountsCount: ref(1),
        allCategories: ref({
            expense: { id: 'expense', type: 13, name: 'Expense', subCategories: [] },
            income: { id: 'income', type: 12, name: 'Income', subCategories: [] }
        }),
        allPrimaryCategories: ref({
            3: [{ id: 'expense', name: 'Expense', icon: 'cart', color: '#fff', hidden: false, subCategories: [
                { id: 'food', name: 'Food', icon: 'fork', color: '#fff', hidden: false }
            ] }]
        }),
        allAvailableCategoriesCount: ref(2),
        allTransactionTags: ref([{ id: 'daily', name: 'Daily', hidden: false }]),
        allAvailableTagsCount: ref(1),
        displayPageTypeName: ref('List'),
        query,
        queryDateRangeName: ref('All'),
        queryMinTime: computed(() => `min:${query.value.minTime}`),
        queryMaxTime: computed(() => `max:${query.value.maxTime}`),
        queryMonthlyData: ref(false),
        queryMonth: ref('2026-07'),
        queryAllFilterCategoryIds: categoryMap,
        queryAllFilterAccountIds: accountMap,
        queryAllFilterTagIds: tagMap,
        queryAllFilterCategoryIdsCount: computed(() => Object.keys(categoryMap.value).length),
        queryAllFilterAccountIdsCount: computed(() => Object.keys(accountMap.value).length),
        queryAllFilterTagIdsCount: computed(() => Object.keys(tagMap.value).length),
        queryAccountName: ref('All accounts'),
        queryCategoryName: ref('All categories'),
        queryAmount: ref('1.00–2.00 CNY'),
        transactionCalendarMinDate: ref('2026-01-01'),
        transactionCalendarMaxDate: ref('2026-12-31'),
        currentMonthTransactionData: ref(null),
        canAddTransaction: ref(true),
        getDisplayTime: jest.fn(),
        getDisplayLongYearMonth: jest.fn(),
        getDisplayTimezone: jest.fn(),
        getDisplayAmount: jest.fn(),
        getDisplayMonthTotalAmount: jest.fn(),
        getTransactionTypeName: jest.fn()
    };
}

const environmentsStore = { framework7DarkMode: false };
const accountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
    getAccountStatementDate: jest.fn(() => 20)
};
const categoriesStore = { loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>() };
const tagsStore = { loadAllTags: jest.fn<(...args: any[]) => Promise<void>>() };
const transactionsStore = {
    transactions: [] as any[],
    noTransaction: false,
    hasMoreTransaction: true,
    transactionListStateInvalid: false,
    initTransactionListFilter: jest.fn<(value: Record<string, unknown>) => void>(),
    updateTransactionListFilter: jest.fn<(value: Record<string, unknown>) => boolean>(),
    loadMonthlyAllTransactions: jest.fn<(...args: any[]) => Promise<void>>(),
    loadTransactions: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteTransaction: jest.fn<(...args: any[]) => Promise<void>>(),
    collapseMonthInTransactionList: jest.fn<(...args: any[]) => void>()
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        onMounted: (callback: () => void) => mountedCallbacks.push(callback),
        onUnmounted: (callback: () => void) => unmountedCallbacks.push(callback),
        watch: (_source: unknown, callback: (newValue: string, oldValue: string) => void) => {
            watchCallbacks.push(callback);
            return jest.fn();
        }
    };
});

jest.mock('@/lib/vue_external_template.ts', () => ({ useExternalTemplateBindings: jest.fn() }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getCurrentLanguageTextDirection: () => 1,
        getAllTransactionTagFilterTypes: () => [
            { type: 1, displayName: 'Any tag' },
            { type: 2, displayName: 'All tags' }
        ]
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showAlert, showToast, routeBackOnError }),
    showLoading,
    hideLoading,
    onSwipeoutDeleted: (...args: any[]) => onSwipeoutDeleted(...args),
    scrollToSelectedItem: (...args: any[]) => scrollToSelectedItem(...args),
    onInfiniteScrolling: (callback: () => void) => infiniteCallbacks.push(callback)
}));
jest.mock('@/views/base/transactions/TransactionListPageBase.ts', () => ({
    TransactionListPageType: pageTypes,
    useTransactionListPageBase: () => {
        lastBase = createBase();
        return lastBase;
    }
}));
jest.mock('@/views/mobile/transactions/useMobileTransactionMonthList.ts', () => {
    const { ref } = jest.requireActual('vue') as any;
    return {
        useMobileTransactionMonthList: () => ({
            transactionInvisibleYearMonths: ref({} as Record<string, boolean>),
            resetTransactionMonthListState: resetMonthState,
            getTransactionMonthTitleDomId: (value: any) => `title-${value.yearDashMonth}`,
            getTransactionMonthListDomId: (value: any) => `month-${value.yearDashMonth}`,
            getTransactionDomId: (value: any) => `transaction-${value.id}`,
            isTransactionMonthListInvisible: () => false,
            getTransactionMonthListHeight: () => 100,
            setTransactionMonthListHeights: (...args: any[]) => setMonthHeights(...args),
            setTransactionInvisibleYearMonthList: setInvisibleMonths,
            getTransactionDateStyle: () => ({ color: 'red' })
        })
    };
});
jest.mock('@/stores/environment.ts', () => ({ useEnvironmentsStore: () => environmentsStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => accountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => categoriesStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => tagsStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => transactionsStore }));
jest.mock('@/core/base.ts', () => ({ keys: (value: object) => Object.keys(value || {}) }));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 1, RTL: 2 } }));
jest.mock('@/core/datetime.ts', () => ({ DateRangeScene: { Normal: 1 }, DateRange: dateRanges }));
jest.mock('@/core/numeral.ts', () => ({
    AmountFilterType: { values: () => [{ type: 'between', name: 'Between' }] }
}));
jest.mock('@/core/transaction.ts', () => ({ TransactionType: transactionTypes }));
jest.mock('@/lib/datetime.ts', () => ({
    getCurrentUnixTime: () => getCurrentUnixTime(),
    parseDateTimeFromUnixTime: (...args: any[]) => parseDateTime(...args),
    getBrowserTimezoneOffsetMinutes: () => getBrowserTimezoneOffsetMinutes(),
    getActualUnixTimeForStore: (...args: any[]) => getActualUnixTimeForStore(...args),
    getDayFirstUnixTimeBySpecifiedUnixTime: (value: number) => getDayFirstUnixTime(value),
    getYearMonthFirstUnixTime: (...args: any[]) => getYearMonthFirstUnixTime(...args),
    getYearMonthLastUnixTime: (...args: any[]) => getYearMonthLastUnixTime(...args),
    getShiftedDateRangeAndDateType: (...args: any[]) => getShiftedDateRange(...args),
    getShiftedDateRangeAndDateTypeForBillingCycle: (...args: any[]) => getShiftedBillingRange(...args),
    getDateTypeByDateRange: (...args: any[]) => getDateTypeByDateRange(...args),
    getDateTypeByBillingCycleDateRange: (...args: any[]) => getDateTypeByBillingCycleDateRange(...args),
    getDateRangeByDateType: (...args: any[]) => getDateRangeByDateType(...args),
    getDateRangeByBillingCycleDateType: (...args: any[]) => getDateRangeByBillingCycleDateType(...args),
    getFullMonthDateRange: (...args: any[]) => getFullMonthDateRange(...args),
    getValidMonthDayOrCurrentDayShortDate: (...args: any[]) => getValidMonthDay(...args)
}));
jest.mock('@/lib/category.ts', () => ({
    categoryTypeToTransactionType: (value: number) => value,
    transactionTypeToCategoryType: (value: number) => value + 10
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: logger }));
jest.mock('@/views/mobile/transactions/components/MobileTransactionMonthBlock.vue', () => ({
    __esModule: true,
    default: { name: 'MobileTransactionMonthBlockStub' }
}));

import ListPage from '@/views/mobile/transactions/ListPage.vue';

function setup(query: Record<string, string> = {}): { bindings: any; router: any } {
    const router = { navigate: jest.fn() };
    const bindings = (ListPage as any).setup(
        { f7route: { query }, f7router: router },
        { expose: jest.fn() }
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function render(bindings: any): any {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return (ListPage as any).render(exposed, [], {}, exposed, {}, {});
}

function collectRenderCallbacks(value: any, callbacks: Array<{ name: string; callback: (...args: any[]) => any }>, seen = new Set<any>()): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);

    if (Array.isArray(value)) {
        for (const item of value) collectRenderCallbacks(item, callbacks, seen);
        return;
    }

    if (value.props && typeof value.props === 'object') {
        for (const [name, callback] of Object.entries(value.props)) {
            if (name.startsWith('on') && typeof callback === 'function') {
                callbacks.push({ name, callback: callback as (...args: any[]) => any });
            }
        }
    }

    if (value.children && typeof value.children === 'object' && !Array.isArray(value.children)) {
        for (const slot of Object.values(value.children)) {
            if (typeof slot === 'function') {
                const rendered = (slot as (...args: any[]) => any)({});
                collectRenderCallbacks(rendered, callbacks, seen);
            } else {
                collectRenderCallbacks(slot, callbacks, seen);
            }
        }
    } else {
        collectRenderCallbacks(value.children, callbacks, seen);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mountedCallbacks.length = 0;
    unmountedCallbacks.length = 0;
    watchCallbacks.length = 0;
    infiniteCallbacks.length = 0;
    (window as any).addEventListener = jest.fn();
    (window as any).removeEventListener = jest.fn();

    environmentsStore.framework7DarkMode = false;
    transactionsStore.transactions = [];
    transactionsStore.noTransaction = false;
    transactionsStore.hasMoreTransaction = true;
    transactionsStore.transactionListStateInvalid = false;

    dateRanges.isBillingCycle.mockImplementation(value => value === 77);
    getDateRangeByDateType.mockImplementation((value?: number) => value ? { dateType: value, minTime: 1000, maxTime: 2000 } : null);
    getDateRangeByBillingCycleDateType.mockReturnValue({ dateType: 77, minTime: 3000, maxTime: 4000 });
    getDateTypeByBillingCycleDateRange.mockReturnValue(null);
    getDateTypeByDateRange.mockReturnValue(55);
    getFullMonthDateRange.mockReturnValue({ dateType: 44, minTime: 5000, maxTime: 6000 });
    getShiftedBillingRange.mockReturnValue(null);
    getShiftedDateRange.mockReturnValue({ dateType: 66, minTime: 7000, maxTime: 8000 });
    getCurrentUnixTime.mockReturnValue(150);
    getActualUnixTimeForStore.mockReturnValue(9000);
    getBrowserTimezoneOffsetMinutes.mockReturnValue(480);
    getDayFirstUnixTime.mockReturnValue(8000);
    getYearMonthFirstUnixTime.mockReturnValue(10_000);
    getYearMonthLastUnixTime.mockReturnValue(20_000);
    getValidMonthDay.mockReturnValue('2026-07-01');
    parseDateTime.mockReturnValue({
        getGregorianCalendarYear: () => 2026,
        getGregorianCalendarMonth: () => 6
    });

    accountsStore.loadAllAccounts.mockResolvedValue(undefined);
    categoriesStore.loadAllCategories.mockResolvedValue(undefined);
    tagsStore.loadAllTags.mockResolvedValue(undefined);
    transactionsStore.loadTransactions.mockResolvedValue(undefined);
    transactionsStore.loadMonthlyAllTransactions.mockResolvedValue(undefined);
    transactionsStore.deleteTransaction.mockResolvedValue(undefined);
    setMonthHeights.mockResolvedValue(undefined);

    transactionsStore.initTransactionListFilter.mockImplementation(value => {
        if (!lastBase) return;
        for (const [key, fieldValue] of Object.entries(value)) {
            if (fieldValue !== undefined) lastBase.query.value[key] = fieldValue;
        }
    });
    transactionsStore.updateTransactionListFilter.mockImplementation(value => {
        let changed = false;
        for (const [key, fieldValue] of Object.entries(value)) {
            if (lastBase.query.value[key] !== fieldValue) {
                lastBase.query.value[key] = fieldValue;
                changed = true;
            }
        }
        return changed;
    });
});

describe('mobile transaction ListPage production behavior', () => {
    test('initializes route filters, loads the first page, and exposes display state', async () => {
        getDateRangeByDateType.mockReturnValueOnce(null);
        const { bindings } = setup({
            dateType: '77', minTime: '100', maxTime: '200', type: '3',
            categoryIds: 'food', accountIds: 'wallet', tagIds: 'daily',
            tagFilterType: '1', keyword: 'coffee'
        });
        await flush();

        expect(transactionsStore.initTransactionListFilter).toHaveBeenCalledWith({
            dateType: 77,
            minTime: 100,
            maxTime: 200,
            type: 3,
            categoryIds: 'food',
            accountIds: 'wallet',
            tagIds: 'daily',
            tagFilterType: 1,
            keyword: 'coffee'
        });
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: false });
        expect(categoriesStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(tagsStore.loadAllTags).toHaveBeenCalledWith({ force: false });
        expect(transactionsStore.loadTransactions).toHaveBeenCalledWith({ reload: true, autoExpand: true, defaultCurrency: 'CNY' });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.textDirection.value).toBe(1);
        expect(bindings.isDarkMode.value).toBe(false);
        environmentsStore.framework7DarkMode = true;
        const darkBindings = setup().bindings;
        expect(darkBindings.isDarkMode.value).toBe(true);
        expect(bindings.allTransactionTagFilterTypes.value).toHaveLength(2);
        expect(resetMonthState).toHaveBeenCalled();
        expect(setMonthHeights).toHaveBeenCalledWith(true);
    });

    test('projects list/calendar transactions, empty states, and zero-cent calendar totals', async () => {
        const { bindings } = setup();
        await flush();
        const listItem = { id: 'list-1', sourceAmountCents: -12_345 };
        transactionsStore.transactions = [{ yearDashMonth: '2026-07', items: [listItem] }];
        expect(bindings.transactions.value).toStrictEqual(transactionsStore.transactions);
        transactionsStore.noTransaction = true;
        expect(bindings.noTransaction.value).toBe(true);

        bindings.loading.value = true;
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.loading.value = false;
        bindings.pageType.value = pageTypes.Calendar.type;
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.queryMonthlyData.value = true;
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.currentMonthTransactionData.value = {};
        expect(bindings.transactions.value).toStrictEqual([]);
        bindings.currentMonthTransactionData.value = {
            year: 2026,
            month: 6,
            yearDashMonth: '2026-07',
            items: [
                { id: 'today', gregorianCalendarYearDashMonthDashDay: '2026-07-15', sourceAmountCents: -12_345 },
                { id: 'other', gregorianCalendarYearDashMonthDashDay: '2026-07-14', sourceAmountCents: 67_890 }
            ]
        };
        bindings.currentCalendarDate.value = '2026-07-15';
        expect(bindings.transactions.value).toStrictEqual([expect.objectContaining({
            items: [expect.objectContaining({ id: 'today', sourceAmountCents: -12_345 })],
            totalAmountCents: {
                incomeCents: 0,
                expenseCents: 0,
                incompleteIncome: false,
                incompleteExpense: false
            },
            dailyTotalAmountsCents: {}
        })]);
        expect(bindings.noTransaction.value).toBe(false);
        bindings.currentCalendarDate.value = '2026-07-13';
        expect(bindings.noTransaction.value).toBe(true);
        bindings.pageType.value = 999;
        expect(bindings.transactions.value).toStrictEqual([]);
        expect(bindings.noTransaction.value).toBe(true);
    });

    test('reloads monthly data, supports pull-to-refresh, and reports refresh failures', async () => {
        const { bindings } = setup();
        await flush();
        bindings.queryMonthlyData.value = true;
        const done = jest.fn();
        bindings.reload(done);
        await flush();
        expect(parseDateTime).toHaveBeenCalledWith(100);
        expect(transactionsStore.loadMonthlyAllTransactions).toHaveBeenCalledWith({
            year: 2026, month: 6, autoExpand: true, defaultCurrency: 'CNY'
        });
        expect(done).toHaveBeenCalled();
        expect(showToast).toHaveBeenCalledWith('Data has been updated');

        transactionsStore.loadMonthlyAllTransactions.mockRejectedValueOnce({ processed: false, message: 'refresh failed' });
        const failedDone = jest.fn();
        bindings.reload(failedDone);
        await flush();
        expect(bindings.loading.value).toBe(false);
        expect(failedDone).toHaveBeenCalled();
        expect(showToast).toHaveBeenCalledWith('refresh failed');

        transactionsStore.loadMonthlyAllTransactions.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.reload();
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled');
    });

    test('retains an unprocessed initial-load error for page recovery', async () => {
        transactionsStore.loadTransactions.mockRejectedValueOnce({ processed: false, message: 'network down' });
        const { bindings, router } = setup();
        await flush();
        expect(bindings.loadingError.value).toEqual({ processed: false, message: 'network down' });
        expect(showToast).toHaveBeenCalledWith('network down');
        bindings.onPageAfterIn();
        expect(routeBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);

        transactionsStore.loadTransactions.mockRejectedValueOnce('plain failure');
        setup();
        await flush();
        expect(showToast).toHaveBeenCalledWith('plain failure');
    });

    test('loads more only when eligible and handles pagination errors', async () => {
        transactionsStore.hasMoreTransaction = false;
        const { bindings } = setup();
        await flush();
        transactionsStore.loadTransactions.mockClear();
        bindings.loadMore(true);
        expect(transactionsStore.loadTransactions).not.toHaveBeenCalled();

        transactionsStore.hasMoreTransaction = true;
        const eligible = setup().bindings;
        await flush();
        transactionsStore.loadTransactions.mockClear();
        eligible.loading.value = true;
        eligible.loadMore(true);
        eligible.loading.value = false;
        eligible.loadingMore.value = true;
        eligible.loadMore(true);
        expect(transactionsStore.loadTransactions).not.toHaveBeenCalled();

        eligible.loadingMore.value = false;
        eligible.loadMore(false);
        await flush();
        expect(transactionsStore.loadTransactions).toHaveBeenCalledWith({ reload: false, autoExpand: false, defaultCurrency: 'CNY' });
        expect(eligible.loadingMore.value).toBe(false);
        expect(setMonthHeights).toHaveBeenCalledWith(false);

        transactionsStore.loadTransactions.mockRejectedValueOnce({ processed: false, message: 'more failed' });
        eligible.loadMore(true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('more failed');
        transactionsStore.loadTransactions.mockRejectedValueOnce({ processed: true, message: 'handled-more' });
        eligible.loadMore(true);
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled-more');
        transactionsStore.loadTransactions.mockRejectedValueOnce('plain-more-failure');
        eligible.loadMore(true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('plain-more-failure');
    });

    test('changes page type and date filters across list, calendar, custom, and billing-cycle paths', async () => {
        const { bindings } = setup();
        await flush();
        transactionsStore.updateTransactionListFilter.mockClear();

        bindings.changePageType(pageTypes.Calendar.type);
        await flush();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenCalledWith({ dateType: 44, minTime: 5000, maxTime: 6000 });

        bindings.query.value.minTime = 0;
        bindings.query.value.maxTime = 0;
        bindings.changeDateFilter(dateRanges.Custom.type);
        expect(bindings.customMaxDatetime.value).toBe(9000);
        expect(bindings.customMinDatetime.value).toBe(8000);
        expect(bindings.showCustomMonthSheet.value).toBe(true);
        expect(bindings.showDatePopover.value).toBe(false);

        bindings.pageType.value = pageTypes.List.type;
        bindings.query.value.minTime = 100;
        bindings.query.value.maxTime = 200;
        bindings.changeDateFilter(dateRanges.Custom.type);
        expect(bindings.customMinDatetime.value).toBe(100);
        expect(bindings.customMaxDatetime.value).toBe(200);
        expect(bindings.showCustomDateRangeSheet.value).toBe(true);

        bindings.query.value.dateType = 77;
        bindings.changeDateFilter(77);
        expect(getDateRangeByBillingCycleDateType).not.toHaveBeenCalled();
        bindings.query.value.dateType = 0;
        bindings.changeDateFilter(77);
        await flush();
        expect(getDateRangeByBillingCycleDateType).toHaveBeenCalled();

        getDateRangeByDateType.mockReturnValueOnce(null);
        bindings.changeDateFilter(12);
        expect(transactionsStore.updateTransactionListFilter).not.toHaveBeenCalledWith(expect.objectContaining({ dateType: 12 }));

        bindings.pageType.value = pageTypes.Calendar.type;
        getDateRangeByDateType.mockReturnValueOnce({ dateType: 12, minTime: 111, maxTime: 222 });
        bindings.changeDateFilter(12);
        await flush();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ dateType: 44, minTime: 5000, maxTime: 6000 });
    });

    test('applies custom ranges, custom months, and shifted ranges', async () => {
        const { bindings } = setup();
        await flush();
        transactionsStore.updateTransactionListFilter.mockClear();
        bindings.changeCustomDateFilter(0, 200);
        expect(transactionsStore.updateTransactionListFilter).not.toHaveBeenCalled();

        getDateTypeByBillingCycleDateRange.mockReturnValueOnce(77);
        bindings.changeCustomDateFilter(100, 200);
        await flush();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenCalledWith({ dateType: 77, minTime: 100, maxTime: 200 });

        bindings.pageType.value = pageTypes.Calendar.type;
        getDateTypeByBillingCycleDateRange.mockReturnValueOnce(null);
        bindings.changeCustomDateFilter(300, 400);
        await flush();
        expect(getDateTypeByDateRange).toHaveBeenCalled();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ dateType: 44, minTime: 5000, maxTime: 6000 });

        bindings.changeCustomMonthDateFilter(null);
        bindings.changeCustomMonthDateFilter({ year: 2026, month: 6 });
        await flush();
        expect(getYearMonthFirstUnixTime).toHaveBeenCalled();
        expect(getYearMonthLastUnixTime).toHaveBeenCalled();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ dateType: 55, minTime: 10_000, maxTime: 20_000 });

        bindings.query.value.dateType = dateRanges.All.type;
        bindings.shiftDateRange(100, 200, 1);
        expect(getShiftedDateRange).not.toHaveBeenCalled();
        bindings.query.value.dateType = 77;
        bindings.shiftDateRange(100, 200, -1);
        await flush();
        expect(getShiftedBillingRange).toHaveBeenCalled();
        expect(getShiftedDateRange).toHaveBeenCalled();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ dateType: 44, minTime: 5000, maxTime: 6000 });
    });

    test('covers valid no-op date decisions without unnecessary reloads', async () => {
        getDateRangeByDateType.mockReturnValueOnce(null);
        const customRoute = setup({ dateType: '99', minTime: '111', maxTime: '222' });
        await flush();
        expect(transactionsStore.initTransactionListFilter).toHaveBeenCalledWith(expect.objectContaining({
            dateType: 99, minTime: 111, maxTime: 222
        }));

        const { bindings } = customRoute;
        transactionsStore.loadTransactions.mockClear();
        transactionsStore.updateTransactionListFilter.mockReturnValue(false);
        bindings.changePageType(pageTypes.List.type);
        getFullMonthDateRange.mockReturnValueOnce(null);
        bindings.changePageType(pageTypes.Calendar.type);
        getFullMonthDateRange.mockReturnValueOnce({ dateType: 44, minTime: 5000, maxTime: 6000 });
        bindings.changePageType(pageTypes.Calendar.type);

        getDateRangeByDateType.mockReturnValueOnce({ dateType: 12, minTime: 300, maxTime: 400 });
        getFullMonthDateRange.mockReturnValueOnce(null);
        bindings.changeDateFilter(12);
        getDateTypeByBillingCycleDateRange.mockReturnValueOnce(null);
        getFullMonthDateRange.mockReturnValueOnce(null);
        bindings.changeCustomDateFilter(300, 400);

        bindings.pageType.value = pageTypes.List.type;
        bindings.changeCustomMonthDateFilter({ year: 2026, month: 6 });
        bindings.query.value.dateType = 12;
        bindings.shiftDateRange(300, 400, 1);
        getShiftedBillingRange.mockReturnValueOnce({ dateType: 77, minTime: 700, maxTime: 800 });
        bindings.query.value.dateType = dateRanges.Custom.type;
        bindings.shiftDateRange(300, 400, -1);

        bindings.pageType.value = pageTypes.Calendar.type;
        bindings.query.value.dateType = 12;
        getFullMonthDateRange.mockReturnValueOnce(null);
        bindings.shiftDateRange(300, 400, 1);
        await flush();
        expect(transactionsStore.loadTransactions).not.toHaveBeenCalled();
    });

    test('updates type/category/account/tag/keyword filters and preserves category compatibility', async () => {
        const { bindings, router } = setup();
        await flush();
        bindings.query.value.type = transactionTypes.Expense;
        bindings.query.value.categoryIds = 'expense,income,missing';
        bindings.queryAllFilterCategoryIds.value = { expense: true, income: true, missing: true };
        bindings.changeTypeFilter(transactionTypes.Income);
        await flush();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenCalledWith({ type: transactionTypes.Income, categoryIds: 'income' });
        bindings.query.value.type = transactionTypes.Expense;
        bindings.query.value.flowDirection = 'inflow';
        bindings.changeTypeFilter(transactionTypes.Income);
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({
            type: transactionTypes.Income,
            categoryIds: 'income',
            flowDirection: ''
        });

        bindings.changeCategoryFilter('food');
        bindings.changeCategoryFilter('food');
        bindings.changeAccountFilter('wallet');
        bindings.changeAccountFilter('wallet');
        bindings.changeTagFilter('daily');
        bindings.changeTagFilter('daily');
        bindings.changeTagFilterType(2);
        bindings.changeTagFilterType(2);
        bindings.changeKeywordFilter('coffee');
        bindings.changeKeywordFilter('coffee');
        await flush();
        expect(bindings.query.value).toMatchObject({
            categoryIds: 'food', accountIds: 'wallet', tagIds: 'daily', tagFilterType: 2, keyword: 'coffee'
        });

        bindings.filterMultipleCategories();
        expect(router.navigate).toHaveBeenLastCalledWith('/settings/filter/category?type=transactionListCurrent&allowCategoryTypes=12');
        bindings.query.value.type = transactionTypes.Investment;
        bindings.filterMultipleCategories();
        expect(router.navigate).toHaveBeenLastCalledWith('/settings/filter/category?type=transactionListCurrent');
        bindings.filterMultipleAccounts();
        bindings.filterMultipleTags();
        expect(router.navigate).toHaveBeenCalledWith('/settings/filter/account?type=transactionListCurrent');
        expect(router.navigate).toHaveBeenCalledWith('/settings/filter/tag?type=transactionListCurrent');
    });

    test('keeps changed filters local when the store reports a semantic no-op', async () => {
        const { bindings } = setup();
        await flush();
        transactionsStore.loadTransactions.mockClear();
        transactionsStore.updateTransactionListFilter.mockReturnValue(false);
        bindings.query.value.type = transactionTypes.Income;
        bindings.query.value.categoryIds = 'expense,expense-two';
        bindings.allCategories.value['expense-two'] = { id: 'expense-two', type: 13 };
        bindings.queryAllFilterCategoryIds.value = { expense: true, 'expense-two': true };
        bindings.changeTypeFilter(transactionTypes.Expense);
        bindings.changeTypeFilter(0);
        bindings.changeCategoryFilter('food');
        bindings.changeAccountFilter('wallet');
        bindings.changeTagFilter('daily');
        bindings.changeTagFilterType(2);
        bindings.changeKeywordFilter('coffee');
        bindings.query.value.amountFilterCents = 'between:12345:67890';
        bindings.changeAmountFilter('');
        await flush();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenCalledWith({
            type: transactionTypes.Expense,
            categoryIds: 'expense,expense-two'
        });
        expect(transactionsStore.loadTransactions).not.toHaveBeenCalled();
    });

    test('routes cents-based amount filters without converting units and clears them in place', async () => {
        const { bindings, router } = setup();
        await flush();
        bindings.query.value.amountFilterCents = 'between:12345:67890';
        bindings.changeAmountFilter('between');
        expect(router.navigate).toHaveBeenCalledWith('/transaction/filter/amount?type=between&value=between:12345:67890');

        bindings.changeAmountFilter('between:12345:67890');
        expect(router.navigate).toHaveBeenCalledTimes(1);
        bindings.changeAmountFilter('');
        await flush();
        expect(transactionsStore.updateTransactionListFilter).toHaveBeenCalledWith({ amountFilterCents: '' });
        expect(bindings.query.value.amountFilterCents).toBe('');
    });

    test('builds add, duplicate, and edit navigation with the active filter context', async () => {
        const { bindings, router } = setup();
        await flush();
        bindings.query.value = {
            ...bindings.query.value,
            minTime: 200,
            maxTime: 300,
            type: transactionTypes.Expense,
            categoryIds: 'food',
            accountIds: 'wallet',
            tagIds: 'daily'
        };
        bindings.queryAllFilterCategoryIds.value = { food: true };
        bindings.queryAllFilterAccountIds.value = { wallet: true };
        getCurrentUnixTime.mockReturnValueOnce(100);
        bindings.add();
        expect(router.navigate).toHaveBeenLastCalledWith('/transaction/add?time=200&type=3&categoryId=food&accountId=wallet&tagIds=daily');

        getCurrentUnixTime.mockReturnValueOnce(400);
        bindings.add();
        expect(router.navigate).toHaveBeenLastCalledWith('/transaction/add?time=300&type=3&categoryId=food&accountId=wallet&tagIds=daily');

        bindings.query.value.minTime = 100;
        bindings.query.value.maxTime = 300;
        bindings.query.value.type = transactionTypes.ModifyBalance;
        bindings.query.value.categoryIds = '';
        bindings.query.value.accountIds = '';
        bindings.query.value.tagIds = '';
        bindings.queryAllFilterCategoryIds.value = {};
        bindings.queryAllFilterAccountIds.value = {};
        getCurrentUnixTime.mockReturnValueOnce(200);
        bindings.add();
        expect(router.navigate).toHaveBeenLastCalledWith('/transaction/add?');
        bindings.query.value.minTime = 0;
        bindings.query.value.maxTime = 0;
        bindings.add();
        expect(router.navigate).toHaveBeenLastCalledWith('/transaction/add?');

        bindings.duplicate({ id: 'transaction-1', type: transactionTypes.Transfer });
        bindings.edit({ id: 'transaction-2', type: transactionTypes.Investment });
        expect(router.navigate).toHaveBeenCalledWith('/transaction/add?id=transaction-1&type=4');
        expect(router.navigate).toHaveBeenCalledWith('/transaction/edit?id=transaction-2&type=5');

        router.navigate.mockClear();
        lastBase.canAddTransaction.value = false;
        bindings.showAccountPopover.value = false;
        bindings.add();
        expect(showToast).toHaveBeenLastCalledWith('Select Account');
        expect(bindings.showAccountPopover.value).toBe(true);
        expect(router.navigate).not.toHaveBeenCalled();
    });

    test('confirms deletion, waits for swipeout removal, and handles delete errors', async () => {
        const { bindings } = setup();
        await flush();
        bindings.remove(null, false);
        expect(showAlert).toHaveBeenCalledWith('An error occurred');

        const transaction = { id: 'transaction-1', type: transactionTypes.Expense };
        bindings.remove(transaction, false);
        expect(bindings.transactionToDelete.value).toStrictEqual(transaction);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        transactionsStore.deleteTransaction.mockImplementationOnce(async options => {
            options.beforeResolve((doneValue?: unknown) => doneValue);
        });
        bindings.remove(transaction, true);
        await flush();
        expect(showLoading).toHaveBeenCalled();
        expect(onSwipeoutDeleted).toHaveBeenCalledWith('transaction-transaction-1', expect.any(Function));
        expect(hideLoading).toHaveBeenCalled();

        transactionsStore.deleteTransaction.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(transaction, true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('delete failed');
        transactionsStore.deleteTransaction.mockRejectedValueOnce({ processed: true, message: 'handled-delete' });
        bindings.remove(transaction, true);
        await flush();
        expect(showToast).not.toHaveBeenCalledWith('handled-delete');
        transactionsStore.deleteTransaction.mockRejectedValueOnce('plain-delete-failure');
        bindings.remove(transaction, true);
        await flush();
        expect(showToast).toHaveBeenCalledWith('plain-delete-failure');
    });

    test('maintains collapsed-month visibility and page lifecycle callbacks', async () => {
        const { bindings, router } = setup();
        await flush();
        const month = { yearDashMonth: '2026-07' };
        bindings.transactionInvisibleYearMonths.value['2026-07'] = true;
        bindings.collapseTransactionMonthList(month, false);
        expect(transactionsStore.collapseMonthInTransactionList).toHaveBeenCalledWith({ monthList: month, collapse: false });
        expect(bindings.transactionInvisibleYearMonths.value['2026-07']).toBeUndefined();
        bindings.collapseTransactionMonthList(month, true);

        bindings.onPopoverOpen({ $el: { id: 'popover' } });
        expect(scrollToSelectedItem).toHaveBeenCalledWith({ id: 'popover' }, '.popover-inner', 'li.list-item-selected');
        transactionsStore.transactionListStateInvalid = true;
        bindings.onPageAfterIn();
        await flush();
        expect(routeBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);

        bindings.onResize();
        bindings.onScroll();
        bindings.onTransactionMonthListCollapseStateChanged();
        await flush();
        expect(setMonthHeights).toHaveBeenCalledWith(true);
        expect(setMonthHeights).toHaveBeenCalledWith(false);
        expect(setInvisibleMonths).toHaveBeenCalled();

        mountedCallbacks.forEach(callback => callback());
        expect(infiniteCallbacks).toHaveLength(1);
        infiniteCallbacks[0]?.();
        unmountedCallbacks.forEach(callback => callback());
    });

    test('reloads only when the watched calendar day actually changes', async () => {
        const { bindings } = setup();
        await flush();
        transactionsStore.loadTransactions.mockClear();
        bindings.pageType.value = pageTypes.List.type;
        watchCallbacks[0]?.('2026-07-16', '2026-07-15');
        expect(transactionsStore.loadTransactions).not.toHaveBeenCalled();
        bindings.pageType.value = pageTypes.Calendar.type;
        watchCallbacks[0]?.('2026-07-15', '2026-07-15');
        expect(transactionsStore.loadTransactions).not.toHaveBeenCalled();
        watchCallbacks[0]?.('2026-07-16', '2026-07-15');
        await flush();
        expect(logger.debug).toHaveBeenCalledWith('[ListPage] Calendar date changed, reloading transactions', { date: '2026-07-16' });
        expect(transactionsStore.loadTransactions).toHaveBeenCalled();
    });

    test('renders the production SFC facade after exercising external-template bindings', async () => {
        const { bindings } = setup();
        await flush();
        bindings.query.value.categoryIds = 'food';
        bindings.query.value.accountIds = 'wallet';
        bindings.query.value.tagIds = 'daily';
        bindings.query.value.amountFilterCents = 'between:12345:67890';
        bindings.queryAllFilterCategoryIds.value = { food: true };
        bindings.queryAllFilterAccountIds.value = { wallet: true };
        bindings.queryAllFilterTagIds.value = { daily: true };
        transactionsStore.transactions = [{
            year: 2026,
            month: 6,
            yearDashMonth: '2026-07',
            opened: true,
            items: [{ id: 'transaction-1', type: transactionTypes.Expense }],
            totalAmountCents: { incomeCents: 0, expenseCents: -12_345 },
            dailyTotalAmountsCents: {}
        }];

        const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
        const rendered = render(bindings);
        collectRenderCallbacks(rendered, callbacks);
        expect(rendered).toBeDefined();

        for (const { name, callback } of callbacks) {
            if (name === 'onChange') callback({ target: { value: 'render-search' } });
            else if (name === 'onPopover:open' || name === 'onPopoverOpen') callback({ $el: {} });
            else if (name === 'onDateRange:change' || name === 'onDateRangeChange') callback(100, 200);
            else if (name === 'onUpdate:modelValue') callback({ year: 2026, month: 6 });
            else if (name === 'onCollapseMonth') callback(transactionsStore.transactions[0], false);
            else if (name === 'onDuplicate' || name === 'onEdit') callback({ id: 'render-transaction', type: transactionTypes.Expense });
            else if (name === 'onRemove') callback({ id: 'render-transaction', type: transactionTypes.Expense }, false);
            else if (name.startsWith('onUpdate:')) callback(false);
            else if (name === 'onPtr:refresh' || name === 'onPtrRefresh') callback(jest.fn());
            else callback();
        }
        await flush(12);
    });

    test('exposes enabled accessible names for add and account-recovery actions', async () => {
        const { bindings, router } = setup();
        await flush();
        lastBase.canAddTransaction.value = false;
        bindings.add();
        expect(bindings.showAccountPopover.value).toBe(true);
        expect(router.navigate).not.toHaveBeenCalled();

        const { readFileSync } = jest.requireActual('node:fs') as typeof import('node:fs');
        const template = readFileSync('src/views/mobile/transactions/list-page/ListPage.template.html', 'utf8');
        expect(template).toContain(":aria-label=\"tt(canAddTransaction ? 'Add' : 'Select Account')\"");
        expect(template).not.toContain(':aria-disabled="!canAddTransaction"');
    });
});
