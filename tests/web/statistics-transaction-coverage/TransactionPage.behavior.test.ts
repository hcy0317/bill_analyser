import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const Analysis = {
    CategoricalAnalysis: 1,
    TrendAnalysis: 2,
    AssetTrends: 3
} as const;

const chartTypes = {
    Overview: 0,
    OutflowsByAccount: 1,
    ExpenseByAccount: 2,
    ExpenseByPrimaryCategory: 3,
    ExpenseBySecondaryCategory: 4,
    InflowsByAccount: 5,
    IncomeByAccount: 6,
    IncomeByPrimaryCategory: 7,
    IncomeBySecondaryCategory: 8,
    TotalOutflows: 9,
    TotalExpense: 10,
    TotalInflows: 11,
    TotalIncome: 12,
    NetCashFlow: 13,
    NetIncome: 14,
    NetWorth: 15,
    AccountTotalAssets: 16,
    AccountTotalLiabilities: 17,
    Special: 99,
    Unsupported: 500
} as const;

const mockChartDataType: any = {
    Default: { type: chartTypes.Overview },
    DefaultForAssetTrends: { type: chartTypes.AccountTotalAssets },
    ...Object.fromEntries(Object.entries(chartTypes).map(([name, type]) => [name, { type }])),
    valueOf: jest.fn((type: number) => type === chartTypes.Special
        ? { type, specialChart: true }
        : type >= chartTypes.Overview && type <= chartTypes.Unsupported
            ? { type, specialChart: false }
            : null),
    isAvailableForAnalysisType: jest.fn(() => true)
};

const mockDateRange = {
    All: { type: 0 },
    Custom: { type: 99 }
};

const mockRouter = { push: jest.fn() };
const mockDisplay = { mdAndUp: null as any };
const mockTheme = { global: { name: null as any } };
const mockRouteCallbacks: Array<(to: any) => void> = [];
const mockWatchCallbacks: Array<(newValue: any, oldValue?: any) => void> = [];
const mockTemplateBindings = jest.fn();

const mockGetDateRangeByDateType = jest.fn<(...args: any[]) => any>();
const mockGetDateTypeByDateRange = jest.fn<(...args: any[]) => number>();
const mockGetShiftedDateRange = jest.fn<(...args: any[]) => any>();
const mockGetYearMonthFirstUnixTime = jest.fn<(value: string) => number>();
const mockGetYearMonthLastUnixTime = jest.fn<(value: string) => number>();
const mockGetGregorianYearMonth = jest.fn<(value: number) => string>();
const mockGetFilterLinkUrl = jest.fn<(...args: any[]) => string>();
const mockGetTransactionItemLinkUrl = jest.fn<(...args: any[]) => string>();

let mockLastBase: any;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const query = ref({
        chartDataType: chartTypes.Overview,
        categoricalChartType: 11,
        categoricalChartDateType: 10,
        categoricalChartStartTime: 100,
        categoricalChartEndTime: 200,
        trendChartType: 21,
        trendChartDateType: 20,
        trendChartStartYearMonth: '2026-01',
        trendChartEndYearMonth: '2026-06',
        assetTrendsChartType: 31,
        assetTrendsChartDateType: 30,
        assetTrendsChartStartTime: 300,
        assetTrendsChartEndTime: 400,
        filterAccountIds: {},
        filterCategoryIds: {},
        tagIds: '',
        tagFilterType: 0,
        keyword: '',
        sortingType: 1
    });

    return {
        loading: ref(false),
        analysisType: ref(Analysis.CategoricalAnalysis as number),
        trendDateAggregationType: ref(41),
        assetTrendsDateAggregationType: ref(42),
        defaultCurrency: ref('CNY'),
        firstDayOfWeek: ref(1),
        fiscalYearStart: ref(4),
        allDateRanges: ref([{ type: 10 }]),
        allSortingTypes: ref([{ type: 1 }]),
        allTrendAnalysisDateAggregationTypes: ref([{ type: 41 }]),
        allAssetTrendsDateAggregationTypes: ref([{ type: 42 }]),
        query,
        queryChartDataCategory: computed(() => 'category'),
        queryDateType: computed(() => query.value.categoricalChartDateType),
        queryStartTime: computed(() => query.value.categoricalChartStartTime),
        queryEndTime: computed(() => query.value.categoricalChartEndTime),
        queryDateRangeName: computed(() => 'Current range'),
        queryTrendDateAggregationTypeName: computed(() => 'Monthly'),
        queryAssetTrendsDateAggregationTypeName: computed(() => 'Daily'),
        canChangeDateRange: computed(() => true),
        canShiftDateRange: computed(() => true),
        canUseCategoryFilter: computed(() => true),
        canUseTagFilter: computed(() => true),
        canUseKeywordFilter: computed(() => true),
        showAmountInChart: computed(() => true),
        totalAmountName: computed(() => 'Total'),
        showPercentInCategoricalChart: computed(() => true),
        showTotalAmountInTrendsChart: computed(() => true),
        showStackedInTrendsChart: computed(() => false),
        translateNameInTrendsChart: computed(() => false),
        categoricalOverviewAnalysisData: ref({ totalAmountCents: 0 }),
        categoricalAnalysisData: ref(null as any),
        trendsAnalysisData: ref(null as any),
        assetTrendsData: ref(null as any),
        canShowCustomDateRange: computed(() => true),
        getTransactionCategoricalAnalysisDataItemDisplayColor: jest.fn(() => '#123456'),
        getDisplayAmount: jest.fn((amount: number) => `display:${amount}`)
    };
}

const mockAccountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<any>>()
};

const mockCategoryStore = {
    allTransactionCategoriesMap: {} as Record<string, any>,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<any>>()
};

const mockStatisticsStore = {
    transactionStatisticsStateInvalid: false,
    initTransactionStatisticsFilter: jest.fn((analysisType: number, filter?: Record<string, any>) => {
        if (!mockLastBase || !filter) return;
        Object.entries(filter).forEach(([key, value]) => {
            if (value !== undefined) mockLastBase.query.value[key] = value;
        });
    }),
    updateTransactionStatisticsFilter: jest.fn((filter: Record<string, any>) => {
        if (!mockLastBase) return false;
        let changed = false;
        Object.entries(filter).forEach(([key, value]) => {
            if (mockLastBase.query.value[key] !== value) {
                mockLastBase.query.value[key] = value;
                changed = true;
            }
        });
        return changed;
    }),
    updateTransactionStatisticsInvalidState: jest.fn(),
    loadCategoricalAnalysis: jest.fn<(...args: any[]) => Promise<any>>(),
    loadTrendAnalysis: jest.fn<(...args: any[]) => Promise<any>>(),
    loadAssetTrends: jest.fn<(...args: any[]) => Promise<any>>(),
    getTransactionStatisticsPageParams: jest.fn(() => 'filter=params'),
    getTransactionListPageParams: jest.fn(() => 'list=params')
};

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(null),
        watch: (_source: any, callback: (newValue: any, oldValue?: any) => void) => {
            mockWatchCallbacks.push(callback);
            return jest.fn();
        }
    };
});

jest.mock('vue-router', () => ({
    useRouter: () => mockRouter,
    onBeforeRouteUpdate: (callback: (to: any) => void) => mockRouteCallbacks.push(callback)
}));

jest.mock('vuetify', () => {
    const { ref } = jest.requireActual('vue') as any;
    mockDisplay.mdAndUp ??= ref(true);
    mockTheme.global.name ??= ref('light');
    return {
        useDisplay: () => mockDisplay,
        useTheme: () => mockTheme
    };
});

jest.mock('@/lib/vue_external_template.ts', () => ({
    useExternalTemplateBindings: (...args: any[]) => mockTemplateBindings(...args)
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `t:${key}`,
        getAllCategoricalChartTypes: (includeSpecial: boolean) => [{ type: 1, includeSpecial }],
        getAllTrendChartTypes: () => [{ type: 2 }],
        formatAmountToWesternArabicNumeralsWithoutDigitGrouping: (value: number) => `amount:${value}`,
        formatPercentToLocalizedNumerals: (value: number) => `percent:${value}`
    })
}));

jest.mock('@/views/base/statistics/StatisticsTransactionPageBase.ts', () => ({
    useStatisticsTransactionPageBase: () => {
        mockLastBase = createBase();
        return mockLastBase;
    }
}));

jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));

jest.mock('@/core/datetime.ts', () => ({
    DateRangeScene: {
        Normal: 'normal',
        TrendAnalysis: 'trend',
        AssetTrends: 'assets'
    },
    DateRange: mockDateRange
}));

jest.mock('@/core/theme.ts', () => ({
    isDarkApplicationTheme: (name: string) => name === 'dark'
}));

jest.mock('@/core/statistics.ts', () => ({
    ChartDataAggregationType: { Total: 1 },
    StatisticsAnalysisType: Analysis,
    CategoricalChartType: { Pie: 1 },
    ChartDataType: mockChartDataType,
    ChartSortingType: { Amount: { type: 1 }, Name: { type: 2 } },
    ChartDateAggregationType: { Default: { type: 30 } }
}));

jest.mock('@/lib/common.ts', () => ({
    isDefined: (value: any) => value !== undefined && value !== null,
    isString: (value: any) => typeof value === 'string',
    isNumber: (value: any) => typeof value === 'number',
    arrayItemToObjectField: (items: string[], value: any) => Object.fromEntries(items.map(item => [item, value]))
}));

