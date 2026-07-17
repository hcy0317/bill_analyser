import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const Analysis = {
    CategoricalAnalysis: 1,
    TrendAnalysis: 2,
    AssetTrends: 3
} as const;

const chartType = {
    Default: 0,
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
    Unsupported: 99
} as const;

const dateRange = {
    All: { type: 0 },
    Custom: { type: 99 }
};

const showPrompt = jest.fn<(...args: any[]) => void>();
const showToast = jest.fn<(...args: any[]) => void>();
const routeBackOnError = jest.fn<(...args: any[]) => void>();
const scrollToSelectedItem = jest.fn<(...args: any[]) => void>();
const getDateRangeByDateType = jest.fn<(...args: any[]) => any>();
const getDateTypeByDateRange = jest.fn<(...args: any[]) => number>();
const getShiftedDateRangeAndDateType = jest.fn<(...args: any[]) => any>();
const getYearMonthFirstUnixTime = jest.fn<(value: string) => number>();
const getYearMonthLastUnixTime = jest.fn<(value: string) => number>();
const getGregorianCalendarYearAndMonthFromUnixTime = jest.fn<(value: number) => string>();

const chartDataType: any = {
    Default: { type: chartType.Default },
    OutflowsByAccount: { type: chartType.OutflowsByAccount },
    ExpenseByAccount: { type: chartType.ExpenseByAccount },
    ExpenseByPrimaryCategory: { type: chartType.ExpenseByPrimaryCategory },
    ExpenseBySecondaryCategory: { type: chartType.ExpenseBySecondaryCategory },
    InflowsByAccount: { type: chartType.InflowsByAccount },
    IncomeByAccount: { type: chartType.IncomeByAccount },
    IncomeByPrimaryCategory: { type: chartType.IncomeByPrimaryCategory },
    IncomeBySecondaryCategory: { type: chartType.IncomeBySecondaryCategory },
    TotalOutflows: { type: chartType.TotalOutflows },
    TotalExpense: { type: chartType.TotalExpense },
    TotalInflows: { type: chartType.TotalInflows },
    TotalIncome: { type: chartType.TotalIncome },
    NetCashFlow: { type: chartType.NetCashFlow },
    NetIncome: { type: chartType.NetIncome },
    NetWorth: { type: chartType.NetWorth },
    AccountTotalAssets: { type: chartType.AccountTotalAssets },
    AccountTotalLiabilities: { type: chartType.AccountTotalLiabilities },
    values: jest.fn((analysis: number) => [{ type: analysis, name: `analysis-${analysis}` }]),
    isAvailableForAnalysisType: jest.fn(() => true)
};

let nextAnalysisType: number = Analysis.CategoricalAnalysis;
let lastBase: any;

