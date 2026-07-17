import { beforeEach, describe, expect, jest, test } from '@jest/globals';
import { createPinia, setActivePinia } from 'pinia';

import { AccountCategory, AccountType } from '@/core/account.ts';
import { CategoryType } from '@/core/category.ts';
import { DateRange } from '@/core/datetime.ts';
import {
    CategoricalChartType,
    ChartDataType,
    ChartSortingType,
    StatisticsAnalysisType,
    TrendChartType
} from '@/core/statistics.ts';
import { TransactionRelatedAccountType, TransactionTagFilterType } from '@/core/transaction.ts';
import { Account, type AccountInfoResponse } from '@/models/account.ts';
import type {
    TransactionStatisticAssetTrendsResponseItem,
    TransactionStatisticResponse,
    TransactionStatisticResponseItem
} from '@/models/transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';

const mockSettingsStore = {
    appSettings: {
        statistics: {
            defaultTimezoneType: 1,
            defaultChartDataType: ChartDataType.ExpenseByPrimaryCategory.type,
            defaultCategoricalChartType: CategoricalChartType.Default.type,
            defaultCategoricalChartDataRangeType: DateRange.ThisMonth.type,
            defaultTrendChartType: TrendChartType.Default.type,
            defaultTrendChartDataRangeType: DateRange.ThisYear.type,
            defaultAssetTrendsChartType: TrendChartType.Default.type,
            defaultAssetTrendsChartDataRangeType: DateRange.ThisYear.type,
            defaultAccountFilter: {} as Record<string, boolean> | null,
            defaultTransactionCategoryFilter: {} as Record<string, boolean> | null,
            defaultSortingType: ChartSortingType.Default.type
        }
    }
};
const mockUserStore = {
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: 1,
    currentUserDefaultCurrency: 'CNY'
};
const mockAccountsStore = {
    allAccountsMap: {} as Record<string, Account>,
    allPlainAccounts: [] as Account[] | null
};
const mockCategoriesStore = {
    allTransactionCategoriesMap: {} as Record<string, TransactionCategory>
};
const mockGetExchangedAmount = jest.fn<(amount: number, from: string, to: string) => number | null>();
const mockGetTransactionStatisticsAssetTrends = jest.fn<(request: unknown) => Promise<unknown>>();

