import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;

const mockGetAllDateRanges = jest.fn((...args: any[]) => [{
    dateType: 101,
    minTime: 1_000,
    maxTime: 2_000,
    args,
}]);
const mockFormatAmount = jest.fn((amountCents: number, currency: string | false) => `${String(currency)}:${amountCents}`);
const mockNumeralSystem = {
    replaceWesternArabicDigitsToLocalizedDigits: jest.fn((value: string) => `localized(${value})`),
};
const mockCurrentDateTime = {
    toGregorianCalendarYear0BasedMonth: jest.fn(() => ({ year: 2030, month0base: 8 })),
};
const mockParsedDateTime = {
    toGregorianCalendarYear0BasedMonth: jest.fn(() => ({ year: 2026, month0base: 6 })),
    getGregorianCalendarYear: jest.fn(() => 2026),
    getGregorianCalendarMonth: jest.fn(() => 7),
};
const mockGetTimezoneOffsetMinutes = jest.fn((timezone?: string) => timezone === 'Asia/Shanghai' ? 480 : 60);
const mockGetBrowserTimezoneOffsetMinutes = jest.fn(() => -120);
const mockGetDummyUnixTimeForLocalUsage = jest.fn((unixTime: number, timezoneOffset: number, browserOffset: number) => (
    unixTime + timezoneOffset - browserOffset
));
const mockGetLocalDatetimeFromUnixTime = jest.fn((unixTime: number) => new Date(unixTime * 1_000));
const mockParseDateTimeFromUnixTime = jest.fn((_unixTime?: number) => mockParsedDateTime);
const mockUnifiedCurrency = jest.fn((
    _accountsMap: Record<string, any>,
    selectedIds: Record<string, boolean>,
    defaultCurrency: string,
) => selectedIds['usd'] ? 'USD' : defaultCurrency);

const mockSettingsStore: any = actualVue.reactive({
    appSettings: {
        timeZone: 'Asia/Shanghai',
        showTotalAmountInTransactionListPage: true,
        showTagInTransactionListPage: false,
    },
});
const mockUserStore: any = actualVue.reactive({
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: 0x0401,
    currentUserDefaultCurrency: 'CNY',
});
const mockAccountsStore: any = actualVue.reactive({
    allMixedPlainAccounts: [] as any[],
    allAccountsMap: {} as Record<string, any>,
    allAvailableAccountsCount: 0,
    getAccountStatementDate: jest.fn((accountIds: string) => accountIds === 'credit' ? 15 : 0),
});
const mockCategoryStore: any = actualVue.reactive({
    allTransactionCategories: {} as Record<number, any[] | undefined>,
    allTransactionCategoriesMap: {} as Record<string, any>,
});
const mockTagStore: any = actualVue.reactive({
    allTransactionTagsMap: {} as Record<string, any>,
    allAvailableTagsCount: 0,
});
const mockTransactionsStore: any = actualVue.reactive({
    transactionsFilter: {} as Record<string, any>,
    allFilterCategoryIds: {} as Record<string, boolean>,
    allFilterAccountIds: {} as Record<string, boolean>,
    allFilterTagIds: {} as Record<string, boolean>,
    allFilterCategoryIdsCount: 0,
    allFilterAccountIdsCount: 0,
    allFilterTagIdsCount: 0,
    transactions: [] as any[],
    updateTransactionListFilter: jest.fn((patch: Record<string, unknown>) => {
        Object.assign(mockTransactionsStore.transactionsFilter, patch);
    }),
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => key,
        getAllDateRanges: mockGetAllDateRanges,
        getCurrentNumeralSystemType: () => mockNumeralSystem,
        formatUnixTimeToLongDateTime: (unixTime: number) => `long-datetime:${unixTime}`,
        formatUnixTimeToLongDate: (unixTime: number, utcOffset: number, currentOffset: number) => (
            `long-date:${unixTime}:${utcOffset}:${currentOffset}`
        ),
        formatUnixTimeToShortTime: (unixTime: number, utcOffset: number, currentOffset: number) => (
            `short-time:${unixTime}:${utcOffset}:${currentOffset}`
        ),
        formatUnixTimeToGregorianLikeLongYearMonth: (unixTime: number) => `year-month:${unixTime}`,
        formatDateRange: (dateType: number, minTime: number, maxTime: number) => (
            `range:${dateType}:${minTime}:${maxTime}`
        ),
        formatAmountToLocalizedNumeralsWithCurrency: mockFormatAmount,
    }),
}));

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/stores/transaction.ts', () => ({ useTransactionsStore: () => mockTransactionsStore }));

