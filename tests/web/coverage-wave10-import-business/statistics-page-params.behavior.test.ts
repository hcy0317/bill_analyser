import { describe, expect, jest, test } from '@jest/globals';

const resolveAccountIds = jest.fn<(map: unknown, ids: unknown) => string>(() => 'resolved-account-ids');
const resolveCategoryIds = jest.fn<(map: unknown, ids: unknown) => string>(() => 'resolved-category-ids');

jest.mock('@/lib/account.ts', () => ({
    __esModule: true,
    getFinalAccountIdsByFilteredAccountIds: (map: unknown, ids: unknown) => resolveAccountIds(map, ids)
}));

jest.mock('@/lib/category.ts', () => ({
    __esModule: true,
    getFinalCategoryIdsByFilteredCategoryIds: (map: unknown, ids: unknown) => resolveCategoryIds(map, ids)
}));

import { DateRange } from '@/core/datetime.ts';
import {
    ChartDataType,
    ChartDateAggregationType,
    StatisticsAnalysisType
} from '@/core/statistics.ts';
import {
    buildTransactionListPageParams,
    buildTransactionStatisticsPageParams
} from '@/stores/statistics/pageParams.ts';
import type { TransactionStatisticsFilter } from '@/stores/statistics/types.ts';

function makeFilter(overrides: Partial<TransactionStatisticsFilter> = {}): TransactionStatisticsFilter {
    return {
        chartDataType: ChartDataType.ExpenseByAccount.type,
        categoricalChartType: 1,
        categoricalChartDateType: DateRange.Custom.type,
        categoricalChartStartTime: 100,
        categoricalChartEndTime: 200,
        trendChartType: 2,
        trendChartDateType: DateRange.Custom.type,
        trendChartStartYearMonth: '2025-01',
        trendChartEndYearMonth: '2025-12',
        assetTrendsChartType: 3,
        assetTrendsChartDateType: DateRange.Custom.type,
        assetTrendsChartStartTime: 300,
        assetTrendsChartEndTime: 400,
        filterAccountIds: {},
        filterCategoryIds: {},
        tagIds: '',
        tagFilterType: 0,
        keyword: '',
        sortingType: 2,
        ...overrides
    };
}

function queryFrom(value: string): URLSearchParams {
    return new URLSearchParams(value);
}

const accountMap = {};
const categoryMap = {};

describe('statistics page query serialization', () => {
    test('serializes categorical custom range and every shared filter without losing false-valued selections', () => {
        const query = queryFrom(buildTransactionStatisticsPageParams(
            makeFilter({
                chartDataType: ChartDataType.IncomeByAccount.type,
                filterAccountIds: { cash: true, hidden: false },
                filterCategoryIds: { salary: true },
                tagIds: 'tag-1,tag-2',
                tagFilterType: 3,
                keyword: 'coffee & lunch'
            }),
            StatisticsAnalysisType.CategoricalAnalysis,
            ChartDateAggregationType.Default.type,
            ChartDateAggregationType.Default.type
        ));

        expect(Object.fromEntries(query)).toEqual({
            analysisType: String(StatisticsAnalysisType.CategoricalAnalysis),
            chartDataType: String(ChartDataType.IncomeByAccount.type),
            chartType: '1',
            chartDateType: String(DateRange.Custom.type),
            startTime: '100',
            endTime: '200',
            filterAccountIds: 'cash,hidden',
            filterCategoryIds: 'salary',
            tagIds: 'tag-1,tag-2',
            tagFilterType: '3',
            keyword: 'coffee & lunch',
            sortingType: '2'
        });
    });

    test('uses year-month ranges for trend analysis and includes only non-default aggregation', () => {
        const query = queryFrom(buildTransactionStatisticsPageParams(
            makeFilter(),
            StatisticsAnalysisType.TrendAnalysis,
            ChartDateAggregationType.Year.type,
            ChartDateAggregationType.Default.type
        ));

        expect(query.get('chartType')).toBe('2');
        expect(query.get('startTime')).toBe('2025-01');
        expect(query.get('endTime')).toBe('2025-12');
        expect(query.get('trendDateAggregationType')).toBe(String(ChartDateAggregationType.Year.type));
        expect(query.has('assetTrendsDateAggregationType')).toBe(false);
    });

    test('uses timestamp ranges for asset trends and omits empty shared filters', () => {
        const query = queryFrom(buildTransactionStatisticsPageParams(
            makeFilter(),
            StatisticsAnalysisType.AssetTrends,
            ChartDateAggregationType.Default.type,
            ChartDateAggregationType.Quarter.type
        ));

        expect(query.get('chartType')).toBe('3');
        expect(query.get('startTime')).toBe('300');
        expect(query.get('endTime')).toBe('400');
        expect(query.get('assetTrendsDateAggregationType')).toBe(String(ChartDateAggregationType.Quarter.type));
        expect(query.has('filterAccountIds')).toBe(false);
        expect(query.has('filterCategoryIds')).toBe(false);
        expect(query.has('keyword')).toBe(false);
    });
});

