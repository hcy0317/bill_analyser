import { afterEach, beforeEach, describe, expect, jest, test } from '@jest/globals';
import { disposePinia, createPinia, setActivePinia, type Pinia } from 'pinia';

const actualVue = jest.requireActual('vue') as any;

const mockSettingsStore = actualVue.reactive({
    appSettings: {
        timezoneUsedForStatisticsInHomePage: 0,
        overviewAccountFilterInHomePage: {} as Record<string, boolean>,
        overviewTransactionCategoryFilterInHomePage: {} as Record<string, boolean>
    }
});
const mockUserStore = actualVue.reactive({
    currentUserDefaultCurrency: 'CNY',
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: 4
});
const mockAccountsStore = actualVue.reactive({ allAccountsMap: { cash: { id: 'cash' } } });
const mockCategoriesStore = actualVue.reactive({ allTransactionCategoriesMap: { food: { id: 'food' } } });
const mockGetExchangedAmount = jest.fn<(amount: number, from: string, to: string) => number | null>();
const mockExchangeRatesStore = { getExchangedAmount: mockGetExchangedAmount };
const mockGetTransactionAmounts = jest.fn<(...args: unknown[]) => Promise<unknown>>();
const mockLoggerError = jest.fn<(...args: unknown[]) => void>();
const mockFinalAccountIds = jest.fn<(...args: unknown[]) => string[]>(() => ['account-final']);
const mockFinalCategoryIds = jest.fn<(...args: unknown[]) => string[]>(() => ['category-final']);

let mockTodayStart = 100;
const mockGetTodayFirst = jest.fn(() => mockTodayStart);
const mockGetTodayLast = jest.fn(() => 199);
const mockGetWeekFirst = jest.fn((firstDay: number) => 200 + firstDay);
const mockGetWeekLast = jest.fn((firstDay: number) => 299 + firstDay);
const mockGetMonthFirst = jest.fn(() => 1_000);
const mockGetMonthLast = jest.fn(() => 1_999);
const mockGetYearFirst = jest.fn((month: number) => 3_000 + month);
const mockGetYearLast = jest.fn((month: number) => 3_999 + month);
const mockGetBefore = jest.fn((time: number, count: number, unit: string) => (
    unit === 'months' ? time - count * 1_000 : time - count
));

jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    useTransactionCategoriesStore: () => mockCategoriesStore
}));
jest.mock('@/stores/exchangeRates.ts', () => ({
    useExchangeRatesStore: () => mockExchangeRatesStore
}));
jest.mock('@/lib/datetime.ts', () => ({
    getUnixTimeBeforeUnixTime: (time: number, count: number, unit: string) => mockGetBefore(time, count, unit),
    getTodayFirstUnixTime: () => mockGetTodayFirst(),
    getTodayLastUnixTime: () => mockGetTodayLast(),
    getThisWeekFirstUnixTime: (firstDay: number) => mockGetWeekFirst(firstDay),
    getThisWeekLastUnixTime: (firstDay: number) => mockGetWeekLast(firstDay),
    getThisMonthFirstUnixTime: () => mockGetMonthFirst(),
    getThisMonthLastUnixTime: () => mockGetMonthLast(),
    getThisYearFirstUnixTime: (month: number) => mockGetYearFirst(month),
    getThisYearLastUnixTime: (month: number) => mockGetYearLast(month)
}));
jest.mock('@/lib/account.ts', () => ({
    getFinalAccountIdsByFilteredAccountIds: (...args: unknown[]) => mockFinalAccountIds(...args)
}));
jest.mock('@/lib/category.ts', () => ({
    getFinalCategoryIdsByFilteredCategoryIds: (...args: unknown[]) => mockFinalCategoryIds(...args)
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { error: (...args: unknown[]) => mockLoggerError(...args) }
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: { getTransactionAmounts: (...args: unknown[]) => mockGetTransactionAmounts(...args) }
}));

import { DateRange } from '@/core/datetime.ts';
import { TimezoneTypeForStatistics } from '@/core/timezone.ts';
import { useOverviewStore } from '@/stores/overview.ts';

let pinia: Pinia;

function success(result: Record<string, unknown>): unknown {
    return { data: { success: true, result } };
}

beforeEach(() => {
    jest.clearAllMocks();
    pinia = createPinia();
    setActivePinia(pinia);
    mockTodayStart = 100;
    mockUserStore.currentUserDefaultCurrency = 'CNY';
    mockUserStore.currentUserFirstDayOfWeek = 1;
    mockUserStore.currentUserFiscalYearStart = 4;
    mockSettingsStore.appSettings.timezoneUsedForStatisticsInHomePage = (
        TimezoneTypeForStatistics.TransactionTimezone.type
    );
    mockSettingsStore.appSettings.overviewAccountFilterInHomePage = {};
    mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = {};
    mockGetExchangedAmount.mockImplementation(amount => amount);
});