jest.mock('@/lib/datetime.ts', () => {
    const actual = jest.requireActual('@/lib/datetime.ts') as any;
    return {
        ...actual,
        getUtcOffsetByUtcOffsetMinutes: (minutes: number) => minutes === 480 ? '+08:00' : '+00:00',
        getTimezoneOffset: (timezone: string) => timezone === 'Asia/Shanghai' ? '+08:00' : '+00:00',
        getTimezoneOffsetMinutes: mockGetTimezoneOffsetMinutes,
        getBrowserTimezoneOffsetMinutes: mockGetBrowserTimezoneOffsetMinutes,
        getLocalDatetimeFromUnixTime: mockGetLocalDatetimeFromUnixTime,
        getDummyUnixTimeForLocalUsage: mockGetDummyUnixTimeForLocalUsage,
        getCurrentDateTime: () => mockCurrentDateTime,
        parseDateTimeFromUnixTime: mockParseDateTimeFromUnixTime,
        getYearMonthFirstUnixTime: (yearDashMonth: string) => Number(yearDashMonth.replace('-', '')),
        isDateRangeMatchOneMonth: (minTime: number, maxTime: number) => minTime === 1_000 && maxTime === 2_000,
    };
});

jest.mock('@/lib/account.ts', () => ({
    getUnifiedSelectedAccountsCurrencyOrDefaultCurrency: mockUnifiedCurrency,
}));

const {
    TransactionListPageType,
    useTransactionListPageBase,
} = require('@/views/base/transactions/TransactionListPageBase.ts') as any;
const { AccountType } = require('@/core/account.ts') as any;
const { CategoryType } = require('@/core/category.ts') as any;
const { DateRange, DateRangeScene } = require('@/core/datetime.ts') as any;
const { TransactionType } = require('@/core/transaction.ts') as any;
const { DISPLAY_HIDDEN_AMOUNT, INCOMPLETE_AMOUNT_SUFFIX } = require('@/consts/numeral.ts') as any;

function createAccount(
    id: string,
    name: string,
    currency = 'CNY',
    type = AccountType.SingleAccount.type,
    parentId = '',
): any {
    return { id, name, currency, type, parentId };
}

function createTransaction(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'tx-1',
        type: TransactionType.Transfer,
        time: 1_700_000_000,
        utcOffset: 480,
        sourceAccountId: 'cash',
        destinationAccountId: 'usd',
        sourceAmountCents: 12_345,
        destinationAmountCents: 67_890,
        hideAmount: false,
        sourceAccount: createAccount('cash', 'Cash', 'CNY'),
        destinationAccount: createAccount('usd', 'USD Wallet', 'USD'),
        ...overrides,
    };
}

function setFilter(overrides: Record<string, unknown> = {}): void {
    mockTransactionsStore.transactionsFilter = {
        dateType: DateRange.All.type,
        minTime: 0,
        maxTime: 0,
        type: 0,
        categoryIds: '',
        accountIds: '',
        tagIds: '',
        tagFilterType: 0,
        amountFilterCents: '',
        keyword: '',
        ...overrides,
    };
}

function setupBase(): any {
    return useTransactionListPageBase();
}