describe('statistics drilldown query behavior', () => {
    test.each([
        ChartDataType.IncomeByAccount.type,
        ChartDataType.IncomeByPrimaryCategory.type,
        ChartDataType.IncomeBySecondaryCategory.type,
        ChartDataType.TotalIncome.type
    ])('maps income chart type %s to income transactions', chartDataType => {
        const query = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.TrendAnalysis,
            itemId: ''
        }));
        expect(query.get('type')).toBe('2');
    });

    test.each([
        ChartDataType.ExpenseByAccount.type,
        ChartDataType.ExpenseByPrimaryCategory.type,
        ChartDataType.ExpenseBySecondaryCategory.type,
        ChartDataType.TotalExpense.type
    ])('maps expense chart type %s to expense transactions', chartDataType => {
        const query = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.TrendAnalysis,
            itemId: ''
        }));
        expect(query.get('type')).toBe('3');
    });

    test('interprets overview source and target identities and marks two-account transfers', () => {
        const twoAccounts = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType: ChartDataType.Overview.type }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.CategoricalAnalysis,
            itemId: 'account:cash-account:card'
        }));
        expect(twoAccounts.get('type')).toBe('4');
        expect(twoAccounts.get('accountIds')).toBe('cash,card');
        expect(twoAccounts.get('categoryIds')).toBe('resolved-category-ids');

        const mixed = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType: ChartDataType.Overview.type }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.CategoricalAnalysis,
            itemId: 'category:food-account:cash'
        }));
        expect(mixed.get('accountIds')).toBe('cash');
        expect(mixed.get('categoryIds')).toBe('food');
        expect(mixed.has('type')).toBe(false);
    });

    test('falls back to the active account and category filters for malformed overview identities', () => {
        const query = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType: ChartDataType.Overview.type }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.CategoricalAnalysis,
            itemId: 'merchant:coffee-invalid'
        }));

        expect(query.get('accountIds')).toBe('resolved-account-ids');
        expect(query.get('categoryIds')).toBe('resolved-category-ids');
    });

    test.each([
        ChartDataType.InflowsByAccount.type,
        ChartDataType.IncomeByAccount.type,
        ChartDataType.OutflowsByAccount.type,
        ChartDataType.ExpenseByAccount.type,
        ChartDataType.AccountTotalAssets.type,
        ChartDataType.AccountTotalLiabilities.type
    ])('drills account chart type %s into the selected account', chartDataType => {
        const query = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType, filterCategoryIds: { food: true } }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.CategoricalAnalysis,
            itemId: 'cash'
        }));
        expect(query.get('accountIds')).toBe('cash');
        expect(query.get('categoryIds')).toBe('resolved-category-ids');
    });

    test.each([
        ChartDataType.IncomeByPrimaryCategory.type,
        ChartDataType.IncomeBySecondaryCategory.type,
        ChartDataType.ExpenseByPrimaryCategory.type,
        ChartDataType.ExpenseBySecondaryCategory.type
    ])('drills category chart type %s into the selected category', chartDataType => {
        const query = queryFrom(buildTransactionListPageParams({
            filter: makeFilter({ chartDataType, filterAccountIds: { cash: true } }),
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.CategoricalAnalysis,
            itemId: 'food'
        }));
        expect(query.get('categoryIds')).toBe('food');
        expect(query.get('accountIds')).toBe('resolved-account-ids');
    });

    test('applies current filters without a chart item and scopes text/tag filters to categorical or trend views', () => {
        const filter = makeFilter({
            filterAccountIds: { cash: true },
            filterCategoryIds: { food: true },
            tagIds: 'tag-1',
            tagFilterType: 2,
            keyword: '餐饮'
        });
        const categorical = queryFrom(buildTransactionListPageParams({
            filter,
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.CategoricalAnalysis,
            itemId: ''
        }));
        expect(categorical.get('accountIds')).toBe('resolved-account-ids');
        expect(categorical.get('categoryIds')).toBe('resolved-category-ids');
        expect(categorical.get('tagIds')).toBe('tag-1');
        expect(categorical.get('tagFilterType')).toBe('2');
        expect(categorical.get('keyword')).toBe('餐饮');
        expect(categorical.get('dateType')).toBe(String(DateRange.Custom.type));
        expect(categorical.get('minTime')).toBe('100');
        expect(categorical.get('maxTime')).toBe('200');

        const asset = queryFrom(buildTransactionListPageParams({
            filter,
            accountsMap: accountMap,
            categoriesMap: categoryMap,
            analysisType: StatisticsAnalysisType.AssetTrends,
            itemId: '',
            dateRange: { dateType: 7, minTime: 500, maxTime: 600 }
        }));
        expect(asset.has('tagIds')).toBe(false);
        expect(asset.has('keyword')).toBe(false);
        expect(asset.get('dateType')).toBe('7');
        expect(asset.get('minTime')).toBe('500');
        expect(asset.get('maxTime')).toBe('600');
    });

    test('does not attach a categorical date range to total asset and liability drilldowns', () => {
        for (const chartDataType of [
            ChartDataType.AccountTotalAssets.type,
            ChartDataType.AccountTotalLiabilities.type
        ]) {
            const query = queryFrom(buildTransactionListPageParams({
                filter: makeFilter({ chartDataType }),
                accountsMap: accountMap,
                categoriesMap: categoryMap,
                analysisType: StatisticsAnalysisType.CategoricalAnalysis,
                itemId: 'cash'
            }));
            expect(query.has('dateType')).toBe(false);
            expect(query.has('minTime')).toBe(false);
            expect(query.has('maxTime')).toBe(false);
        }
    });
});
