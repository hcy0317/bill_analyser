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
    TransactionStatisticResponseItem,
    TransactionStatisticTrendsResponseItem
} from '@/models/transaction.ts';
import { TransactionCategory } from '@/models/transaction_category.ts';
import { useStatisticsStore } from '@/stores/statistics.ts';

type LoosePromise = Promise<unknown>;

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
            defaultAccountFilter: {} as Record<string, boolean>,
            defaultTransactionCategoryFilter: {} as Record<string, boolean>,
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
const mockServices = {
    getTransactionStatistics: jest.fn<(request: unknown) => LoosePromise>(),
    getTransactionStatisticsTrends: jest.fn<(request: unknown) => LoosePromise>(),
    getTransactionStatisticsAssetTrends: jest.fn<(request: unknown) => LoosePromise>()
};

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
        getTransactionStatistics: (request: unknown) => mockServices.getTransactionStatistics(request),
        getTransactionStatisticsTrends: (request: unknown) => mockServices.getTransactionStatisticsTrends(request),
        getTransactionStatisticsAssetTrends: (request: unknown) => mockServices.getTransactionStatisticsAssetTrends(request)
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
        icon: '1',
        color: '#111',
        currency: 'CNY',
        balanceCents: 1_000,
        comment: '',
        displayOrder: 1,
        hidden: false,
        ...overrides
    };
}

function makeAccount(overrides: Partial<AccountInfoResponse> = {}): Account {
    return Account.of(accountResponse(overrides));
}

function makeCategory(type: CategoryType, id: string, parentId = '0', displayOrder = 1): TransactionCategory {
    const category = TransactionCategory.createNewCategory(type, parentId);
    category.id = id;
    category.name = id;
    category.displayOrder = displayOrder;
    category.icon = '';
    category.color = '';
    return category;
}