beforeEach(() => {
    jest.clearAllMocks();
    mockSettingsStore.appSettings.timeZone = 'Asia/Shanghai';
    mockSettingsStore.appSettings.showTotalAmountInTransactionListPage = true;
    mockSettingsStore.appSettings.showTagInTransactionListPage = false;
    mockUserStore.currentUserFirstDayOfWeek = 1;
    mockUserStore.currentUserFiscalYearStart = 0x0401;
    mockUserStore.currentUserDefaultCurrency = 'CNY';

    const cash = createAccount('cash', 'Cash');
    const usd = createAccount('usd', 'USD Wallet', 'USD');
    const parent = createAccount('parent', 'Grouped', 'CNY', AccountType.MultiSubAccounts.type);
    const child = createAccount('child', 'Child', 'CNY', AccountType.SingleAccount.type, 'parent');
    const credit = createAccount('credit', 'Credit', 'CNY');
    mockAccountsStore.allMixedPlainAccounts = [cash, usd, parent, child, credit];
    mockAccountsStore.allAccountsMap = { cash, usd, parent, child, credit };
    mockAccountsStore.allAvailableAccountsCount = 5;
    mockAccountsStore.getAccountStatementDate.mockImplementation((accountIds: string) => accountIds === 'credit' ? 15 : 0);

    const salary = { id: 'salary', name: 'Salary', type: CategoryType.Income };
    const food = { id: 'food', name: 'Food', type: CategoryType.Expense };
    const transfer = { id: 'transfer', name: 'Transfer', type: CategoryType.Transfer };
    mockCategoryStore.allTransactionCategories = {
        [CategoryType.Income]: [salary],
        [CategoryType.Expense]: [food],
        [CategoryType.Transfer]: [transfer],
        [CategoryType.Investment]: undefined,
    };
    mockCategoryStore.allTransactionCategoriesMap = { salary, food, transfer };
    mockTagStore.allTransactionTagsMap = {
        daily: { id: 'daily', name: 'Daily' },
        work: { id: 'work', name: 'Work' },
    };
    mockTagStore.allAvailableTagsCount = 2;
    setFilter();
    mockTransactionsStore.allFilterCategoryIds = {};
    mockTransactionsStore.allFilterAccountIds = {};
    mockTransactionsStore.allFilterTagIds = {};
    mockTransactionsStore.allFilterCategoryIdsCount = 0;
    mockTransactionsStore.allFilterAccountIdsCount = 0;
    mockTransactionsStore.allFilterTagIdsCount = 0;
    mockTransactionsStore.transactions = [];
    mockTransactionsStore.updateTransactionListFilter.mockImplementation((patch: Record<string, unknown>) => {
        Object.assign(mockTransactionsStore.transactionsFilter, patch);
    });
});

describe('TransactionListPageBase production-loaded static and store projections', () => {
    test('keeps page-type registry and initial state stable', () => {
        const bindings = setupBase();
        expect(TransactionListPageType.values()).toStrictEqual([
            TransactionListPageType.List,
            TransactionListPageType.Calendar,
        ]);
        expect(TransactionListPageType.valueOf(0)).toBe(TransactionListPageType.List);
        expect(TransactionListPageType.valueOf(1)).toBe(TransactionListPageType.Calendar);
        expect(TransactionListPageType.valueOf(99)).toBeUndefined();
        expect(TransactionListPageType.Default).toBe(TransactionListPageType.List);
        expect(bindings.pageType.value).toBe(0);
        expect(bindings.loading.value).toBe(true);
        expect(bindings.customMinDatetime.value).toBe(0);
        expect(bindings.customMaxDatetime.value).toBe(0);
        expect(bindings.currentCalendarDate.value).toBe('');
        expect(bindings.displayPageTypeName.value).toBe('Transaction List');
        bindings.pageType.value = TransactionListPageType.Calendar.type;
        expect(bindings.displayPageTypeName.value).toBe('Transaction Calendar');
        bindings.pageType.value = 99;
        expect(bindings.displayPageTypeName.value).toBe('Transaction List');
    });

    test('projects settings, accounts, categories, tags, and selected-account currency', () => {
        const bindings = setupBase();
        expect(bindings.currentTimezoneOffsetMinutes.value).toBe(480);
        expect(bindings.firstDayOfWeek.value).toBe(1);
        expect(bindings.fiscalYearStart.value).toBe(0x0401);
        expect(bindings.showTotalAmountInTransactionListPage.value).toBe(true);
        expect(bindings.showTagInTransactionListPage.value).toBe(false);
        expect(bindings.allAccounts.value).toHaveLength(5);
        expect(bindings.allAccountsMap.value.cash.name).toBe('Cash');
        expect(bindings.allAvailableAccountsCount.value).toBe(5);
        expect(bindings.allCategories.value.food.name).toBe('Food');
        expect(bindings.allTransactionTags.value.daily.name).toBe('Daily');
        expect(bindings.allAvailableTagsCount.value).toBe(2);
        expect(bindings.defaultCurrency.value).toBe('CNY');
        mockTransactionsStore.allFilterAccountIds = { usd: true };
        expect(bindings.defaultCurrency.value).toBe('USD');
        expect(mockUnifiedCurrency).toHaveBeenLastCalledWith(
            mockAccountsStore.allAccountsMap,
            { usd: true },
            'CNY',
        );
    });

    test('filters primary categories and counts only the active transaction type', () => {
        const bindings = setupBase();
        expect(Object.keys(bindings.allPrimaryCategories.value)).toStrictEqual(['2', '3', '4', '5']);
        expect(bindings.allAvailableCategoriesCount.value).toBe(3);

        setFilter({ type: TransactionType.Expense });
        expect(Object.keys(bindings.allPrimaryCategories.value)).toStrictEqual(['3']);
        expect(bindings.allAvailableCategoriesCount.value).toBe(1);
        setFilter({ type: TransactionType.Investment });
        expect(bindings.allPrimaryCategories.value).toStrictEqual({ '5': undefined });
        expect(bindings.allAvailableCategoriesCount.value).toBe(0);
    });

    test('requests date presets with billing-cycle awareness', () => {
        const bindings = setupBase();
        expect(bindings.allDateRanges.value).toHaveLength(1);
        expect(mockGetAllDateRanges).toHaveBeenLastCalledWith(DateRangeScene.Normal, true, false);
        setFilter({ accountIds: 'credit' });
        expect(bindings.allDateRanges.value).toHaveLength(1);
        expect(mockGetAllDateRanges).toHaveBeenLastCalledWith(DateRangeScene.Normal, true, true);
    });
});