jest.mock('@/lib/datetime.ts', () => ({
    getGregorianCalendarYearAndMonthFromUnixTime: (value: number) => mockGetGregorianYearMonth(value),
    getYearMonthFirstUnixTime: (value: string) => mockGetYearMonthFirstUnixTime(value),
    getYearMonthLastUnixTime: (value: string) => mockGetYearMonthLastUnixTime(value),
    getShiftedDateRangeAndDateType: (...args: any[]) => mockGetShiftedDateRange(...args),
    getDateTypeByDateRange: (...args: any[]) => mockGetDateTypeByDateRange(...args),
    getDateRangeByDateType: (...args: any[]) => mockGetDateRangeByDateType(...args)
}));

jest.mock('@/views/desktop/statistics/transaction/pageLinks.ts', () => ({
    getFilterLinkUrl: (...args: any[]) => mockGetFilterLinkUrl(...args),
    getTransactionItemLinkUrl: (...args: any[]) => mockGetTransactionItemLinkUrl(...args)
}));

jest.mock('@mdi/js', () => new Proxy({}, { get: (_target, key) => String(key) }));

for (const componentPath of [
    '@/components/desktop/SnackBar.vue',
    '@/components/desktop/TrendsChart.vue',
    '@/views/desktop/common/cards/AccountFilterSettingsCard.vue',
    '@/views/desktop/common/cards/CategoryFilterSettingsCard.vue',
    '@/views/desktop/common/cards/TransactionTagFilterSettingsCard.vue',
    '@/views/desktop/statistics/transaction/dialogs/ExportDialog.vue'
]) {
    jest.mock(componentPath, () => ({ __esModule: true, default: { name: 'StatisticsTransactionCoverageStub' } }));
}

import TransactionPage from '@/views/desktop/statistics/TransactionPage.vue';

function setupPage(props: Record<string, any> = {
    initAnalysisType: String(Analysis.CategoricalAnalysis),
    initChartDateType: '10'
}): any {
    return (TransactionPage as any).setup(props, { expose: jest.fn() });
}

async function flushPromises(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) {
        await Promise.resolve();
    }
}

function installUi(bindings: any): any {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const monthly = { exportData: jest.fn(() => ({ headers: ['month'], data: [['100']] })) };
    const daily = { exportData: jest.fn(() => ({ headers: ['day'], data: [['200']] })) };
    const exporter = { open: jest.fn() };
    bindings.snackbar.value = snackbar;
    bindings.monthlyTrendsChart.value = monthly;
    bindings.dailyTrendsChart.value = daily;
    bindings.exportDialog.value = exporter;
    return { snackbar, monthly, daily, exporter };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockRouteCallbacks.length = 0;
    mockWatchCallbacks.length = 0;
    mockStatisticsStore.transactionStatisticsStateInvalid = false;
    mockCategoryStore.allTransactionCategoriesMap = {};
    mockDisplay.mdAndUp.value = true;
    mockTheme.global.name.value = 'light';
    mockChartDataType.isAvailableForAnalysisType.mockReturnValue(true);
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockStatisticsStore.loadCategoricalAnalysis.mockResolvedValue(undefined);
    mockStatisticsStore.loadTrendAnalysis.mockResolvedValue(undefined);
    mockStatisticsStore.loadAssetTrends.mockResolvedValue(undefined);
    mockGetFilterLinkUrl.mockImplementation((_store, analysisType, trend, assets) => `filter:${analysisType}:${trend}:${assets}`);
    mockGetTransactionItemLinkUrl.mockImplementation((_store, analysisType, itemId, range) => `item:${analysisType}:${itemId}:${range?.dateType ?? ''}`);
    mockGetDateRangeByDateType.mockImplementation((dateType: number) => dateType < 0
        ? null
        : { dateType, minTime: 1_000 + dateType, maxTime: 2_000 + dateType });
    mockGetDateTypeByDateRange.mockReturnValue(77);
    mockGetShiftedDateRange.mockReturnValue({ dateType: 88, minTime: 3_000, maxTime: 4_000 });
    mockGetYearMonthFirstUnixTime.mockImplementation(value => value === '2026-01' ? 100 : 300);
    mockGetYearMonthLastUnixTime.mockImplementation(value => value === '2026-06' ? 200 : 400);
    mockGetGregorianYearMonth.mockImplementation(value => value === 3_000 ? '2026-03' : value === 4_000 ? '2026-04' : `ym:${value}`);
});