function installDomain(): void {
    const bankParent = makeAccount({
        id: 'bank-parent',
        name: 'Bank',
        category: AccountCategory.CheckingAccount.type,
        type: AccountType.MultiSubAccounts.type,
        currency: '---',
        balanceCents: 0,
        displayOrder: 2,
        subAccounts: [
            accountResponse({
                id: 'bank-cny', name: 'Bank CNY', parentId: 'bank-parent',
                category: AccountCategory.CheckingAccount.type, currency: 'CNY', balanceCents: 2_000
            }),
            accountResponse({
                id: 'bank-usd', name: 'Bank USD', parentId: 'bank-parent',
                category: AccountCategory.CheckingAccount.type, currency: 'USD', balanceCents: 100,
                displayOrder: 2
            })
        ]
    });
    const cash = makeAccount({ id: 'cash', icon: '', color: '', balanceCents: 1_000 });
    const debt = makeAccount({
        id: 'debt', name: 'Debt', category: AccountCategory.CreditCard.type,
        balanceCents: -500, icon: '', color: '', displayOrder: 3
    });
    const destination = makeAccount({
        id: 'destination', name: 'Destination', category: AccountCategory.InvestmentAccount.type,
        balanceCents: 3_000, displayOrder: 4
    });
    const zero = makeAccount({ id: 'zero', name: 'Zero', balanceCents: 0, displayOrder: 5 });
    const hidden = makeAccount({ id: 'hidden', hidden: true, balanceCents: 999, displayOrder: 6 });
    mockAccountsStore.allAccountsMap = {
        cash,
        debt,
        destination,
        zero,
        hidden,
        'bank-parent': bankParent
    };
    for (const child of bankParent.subAccounts!) mockAccountsStore.allAccountsMap[child.id] = child;
    mockAccountsStore.allPlainAccounts = [cash, ...bankParent.subAccounts!, debt, destination, zero, hidden];

    mockCategoriesStore.allTransactionCategoriesMap = {};
    for (const [type, prefix] of [
        [CategoryType.Expense, 'expense'],
        [CategoryType.Income, 'income'],
        [CategoryType.Transfer, 'transfer'],
        [CategoryType.Investment, 'investment']
    ] as const) {
        const primary = makeCategory(type, `${prefix}-primary`, '0', 1);
        const secondary = makeCategory(type, `${prefix}-secondary`, primary.id, 2);
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

function categoricalItems(): TransactionStatisticResponseItem[] {
    return [
        item({ accountId: 'cash', amountCents: -100 }),
        item({ accountId: 'cash', amountCents: -50 }),
        item({ categoryId: 'income-secondary', accountId: 'cash', amountCents: 500 }),
        item({ categoryId: 'income-secondary', accountId: 'bank-usd', amountCents: -10 }),
        item({
            categoryId: 'transfer-secondary', accountId: 'cash', relatedAccountId: 'destination',
            relatedAccountType: TransactionRelatedAccountType.TransferTo, amountCents: 200
        }),
        item({
            categoryId: 'transfer-secondary', accountId: 'destination', relatedAccountId: 'cash',
            relatedAccountType: TransactionRelatedAccountType.TransferFrom, amountCents: 200
        }),
        item({
            categoryId: 'investment-secondary', accountId: 'cash', relatedAccountId: 'destination',
            relatedAccountType: TransactionRelatedAccountType.TransferTo, amountCents: 300
        }),
        item({
            categoryId: 'investment-secondary', accountId: 'destination', relatedAccountId: 'cash',
            relatedAccountType: TransactionRelatedAccountType.TransferFrom, amountCents: 300
        }),
        item({ categoryId: 'expense-primary', accountId: 'debt', amountCents: -40 }),
        item({ categoryId: 'missing-category', accountId: 'cash', amountCents: 1 }),
        item({ categoryId: 'expense-secondary', accountId: 'missing-account', amountCents: 1 }),
        item({ categoryId: 'expense-secondary', accountId: '', amountCents: 1 })
    ];
}

function categoricalResponse(items = categoricalItems()): TransactionStatisticResponse {
    return { startTime: 1, endTime: 2, items };
}

function trendsResponse(): TransactionStatisticTrendsResponseItem[] {
    return [
        { year: 2026, month: 1, items: categoricalItems() },
        { year: 2026, month: 2, items: [
            item({ accountId: 'cash', amountCents: -20 }),
            item({ categoryId: 'income-secondary', accountId: 'cash', amountCents: 100 })
        ] },
        { year: 2026, month: 3, items: [] }
    ];
}

function assetTrendsResponse(): TransactionStatisticAssetTrendsResponseItem[] {
    return [
        {
            year: 2026,
            month: 1,
            day: 1,
            items: [
                { accountId: 'cash', accountOpeningBalanceCents: 900, accountClosingBalanceCents: 1_000 },
                { accountId: 'bank-usd', accountOpeningBalanceCents: 90, accountClosingBalanceCents: 100 },
                { accountId: 'debt', accountOpeningBalanceCents: -400, accountClosingBalanceCents: -500 },
                { accountId: 'zero', accountOpeningBalanceCents: 0, accountClosingBalanceCents: 0 }
            ]
        },
        {
            year: 2026,
            month: 1,
            day: 3,
            items: [
                { accountId: 'cash', accountOpeningBalanceCents: 1_000, accountClosingBalanceCents: 1_100 },
                { accountId: 'debt', accountOpeningBalanceCents: -500, accountClosingBalanceCents: -450 }
            ]
        }
    ];
}

function success<T>(result: T): Promise<{ data: { success: true; result: T } }> {
    return Promise.resolve({ data: { success: true, result } });
}

function invalidResponse(): Promise<{ data: { success: false; result: null } }> {
    return Promise.resolve({ data: { success: false, result: null } });
}

async function expectThreeErrorChannels(
    service: { mockReturnValue(value: LoosePromise): unknown },
    invoke: () => Promise<unknown>
): Promise<void> {
    service.mockReturnValue(Promise.reject({ response: { data: { message: 'backend detail' } } }));
    await expect(invoke()).rejects.toEqual({ error: { message: 'backend detail' } });
    service.mockReturnValue(Promise.reject({ processed: false }));
    await expect(invoke()).rejects.toEqual({ message: 'Unable to retrieve transaction statistics' });
    const processed = { processed: true, marker: 'processed' };
    service.mockReturnValue(Promise.reject(processed));
    await expect(invoke()).rejects.toBe(processed);
}

describe('statistics store behavior coverage', () => {
    beforeEach(() => {
        setActivePinia(createPinia());
        installDomain();
        mockSettingsStore.appSettings.statistics.defaultTimezoneType = 1;
        mockSettingsStore.appSettings.statistics.defaultChartDataType = ChartDataType.ExpenseByPrimaryCategory.type;
        mockSettingsStore.appSettings.statistics.defaultCategoricalChartType = CategoricalChartType.Default.type;
        mockSettingsStore.appSettings.statistics.defaultCategoricalChartDataRangeType = DateRange.ThisMonth.type;
        mockSettingsStore.appSettings.statistics.defaultTrendChartType = TrendChartType.Default.type;
        mockSettingsStore.appSettings.statistics.defaultTrendChartDataRangeType = DateRange.ThisYear.type;
        mockSettingsStore.appSettings.statistics.defaultAssetTrendsChartType = TrendChartType.Default.type;
        mockSettingsStore.appSettings.statistics.defaultAssetTrendsChartDataRangeType = DateRange.ThisYear.type;
        mockSettingsStore.appSettings.statistics.defaultAccountFilter = {};
        mockSettingsStore.appSettings.statistics.defaultTransactionCategoryFilter = {};
        mockSettingsStore.appSettings.statistics.defaultSortingType = ChartSortingType.Default.type;
        mockUserStore.currentUserDefaultCurrency = 'CNY';
        mockGetExchangedAmount.mockReset();
        mockGetExchangedAmount.mockImplementation((amount, from, to) => {
            if (from === to) return amount;
            if (from === 'USD' && to === 'CNY') return amount * 7;
            return null;
        });
        for (const service of Object.values(mockServices)) service.mockReset();
    });

    test('assembles categorical overview flows across income, expense, transfer and investment', () => {
        const store = useStatisticsStore();
        expect(store.categoricalOverviewAnalysisData).toBeNull();
        store.transactionCategoryStatisticsData = categoricalResponse();
        store.transactionStatisticsFilter.sortingType = ChartSortingType.DisplayOrder.type;
        const overview = store.categoricalOverviewAnalysisData!;
        expect(overview.totalIncomeCents).toBe(430);
        expect(overview.totalExpenseCents).toBe(-190);
        expect(overview.items.length).toBeGreaterThan(8);
        expect(overview.items.some(data => data.inflows.length > 0)).toBe(true);
        expect(overview.items.some(data => data.outflows.length > 0)).toBe(true);
        expect(overview.items.some(data => data.percent !== undefined)).toBe(true);

        store.transactionStatisticsFilter.filterAccountIds = { cash: true };
        store.transactionStatisticsFilter.filterCategoryIds = { 'expense-secondary': true };
        const filtered = store.categoricalOverviewAnalysisData!;
        expect(filtered.items.every(data => data.id !== 'cash')).toBe(true);
    });

    test('projects every category chart data type into account, primary, secondary and total series', () => {
        const store = useStatisticsStore();
        store.transactionCategoryStatisticsData = categoricalResponse();
        const chartTypes = [
            ChartDataType.OutflowsByAccount,
            ChartDataType.ExpenseByAccount,
            ChartDataType.ExpenseByPrimaryCategory,
            ChartDataType.ExpenseBySecondaryCategory,
            ChartDataType.InflowsByAccount,
            ChartDataType.IncomeByAccount,
            ChartDataType.IncomeByPrimaryCategory,
            ChartDataType.IncomeBySecondaryCategory
        ];

        for (const chartType of chartTypes) {
            store.transactionStatisticsFilter.chartDataType = chartType.type;
            store.transactionStatisticsFilter.filterAccountIds = {};
            store.transactionStatisticsFilter.filterCategoryIds = {};
            const data = store.categoricalAnalysisData;
            expect(Number.isFinite(data.totalAmountCents)).toBe(true);
            expect(data.items.length).toBeGreaterThan(0);
            expect(data.items.every(entry => Number.isFinite(entry.percent))).toBe(true);
            expect(store.categoricalAnalysisChartDataCategory).toBe(
                chartType.name.includes('Account') ? 'account'
                    : chartType.name.includes('Category') ? 'category' : ''
            );
        }

        store.transactionStatisticsFilter.chartDataType = ChartDataType.Overview.type;
        expect(store.categoricalAnalysisData).toEqual({ totalAmountCents: 0, items: [] });
        expect(store.categoricalAnalysisChartDataCategory).toBe('');
    });

    test('calculates account asset and liability charts with exchange, defaults, filters and missing rates', () => {
        const store = useStatisticsStore();
        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalAssets.type;
        expect(store.categoricalAnalysisChartDataCategory).toBe('account');
        const assets = store.categoricalAnalysisData;
        expect(assets.items.some(entry => entry.id === 'bank-usd' && entry.totalAmountCents === 700)).toBe(true);
        expect(assets.items.some(entry => entry.id === 'debt')).toBe(false);

        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalLiabilities.type;
        const liabilities = store.categoricalAnalysisData;
        expect(liabilities.items).toEqual([expect.objectContaining({ id: 'debt', totalAmountCents: 500 })]);

        store.transactionStatisticsFilter.filterAccountIds = { debt: true };
        expect(store.categoricalAnalysisData.items).toEqual([]);
        store.transactionStatisticsFilter.filterAccountIds = {};
        mockGetExchangedAmount.mockReturnValue(null);
        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalAssets.type;
        expect(store.categoricalAnalysisData.items.some(entry => entry.id === 'bank-usd')).toBe(false);

        mockAccountsStore.allPlainAccounts = null;
        setActivePinia(createPinia());
        const emptyStore = useStatisticsStore();
        emptyStore.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalAssets.type;
        expect(emptyStore.categoricalAnalysisData).toEqual({ totalAmountCents: 0, items: [] });
    });

    test('combines categorical trend items for all chart projections and empty data', () => {
        const store = useStatisticsStore();
        expect(store.trendsAnalysisData).toBeNull();
        store.transactionCategoryTrendsData = trendsResponse();
        for (const chartType of [
            ChartDataType.ExpenseByAccount,
            ChartDataType.ExpenseByPrimaryCategory,
            ChartDataType.ExpenseBySecondaryCategory,
            ChartDataType.IncomeByAccount,
            ChartDataType.IncomeByPrimaryCategory,
            ChartDataType.IncomeBySecondaryCategory,
            ChartDataType.TotalOutflows,
            ChartDataType.TotalExpense,
            ChartDataType.TotalInflows,
            ChartDataType.TotalIncome,
            ChartDataType.NetCashFlow,
            ChartDataType.NetIncome
        ]) {
            store.transactionStatisticsFilter.chartDataType = chartType.type;
            const trend = store.trendsAnalysisData!;
            expect(trend.items.length).toBeGreaterThan(0);
            expect(trend.items.some(series => series.items.length >= 2)).toBe(true);
        }
    });

    test('fills missing asset-trend days/accounts and derives asset, liability and net-worth series', async () => {
        const store = useStatisticsStore();
        mockServices.getTransactionStatisticsAssetTrends.mockReturnValue(success(assetTrendsResponse()));
        await store.loadAssetTrends({ force: false });

        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalAssets.type;
        const assets = store.assetTrendsData!;
        expect(assets.items.find(entry => entry.id === 'cash')?.items).toHaveLength(3);
        expect(assets.items.find(entry => entry.id === 'bank-usd')?.items[1]).toEqual(expect.objectContaining({
            totalAmountCents: 700,
            totalOpeningAmountCents: 700
        }));
        expect(assets.items.some(entry => entry.id === 'zero')).toBe(false);

        store.transactionStatisticsFilter.chartDataType = ChartDataType.AccountTotalLiabilities.type;
        expect(store.assetTrendsData!.items.find(entry => entry.id === 'debt')?.items[0]).toEqual(expect.objectContaining({
            totalAmountCents: 500,
            totalOpeningAmountCents: 400
        }));

        store.transactionStatisticsFilter.chartDataType = ChartDataType.NetWorth.type;
        const netWorth = store.assetTrendsData!;
        expect(netWorth.items).toEqual([expect.objectContaining({
            id: 'netWorth',
            name: 'translated:Net Worth',
            items: expect.any(Array)
        })]);
        expect(netWorth.items[0]!.items).toHaveLength(3);

        store.transactionStatisticsFilter.filterAccountIds = { cash: true, debt: true, 'bank-usd': true, zero: true };
        expect(store.assetTrendsData!.items).toEqual([]);
    });

    test('fails closed when asset trend rows lack accounts or exchange rates', async () => {
        const store = useStatisticsStore();
        mockGetExchangedAmount.mockReturnValue(null);
        mockServices.getTransactionStatisticsAssetTrends.mockReturnValue(success([{
            year: 2026,
            month: 1,
            day: 1,
            items: [
                { accountId: 'missing', accountOpeningBalanceCents: 1, accountClosingBalanceCents: 2 },
                { accountId: 'bank-usd', accountOpeningBalanceCents: 1, accountClosingBalanceCents: 2 }
            ]
        }]));
        await store.loadAssetTrends({ force: false });
        store.transactionStatisticsFilter.chartDataType = ChartDataType.NetWorth.type;
        expect(store.assetTrendsData!.items).toEqual([]);
    });

    test('initializes custom filters, rejects invalid chart/date choices and restores defaults', () => {
        const store = useStatisticsStore();
        store.initTransactionStatisticsFilter(StatisticsAnalysisType.CategoricalAnalysis, {
            chartDataType: ChartDataType.ExpenseByAccount.type,
            categoricalChartType: CategoricalChartType.Bar.type,
            categoricalChartDateType: DateRange.Custom.type,
            categoricalChartStartTime: 10,
            categoricalChartEndTime: 20,
            trendChartType: TrendChartType.Area.type,
            trendChartDateType: DateRange.Custom.type,
            trendChartStartYearMonth: '2026-01',
            trendChartEndYearMonth: '2026-12',
            assetTrendsChartType: TrendChartType.Bubble.type,
            assetTrendsChartDateType: DateRange.Custom.type,
            assetTrendsChartStartTime: 30,
            assetTrendsChartEndTime: 40,
            filterAccountIds: { cash: true },
            filterCategoryIds: { 'expense-secondary': true },
            tagIds: 'tag-a',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            keyword: 'coffee',
            sortingType: ChartSortingType.Name.type
        });
        expect(store.transactionStatisticsFilter).toEqual(expect.objectContaining({
            chartDataType: ChartDataType.ExpenseByAccount.type,
            categoricalChartStartTime: 10,
            categoricalChartEndTime: 20,
            trendChartStartYearMonth: '2026-01',
            trendChartEndYearMonth: '2026-12',
            assetTrendsChartStartTime: 30,
            assetTrendsChartEndTime: 40,
            filterAccountIds: { cash: true },
            filterCategoryIds: { 'expense-secondary': true },
            tagIds: 'tag-a',
            keyword: 'coffee',
            sortingType: ChartSortingType.Name.type
        }));

        store.initTransactionStatisticsFilter(StatisticsAnalysisType.AssetTrends, {
            chartDataType: 999,
            categoricalChartType: 999,
            categoricalChartDateType: 999,
            trendChartType: 999,
            trendChartDateType: 999,
            assetTrendsChartType: 999,
            assetTrendsChartDateType: 999,
            sortingType: 999
        });
        expect(store.transactionStatisticsFilter.chartDataType).toBe(ChartDataType.DefaultForAssetTrends.type);
        expect(store.transactionStatisticsFilter.categoricalChartType).toBe(CategoricalChartType.Default.type);
        expect(store.transactionStatisticsFilter.trendChartType).toBe(TrendChartType.Default.type);
        expect(store.transactionStatisticsFilter.assetTrendsChartType).toBe(TrendChartType.Default.type);
        expect(store.transactionStatisticsFilter.sortingType).toBe(ChartSortingType.Default.type);

        store.resetTransactionStatistics();
        expect(store.transactionCategoryStatisticsData).toBeNull();
        expect(store.transactionCategoryTrendsData).toEqual([]);
        expect(store.transactionStatisticsStateInvalid).toBe(true);
    });

    test('initializes non-custom date ranges and all-range trend sentinels from defaults', () => {
        const store = useStatisticsStore();
        store.initTransactionStatisticsFilter(StatisticsAnalysisType.TrendAnalysis, {
            chartDataType: ChartDataType.TotalIncome.type,
            categoricalChartDateType: DateRange.ThisMonth.type,
            trendChartDateType: DateRange.All.type,
            assetTrendsChartDateType: DateRange.ThisYear.type
        });
        expect(store.transactionStatisticsFilter.categoricalChartStartTime).toBeGreaterThan(0);
        expect(store.transactionStatisticsFilter.trendChartStartYearMonth).toBe('');
        expect(store.transactionStatisticsFilter.trendChartEndYearMonth).toBe('');
        expect(store.transactionStatisticsFilter.assetTrendsChartStartTime).toBeGreaterThan(0);

        mockSettingsStore.appSettings.statistics.defaultAccountFilter = { hidden: true };
        mockSettingsStore.appSettings.statistics.defaultTransactionCategoryFilter = { hidden: true };
        store.initTransactionStatisticsFilter(StatisticsAnalysisType.CategoricalAnalysis);
        expect(store.transactionStatisticsFilter.filterAccountIds).toEqual({ hidden: true });
        expect(store.transactionStatisticsFilter.filterCategoryIds).toEqual({ hidden: true });
    });

    test('updates every statistics filter and derives date bounds for non-custom ranges', () => {
        const store = useStatisticsStore();
        expect(store.updateTransactionStatisticsFilter({})).toBe(false);
        expect(store.updateTransactionStatisticsFilter({
            chartDataType: ChartDataType.IncomeByAccount.type,
            categoricalChartType: CategoricalChartType.Bar.type,
            categoricalChartDateType: DateRange.ThisMonth.type,
            categoricalChartStartTime: 1,
            categoricalChartEndTime: 2,
            trendChartType: TrendChartType.Area.type,
            trendChartDateType: DateRange.ThisYear.type,
            trendChartStartYearMonth: '2026-01',
            trendChartEndYearMonth: '2026-12',
            assetTrendsChartType: TrendChartType.Bubble.type,
            assetTrendsChartDateType: DateRange.ThisYear.type,
            assetTrendsChartStartTime: 3,
            assetTrendsChartEndTime: 4,
            filterAccountIds: { cash: true },
            filterCategoryIds: { 'income-secondary': true },
            tagIds: 'tag',
            tagFilterType: TransactionTagFilterType.NotHasAny.type,
            keyword: 'salary',
            sortingType: ChartSortingType.Name.type
        })).toBe(true);
        expect(store.transactionStatisticsFilter).toEqual(expect.objectContaining({
            chartDataType: ChartDataType.IncomeByAccount.type,
            filterAccountIds: { cash: true },
            filterCategoryIds: { 'income-secondary': true },
            keyword: 'salary'
        }));

        expect(store.updateTransactionStatisticsFilter({ trendChartDateType: DateRange.All.type })).toBe(true);
        expect(store.transactionStatisticsFilter.trendChartStartYearMonth).toBe('');
        expect(store.transactionStatisticsFilter.trendChartEndYearMonth).toBe('');
        expect(store.updateTransactionStatisticsFilter({
            categoricalChartDateType: DateRange.Custom.type,
            trendChartDateType: DateRange.Custom.type,
            assetTrendsChartDateType: DateRange.Custom.type
        })).toBe(true);
    });

    test('serializes statistics and drill-down page params through canonical builders', () => {
        const store = useStatisticsStore();
        store.initTransactionStatisticsFilter(StatisticsAnalysisType.CategoricalAnalysis, {
            chartDataType: ChartDataType.ExpenseByAccount.type,
            categoricalChartDateType: DateRange.Custom.type,
            categoricalChartStartTime: 100,
            categoricalChartEndTime: 200,
            tagIds: 'tag',
            keyword: 'coffee'
        });
        expect(store.getTransactionStatisticsPageParams(StatisticsAnalysisType.CategoricalAnalysis, 0, 4))
            .toContain('chartDataType=0');
        expect(store.getTransactionListPageParams(StatisticsAnalysisType.CategoricalAnalysis, 'cash'))
            .toContain('accountIds=cash');
    });

    test('loads all statistics resources and detects force-refresh equality', async () => {
        const store = useStatisticsStore();
        const categorical = categoricalResponse();
        const trends = trendsResponse();
        const assets = assetTrendsResponse();
        mockServices.getTransactionStatistics.mockReturnValue(success(categorical));
        await expect(store.loadCategoricalAnalysis({ force: false })).resolves.toBe(categorical);
        expect(store.transactionStatisticsStateInvalid).toBe(false);
        mockServices.getTransactionStatistics.mockReturnValue(success(categorical));
        await expect(store.loadCategoricalAnalysis({ force: true })).rejects.toEqual({
            message: 'Data is up to date', isUpToDate: true
        });

        store.transactionStatisticsFilter.trendChartDateType = DateRange.All.type;
        mockServices.getTransactionStatisticsTrends.mockReturnValue(success(trends));
        await expect(store.loadTrendAnalysis({ force: false })).resolves.toBe(trends);
        expect(mockServices.getTransactionStatisticsTrends).toHaveBeenCalledWith(expect.objectContaining({
            startYearMonth: '197001', endYearMonth: '197001'
        }));
        mockServices.getTransactionStatisticsTrends.mockReturnValue(success(trends));
        await expect(store.loadTrendAnalysis({ force: true })).rejects.toEqual({
            message: 'Data is up to date', isUpToDate: true
        });

        mockServices.getTransactionStatisticsAssetTrends.mockReturnValue(success(assets));
        await expect(store.loadAssetTrends({ force: false })).resolves.toBe(assets);
        mockServices.getTransactionStatisticsAssetTrends.mockReturnValue(success(assets));
        await expect(store.loadAssetTrends({ force: true })).rejects.toEqual({
            message: 'Data is up to date', isUpToDate: true
        });
    });

    test('all load actions reject invalid envelopes and preserve three transport error channels', async () => {
        const store = useStatisticsStore();
        const actions = [
            {
                service: mockServices.getTransactionStatistics,
                invoke: () => store.loadCategoricalAnalysis({ force: false })
            },
            {
                service: mockServices.getTransactionStatisticsTrends,
                invoke: () => store.loadTrendAnalysis({ force: false })
            },
            {
                service: mockServices.getTransactionStatisticsAssetTrends,
                invoke: () => store.loadAssetTrends({ force: false })
            }
        ];
        for (const action of actions) {
            action.service.mockReturnValue(invalidResponse());
            await expect(action.invoke()).rejects.toEqual({ message: 'Unable to retrieve transaction statistics' });
            await expectThreeErrorChannels(action.service, action.invoke);
        }
    });
});