afterEach(() => {
    disposePinia(pinia);
});

describe('overview store date ranges and amount projection', () => {
    test('initializes every range, watches fiscal year changes, and resets state', async () => {
        const store = useOverviewStore();

        expect(store.transactionDataRange).toMatchObject({
            today: { startTime: 100, endTime: 199 },
            thisWeek: { startTime: 201, endTime: 300 },
            thisMonth: { startTime: 1_000, endTime: 1_999 },
            thisYear: { startTime: 3_004, endTime: 4_003 },
            lastMonth: { startTime: 0, endTime: 999 },
            monthBeforeLastMonth: { startTime: -1_000, endTime: -1 },
            monthBeforeLast10Months: { startTime: -10_000, endTime: -9_001 }
        });

        mockUserStore.currentUserFiscalYearStart = 7;
        await actualVue.nextTick();
        expect(mockGetYearFirst).toHaveBeenLastCalledWith(7);
        expect(store.transactionDataRange.thisYear.startTime).toBe(3_007);

        store.transactionOverviewOptions.loadLast11Months = true;
        store.transactionOverviewData = { thisMonth: { amounts: [] } } as never;
        store.updateTransactionOverviewInvalidState(false);
        store.resetTransactionOverview();
        expect(store.transactionOverviewOptions.loadLast11Months).toBe(false);
        expect(store.transactionOverviewData).toStrictEqual({});
        expect(store.transactionOverviewStateInvalid).toBe(true);
    });

    test('returns an invalid month until data exists and sums mixed currencies in integer cents', () => {
        const store = useOverviewStore();
        expect(store.transactionOverview.thisMonth).toMatchObject({
            valid: false,
            incomeAmountCents: 0,
            expenseAmountCents: 0,
            incompleteIncomeAmount: false,
            incompleteExpenseAmount: false
        });

        mockGetExchangedAmount.mockImplementation(amount => {
            if (amount === 25) return 37.9;
            if (amount === 40) return null;
            if (amount === 45) return null;
            if (amount === 60) return 61.8;
            return amount;
        });
        store.transactionOverviewData = {
            today: {},
            thisMonth: {
                amounts: [
                    { currency: 'CNY', incomeAmountCents: 100, expenseAmountCents: 50 },
                    { currency: 'USD', incomeAmountCents: 25, expenseAmountCents: 40 },
                    { currency: 'EUR', incomeAmountCents: 45, expenseAmountCents: 60 }
                ]
            }
        } as never;

        expect(store.transactionOverview.today).toMatchObject({
            valid: true,
            incomeAmountCents: 0,
            expenseAmountCents: 0,
            amounts: []
        });
        expect(store.transactionOverview.thisMonth).toStrictEqual({
            valid: true,
            incomeAmountCents: 137,
            expenseAmountCents: 111,
            incompleteIncomeAmount: true,
            incompleteExpenseAmount: true,
            amounts: [
                { currency: 'CNY', incomeAmountCents: 100, expenseAmountCents: 50 },
                { currency: 'USD', incomeAmountCents: 25, expenseAmountCents: 40 },
                { currency: 'EUR', incomeAmountCents: 45, expenseAmountCents: 60 }
            ]
        });
        expect(Number.isInteger(store.transactionOverview.thisMonth!.incomeAmountCents)).toBe(true);
        expect(mockGetExchangedAmount).toHaveBeenCalledWith(25, 'USD', 'CNY');
        expect(mockGetExchangedAmount).toHaveBeenCalledWith(40, 'USD', 'CNY');
    });
});