describe('desktop statistics TransactionPage production behavior', () => {
    test('exposes computed chart state for categorical, trend, asset, and fallback analysis', async () => {
        const bindings = setupPage();
        const ui = installUi(bindings);
        await flushPromises();

        expect(bindings.isDarkMode.value).toBe(false);
        mockTheme.global.name.value = 'dark';
        expect(bindings.isDarkMode.value).toBe(true);

        expect(bindings.statisticsDataHasData.value).toBe(false);
        bindings.categoricalAnalysisData.value = { items: [{ id: 'food' }] };
        expect(bindings.statisticsDataHasData.value).toBe(true);
        expect(bindings.allChartTypes.value).toEqual([{ type: 1, includeSpecial: true }]);
        expect(bindings.queryChartType.value).toBe(11);

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.trendsAnalysisData.value = { items: [{ id: 'month' }] };
        bindings.monthlyTrendsChart.value = null;
        expect(bindings.statisticsDataHasData.value).toBe(false);
        bindings.monthlyTrendsChart.value = ui.monthly;
        expect(bindings.statisticsDataHasData.value).toBe(true);
        expect(bindings.allChartTypes.value).toEqual([{ type: 2 }]);
        expect(bindings.queryChartType.value).toBe(21);

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.assetTrendsData.value = { items: [{ id: 'day' }] };
        bindings.dailyTrendsChart.value = null;
        expect(bindings.statisticsDataHasData.value).toBe(false);
        bindings.dailyTrendsChart.value = ui.daily;
        expect(bindings.statisticsDataHasData.value).toBe(true);
        expect(bindings.allChartTypes.value).toEqual([{ type: 2 }]);
        expect(bindings.queryChartType.value).toBe(31);

        bindings.analysisType.value = 999;
        expect(bindings.statisticsDataHasData.value).toBe(false);
        expect(bindings.allChartTypes.value).toEqual([]);
        expect(bindings.queryChartType.value).toBeUndefined();

        bindings.query.value.chartDataType = chartTypes.Special;
        expect(bindings.isQuerySpecialChartType.value).toBe(true);
        bindings.query.value.chartDataType = -999;
        expect(bindings.isQuerySpecialChartType.value).toBe(false);

        for (const type of [
            chartTypes.OutflowsByAccount,
            chartTypes.ExpenseByAccount,
            chartTypes.ExpenseByPrimaryCategory,
            chartTypes.ExpenseBySecondaryCategory
        ]) {
            bindings.query.value.chartDataType = type;
            expect(bindings.statisticsTextColor.value).toBe('text-expense');
        }
        for (const type of [
            chartTypes.InflowsByAccount,
            chartTypes.IncomeByAccount,
            chartTypes.IncomeByPrimaryCategory,
            chartTypes.IncomeBySecondaryCategory
        ]) {
            bindings.query.value.chartDataType = type;
            expect(bindings.statisticsTextColor.value).toBe('text-income');
        }
        bindings.query.value.chartDataType = chartTypes.Overview;
        expect(bindings.statisticsTextColor.value).toBe('text-default');
    });

    test('updates analysis, chart, sorting, and aggregation selectors through navigable filters', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();

        bindings.queryAnalysisType.value = Analysis.CategoricalAnalysis;
        expect(mockRouter.push).not.toHaveBeenCalled();

        mockChartDataType.isAvailableForAnalysisType.mockReturnValue(false);
        bindings.queryAnalysisType.value = Analysis.TrendAnalysis;
        expect(bindings.query.value.chartDataType).toBe(chartTypes.Overview);
        expect(bindings.trendDateAggregationType.value).toBe(30);
        expect(mockStatisticsStore.updateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);

        bindings.query.value.chartDataType = chartTypes.Unsupported;
        bindings.queryAnalysisType.value = Analysis.AssetTrends;
        expect(bindings.query.value.chartDataType).toBe(chartTypes.AccountTotalAssets);
        expect(bindings.assetTrendsDateAggregationType.value).toBe(30);

        mockChartDataType.isAvailableForAnalysisType.mockReturnValue(true);
        bindings.queryAnalysisType.value = Analysis.CategoricalAnalysis;
        expect(bindings.analysisType.value).toBe(Analysis.CategoricalAnalysis);

        bindings.queryChartType.value = 12;
        expect(bindings.query.value.categoricalChartType).toBe(12);
        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.queryChartType.value = 22;
        expect(bindings.query.value.trendChartType).toBe(22);
        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.queryChartType.value = 32;
        expect(bindings.query.value.assetTrendsChartType).toBe(32);
        bindings.analysisType.value = 999;
        bindings.setChartType(44);

        bindings.queryChartDataType.value = chartTypes.TotalIncome;
        expect(bindings.query.value.chartDataType).toBe(chartTypes.TotalIncome);
        bindings.queryChartDataType.value = chartTypes.TotalIncome;

        bindings.querySortingType.value = 0;
        bindings.querySortingType.value = 3;
        bindings.querySortingType.value = 2;
        expect(bindings.query.value.sortingType).toBe(2);
        bindings.querySortingType.value = 2;

        bindings.setTrendDateAggregationType(41);
        bindings.setTrendDateAggregationType(45);
        expect(bindings.trendDateAggregationType.value).toBe(45);
        bindings.setAssetTrendsDateAggregationType(42);
        bindings.setAssetTrendsDateAggregationType(46);
        expect(bindings.assetTrendsDateAggregationType.value).toBe(46);
        expect(mockRouter.push).toHaveBeenCalled();
    });

    test('initializes default and categorical routes, including cached and reload decisions', async () => {
        const defaultBindings = setupPage({});
        const defaultUi = installUi(defaultBindings);
        await flushPromises();
        expect(defaultBindings.analysisType.value).toBe(Analysis.CategoricalAnalysis);
        expect(mockStatisticsStore.initTransactionStatisticsFilter).toHaveBeenCalledWith(Analysis.CategoricalAnalysis);
        expect(mockStatisticsStore.loadCategoricalAnalysis).toHaveBeenCalledWith({ force: false });
        expect(defaultBindings.loading.value).toBe(false);
        expect(defaultBindings.initing.value).toBe(false);
        expect(defaultUi.snackbar.showError).not.toHaveBeenCalled();

        jest.clearAllMocks();
        const cached = setupPage({
            initAnalysisType: String(Analysis.CategoricalAnalysis),
            initChartDataType: String(chartTypes.ExpenseByAccount),
            initChartType: '12',
            initChartDateType: '10',
            initFilterAccountIds: 'a,b',
            initFilterCategoryIds: 'c,d',
            initTagIds: 'tag-1',
            initTagFilterType: '-1',
            initKeyword: 'coffee',
            initSortingType: '2'
        });
        await flushPromises();
        expect(cached.filterKeyword.value).toBe('coffee');
        expect(cached.query.value.filterAccountIds).toEqual({ a: true, b: true });
        expect(cached.query.value.filterCategoryIds).toEqual({ c: true, d: true });
        expect(cached.query.value.tagFilterType).toBe(0);
        expect(cached.loading.value).toBe(false);
        expect(mockStatisticsStore.loadCategoricalAnalysis).not.toHaveBeenCalled();

        mockStatisticsStore.transactionStatisticsStateInvalid = true;
        cached.init({
            initAnalysisType: String(Analysis.CategoricalAnalysis),
            initChartDateType: String(mockDateRange.Custom.type),
            initStartTime: '123',
            initEndTime: '456',
            initTagFilterType: '3'
        });
        await flushPromises();
        expect(mockStatisticsStore.loadCategoricalAnalysis).toHaveBeenCalled();
        expect(cached.query.value.tagFilterType).toBe(3);
    });

    test('initializes trend and asset routes and reports unprocessed load failures', async () => {
        const trend = setupPage({
            initAnalysisType: String(Analysis.TrendAnalysis),
            initChartType: '22',
            initChartDateType: String(mockDateRange.Custom.type),
            initStartTime: '2026-02',
            initEndTime: '2026-05',
            initTrendDateAggregationType: '55'
        });
        const trendUi = installUi(trend);
        await flushPromises();
        expect(trend.analysisType.value).toBe(Analysis.TrendAnalysis);
        expect(trend.trendDateAggregationType.value).toBe(55);
        expect(mockStatisticsStore.loadTrendAnalysis).toHaveBeenCalledWith({ force: false });
        expect(trendUi.snackbar.showError).not.toHaveBeenCalled();

        mockStatisticsStore.loadAssetTrends.mockRejectedValueOnce({ message: 'offline', processed: false });
        const assets = setupPage({
            initAnalysisType: String(Analysis.AssetTrends),
            initChartType: '32',
            initChartDateType: String(mockDateRange.Custom.type),
            initStartTime: '500',
            initEndTime: '800',
            initAssetTrendsDateAggregationType: '66'
        });
        const assetUi = installUi(assets);
        await flushPromises();
        expect(assets.analysisType.value).toBe(Analysis.AssetTrends);
        expect(assets.assetTrendsDateAggregationType.value).toBe(66);
        expect(assetUi.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'offline' }));

        mockStatisticsStore.loadAssetTrends.mockRejectedValueOnce({ message: 'handled', processed: true });
        assets.init({
            initAnalysisType: String(Analysis.AssetTrends),
            initChartDateType: '31'
        });
        await flushPromises();
        expect(assetUi.snackbar.showError).toHaveBeenCalledTimes(1);
    });

    test('handles unsupported initialization after master-data loading', async () => {
        const bindings = setupPage();
        const ui = installUi(bindings);
        await flushPromises();
        bindings.analysisType.value = 999;
        mockStatisticsStore.transactionStatisticsStateInvalid = true;
        bindings.init({ initAnalysisType: '999' });
        await flushPromises();
        expect(ui.snackbar.showError).toHaveBeenCalledWith('An error occurred');
    });

    test('reloads all supported statistics and account-backed data with success feedback', async () => {
        const bindings = setupPage();
        const ui = installUi(bindings);
        await flushPromises();

        for (const type of [
            chartTypes.Overview,
            chartTypes.OutflowsByAccount,
            chartTypes.ExpenseByAccount,
            chartTypes.ExpenseByPrimaryCategory,
            chartTypes.ExpenseBySecondaryCategory,
            chartTypes.InflowsByAccount,
            chartTypes.IncomeByAccount,
            chartTypes.IncomeByPrimaryCategory,
            chartTypes.IncomeBySecondaryCategory,
            chartTypes.TotalOutflows,
            chartTypes.TotalExpense,
            chartTypes.TotalInflows,
            chartTypes.TotalIncome,
            chartTypes.NetCashFlow,
            chartTypes.NetIncome,
            chartTypes.NetWorth
        ]) {
            bindings.query.value.chartDataType = type;
            bindings.analysisType.value = Analysis.CategoricalAnalysis;
            await bindings.reload(type === chartTypes.Overview);
            bindings.analysisType.value = Analysis.TrendAnalysis;
            await bindings.reload(false);
            bindings.analysisType.value = Analysis.AssetTrends;
            await bindings.reload(false);
        }
        expect(mockStatisticsStore.loadCategoricalAnalysis).toHaveBeenCalled();
        expect(mockStatisticsStore.loadTrendAnalysis).toHaveBeenCalled();
        expect(mockStatisticsStore.loadAssetTrends).toHaveBeenCalled();
        expect(ui.snackbar.showMessage).toHaveBeenCalledWith('Data has been updated');

        bindings.query.value.chartDataType = chartTypes.AccountTotalAssets;
        bindings.analysisType.value = Analysis.CategoricalAnalysis;
        await bindings.reload(true);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: true });
        bindings.analysisType.value = Analysis.AssetTrends;
        await bindings.reload(false);
        bindings.analysisType.value = Analysis.TrendAnalysis;
        expect(bindings.reload(false)).toBeNull();

        bindings.query.value.chartDataType = chartTypes.Unsupported;
        bindings.analysisType.value = Analysis.CategoricalAnalysis;
        expect(bindings.reload(false)).toBeNull();
    });

    test('surfaces reload failures only when they have not already been processed', async () => {
        const bindings = setupPage();
        const ui = installUi(bindings);
        await flushPromises();
        bindings.query.value.chartDataType = chartTypes.Overview;

        mockStatisticsStore.loadCategoricalAnalysis.mockRejectedValueOnce({ message: 'network', processed: false });
        await expect(bindings.reload(true)).rejects.toEqual(expect.objectContaining({ message: 'network' }));
        await flushPromises();
        expect(ui.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'network' }));

        mockStatisticsStore.loadCategoricalAnalysis.mockRejectedValueOnce({ message: 'handled', processed: true });
        await expect(bindings.reload(false)).rejects.toEqual(expect.objectContaining({ message: 'handled' }));
        await flushPromises();
        expect(ui.snackbar.showError).toHaveBeenCalledTimes(1);
    });

    test('applies preset date filters for every analysis and opens custom range dialogs', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();

        bindings.setDateFilter(mockDateRange.Custom.type);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);
        bindings.showCustomDateRangeDialog.value = false;
        bindings.setDateFilter(10);
        expect(mockRouter.push).not.toHaveBeenCalled();
        bindings.setDateFilter(-1);
        expect(mockRouter.push).not.toHaveBeenCalled();
        bindings.setDateFilter(12);
        expect(bindings.query.value.categoricalChartDateType).toBe(12);

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.setDateFilter(mockDateRange.Custom.type);
        expect(bindings.showCustomMonthRangeDialog.value).toBe(true);
        bindings.showCustomMonthRangeDialog.value = false;
        bindings.setDateFilter(20);
        bindings.setDateFilter(mockDateRange.All.type);
        expect(bindings.query.value.trendChartStartYearMonth).toBe('');
        expect(bindings.query.value.trendChartEndYearMonth).toBe('');
        bindings.setDateFilter(23);
        expect(bindings.query.value.trendChartStartYearMonth).toBe('ym:1023');

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.setDateFilter(mockDateRange.Custom.type);
        expect(bindings.showCustomDateRangeDialog.value).toBe(true);
        bindings.showCustomDateRangeDialog.value = false;
        bindings.setDateFilter(30);
        bindings.setDateFilter(33);
        expect(bindings.query.value.assetTrendsChartDateType).toBe(33);

        bindings.analysisType.value = 999;
        bindings.setDateFilter(34);
        expect(mockRouter.push).toHaveBeenCalled();
    });

    test('accepts valid custom date ranges and ignores incomplete or mismatched input', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();

        bindings.setCustomDateFilter(0, 2);
        bindings.setCustomDateFilter(1, 0);
        bindings.setCustomDateFilter('2026-01', '2026-02');
        expect(mockRouter.push).not.toHaveBeenCalled();

        bindings.showCustomDateRangeDialog.value = true;
        bindings.setCustomDateFilter(1_111, 2_222);
        expect(bindings.query.value.categoricalChartDateType).toBe(77);
        expect(bindings.showCustomDateRangeDialog.value).toBe(false);

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.showCustomMonthRangeDialog.value = true;
        bindings.setCustomDateFilter('2026-02', '2026-05');
        expect(bindings.query.value.trendChartStartYearMonth).toBe('2026-02');
        expect(bindings.showCustomMonthRangeDialog.value).toBe(false);
        bindings.setCustomDateFilter(100, 200);

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.showCustomDateRangeDialog.value = true;
        bindings.setCustomDateFilter(3_333, 4_444);
        expect(bindings.query.value.assetTrendsChartStartTime).toBe(3_333);
        expect(bindings.showCustomDateRangeDialog.value).toBe(false);
        bindings.setCustomDateFilter('2026-01', '2026-02');

        bindings.setCustomDateFilter(3_333, 4_444);
        expect(mockRouter.push).toHaveBeenCalled();
    });

    test('shifts categorical, trend, and asset ranges while respecting all-time ranges', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();

        bindings.query.value.categoricalChartDateType = mockDateRange.All.type;
        bindings.shiftDateRange(1);
        expect(mockGetShiftedDateRange).not.toHaveBeenCalled();
        bindings.query.value.categoricalChartDateType = 10;
        bindings.shiftDateRange(-1);
        expect(bindings.query.value.categoricalChartStartTime).toBe(3_000);

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.query.value.trendChartDateType = mockDateRange.All.type;
        bindings.shiftDateRange(1);
        bindings.query.value.trendChartDateType = 20;
        bindings.shiftDateRange(1);
        expect(bindings.query.value.trendChartStartYearMonth).toBe('2026-03');
        expect(bindings.query.value.trendChartEndYearMonth).toBe('2026-04');

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.query.value.assetTrendsChartDateType = mockDateRange.All.type;
        bindings.shiftDateRange(1);
        bindings.query.value.assetTrendsChartDateType = 30;
        bindings.shiftDateRange(1);
        expect(bindings.query.value.assetTrendsChartStartTime).toBe(3_000);

        bindings.analysisType.value = 999;
        bindings.shiftDateRange(1);
        expect(mockRouter.push).toHaveBeenCalled();
    });

    test('closes account, category, and tag dialogs and navigates only for changed filters', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();

        bindings.showFilterAccountDialog.value = true;
        bindings.setAccountFilter(false);
        expect(bindings.showFilterAccountDialog.value).toBe(false);
        bindings.setAccountFilter(true);

        bindings.showFilterCategoryDialog.value = true;
        bindings.setCategoryFilter(false);
        expect(bindings.showFilterCategoryDialog.value).toBe(false);
        bindings.setCategoryFilter(true);

        bindings.showFilterTagDialog.value = true;
        bindings.setTagFilter(false);
        expect(bindings.showFilterTagDialog.value).toBe(false);
        bindings.setTagFilter(true);
        expect(mockRouter.push).toHaveBeenCalledTimes(3);
    });

    test('applies keyword filters to supported analyses and ignores unchanged or asset keywords', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();

        bindings.setKeywordFilter('');
        expect(mockRouter.push).not.toHaveBeenCalled();
        bindings.setKeywordFilter('coffee');
        expect(bindings.query.value.keyword).toBe('coffee');

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.setKeywordFilter('rent');
        expect(bindings.query.value.keyword).toBe('rent');

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.setKeywordFilter('ignored');
        expect(bindings.query.value.keyword).toBe('rent');

        bindings.analysisType.value = 999;
        bindings.setKeywordFilter('unknown');
        expect(mockRouter.push).toHaveBeenCalledTimes(2);
    });

    test('exports visible categorical values and chart-provided trend and asset datasets', async () => {
        const bindings = setupPage();
        const ui = installUi(bindings);
        await flushPromises();

        bindings.categoricalAnalysisData.value = {
            items: [
                { name: 'Food', totalAmountCents: 12_345, percent: 12.34567, hidden: false },
                { name: 'Hidden', totalAmountCents: 99, percent: 1, hidden: true }
            ]
        };
        bindings.exportResults();
        expect(ui.exporter.open).toHaveBeenLastCalledWith({
            headers: ['t:Name', 't:Amount (CNY)', 't:Proportion (%)'],
            data: [['Food', 'amount:12345', '12.3457']]
        });

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.trendsAnalysisData.value = { items: [{ id: 'month' }] };
        bindings.exportResults();
        expect(ui.exporter.open).toHaveBeenLastCalledWith({ headers: ['month'], data: [['100']] });
        ui.monthly.exportData.mockReturnValueOnce({ headers: undefined, data: undefined });
        bindings.exportResults();
        expect(ui.exporter.open).toHaveBeenLastCalledWith({ headers: [], data: [] });

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.assetTrendsData.value = { items: [{ id: 'day' }] };
        bindings.exportResults();
        expect(ui.exporter.open).toHaveBeenLastCalledWith({ headers: ['day'], data: [['200']] });
        ui.daily.exportData.mockReturnValueOnce({ headers: undefined, data: undefined });
        bindings.exportResults();
        expect(ui.exporter.open).toHaveBeenLastCalledWith({ headers: [], data: [] });

        bindings.dailyTrendsChart.value = null;
        bindings.exportResults();
        bindings.assetTrendsData.value = null;
        bindings.exportResults();
    });

    test('navigates from sankey, pie, and trend items using parent-child category semantics', async () => {
        const bindings = setupPage();
        await flushPromises();
        mockRouter.push.mockClear();
        mockCategoryStore.allTransactionCategoriesMap = {
            parent: { id: 'parent', parentId: '' },
            child: { id: 'child', parentId: 'parent' },
            peer: { id: 'peer', parentId: '' }
        };

        bindings.onClickSankeyChartItem('category', 'child', 'category', 'parent');
        expect(mockGetTransactionItemLinkUrl).toHaveBeenLastCalledWith(
            mockStatisticsStore, Analysis.CategoricalAnalysis, 'category:child'
        );
        bindings.onClickSankeyChartItem('category', 'parent', 'category', 'child');
        expect(mockGetTransactionItemLinkUrl).toHaveBeenLastCalledWith(
            mockStatisticsStore, Analysis.CategoricalAnalysis, 'category:child'
        );
        bindings.onClickSankeyChartItem('category', 'parent', 'category', 'peer');
        expect(mockGetTransactionItemLinkUrl).toHaveBeenLastCalledWith(
            mockStatisticsStore, Analysis.CategoricalAnalysis, 'category:parent-category:peer'
        );
        bindings.onClickSankeyChartItem('account', 'cash');
        expect(mockGetTransactionItemLinkUrl).toHaveBeenLastCalledWith(
            mockStatisticsStore, Analysis.CategoricalAnalysis, 'account:cash'
        );

        bindings.onClickPieChartItem({ id: 'pie-food' });
        const dateRange = { dateType: 10, minTime: 100, maxTime: 200 };
        bindings.onClickTrendChartItem({ itemId: 'trend-food', dateRange });
        expect(mockGetTransactionItemLinkUrl).toHaveBeenLastCalledWith(
            mockStatisticsStore, Analysis.CategoricalAnalysis, 'trend-food', dateRange
        );
        expect(mockRouter.push).toHaveBeenCalledTimes(6);
    });

    test('reports date errors, responds to route updates, and synchronizes responsive navigation', async () => {
        const bindings = setupPage();
        const ui = installUi(bindings);
        await flushPromises();

        bindings.onShowDateRangeError('Invalid range');
        expect(ui.snackbar.showError).toHaveBeenCalledWith('Invalid range');
        bindings.snackbar.value = null;
        bindings.onShowDateRangeError('Ignored without snackbar');

        expect(mockRouteCallbacks).toHaveLength(1);
        mockRouteCallbacks[0]!({
            query: {
                analysisType: String(Analysis.TrendAnalysis),
                chartDataType: String(chartTypes.TotalExpense),
                chartType: '24',
                chartDateType: '25',
                startTime: '2026-02',
                endTime: '2026-03',
                filterAccountIds: 'cash',
                filterCategoryIds: 'food',
                tagIds: 'tag-2',
                tagFilterType: '1',
                keyword: 'lunch',
                sortingType: '2',
                trendDateAggregationType: '52',
                assetTrendsDateAggregationType: '62'
            }
        });
        await flushPromises();
        expect(bindings.analysisType.value).toBe(Analysis.TrendAnalysis);
        expect(bindings.filterKeyword.value).toBe('lunch');
        mockRouteCallbacks[0]!({ query: {} });
        await flushPromises();
        expect(bindings.analysisType.value).toBe(Analysis.CategoricalAnalysis);
        mockRouteCallbacks[0]!({ query: null });
        await flushPromises();
        expect(bindings.analysisType.value).toBe(Analysis.CategoricalAnalysis);

        expect(mockWatchCallbacks).toHaveLength(1);
        bindings.showNav.value = false;
        mockWatchCallbacks[0]!(false, true);
        expect(bindings.alwaysShowNav.value).toBe(false);
        expect(bindings.showNav.value).toBe(false);
        mockWatchCallbacks[0]!(true, false);
        expect(bindings.alwaysShowNav.value).toBe(true);
        expect(bindings.showNav.value).toBe(true);
        bindings.showNav.value = true;
        mockWatchCallbacks[0]!(false, true);
        expect(bindings.showNav.value).toBe(true);
        expect(mockTemplateBindings).toHaveBeenCalled();
    });
});
