import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const { reactive } = jest.requireActual('vue') as any;

const mockAnalysis = { CategoricalAnalysis: 1, TrendAnalysis: 2, AssetTrends: 3 } as const;
const mockDateRange = { All: { type: 0, name: 'All' }, Custom: { type: 99, name: 'Custom' } } as const;
const mockChartDataType = {
    Default: { type: 0, name: 'Default' },
    InflowsByAccount: { type: 1, name: 'Inflows' },
    OutflowsByAccount: { type: 2, name: 'Outflows' },
    IncomeByAccount: { type: 3, name: 'Income Account' },
    IncomeByPrimaryCategory: { type: 4, name: 'Income Primary' },
    IncomeBySecondaryCategory: { type: 5, name: 'Income Secondary' },
    ExpenseByAccount: { type: 6, name: 'Expense Account' },
    ExpenseByPrimaryCategory: { type: 7, name: 'Expense Primary' },
    ExpenseBySecondaryCategory: { type: 8, name: 'Expense Secondary' },
    AccountTotalAssets: { type: 9, name: 'Assets' },
    AccountTotalLiabilities: { type: 10, name: 'Liabilities' },
    TotalOutflows: { type: 11, name: 'Total Outflows Type' },
    TotalExpense: { type: 12, name: 'Total Expense Type' },
    TotalInflows: { type: 13, name: 'Total Inflows Type' },
    TotalIncome: { type: 14, name: 'Total Income Type' },
    NetCashFlow: { type: 15, name: 'Net Cash Flow' },
    NetIncome: { type: 16, name: 'Net Income' },
    NetWorth: { type: 17, name: 'Net Worth' },
    values: () => Object.values(mockChartDataType).filter((value: any) => value && typeof value === 'object' && 'type' in value)
} as const;
const mockSorting = {
    Default: { type: 0, name: 'Default Sort' },
    Amount: { type: 1, name: 'Amount Sort' },
    values: () => [mockSorting.Default, mockSorting.Amount]
} as const;
const mockDateAggregation = { Default: { type: 0 } } as const;
const mockTrendType = { Area: { type: 1 }, Column: { type: 2 }, Line: { type: 3 } } as const;

const mockSettingsStore = reactive({
    appSettings: {
        showAccountBalance: true,
        statistics: {
            defaultCategoricalChartDataRangeType: 10,
            defaultTrendChartDataRangeType: 20,
            defaultAssetTrendsChartDataRangeType: 30
        }
    }
});
const mockUserStore = reactive({
    currentUserDefaultCurrency: 'CNY',
    currentUserFirstDayOfWeek: 1,
    currentUserFiscalYearStart: 4
});
const mockFilter = reactive({
    chartDataType: mockChartDataType.IncomeByAccount.type,
    sortingType: mockSorting.Amount.type,
    categoricalChartDateType: 10,
    categoricalChartStartTime: 100,
    categoricalChartEndTime: 200,
    trendChartDateType: 20,
    trendChartStartYearMonth: '2024-01',
    trendChartEndYearMonth: '2024-12',
    assetTrendsChartDateType: 30,
    assetTrendsChartStartTime: 300,
    assetTrendsChartEndTime: 400,
    trendChartType: mockTrendType.Area.type
});
const mockStatisticsStore = reactive({
    transactionStatisticsFilter: mockFilter,
    categoricalAnalysisChartDataCategory: 'category',
    categoricalOverviewAnalysisData: { overview: true },
    categoricalAnalysisData: { totalAmountCents: 1234, items: [] },
    trendsAnalysisData: { items: [{ id: 'trend' }] },
    assetTrendsData: { items: [{ id: 'asset' }] }
});