jest.mock('@/locales/helpers.ts', () => ({
    __esModule: true,
    useI18n: () => ({ tt: (value: string) => `translated:${value}` })
}));
jest.mock('@/stores/setting.ts', () => ({ __esModule: true, useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ __esModule: true, useUserStore: () => mockUserStore }));
jest.mock('@/stores/account.ts', () => ({ __esModule: true, useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({
    __esModule: true,
    useTransactionCategoriesStore: () => mockCategoriesStore
}));
jest.mock('@/stores/exchangeRates.ts', () => ({
    __esModule: true,
    useExchangeRatesStore: () => ({ getExchangedAmount: mockGetExchangedAmount })
}));
jest.mock('@/lib/services.ts', () => ({
    __esModule: true,
    default: {
        getTransactionStatistics: jest.fn(),
        getTransactionStatisticsTrends: jest.fn(),
        getTransactionStatisticsAssetTrends: (request: unknown) => mockGetTransactionStatisticsAssetTrends(request)
    }
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: { debug: jest.fn(), info: jest.fn(), warn: jest.fn(), error: jest.fn() }
}));

function accountResponse(overrides: Partial<AccountInfoResponse> = {}): AccountInfoResponse {
    return {
        id: 'cash',
        name: 'Cash',
        parentId: '0',
        category: AccountCategory.Cash.type,
        type: AccountType.SingleAccount.type,
        icon: '',
        color: '',
        currency: 'CNY',
        balanceCents: 1_000,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function makeAccount(overrides: Partial<AccountInfoResponse>): Account {
    return Account.of(accountResponse(overrides));
}

function makeCategory(type: CategoryType, id: string, parentId = '0'): TransactionCategory {
    const value = TransactionCategory.createNewCategory(type, parentId);
    value.id = id;
    value.name = id;
    value.icon = '';
    value.color = '';
    value.displayOrder = 1;
    return value;
}

function installDomain(): void {
    const cash = makeAccount({ id: 'cash' });
    const destination = makeAccount({ id: 'destination', name: 'Destination', displayOrder: 2 });
    const investmentParent = makeAccount({
        id: 'investment-parent',
        name: 'Investments',
        category: AccountCategory.InvestmentAccount.type,
        type: AccountType.MultiSubAccounts.type,
        balanceCents: 0,
        displayOrder: 3
    });
    const investmentChild = makeAccount({
        id: 'investment-child',
        name: 'Fund',
        parentId: 'investment-parent',
        category: AccountCategory.InvestmentAccount.type,
        hidden: true,
        displayOrder: 4
    });
    const liability = makeAccount({
        id: 'liability',
        name: 'Debt',
        category: AccountCategory.CreditCard.type,
        balanceCents: -500,
        displayOrder: 5
    });
    mockAccountsStore.allAccountsMap = {
        cash,
        destination,
        'investment-parent': investmentParent,
        'investment-child': investmentChild,
        liability
    };
    mockAccountsStore.allPlainAccounts = [cash, destination, investmentChild, liability];

    mockCategoriesStore.allTransactionCategoriesMap = {};
    for (const [type, name] of [
        [CategoryType.Expense, 'expense'],
        [CategoryType.Income, 'income'],
        [CategoryType.Transfer, 'transfer'],
        [CategoryType.Investment, 'investment'],
        [999 as CategoryType, 'unknown']
    ] as const) {
        const primary = makeCategory(type, `${name}-primary`);
        const secondary = makeCategory(type, `${name}-secondary`, primary.id);
        primary.subCategories = [secondary];
        mockCategoriesStore.allTransactionCategoriesMap[primary.id] = primary;
        mockCategoriesStore.allTransactionCategoriesMap[secondary.id] = secondary;
    }
}

function item(overrides: Partial<TransactionStatisticResponseItem> = {}): TransactionStatisticResponseItem {
    return {
        categoryId: 'expense-secondary',
        accountId: 'cash',
        amountCents: -100,
        ...overrides
    };
}

function response(items: TransactionStatisticResponseItem[]): TransactionStatisticResponse {
    return { startTime: 1, endTime: 2, items };
}

function assetResult(items: TransactionStatisticAssetTrendsResponseItem[] | unknown[]): Promise<unknown> {
    return Promise.resolve({ data: { success: true, result: items } });
}

describe('statistics store uncovered boundaries', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        installDomain();
        mockSettingsStore.appSettings.statistics.defaultAccountFilter = {};
        mockSettingsStore.appSettings.statistics.defaultTransactionCategoryFilter = {};
        mockGetExchangedAmount.mockReset();
        mockGetExchangedAmount.mockImplementation((amount, from, to) => from === to ? amount : null);
        mockGetTransactionStatisticsAssetTrends.mockReset();
    });

    test('keeps empty, invalid and independently filtered categorical data out of projections', () => {
        const store = useStatisticsStore();
        store.transactionStatisticsFilter.chartDataType = ChartDataType.ExpenseByAccount.type;

        expect(store.categoricalAnalysisData).toEqual({ totalAmountCents: 0, items: [] });

        store.transactionCategoryStatisticsData = response([]);
        expect(store.categoricalOverviewAnalysisData).toMatchObject({ items: [] });

        store.transactionCategoryStatisticsData = response([
            item({ amountCents: null as unknown as number }),
            item({ amountCents: -50 })
        ]);
        store.transactionStatisticsFilter.filterCategoryIds = { 'expense-secondary': true };
        expect(store.categoricalOverviewAnalysisData).toMatchObject({
            totalExpenseCents: 0,
            items: expect.not.arrayContaining([expect.objectContaining({ id: 'expense-secondary' })])
        });

        store.transactionStatisticsFilter.filterCategoryIds = {};
        expect(store.categoricalOverviewAnalysisData?.totalExpenseCents).toBe(-50);
        expect(store.categoricalAnalysisData.items).toEqual([
            expect.objectContaining({ id: 'cash', totalAmountCents: -50 })
        ]);

        store.transactionStatisticsFilter.filterAccountIds = { cash: true };
        expect(store.categoricalAnalysisData.items).toEqual([]);
        store.transactionStatisticsFilter.filterAccountIds = {};
        store.transactionStatisticsFilter.filterCategoryIds = { 'expense-secondary': true };
        expect(store.categoricalAnalysisData.items).toEqual([]);
    });

    test('reuses transfer targets, creates nested investment targets and ignores unknown category types', () => {
        const store = useStatisticsStore();
        store.transactionCategoryStatisticsData = response([
            item({ accountId: 'investment-child', amountCents: -20 }),
            item({
                categoryId: 'transfer-secondary',
                accountId: 'cash',
                relatedAccountId: 'investment-child',
                relatedAccountType: TransactionRelatedAccountType.TransferTo,
                amountCents: 40
            }),
            item({
                categoryId: 'investment-secondary',
                accountId: 'cash',
                relatedAccountId: 'destination',
                relatedAccountType: TransactionRelatedAccountType.TransferTo,
                amountCents: 60
            }),
            item({
                categoryId: 'unknown-secondary',
                accountId: 'cash',
                relatedAccountId: 'destination',
                relatedAccountType: TransactionRelatedAccountType.TransferTo,
                amountCents: 1
            })
        ]);

        const overview = store.categoricalOverviewAnalysisData!;
        expect(overview.items.some(value => value.id === 'investment-child')).toBe(true);
        expect(overview.items.some(value => value.id === 'destination' && value.inflows.length > 0)).toBe(true);
        expect(overview.items.every(value => Number.isFinite(value.totalAmountCents))).toBe(true);
    });

    test('handles zero cash-flow and zero-percent account projections without negative percentages', () => {
        const store = useStatisticsStore();
        store.transactionCategoryStatisticsData = response([item({ amountCents: 0 })]);

        expect(store.categoricalOverviewAnalysisData?.items.every(value => value.totalAmountCents === 0)).toBe(true);
        store.transactionStatisticsFilter.chartDataType = ChartDataType.OutflowsByAccount.type;
        expect(store.categoricalAnalysisData.items).toEqual([
            expect.objectContaining({ id: 'cash', percent: 0 })
        ]);
        store.transactionStatisticsFilter.chartDataType = ChartDataType.ExpenseByAccount.type;
        expect(store.categoricalAnalysisData.items).toEqual([
            expect.objectContaining({ id: 'cash', percent: 0 })
        ]);
    });

    test('covers every invalid trend amount projection and unsupported chart fallback', () => {
        const store = useStatisticsStore();
        store.transactionCategoryTrendsData = [{
            year: 2026,
            month: 1,
            items: [item({ amountCents: null as unknown as number })]
        }];

        for (const chartType of [
            ChartDataType.ExpenseByAccount,
            ChartDataType.ExpenseByPrimaryCategory,
            ChartDataType.ExpenseBySecondaryCategory,
            ChartDataType.TotalExpense
        ]) {
            store.transactionStatisticsFilter.chartDataType = chartType.type;
            expect(store.trendsAnalysisData?.items).toEqual([]);
        }

        store.transactionStatisticsFilter.chartDataType = 999;
        expect(store.trendsAnalysisData?.items).toEqual([]);

        store.transactionCategoryTrendsData = [{
            year: 2026,
            month: 2,
            items: [item({ amountCents: -10 })]
        }];
        store.transactionStatisticsFilter.chartDataType = ChartDataType.ExpenseByAccount.type;
        store.transactionStatisticsFilter.filterAccountIds = { cash: true };
        expect(store.trendsAnalysisData?.items).toEqual([]);
        store.transactionStatisticsFilter.filterAccountIds = {};
        store.transactionStatisticsFilter.filterCategoryIds = { 'expense-secondary': true };
        expect(store.trendsAnalysisData?.items).toEqual([]);
    });

    test('short-circuits empty and malformed asset-trend payloads', async () => {
        const store = useStatisticsStore();
        expect(store.assetTrendsData).toBeNull();

        mockGetTransactionStatisticsAssetTrends.mockReturnValue(assetResult([undefined]));
        await store.loadAssetTrends({ force: false });
        expect(store.assetTrendsData).toBeNull();
    });

    test('projects asset, liability and net-worth trend modes from the same cents payload', async () => {
        const store = useStatisticsStore();
        mockGetTransactionStatisticsAssetTrends.mockReturnValue(assetResult([{
            year: 2026,
            month: 1,
            day: 1,
            items: [
                { accountId: 'cash', accountOpeningBalanceCents: 900, accountClosingBalanceCents: 1_000 },
                { accountId: 'liability', accountOpeningBalanceCents: -400, accountClosingBalanceCents: -500 }
            ]
        }]));
        await store.loadAssetTrends({ force: false });

        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalAssets.type;
        expect(store.assetTrendsData?.items.some(value => value.id === 'cash')).toBe(true);
        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalLiabilities.type;
        expect(store.assetTrendsData?.items.some(value => value.id === 'liability')).toBe(true);
        store.transactionStatisticsFilter.chartDataType = ChartDataType.NetWorth.type;
        expect(store.assetTrendsData?.items).toEqual([
            expect.objectContaining({ id: 'netWorth', totalAmountCents: 1_500 })
        ]);
        store.transactionStatisticsFilter.chartDataType = 999;
        expect(store.assetTrendsData?.items).toEqual([]);
    });

    test('initializes missing custom bounds and null default filters for every analysis type', () => {
        const store = useStatisticsStore();
        mockSettingsStore.appSettings.statistics.defaultAccountFilter = null;
        mockSettingsStore.appSettings.statistics.defaultTransactionCategoryFilter = null;

        store.initTransactionStatisticsFilter(StatisticsAnalysisType.AssetTrends, {
            chartDataType: ChartDataType.NetWorth.type,
            categoricalChartDateType: DateRange.Custom.type,
            trendChartDateType: DateRange.Custom.type,
            assetTrendsChartDateType: DateRange.Custom.type
        });
        expect(store.transactionStatisticsFilter).toMatchObject({
            chartDataType: ChartDataType.NetWorth.type,
            categoricalChartStartTime: 0,
            categoricalChartEndTime: 0,
            trendChartStartYearMonth: '',
            trendChartEndYearMonth: '',
            assetTrendsChartStartTime: 0,
            assetTrendsChartEndTime: 0,
            filterAccountIds: {},
            filterCategoryIds: {}
        });

        store.initTransactionStatisticsFilter(StatisticsAnalysisType.CategoricalAnalysis, {
            chartDataType: ChartDataType.NetWorth.type
        });
        expect(store.transactionStatisticsFilter.chartDataType).toBe(ChartDataType.Default.type);
        store.initTransactionStatisticsFilter(999 as StatisticsAnalysisType, {
            chartDataType: ChartDataType.ExpenseByAccount.type
        });
        expect(store.transactionStatisticsFilter.chartDataType).toBe(ChartDataType.ExpenseByAccount.type);
    });

    test('updates valid and invalid non-custom ranges across all three date domains', () => {
        const store = useStatisticsStore();
        store.updateTransactionStatisticsFilter({
            categoricalChartDateType: DateRange.Custom.type,
            trendChartDateType: DateRange.Custom.type,
            assetTrendsChartDateType: DateRange.Custom.type
        });

        expect(store.updateTransactionStatisticsFilter({
            categoricalChartDateType: DateRange.ThisMonth.type,
            trendChartDateType: DateRange.ThisYear.type,
            assetTrendsChartDateType: DateRange.ThisYear.type
        })).toBe(true);
        expect(store.transactionStatisticsFilter.categoricalChartStartTime).toBeGreaterThan(0);
        expect(store.transactionStatisticsFilter.trendChartStartYearMonth).toMatch(/^\d{4}-\d{2}$/);
        expect(store.transactionStatisticsFilter.assetTrendsChartStartTime).toBeGreaterThan(0);

        expect(store.updateTransactionStatisticsFilter({
            categoricalChartDateType: 999,
            trendChartDateType: 999,
            assetTrendsChartDateType: 999
        })).toBe(true);
        expect(store.transactionStatisticsFilter).toMatchObject({
            categoricalChartDateType: 999,
            trendChartDateType: 999,
            assetTrendsChartDateType: 999
        });
    });

    test('retains the default tag filter when reset after boundary exercises', () => {
        const store = useStatisticsStore();
        store.transactionStatisticsFilter.tagFilterType = TransactionTagFilterType.NotHasAny.type;
        store.resetTransactionStatistics();

        expect(store.transactionStatisticsFilter.tagFilterType).toBe(TransactionTagFilterType.Default.type);
        expect(store.transactionStatisticsStateInvalid).toBe(true);
    });
});