function createBase(): any {
    const { computed, ref } = jest.requireActual('vue') as any;
    const query = ref({
        chartDataType: chartType.OutflowsByAccount,
        categoricalChartType: 1,
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
        sortingType: 1,
        keyword: ''
    });

    return {
        loading: ref(true),
        analysisType: ref(nextAnalysisType),
        trendDateAggregationType: ref(41),
        assetTrendsDateAggregationType: ref(42),
        defaultCurrency: ref('CNY'),
        firstDayOfWeek: ref(1),
        fiscalYearStart: ref(4),
        allDateRanges: ref([{ type: 10, displayName: 'Current' }]),
        allSortingTypes: ref([{ type: 1, displayName: 'Amount' }, { type: 2, displayName: 'Name' }]),
        allTrendAnalysisDateAggregationTypes: ref([{ type: 41, displayName: 'Month' }]),
        allAssetTrendsDateAggregationTypes: ref([{ type: 42, displayName: 'Day' }]),
        query,
        queryChartDataCategory: computed(() => 'category'),
        queryDateType: computed(() => query.value.categoricalChartDateType),
        queryStartTime: computed(() => query.value.categoricalChartStartTime),
        queryEndTime: computed(() => query.value.categoricalChartEndTime),
        queryDateRangeName: computed(() => 'Current range'),
        queryChartDataTypeName: computed(() => 'Expenses'),
        querySortingTypeName: computed(() => 'Amount'),
        queryTrendDateAggregationTypeName: computed(() => 'Monthly'),
        queryAssetTrendsDateAggregationTypeName: computed(() => 'Daily'),
        isQueryDateRangeChanged: computed(() => true),
        canChangeDateRange: computed(() => true),
        canShiftDateRange: computed(() => true),
        canUseCategoryFilter: computed(() => true),
        canUseTagFilter: computed(() => true),
        canUseKeywordFilter: computed(() => true),
        showAmountInChart: computed(() => true),
        totalAmountName: computed(() => 'Total'),
        showPercentInCategoricalChart: computed(() => true),
        showStackedInTrendsChart: computed(() => false),
        translateNameInTrendsChart: computed(() => false),
        categoricalAnalysisData: ref({ totalAmountCents: 12_345, items: [] } as any),
        trendsAnalysisData: ref({ items: [] } as any),
        assetTrendsData: ref({ items: [] } as any),
        canShowCustomDateRange: computed(() => true),
        getTransactionCategoricalAnalysisDataItemDisplayColor: jest.fn(() => '#123456'),
        getDisplayAmount: jest.fn((value: number, currency: string) => `${value}:${currency}`)
    };
}

const accountsStore = {
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<unknown>>()
};

const categoriesStore = {
    loadAllCategories: jest.fn<(...args: any[]) => Promise<unknown>>()
};

const statisticsStore = {
    transactionStatisticsStateInvalid: false,
    initTransactionStatisticsFilter: jest.fn<(...args: any[]) => void>(),
    updateTransactionStatisticsFilter: jest.fn((values: Record<string, unknown>) => {
        let changed = false;
        for (const [key, value] of Object.entries(values)) {
            if (lastBase.query.value[key] !== value) {
                lastBase.query.value[key] = value;
                changed = true;
            }
        }
        return changed;
    }),
    updateTransactionStatisticsInvalidState: jest.fn<(...args: any[]) => void>(),
    loadCategoricalAnalysis: jest.fn<(...args: any[]) => Promise<unknown>>(),
    loadTrendAnalysis: jest.fn<(...args: any[]) => Promise<unknown>>(),
    loadAssetTrends: jest.fn<(...args: any[]) => Promise<unknown>>(),
    getTransactionListPageParams: jest.fn((analysis: number, itemId: string, range?: any) => (
        `analysis=${analysis}&item=${itemId}&range=${range?.dateType ?? ''}`
    ))
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `t:${key}`,
        getCurrentLanguageTextDirection: () => 1,
        getAllCategoricalChartTypes: () => [
            { type: 1, displayName: 'Pie' },
            { type: 2, displayName: 'Bar' }
        ],
        formatPercentToLocalizedNumerals: (value: number) => `percent:${value}`
    })
}));

jest.mock('@/views/base/statistics/StatisticsTransactionPageBase.ts', () => ({
    useStatisticsTransactionPageBase: () => {
        lastBase = createBase();
        return lastBase;
    }
}));

jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => accountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => categoriesStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => statisticsStore }));

jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 1, RTL: 2 } }));
jest.mock('@/core/datetime.ts', () => ({
    DateRangeScene: { Normal: 'normal', TrendAnalysis: 'trend', AssetTrends: 'assets' },
    DateRange: dateRange
}));
jest.mock('@/core/statistics.ts', () => ({
    ChartDataAggregationType: { Sum: 1, Last: 2 },
    StatisticsAnalysisType: Analysis,
    CategoricalChartType: { Pie: { type: 1 }, Bar: { type: 2 } },
    ChartDataType: chartDataType,
    ChartSortingType: { Amount: { type: 1 }, Name: { type: 2 } },
    ChartDateAggregationType: { Default: { type: 0 } }
}));
jest.mock('@/lib/common.ts', () => ({
    isString: (value: unknown) => typeof value === 'string',
    isNumber: (value: unknown) => typeof value === 'number'
}));
jest.mock('@/lib/datetime.ts', () => ({
    getGregorianCalendarYearAndMonthFromUnixTime: (value: number) => getGregorianCalendarYearAndMonthFromUnixTime(value),
    getYearMonthFirstUnixTime: (value: string) => getYearMonthFirstUnixTime(value),
    getYearMonthLastUnixTime: (value: string) => getYearMonthLastUnixTime(value),
    getShiftedDateRangeAndDateType: (...args: any[]) => getShiftedDateRangeAndDateType(...args),
    getDateTypeByDateRange: (...args: any[]) => getDateTypeByDateRange(...args),
    getDateRangeByDateType: (...args: any[]) => getDateRangeByDateType(...args)
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({ showPrompt, showToast, routeBackOnError }),
    scrollToSelectedItem: (...args: any[]) => scrollToSelectedItem(...args)
}));
jest.mock('@/views/mobile/statistics/components/MobileStatisticsDateControls.vue', () => ({
    __esModule: true,
    default: { name: 'MobileStatisticsDateControlsStub' }
}));

import TransactionPage from '@/views/mobile/statistics/TransactionPage.vue';

function setup(): { bindings: any; router: { navigate: ReturnType<typeof jest.fn> } } {
    const router = { navigate: jest.fn() };
    const bindings = (TransactionPage as any).setup({ f7router: router }, { expose: jest.fn() });
    return { bindings, router };
}

async function flush(times = 10): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function render(bindings: any): any {
    const { proxyRefs } = jest.requireActual('vue') as any;
    const exposed = proxyRefs(bindings);
    return (TransactionPage as any).render(exposed, [], {}, exposed, {}, {});
}

function collectCallbacks(
    value: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => any }>,
    seen = new Set<any>()
): void {
    if (!value || (typeof value !== 'object' && typeof value !== 'function') || seen.has(value)) return;
    seen.add(value);
    if (Array.isArray(value)) {
        for (const item of value) collectCallbacks(item, callbacks, seen);
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
                collectCallbacks((slot as (...args: any[]) => any)({}), callbacks, seen);
            } else {
                collectCallbacks(slot, callbacks, seen);
            }
        }
    } else {
        collectCallbacks(value.children, callbacks, seen);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    nextAnalysisType = Analysis.CategoricalAnalysis;
    statisticsStore.transactionStatisticsStateInvalid = false;
    accountsStore.loadAllAccounts.mockResolvedValue(undefined);
    categoriesStore.loadAllCategories.mockResolvedValue(undefined);
    statisticsStore.loadCategoricalAnalysis.mockResolvedValue(undefined);
    statisticsStore.loadTrendAnalysis.mockResolvedValue(undefined);
    statisticsStore.loadAssetTrends.mockResolvedValue(undefined);
    chartDataType.isAvailableForAnalysisType.mockReturnValue(true);
    getDateRangeByDateType.mockImplementation((type: number) => type < 0
        ? null
        : { dateType: type, minTime: 1_000 + type, maxTime: 2_000 + type });
    getDateTypeByDateRange.mockReturnValue(77);
    getShiftedDateRangeAndDateType.mockReturnValue({ dateType: 88, minTime: 3_000, maxTime: 4_000 });
    getYearMonthFirstUnixTime.mockImplementation(value => value === '2026-01' ? 100 : 300);
    getYearMonthLastUnixTime.mockImplementation(value => value === '2026-06' ? 200 : 400);
    getGregorianCalendarYearAndMonthFromUnixTime.mockImplementation(value => (
        value === 3_000 ? '2026-03' : value === 4_000 ? '2026-04' : `ym:${value}`
    ));
});