const mockGetAllDateRanges = jest.fn((scene: string, includeCustom: boolean) => [{ scene, includeCustom }]);

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getAllDateRanges: (scene: string, includeCustom: boolean) => mockGetAllDateRanges(scene, includeCustom),
        getAllStatisticsSortingTypes: () => [{ type: 1, displayName: 'Amount' }],
        getAllStatisticsDateAggregationTypes: (analysis: number) => [{ type: analysis * 10, displayName: `Aggregation ${analysis}` }],
        formatUnixTimeToLongDateTime: (value: number) => `long:${value}`,
        formatUnixTimeToGregorianLikeLongYearMonth: (value: number) => `ym:${value}`,
        formatDateRange: (type: number, start: number, end: number) => `range:${type}:${start}:${end}`,
        formatAmountToLocalizedNumeralsWithCurrency: (amount: number, currency: string) => `${currency}:${amount}`
    })
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/user.ts', () => ({ useUserStore: () => mockUserStore }));
jest.mock('@/stores/statistics.ts', () => ({ useStatisticsStore: () => mockStatisticsStore }));
jest.mock('@/core/datetime.ts', () => ({
    DateRangeScene: { Normal: 'normal', TrendAnalysis: 'trend', AssetTrends: 'asset' },
    DateRange: mockDateRange
}));
jest.mock('@/core/statistics.ts', () => ({
    StatisticsAnalysisType: mockAnalysis,
    ChartDataType: mockChartDataType,
    ChartSortingType: mockSorting,
    ChartDateAggregationType: mockDateAggregation,
    TrendChartType: mockTrendType
}));
jest.mock('@/consts/numeral.ts', () => ({ DISPLAY_HIDDEN_AMOUNT: '******' }));
jest.mock('@/lib/common.ts', () => ({
    limitText: (value: string, limit: number) => value.slice(0, limit),
    findNameByType: (values: any[], type: number) => values.find(value => value.type === type)?.name,
    findDisplayNameByType: (values: any[], type: number) => values.find(value => value.type === type)?.displayName
}));
jest.mock('@/lib/datetime.ts', () => ({
    getYearMonthFirstUnixTime: (value: string) => value === '2024-01' ? 1_000 : 1_100,
    getYearMonthLastUnixTime: (value: string) => value === '2024-12' ? 2_000 : 2_100
}));
jest.mock('@/lib/color.ts', () => ({
    getDisplayColor: (value: string) => `display:${value}`,
    getCategoryDisplayColor: (value: string) => `category:${value}`,
    getAccountDisplayColor: (value: string) => `account:${value}`
}));

const { useStatisticsTransactionPageBase } = require('@/views/base/statistics/StatisticsTransactionPageBase.ts') as {
    useStatisticsTransactionPageBase: () => any;
};

function resetFilter(): void {
    Object.assign(mockFilter, {
        chartDataType: mockChartDataType.IncomeByAccount.type,
        sortingType: mockSorting.Amount.type,
        categoricalChartDateType: 10,
        categoricalChartStartTime: 100,
        categoricalChartEndTime: 200,
        trendChartDateType: 20,
        trendChartStartYearMonth: '2024-01',
        trendChartEndYearMonth: '2024-12',
        assetTrendsChartDateType: 30,
        assetTrendsChartStartTime: 300,
        assetTrendsChartEndTime: 400,
        trendChartType: mockTrendType.Area.type
    });
}

beforeEach(() => {
    jest.clearAllMocks();
    resetFilter();
    mockSettingsStore.appSettings.showAccountBalance = true;
    mockSettingsStore.appSettings.statistics.defaultCategoricalChartDataRangeType = 10;
    mockSettingsStore.appSettings.statistics.defaultTrendChartDataRangeType = 20;
    mockSettingsStore.appSettings.statistics.defaultAssetTrendsChartDataRangeType = 30;
});