describe('TransactionListPageBase production-loaded filter labels and calendar state', () => {
    test('formats date labels, raw times, monthly state, and month fallback', () => {
        const bindings = setupBase();
        expect(bindings.queryDateRangeName.value).toBe('Date');
        expect(bindings.queryMinTime.value).toBe('long-datetime:0');
        expect(bindings.queryMaxTime.value).toBe('long-datetime:0');
        expect(bindings.queryMonthlyData.value).toBe(false);
        expect(bindings.queryMonth.value).toStrictEqual({ year: 2030, month0base: 8 });

        setFilter({ dateType: 9, minTime: 1_000, maxTime: 2_000 });
        expect(bindings.queryDateRangeName.value).toBe('range:9:1000:2000');
        expect(bindings.queryMinTime.value).toBe('long-datetime:1000');
        expect(bindings.queryMaxTime.value).toBe('long-datetime:2000');
        expect(bindings.queryMonthlyData.value).toBe(true);
        expect(bindings.queryMonth.value).toStrictEqual({ year: 2026, month0base: 6 });
        expect(mockParseDateTimeFromUnixTime).toHaveBeenCalledWith(1_000);

        setFilter({ dateType: 9, minTime: 1_000, maxTime: 0 });
        expect(bindings.queryMonth.value).toStrictEqual({ year: 2030, month0base: 8 });
    });

    test('formats account, category, and tag filter labels for none, one, and many', () => {
        const bindings = setupBase();
        expect(bindings.queryAccountName.value).toBe('Account');
        expect(bindings.queryCategoryName.value).toBe('Category');
        expect(bindings.queryTagName.value).toBe('Tags');

        setFilter({ accountIds: 'cash', categoryIds: 'food', tagIds: 'daily' });
        mockTransactionsStore.allFilterAccountIdsCount = 1;
        mockTransactionsStore.allFilterCategoryIdsCount = 1;
        mockTransactionsStore.allFilterTagIdsCount = 1;
        expect(bindings.queryAccountName.value).toBe('Cash');
        expect(bindings.queryCategoryName.value).toBe('Food');
        expect(bindings.queryTagName.value).toBe('Daily');

        mockTransactionsStore.allFilterAccountIdsCount = 2;
        mockTransactionsStore.allFilterCategoryIdsCount = 2;
        mockTransactionsStore.allFilterTagIdsCount = 2;
        expect(bindings.queryAccountName.value).toBe('Multiple Accounts');
        expect(bindings.queryCategoryName.value).toBe('Multiple Categories');
        expect(bindings.queryTagName.value).toBe('Multiple Tags');

        setFilter({ tagIds: 'none' });
        expect(bindings.queryTagName.value).toBe('Without Tags');
    });

    test('keeps amount-filter values in cents from query text through localized display', () => {
        const bindings = setupBase();
        expect(bindings.queryAmount.value).toBe('');
        setFilter({ amountFilterCents: 'between' });
        expect(bindings.queryAmount.value).toBe('');

        setFilter({ amountFilterCents: 'equal:12345' });
        expect(bindings.queryAmount.value).toBe('false:12345');
        expect(mockFormatAmount).toHaveBeenLastCalledWith(12_345, false);

        setFilter({ amountFilterCents: 'between:-500:98765' });
        expect(bindings.queryAmount.value).toBe('false:-500 ~ false:98765');
        expect(mockFormatAmount).toHaveBeenCalledWith(-500, false);
        expect(mockFormatAmount).toHaveBeenCalledWith(98_765, false);
    });

    test('builds calendar dates with timezone offsets and finds the current month only', () => {
        const bindings = setupBase();
        setFilter({ minTime: 1_000, maxTime: 2_000 });
        expect(bindings.transactionCalendarMinDate.value).toStrictEqual(new Date(1_180_000));
        expect(bindings.transactionCalendarMaxDate.value).toStrictEqual(new Date(2_180_000));
        expect(mockGetDummyUnixTimeForLocalUsage).toHaveBeenCalledWith(1_000, 60, -120);

        expect(bindings.currentMonthTransactionData.value).toBeNull();
        mockTransactionsStore.transactions = undefined;
        expect(bindings.currentMonthTransactionData.value).toBeNull();
        mockTransactionsStore.transactions = [
            { year: 2025, month: 12, yearDashMonth: '2025-12', items: [] },
            { year: 2026, month: 7, yearDashMonth: '2026-07', items: ['match'] },
        ];
        expect(bindings.currentMonthTransactionData.value.items).toStrictEqual(['match']);
        mockTransactionsStore.transactions = [{ year: 2024, month: 1, items: [] }];
        expect(bindings.currentMonthTransactionData.value).toBeNull();
    });

    test('blocks add only for one selected aggregate account', () => {
        const bindings = setupBase();
        expect(bindings.canAddTransaction.value).toBe(true);
        setFilter({ accountIds: 'parent' });
        mockTransactionsStore.allFilterAccountIdsCount = 1;
        expect(bindings.canAddTransaction.value).toBe(false);
        setFilter({ accountIds: 'cash' });
        expect(bindings.canAddTransaction.value).toBe(true);
        setFilter({ accountIds: 'missing' });
        expect(bindings.canAddTransaction.value).toBe(true);
        mockTransactionsStore.allFilterAccountIdsCount = 2;
        expect(bindings.canAddTransaction.value).toBe(true);
    });
});