describe('mobile statistics TransactionPage production-loaded behavior', () => {
    test('initializes each analysis type and exposes computed chart state', async () => {
        const categorical = setup().bindings;
        await flush();
        expect(statisticsStore.initTransactionStatisticsFilter).toHaveBeenCalledWith(Analysis.CategoricalAnalysis);
        expect(statisticsStore.loadCategoricalAnalysis).toHaveBeenCalledWith({ force: false });
        expect(categorical.loading.value).toBe(false);
        expect(categorical.textDirection.value).toBe(1);
        expect(categorical.allChartTypes.value).toHaveLength(2);
        expect(categorical.queryChartType.value).toBe(1);

        nextAnalysisType = Analysis.TrendAnalysis;
        const trend = setup().bindings;
        await flush();
        expect(statisticsStore.loadTrendAnalysis).toHaveBeenCalledWith({ force: false });
        expect(trend.allChartTypes.value).toEqual([]);
        expect(trend.queryChartType.value).toBe(21);

        nextAnalysisType = Analysis.AssetTrends;
        const assets = setup().bindings;
        await flush();
        expect(statisticsStore.loadAssetTrends).toHaveBeenCalledWith({ force: false });
        expect(assets.queryChartType.value).toBe(31);

        assets.analysisType.value = 999;
        expect(assets.queryChartType.value).toBeUndefined();
    });

    test('reports handled, unhandled, and unsupported initialization failures', async () => {
        statisticsStore.loadCategoricalAnalysis.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const handled = setup().bindings;
        await flush();
        expect(handled.loading.value).toBe(false);
        expect(showToast).not.toHaveBeenCalled();

        statisticsStore.loadCategoricalAnalysis.mockRejectedValueOnce({ processed: false, message: 'offline' });
        const failed = setup().bindings;
        await flush();
        expect(failed.loadingError.value).toEqual(expect.objectContaining({ message: 'offline' }));
        expect(showToast).toHaveBeenCalledWith('offline');

        nextAnalysisType = 999;
        setup();
        await flush();
        expect(showToast).toHaveBeenCalledWith('An error occurred');
    });

    test('reloads every statistics source and account-backed source with pull-to-refresh feedback', async () => {
        const { bindings } = setup();
        await flush();
        const done = jest.fn();
        const statisticsTypes = Object.values(chartType).filter(value => (
            typeof value === 'number' && value >= chartType.OutflowsByAccount && value <= chartType.NetWorth
        ));

        for (const type of statisticsTypes) {
            bindings.query.value.chartDataType = type;
            for (const analysis of [Analysis.CategoricalAnalysis, Analysis.TrendAnalysis, Analysis.AssetTrends]) {
                bindings.analysisType.value = analysis;
                bindings.reload(analysis === Analysis.CategoricalAnalysis ? done : undefined);
                await flush();
            }
        }
        expect(statisticsStore.loadCategoricalAnalysis).toHaveBeenCalledWith({ force: true });
        expect(statisticsStore.loadTrendAnalysis).toHaveBeenCalledWith({ force: false });
        expect(statisticsStore.loadAssetTrends).toHaveBeenCalledWith({ force: false });
        expect(done).toHaveBeenCalled();
        expect(showToast).toHaveBeenCalledWith('Data has been updated');

        for (const type of [chartType.AccountTotalAssets, chartType.AccountTotalLiabilities]) {
            bindings.query.value.chartDataType = type;
            bindings.analysisType.value = Analysis.CategoricalAnalysis;
            bindings.reload(done);
            await flush();
            bindings.analysisType.value = Analysis.AssetTrends;
            bindings.reload();
            await flush();
            bindings.analysisType.value = Analysis.TrendAnalysis;
            bindings.reload();
        }
        bindings.query.value.chartDataType = chartType.Unsupported;
        bindings.reload();
        expect(bindings.reloading.value).toBe(false);
        expect(accountsStore.loadAllAccounts).toHaveBeenCalledWith({ force: true });
    });

    test('cleans up reload failures and only displays unprocessed errors', async () => {
        const { bindings } = setup();
        await flush();
        const done = jest.fn();
        showToast.mockClear();
        statisticsStore.loadCategoricalAnalysis.mockRejectedValueOnce({ processed: false, message: 'network' });
        bindings.reload(done);
        await flush();
        expect(bindings.reloading.value).toBe(false);
        expect(done).toHaveBeenCalledTimes(1);
        expect(showToast).toHaveBeenCalledWith('network');

        statisticsStore.loadCategoricalAnalysis.mockRejectedValueOnce({ processed: true, message: 'known' });
        bindings.reload(done);
        await flush();
        expect(done).toHaveBeenCalledTimes(2);
        expect(showToast).toHaveBeenCalledTimes(1);
    });

    test('updates chart data, chart presentation, sorting, and aggregation choices', async () => {
        const { bindings } = setup();
        await flush();
        bindings.queryChartType.value = 2;
        expect(bindings.query.value.categoricalChartType).toBe(2);
        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.queryChartType.value = 22;
        expect(bindings.query.value.trendChartType).toBe(22);
        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.queryChartType.value = 32;
        expect(bindings.query.value.assetTrendsChartType).toBe(32);
        bindings.analysisType.value = 999;
        bindings.setChartType(44);

        bindings.analysisType.value = Analysis.CategoricalAnalysis;
        bindings.setChartDataType(Analysis.CategoricalAnalysis, chartType.TotalIncome);
        expect(bindings.query.value.chartDataType).toBe(chartType.TotalIncome);
        chartDataType.isAvailableForAnalysisType.mockReturnValue(false);
        bindings.setChartDataType(Analysis.TrendAnalysis, chartType.TotalExpense);
        await flush();
        expect(statisticsStore.updateTransactionStatisticsInvalidState).toHaveBeenCalledWith(true);
        expect(bindings.analysisType.value).toBe(Analysis.TrendAnalysis);
        expect(bindings.showChartDataTypePopover.value).toBe(false);

        bindings.showSortingTypePopover.value = true;
        bindings.setSortingType(0);
        bindings.showSortingTypePopover.value = true;
        bindings.setSortingType(3);
        bindings.setSortingType(2);
        expect(bindings.query.value.sortingType).toBe(2);
        bindings.setTrendDateAggregationType(55);
        bindings.setAssetTrendsDateAggregationType(66);
        expect(bindings.trendDateAggregationType.value).toBe(55);
        expect(bindings.assetTrendsDateAggregationType.value).toBe(66);
    });

    test('applies preset and custom date filters for categorical, trend, and asset analyses', async () => {
        const { bindings } = setup();
        await flush();
        bindings.setDateFilter(dateRange.Custom.type);
        expect(bindings.showCustomDateRangeSheet.value).toBe(true);
        bindings.setDateFilter(10);
        bindings.setDateFilter(-1);
        bindings.setDateFilter(12);
        expect(bindings.query.value.categoricalChartDateType).toBe(12);

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.setDateFilter(dateRange.Custom.type);
        expect(bindings.showCustomMonthRangeSheet.value).toBe(true);
        bindings.setDateFilter(20);
        bindings.setDateFilter(dateRange.All.type);
        expect(bindings.query.value.trendChartStartYearMonth).toBe('');
        bindings.setDateFilter(23);
        expect(bindings.query.value.trendChartStartYearMonth).toBe('ym:1023');

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.setDateFilter(dateRange.Custom.type);
        expect(bindings.showCustomDateRangeSheet.value).toBe(true);
        bindings.setDateFilter(30);
        bindings.setDateFilter(33);
        expect(bindings.query.value.assetTrendsChartDateType).toBe(33);
        bindings.analysisType.value = 999;
        bindings.setDateFilter(34);

        bindings.analysisType.value = Analysis.CategoricalAnalysis;
        bindings.setCustomDateFilter(0, 2);
        bindings.setCustomDateFilter(1, 0);
        bindings.setCustomDateFilter('2026-01', '2026-02');
        bindings.setCustomDateFilter(1_111, 2_222);
        expect(bindings.query.value.categoricalChartStartTime).toBe(1_111);
        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.setCustomDateFilter('2026-02', '2026-05');
        expect(bindings.query.value.trendChartStartYearMonth).toBe('2026-02');
        bindings.setCustomDateFilter(100, 200);
        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.setCustomDateFilter(3_333, 4_444);
        expect(bindings.query.value.assetTrendsChartEndTime).toBe(4_444);
        bindings.setCustomDateFilter('2026-03', '2026-04');
    });

    test('shifts each date range while preserving all-time selections', async () => {
        const { bindings } = setup();
        await flush();
        bindings.query.value.categoricalChartDateType = dateRange.All.type;
        bindings.shiftDateRange(1);
        bindings.query.value.categoricalChartDateType = 10;
        bindings.shiftDateRange(-1);
        expect(bindings.query.value.categoricalChartStartTime).toBe(3_000);

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.query.value.trendChartDateType = dateRange.All.type;
        bindings.shiftDateRange(1);
        bindings.query.value.trendChartDateType = 20;
        bindings.shiftDateRange(1);
        expect(bindings.query.value.trendChartStartYearMonth).toBe('2026-03');
        expect(bindings.query.value.trendChartEndYearMonth).toBe('2026-04');

        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.query.value.assetTrendsChartDateType = dateRange.All.type;
        bindings.shiftDateRange(1);
        bindings.query.value.assetTrendsChartDateType = 30;
        bindings.shiftDateRange(1);
        expect(bindings.query.value.assetTrendsChartEndTime).toBe(4_000);
        bindings.analysisType.value = 999;
        bindings.shiftDateRange(1);
    });

    test('navigates filters, settings, and chart drilldowns and handles description prompts', async () => {
        const { bindings, router } = setup();
        await flush();
        bindings.filterAccounts();
        bindings.filterCategories();
        bindings.filterTags();
        bindings.settings();
        expect(router.navigate).toHaveBeenCalledWith('/settings/filter/account?type=statisticsCurrent');
        expect(router.navigate).toHaveBeenCalledWith('/settings/filter/category?type=statisticsCurrent');
        expect(router.navigate).toHaveBeenCalledWith('/settings/filter/tag?type=statisticsCurrent');
        expect(router.navigate).toHaveBeenCalledWith('/statistic/settings');

        bindings.filterDescription();
        let promptCallback = showPrompt.mock.calls.at(-1)?.[2] as (value: string) => void;
        promptCallback('');
        promptCallback('coffee');
        expect(bindings.query.value.keyword).toBe('coffee');
        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.filterDescription();
        promptCallback = showPrompt.mock.calls.at(-1)?.[2] as (value: string) => void;
        promptCallback('rent');
        expect(bindings.query.value.keyword).toBe('rent');
        bindings.analysisType.value = 999;
        bindings.filterDescription();
        promptCallback = showPrompt.mock.calls.at(-1)?.[2] as (value: string) => void;
        promptCallback('unknown');
        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.filterDescription();

        bindings.analysisType.value = Analysis.CategoricalAnalysis;
        bindings.onClickPieChartItem({ id: 'food' });
        const range = { dateType: 10, minTime: 100, maxTime: 200 };
        bindings.onClickTrendChartItem({ itemId: 'rent', dateRange: range });
        expect(router.navigate).toHaveBeenCalledWith('/transaction/list?analysis=1&item=food&range=');
        expect(router.navigate).toHaveBeenCalledWith('/transaction/list?analysis=1&item=rent&range=10');
    });

    test('coordinates popover scrolling and page re-entry reload/error behavior', async () => {
        const { bindings, router } = setup();
        await flush();
        const dom = { id: 'popover' };
        bindings.scrollPopoverToSelectedItem({ $el: dom });
        expect(scrollToSelectedItem).toHaveBeenCalledWith(dom, '.popover-inner', 'li.list-item-selected');

        statisticsStore.transactionStatisticsStateInvalid = true;
        bindings.loading.value = false;
        bindings.onPageAfterIn();
        await flush();
        expect(statisticsStore.loadCategoricalAnalysis).toHaveBeenCalled();
        expect(routeBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
        statisticsStore.loadCategoricalAnalysis.mockClear();
        bindings.loading.value = true;
        bindings.onPageAfterIn();
        expect(statisticsStore.loadCategoricalAnalysis).not.toHaveBeenCalled();
    });

    test('renders every chart and empty/loading state and executes compiled template handlers', async () => {
        const { bindings } = setup();
        await flush();
        const renders: any[] = [];

        bindings.analysisType.value = Analysis.CategoricalAnalysis;
        bindings.query.value.categoricalChartType = 1;
        bindings.loading.value = true;
        renders.push(render(bindings));
        bindings.loading.value = false;
        bindings.categoricalAnalysisData.value = { totalAmountCents: 12_345, items: [] };
        renders.push(render(bindings));
        bindings.categoricalAnalysisData.value = {
            totalAmountCents: 12_345,
            items: [
                { id: 'food', name: 'Food', icon: 'fork', color: '#f00', percent: 50, totalAmountCents: 6_000, hidden: false },
                { id: 'other', name: 'Other', icon: '', color: '', percent: -1, totalAmountCents: 6_345, hidden: true }
            ]
        };
        renders.push(render(bindings));

        bindings.query.value.categoricalChartType = 2;
        bindings.loading.value = true;
        renders.push(render(bindings));
        bindings.loading.value = false;
        bindings.categoricalAnalysisData.value = { totalAmountCents: 0, items: [] };
        renders.push(render(bindings));
        bindings.categoricalAnalysisData.value = {
            totalAmountCents: 12_345,
            items: [
                { id: 'food', name: 'Food', icon: 'fork', color: '#f00', percent: 50, totalAmountCents: 6_000, hidden: false },
                { id: 'other', name: 'Other', icon: '', color: '', percent: -1, totalAmountCents: 6_345, hidden: false }
            ]
        };
        renders.push(render(bindings));

        bindings.analysisType.value = Analysis.TrendAnalysis;
        bindings.trendsAnalysisData.value = { items: [] };
        renders.push(render(bindings));
        bindings.trendsAnalysisData.value = { items: [{ id: 'month', totalAmountCents: 100 }] };
        renders.push(render(bindings));
        bindings.analysisType.value = Analysis.AssetTrends;
        bindings.assetTrendsData.value = { items: [] };
        renders.push(render(bindings));
        bindings.assetTrendsData.value = { items: [{ id: 'day', totalAmountCents: 200 }] };
        renders.push(render(bindings));

        const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
        for (const rendered of renders) collectCallbacks(rendered, callbacks);
        expect(renders.every(Boolean)).toBe(true);
        expect(callbacks.length).toBeGreaterThan(0);

        for (const { name, callback } of callbacks) {
            if (name === 'onPtr:refresh' || name === 'onPtrRefresh') callback(jest.fn());
            else if (name === 'onPage:afterin' || name === 'onPageAfterin') callback();
            else if (name === 'onPopover:open' || name === 'onPopoverOpen') callback({ $el: {} });
            else if (name === 'onDateFilter') callback(12);
            else if (name === 'onTrendDateAggregationType') callback(51);
            else if (name === 'onAssetTrendsDateAggregationType') callback(52);
            else if (name === 'onCustomDateFilter') callback(1_000, 2_000);
            else if (name.startsWith('onUpdate:')) callback(false);
            else if (name === 'onClick') callback({ id: 'food', itemId: 'rent', dateRange: { dateType: 10 } });
            else callback();
        }
        await flush(20);
    });
});