describe('StatisticsTransactionPageBase production behavior', () => {
    test('derives categorical state, names, range controls, filters, and store projections', () => {
        const base = useStatisticsTransactionPageBase();
        expect(base.loading.value).toBe(true);
        expect(base.analysisType.value).toBe(mockAnalysis.CategoricalAnalysis);
        expect(base.showAccountBalance.value).toBe(true);
        expect(base.defaultCurrency.value).toBe('CNY');
        expect(base.firstDayOfWeek.value).toBe(1);
        expect(base.fiscalYearStart.value).toBe(4);
        expect(base.allDateRanges.value).toEqual([{ scene: 'normal', includeCustom: true }]);
        expect(base.allSortingTypes.value).toEqual([{ type: 1, displayName: 'Amount' }]);
        expect(base.allTrendAnalysisDateAggregationTypes.value[0].type).toBe(20);
        expect(base.allAssetTrendsDateAggregationTypes.value[0].type).toBe(30);
        expect(base.query.value).toBe(mockFilter);
        expect(base.queryChartDataCategory.value).toBe('category');
        expect(base.queryDateType.value).toBe(10);
        expect(base.queryStartTime.value).toBe('long:100');
        expect(base.queryEndTime.value).toBe('long:200');
        expect(base.queryDateRangeName.value).toBe('range:10:100:200');
        expect(base.queryChartDataTypeName.value).toBe('tt:Income Account');
        expect(base.querySortingTypeName.value).toBe('tt:Amount Sort');
        expect(base.queryTrendDateAggregationTypeName.value).toBe('');
        base.trendDateAggregationType.value = 20;
        base.assetTrendsDateAggregationType.value = 30;
        expect(base.queryTrendDateAggregationTypeName.value).toBe('Aggregation 2');
        expect(base.queryAssetTrendsDateAggregationTypeName.value).toBe('Aggregation 3');
        expect(base.isQueryDateRangeChanged.value).toBe(false);
        expect(base.canChangeDateRange.value).toBe(true);
        expect(base.canShiftDateRange.value).toBe(true);
        expect(base.canUseCategoryFilter.value).toBe(true);
        expect(base.canUseTagFilter.value).toBe(true);
        expect(base.canUseKeywordFilter.value).toBe(true);
        expect(base.showAmountInChart.value).toBe(true);
        expect(base.categoricalOverviewAnalysisData.value).toEqual({ overview: true });
        expect(base.categoricalAnalysisData.value.totalAmountCents).toBe(1234);
        expect(base.trendsAnalysisData.value.items[0].id).toBe('trend');
        expect(base.assetTrendsData.value.items[0].id).toBe('asset');

        mockFilter.categoricalChartDateType = 11;
        expect(base.isQueryDateRangeChanged.value).toBe(true);
        mockFilter.categoricalChartDateType = mockDateRange.All.type;
        expect(base.canShiftDateRange.value).toBe(false);
        mockFilter.chartDataType = mockChartDataType.AccountTotalAssets.type;
        expect(base.queryDateRangeName.value).toBe('tt:All');
        expect(base.isQueryDateRangeChanged.value).toBe(false);
        expect(base.canChangeDateRange.value).toBe(false);
        expect(base.canShiftDateRange.value).toBe(false);
        expect(base.canUseCategoryFilter.value).toBe(false);
        expect(base.canUseTagFilter.value).toBe(false);
        mockSettingsStore.appSettings.showAccountBalance = false;
        expect(base.showAmountInChart.value).toBe(false);
    });

    test('derives trend ranges including All and missing month boundaries', () => {
        const base = useStatisticsTransactionPageBase();
        base.analysisType.value = mockAnalysis.TrendAnalysis;
        expect(base.allDateRanges.value).toEqual([{ scene: 'trend', includeCustom: true }]);
        expect(base.queryDateType.value).toBe(20);
        expect(base.queryStartTime.value).toBe('ym:1000');
        expect(base.queryEndTime.value).toBe('ym:2000');
        expect(base.queryDateRangeName.value).toBe('range:20:1000:2000');
        expect(base.isQueryDateRangeChanged.value).toBe(false);
        expect(base.canUseCategoryFilter.value).toBe(true);

        mockFilter.trendChartDateType = mockDateRange.All.type;
        expect(base.queryStartTime.value).toBe('');
        expect(base.queryEndTime.value).toBe('');
        expect(base.queryDateRangeName.value).toBe('tt:All');
        expect(base.canShiftDateRange.value).toBe(false);
        mockFilter.trendChartDateType = 21;
        mockFilter.trendChartStartYearMonth = '';
        mockFilter.trendChartEndYearMonth = '';
        expect(base.queryStartTime.value).toBe('');
        expect(base.queryEndTime.value).toBe('');
        expect(base.isQueryDateRangeChanged.value).toBe(false);
        mockFilter.trendChartStartYearMonth = '2024-01';
        expect(base.isQueryDateRangeChanged.value).toBe(true);
        expect(base.canShowCustomDateRange(21)).toBe(false);
        mockFilter.trendChartEndYearMonth = '2024-12';
        expect(base.canShowCustomDateRange(21)).toBe(true);
    });

    test('derives asset ranges and disables server-side category/tag/keyword filters', () => {
        const base = useStatisticsTransactionPageBase();
        base.analysisType.value = mockAnalysis.AssetTrends;
        expect(base.allDateRanges.value).toEqual([{ scene: 'asset', includeCustom: true }]);
        expect(base.queryDateType.value).toBe(30);
        expect(base.queryStartTime.value).toBe('long:300');
        expect(base.queryEndTime.value).toBe('long:400');
        expect(base.queryDateRangeName.value).toBe('range:30:300:400');
        expect(base.canUseCategoryFilter.value).toBe(false);
        expect(base.canUseTagFilter.value).toBe(false);
        expect(base.canUseKeywordFilter.value).toBe(false);
        expect(base.isQueryDateRangeChanged.value).toBe(false);
        mockFilter.assetTrendsChartDateType = 31;
        expect(base.isQueryDateRangeChanged.value).toBe(true);
        expect(base.canShowCustomDateRange(31)).toBe(true);
        mockFilter.assetTrendsChartDateType = mockDateRange.All.type;
        expect(base.canShiftDateRange.value).toBe(false);
    });

    test.each([
        [mockChartDataType.InflowsByAccount.type, 'tt:Total Inflows'],
        [mockChartDataType.OutflowsByAccount.type, 'tt:Total Outflows'],
        [mockChartDataType.IncomeByAccount.type, 'tt:Total Income'],
        [mockChartDataType.IncomeByPrimaryCategory.type, 'tt:Total Income'],
        [mockChartDataType.IncomeBySecondaryCategory.type, 'tt:Total Income'],
        [mockChartDataType.ExpenseByAccount.type, 'tt:Total Expense'],
        [mockChartDataType.ExpenseByPrimaryCategory.type, 'tt:Total Expense'],
        [mockChartDataType.ExpenseBySecondaryCategory.type, 'tt:Total Expense'],
        [mockChartDataType.AccountTotalAssets.type, 'tt:Total Assets'],
        [mockChartDataType.AccountTotalLiabilities.type, 'tt:Total Liabilities'],
        [mockChartDataType.Default.type, 'tt:Total Amount']
    ])('maps total amount title for chart type %s', (chartDataType, title) => {
        mockFilter.chartDataType = chartDataType;
        expect(useStatisticsTransactionPageBase().totalAmountName.value).toBe(title);
    });

    test('maps percent, total, stack, and translated-name presentation contracts', () => {
        const base = useStatisticsTransactionPageBase();
        for (const type of [mockChartDataType.OutflowsByAccount.type, mockChartDataType.InflowsByAccount.type]) {
            mockFilter.chartDataType = type;
            expect(base.showPercentInCategoricalChart.value).toBe(false);
            expect(base.showTotalAmountInTrendsChart.value).toBe(false);
            expect(base.showStackedInTrendsChart.value).toBe(false);
        }
        mockFilter.chartDataType = mockChartDataType.IncomeByAccount.type;
        expect(base.showPercentInCategoricalChart.value).toBe(true);
        expect(base.showTotalAmountInTrendsChart.value).toBe(true);
        expect(base.showStackedInTrendsChart.value).toBe(true);
        mockFilter.trendChartType = mockTrendType.Column.type;
        expect(base.showStackedInTrendsChart.value).toBe(true);
        mockFilter.trendChartType = mockTrendType.Line.type;
        expect(base.showStackedInTrendsChart.value).toBe(false);

        for (const type of [
            mockChartDataType.TotalOutflows.type, mockChartDataType.TotalExpense.type,
            mockChartDataType.TotalInflows.type, mockChartDataType.TotalIncome.type,
            mockChartDataType.NetCashFlow.type, mockChartDataType.NetIncome.type, mockChartDataType.NetWorth.type
        ]) {
            mockFilter.chartDataType = type;
            expect(base.showTotalAmountInTrendsChart.value).toBe(false);
            expect(base.translateNameInTrendsChart.value).toBe(true);
        }
        mockFilter.chartDataType = mockChartDataType.Default.type;
        expect(base.translateNameInTrendsChart.value).toBe(false);
    });

    test('handles custom range predicates, display colors, hidden balances, and text limits', () => {
        const base = useStatisticsTransactionPageBase();
        expect(base.canShowCustomDateRange(10)).toBe(true);
        mockFilter.categoricalChartStartTime = 0;
        expect(base.canShowCustomDateRange(10)).toBe(false);
        expect(base.getTransactionCategoricalAnalysisDataItemDisplayColor({ type: 'category', color: 'red' })).toBe('category:red');
        expect(base.getTransactionCategoricalAnalysisDataItemDisplayColor({ type: 'account', color: 'blue' })).toBe('account:blue');
        expect(base.getTransactionCategoricalAnalysisDataItemDisplayColor({ type: 'other', color: 'gray' })).toBe('display:gray');
        expect(base.getDisplayAmount(1234, 'CNY')).toBe('CNY:1234');
        expect(base.getDisplayAmount(1234, 'CNY', 4)).toBe('CNY:');

        mockSettingsStore.appSettings.showAccountBalance = false;
        mockFilter.chartDataType = mockChartDataType.AccountTotalLiabilities.type;
        expect(base.getDisplayAmount(1234, 'CNY')).toBe('******');
        base.analysisType.value = mockAnalysis.TrendAnalysis;
        expect(base.getDisplayAmount(1234, 'CNY')).toBe('CNY:1234');
    });

    test('returns neutral values for an unsupported analysis mode and missing enum names', () => {
        const base = useStatisticsTransactionPageBase();
        base.analysisType.value = 999;
        mockFilter.chartDataType = 999;
        mockFilter.sortingType = 999;
        expect(base.allDateRanges.value).toEqual([]);
        expect(base.queryDateType.value).toBeNull();
        expect(base.queryStartTime.value).toBe('');
        expect(base.queryEndTime.value).toBe('');
        expect(base.queryDateRangeName.value).toBe('');
        expect(base.queryChartDataTypeName.value).toBe('tt:Statistics');
        expect(base.querySortingTypeName.value).toBe('tt:System Default');
        expect(base.isQueryDateRangeChanged.value).toBe(false);
        expect(base.canShiftDateRange.value).toBe(false);
        expect(base.canShowCustomDateRange(1)).toBe(false);
    });
});