describe('TransactionListPageBase production-loaded transaction formatting', () => {
    test('formats time, long date, year-month, timezone, and default-timezone values', () => {
        const bindings = setupBase();
        const transaction = createTransaction();
        expect(bindings.getDisplayTime(transaction)).toBe('short-time:1700000000:480:480');
        expect(bindings.getDisplayLongDate(transaction)).toBe('long-date:1700000000:480:480');
        expect(bindings.getDisplayLongYearMonth({ yearDashMonth: '2026-07' })).toBe('year-month:202607');
        expect(bindings.getDisplayTimezone(transaction)).toBe('UTClocalized(+08:00)');
        expect(bindings.getDisplayTimeInDefaultTimezone(transaction)).toBe('long-datetime:1700000000 (UTClocalized(+08:00))');
    });

    test('displays source cents without an account filter and respects hidden amounts', () => {
        const bindings = setupBase();
        const transaction = createTransaction();
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
        transaction.hideAmount = true;
        expect(bindings.getDisplayAmount(transaction)).toBe(`CNY:${DISPLAY_HIDDEN_AMOUNT}`);
        expect(mockFormatAmount).toHaveBeenCalledWith(12_345, 'CNY');
        expect(mockFormatAmount).toHaveBeenCalledWith(DISPLAY_HIDDEN_AMOUNT, 'CNY');

        transaction.sourceAccount = null;
        expect(bindings.getDisplayAmount(transaction)).toBe('');
    });

    test('selects source or destination cents for exactly one account id or parent id', () => {
        const bindings = setupBase();
        const transaction = createTransaction({
            sourceAccount: createAccount('child', 'Child', 'CNY', AccountType.SingleAccount.type, 'parent'),
            sourceAccountId: 'child',
        });
        mockTransactionsStore.allFilterAccountIdsCount = 1;
        mockTransactionsStore.allFilterAccountIds = { child: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
        mockTransactionsStore.allFilterAccountIds = { parent: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
        mockTransactionsStore.allFilterAccountIds = { usd: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('USD:67890');

        transaction.destinationAccount = createAccount('usd-child', 'USD Child', 'USD', AccountType.SingleAccount.type, 'usd-parent');
        mockTransactionsStore.allFilterAccountIds = { 'usd-parent': true };
        expect(bindings.getDisplayAmount(transaction)).toBe('USD:67890');
        mockTransactionsStore.allFilterAccountIds = { missing: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
    });

    test('selects the external side of multi-account transfers and falls back safely', () => {
        const bindings = setupBase();
        const transaction = createTransaction({
            sourceAccount: createAccount('cash-child', 'Cash Child', 'CNY', AccountType.SingleAccount.type, 'cash-parent'),
            destinationAccount: createAccount('usd-child', 'USD Child', 'USD', AccountType.SingleAccount.type, 'usd-parent'),
        });
        mockTransactionsStore.allFilterAccountIdsCount = 2;
        mockTransactionsStore.allFilterAccountIds = { 'cash-child': true, extra: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
        mockTransactionsStore.allFilterAccountIds = { 'cash-parent': true, extra: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
        mockTransactionsStore.allFilterAccountIds = { 'usd-child': true, extra: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('USD:67890');
        mockTransactionsStore.allFilterAccountIds = { 'usd-parent': true, extra: true };
        expect(bindings.getDisplayAmount(transaction)).toBe('USD:67890');
        mockTransactionsStore.allFilterAccountIds = { 'cash-child': true, 'usd-child': true };
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');

        transaction.destinationAccount = null;
        expect(bindings.getDisplayAmount(transaction)).toBe('CNY:12345');
        transaction.sourceAccount = null;
        expect(bindings.getDisplayAmount(transaction)).toBe('');
    });

    test('formats monthly cents totals and every transaction type label', () => {
        const bindings = setupBase();
        expect(bindings.getDisplayMonthTotalAmount(12_345, 'CNY', '+', false)).toBe('+CNY:12345');
        expect(bindings.getDisplayMonthTotalAmount(-500, false, '-', true)).toBe(`-false:-500${INCOMPLETE_AMOUNT_SUFFIX}`);
        expect(bindings.getTransactionTypeName(TransactionType.ModifyBalance, 'Unknown')).toBe('Modify Balance');
        expect(bindings.getTransactionTypeName(TransactionType.Income, 'Unknown')).toBe('Income');
        expect(bindings.getTransactionTypeName(TransactionType.Expense, 'Unknown')).toBe('Expense');
        expect(bindings.getTransactionTypeName(TransactionType.Transfer, 'Unknown')).toBe('Transfer');
        expect(bindings.getTransactionTypeName(TransactionType.Investment, 'Unknown')).toBe('Investment');
        expect(bindings.getTransactionTypeName(null, 'Unknown')).toBe('Unknown');
        expect(bindings.getTransactionTypeName(999, 'Fallback')).toBe('Fallback');
    });
});

describe('TransactionListPageBase production-loaded filter mutations', () => {
    test('changes page type only when needed and forwards every filter field', () => {
        const bindings = setupBase();
        bindings.changePageType(TransactionListPageType.List.type);
        expect(bindings.pageType.value).toBe(TransactionListPageType.List.type);
        bindings.changePageType(TransactionListPageType.Calendar.type);
        expect(bindings.pageType.value).toBe(TransactionListPageType.Calendar.type);

        bindings.changeDateFilter({ dateType: 7, minTime: 111, maxTime: 999 });
        expect(mockTransactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({
            dateType: 7,
            minTime: 111,
            maxTime: 999,
        });
        bindings.changeTypeFilter(TransactionType.Income);
        expect(mockTransactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ type: TransactionType.Income });
        bindings.changeAccountFilter('cash,usd');
        expect(mockTransactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ accountIds: 'cash,usd' });
        bindings.changeCategoryFilter('food,salary');
        expect(mockTransactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ categoryIds: 'food,salary' });
        bindings.changeTagFilter('daily,work');
        expect(mockTransactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({ tagIds: 'daily,work' });

        bindings.changeAmountFilter('between:-500:98765');
        expect(mockTransactionsStore.updateTransactionListFilter).toHaveBeenLastCalledWith({
            amountFilterCents: 'between:-500:98765',
        });
        expect(bindings.query.value.amountFilterCents).toBe('between:-500:98765');
        expect(bindings.queryAmount.value).toBe('false:-500 ~ false:98765');
    });
});