describe('overview store loading contract', () => {
    test('loads current ranges, caches them, expands months, and refreshes when the date changes', async () => {
        const store = useOverviewStore();
        const firstResult = { thisMonth: { amounts: [] } };
        mockSettingsStore.appSettings.overviewAccountFilterInHomePage = { hidden: true };
        mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = { ignored: true };
        mockGetTransactionAmounts.mockResolvedValue(success(firstResult));

        await expect(store.loadTransactionOverview({ force: false })).resolves.toStrictEqual(firstResult);
        expect(store.transactionOverviewStateInvalid).toBe(false);
        expect(mockGetTransactionAmounts).toHaveBeenCalledWith(
            expect.objectContaining({
                useTransactionTimezone: true,
                today: { startTime: 100, endTime: 199 },
                thisMonth: { startTime: 1_000, endTime: 1_999 }
            }),
            ['hidden'],
            ['ignored']
        );

        await expect(store.loadTransactionOverview({ force: false })).resolves.toStrictEqual(firstResult);
        expect(mockGetTransactionAmounts).toHaveBeenCalledTimes(1);

        const expandedResult = { thisMonth: { amounts: [] }, lastMonth: { amounts: [] } };
        mockGetTransactionAmounts.mockResolvedValueOnce(success(expandedResult));
        await store.loadTransactionOverview({ force: false, loadLast11Months: true });
        expect(mockGetTransactionAmounts.mock.calls[1]![0]).toEqual(expect.objectContaining({
            lastMonth: store.transactionDataRange.lastMonth,
            monthBeforeLastMonth: store.transactionDataRange.monthBeforeLastMonth,
            monthBeforeLast10Months: store.transactionDataRange.monthBeforeLast10Months
        }));
        expect(store.transactionOverviewOptions.loadLast11Months).toBe(true);

        mockTodayStart = 500;
        const nextDay = { thisMonth: { amounts: [{ currency: 'CNY', incomeAmountCents: 1, expenseAmountCents: 2 }] } };
        mockGetTransactionAmounts.mockResolvedValueOnce(success(nextDay));
        await store.loadTransactionOverview({ force: false });
        expect(store.transactionDataRange.today.startTime).toBe(500);
        expect(mockGetTransactionAmounts).toHaveBeenCalledTimes(3);
    });

    test('rejects an unchanged forced response and supports transaction-timezone opt-out', async () => {
        const store = useOverviewStore();
        const result = { thisMonth: { amounts: [] } };
        store.transactionOverviewData = result as never;
        store.updateTransactionOverviewInvalidState(false);
        mockSettingsStore.appSettings.timezoneUsedForStatisticsInHomePage = -1;
        mockGetTransactionAmounts.mockResolvedValue(success(result));

        await expect(store.loadTransactionOverview({ force: true })).rejects.toStrictEqual({
            message: 'Data is up to date',
            isUpToDate: true
        });
        expect(mockGetTransactionAmounts.mock.calls[0]![0]).toEqual(expect.objectContaining({
            useTransactionTimezone: false
        }));
    });

    test('normalizes invalid envelopes and every service rejection shape', async () => {
        const store = useOverviewStore();
        mockGetTransactionAmounts.mockResolvedValueOnce({ data: { success: false, result: null } });
        await expect(store.loadTransactionOverview({ force: true })).rejects.toStrictEqual({
            message: 'Unable to retrieve transaction overview'
        });

        const responseError = { response: { data: { message: 'backend detail' } } };
        mockGetTransactionAmounts.mockRejectedValueOnce(responseError);
        await expect(store.loadTransactionOverview({ force: false })).rejects.toStrictEqual({
            error: responseError.response.data
        });
        expect(mockLoggerError).toHaveBeenCalledWith('failed to load transaction overview', responseError);

        const unprocessed = { processed: false };
        mockGetTransactionAmounts.mockRejectedValueOnce(unprocessed);
        await expect(store.loadTransactionOverview({ force: true })).rejects.toStrictEqual({
            message: 'Unable to retrieve transaction overview'
        });
        expect(mockLoggerError).toHaveBeenCalledWith('failed to force load transaction overview', unprocessed);

        const processed = { processed: true, marker: 'keep' };
        mockGetTransactionAmounts.mockRejectedValueOnce(processed);
        await expect(store.loadTransactionOverview({ force: true })).rejects.toBe(processed);
    });
});

describe('overview store transaction list query contract', () => {
    test('projects custom ranges and active account/category filters', () => {
        const store = useOverviewStore();
        mockSettingsStore.appSettings.overviewAccountFilterInHomePage = { hidden: true };
        mockSettingsStore.appSettings.overviewTransactionCategoryFilterInHomePage = { ignored: true };

        expect(store.getTransactionListPageParams({
            type: 'expense' as never,
            dateType: DateRange.Custom.type,
            minTime: 123,
            maxTime: 456
        })).toBe([
            'type=expense',
            `dateType=${DateRange.Custom.type}`,
            'minTime=123',
            'maxTime=456',
            'categoryIds=category-final',
            'accountIds=account-final'
        ].join('&'));
        expect(mockFinalCategoryIds).toHaveBeenCalledWith(
            mockCategoriesStore.allTransactionCategoriesMap,
            { ignored: true }
        );
        expect(mockFinalAccountIds).toHaveBeenCalledWith(
            mockAccountsStore.allAccountsMap,
            { hidden: true }
        );
    });

    test('omits undefined values, invalid custom bounds, and empty filters', () => {
        const store = useOverviewStore();
        expect(store.getTransactionListPageParams({
            dateType: DateRange.Custom.type,
            minTime: 0,
            maxTime: Number.NaN
        })).toBe(`dateType=${DateRange.Custom.type}`);
        expect(store.getTransactionListPageParams({ dateType: DateRange.ThisMonth.type })).toBe(
            `dateType=${DateRange.ThisMonth.type}`
        );
        expect(store.getTransactionListPageParams({})).toBe('');
    });
});
