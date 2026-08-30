import type { TimeRangeAndDateType } from '@/core/datetime.ts';
import { DateRange } from '@/core/datetime.ts';
import {
    ChartDataType,
    ChartDateAggregationType,
    StatisticsAnalysisType
} from '@/core/statistics.ts';
import type { Account } from '@/models/account.ts';
import type { TransactionCategory } from '@/models/transaction_category.ts';
import { objectFieldToArrayItem, isObjectEmpty } from '@/lib/common.ts';
import { getFinalAccountIdsByFilteredAccountIds } from '@/lib/account.ts';
import { getFinalCategoryIdsByFilteredCategoryIds } from '@/lib/category.ts';
import type { TransactionStatisticsFilter } from './types.ts';

/**
 * 将统计页当前筛选状态序列化为 URL query，供页面刷新和筛选链接复现图表状态。
 */
export function buildTransactionStatisticsPageParams(
    filter: TransactionStatisticsFilter,
    analysisType: StatisticsAnalysisType,
    trendDateAggregationType: number,
    assetTrendsDateAggregationType: number
): string {
    const querys: string[] = [];

    querys.push('analysisType=' + analysisType);
    querys.push('chartDataType=' + filter.chartDataType);

    if (analysisType === StatisticsAnalysisType.CategoricalAnalysis) {
        querys.push('chartType=' + filter.categoricalChartType);
        querys.push('chartDateType=' + filter.categoricalChartDateType);

        if (filter.categoricalChartDateType === DateRange.Custom.type) {
            querys.push('startTime=' + filter.categoricalChartStartTime);
            querys.push('endTime=' + filter.categoricalChartEndTime);
        }
    } else if (analysisType === StatisticsAnalysisType.TrendAnalysis) {
        querys.push('chartType=' + filter.trendChartType);
        querys.push('chartDateType=' + filter.trendChartDateType);

        if (filter.trendChartDateType === DateRange.Custom.type) {
            querys.push('startTime=' + filter.trendChartStartYearMonth);
            querys.push('endTime=' + filter.trendChartEndYearMonth);
        }

        if (trendDateAggregationType !== ChartDateAggregationType.Default.type) {
            querys.push('trendDateAggregationType=' + trendDateAggregationType);
        }
    } else if (analysisType === StatisticsAnalysisType.AssetTrends) {
        querys.push('chartType=' + filter.assetTrendsChartType);
        querys.push('chartDateType=' + filter.assetTrendsChartDateType);

        if (filter.assetTrendsChartDateType === DateRange.Custom.type) {
            querys.push('startTime=' + filter.assetTrendsChartStartTime);
            querys.push('endTime=' + filter.assetTrendsChartEndTime);
        }

        if (assetTrendsDateAggregationType !== ChartDateAggregationType.Default.type) {
            querys.push('assetTrendsDateAggregationType=' + assetTrendsDateAggregationType);
        }
    }

    if (filter.filterAccountIds) {
        const ids = objectFieldToArrayItem(filter.filterAccountIds);

        if (ids && ids.length) {
            querys.push('filterAccountIds=' + ids.join(','));
        }
    }

    if (filter.filterCategoryIds) {
        const ids = objectFieldToArrayItem(filter.filterCategoryIds);

        if (ids && ids.length) {
            querys.push('filterCategoryIds=' + ids.join(','));
        }
    }

    if (filter.tagIds) {
        querys.push('tagIds=' + filter.tagIds);
    }

    if (filter.tagFilterType) {
        querys.push('tagFilterType=' + filter.tagFilterType);
    }

    if (filter.keyword) {
        querys.push('keyword=' + encodeURIComponent(filter.keyword));
    }

    querys.push('sortingType=' + filter.sortingType);

    return querys.join('&');
}

/**
 * 根据图表点击项、筛选条件和日期范围生成交易列表 query，保证统计钻取时账户/分类/标签条件一致。
 */
export function buildTransactionListPageParams({
    filter,
    accountsMap,
    categoriesMap,
    analysisType,
    itemId,
    dateRange
}: {
    filter: TransactionStatisticsFilter;
    accountsMap: Record<string, Account>;
    categoriesMap: Record<string, TransactionCategory>;
    analysisType: StatisticsAnalysisType;
    itemId: string;
    dateRange?: TimeRangeAndDateType;
}): string {
    const querys: string[] = [];

    if (filter.chartDataType === ChartDataType.IncomeByAccount.type
        || filter.chartDataType === ChartDataType.IncomeByPrimaryCategory.type
        || filter.chartDataType === ChartDataType.IncomeBySecondaryCategory.type
        || filter.chartDataType === ChartDataType.TotalIncome.type) {
        querys.push('type=2');
    } else if (filter.chartDataType === ChartDataType.ExpenseByAccount.type
        || filter.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type
        || filter.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type
        || filter.chartDataType === ChartDataType.TotalExpense.type) {
        querys.push('type=3');
    }

    if (filter.chartDataType === ChartDataType.InflowsByAccount.type) {
        querys.push('flowDirection=inflow');
    } else if (filter.chartDataType === ChartDataType.OutflowsByAccount.type) {
        querys.push('flowDirection=outflow');
    }

    if (itemId && filter.chartDataType === ChartDataType.Overview.type) {
        const items = itemId.split('-');
        const sourceItems = (items[0] || '').split(':');
        const queryAccountIds: string[] = [];
        const queryCategoryIds: string[] = [];

        if (sourceItems.length === 2) {
            if (sourceItems[0] === 'account') {
                queryAccountIds.push(sourceItems[1] as string);
            } else if (sourceItems[0] === 'category') {
                queryCategoryIds.push(sourceItems[1] as string);
            }
        }

        if (items.length === 2) {
            const targetItems = (items[1] || '').split(':');

            if (targetItems.length === 2) {
                if (targetItems[0] === 'account') {
                    queryAccountIds.push(targetItems[1] as string);
                } else if (targetItems[0] === 'category') {
                    queryCategoryIds.push(targetItems[1] as string);
                }
            }
        }

        if (queryAccountIds.length) {
            if (queryAccountIds.length === 2) {
                querys.push('type=4');
            }

            querys.push('accountIds=' + queryAccountIds.join(','));
        } else {
            querys.push('accountIds=' + getFinalAccountIdsByFilteredAccountIds(accountsMap, filter.filterAccountIds));
        }

        if (queryCategoryIds.length) {
            querys.push('categoryIds=' + queryCategoryIds.join(','));
        } else {
            querys.push('categoryIds=' + getFinalCategoryIdsByFilteredCategoryIds(categoriesMap, filter.filterCategoryIds));
        }
    } else if (itemId && (filter.chartDataType === ChartDataType.InflowsByAccount.type ||
        filter.chartDataType === ChartDataType.IncomeByAccount.type ||
        filter.chartDataType === ChartDataType.OutflowsByAccount.type ||
        filter.chartDataType === ChartDataType.ExpenseByAccount.type ||
        filter.chartDataType === ChartDataType.AccountTotalAssets.type ||
        filter.chartDataType === ChartDataType.AccountTotalLiabilities.type)
    ) {
        querys.push('accountIds=' + itemId);

        if ((analysisType === StatisticsAnalysisType.CategoricalAnalysis || analysisType === StatisticsAnalysisType.TrendAnalysis) && !isObjectEmpty(filter.filterCategoryIds)) {
            querys.push('categoryIds=' + getFinalCategoryIdsByFilteredCategoryIds(categoriesMap, filter.filterCategoryIds));
        }
    } else if (itemId && (filter.chartDataType === ChartDataType.IncomeByPrimaryCategory.type ||
        filter.chartDataType === ChartDataType.IncomeBySecondaryCategory.type ||
        filter.chartDataType === ChartDataType.ExpenseByPrimaryCategory.type ||
        filter.chartDataType === ChartDataType.ExpenseBySecondaryCategory.type)
    ) {
        querys.push('categoryIds=' + itemId);

        if (!isObjectEmpty(filter.filterAccountIds)) {
            querys.push('accountIds=' + getFinalAccountIdsByFilteredAccountIds(accountsMap, filter.filterAccountIds));
        }
    } else if (!itemId) {
        if (!isObjectEmpty(filter.filterCategoryIds)) {
            querys.push('categoryIds=' + getFinalCategoryIdsByFilteredCategoryIds(categoriesMap, filter.filterCategoryIds));
        }

        if (!isObjectEmpty(filter.filterAccountIds)) {
            querys.push('accountIds=' + getFinalAccountIdsByFilteredAccountIds(accountsMap, filter.filterAccountIds));
        }
    }

    if (analysisType === StatisticsAnalysisType.CategoricalAnalysis || analysisType === StatisticsAnalysisType.TrendAnalysis) {
        if (filter.tagIds) {
            querys.push('tagIds=' + filter.tagIds);
        }

        if (filter.tagFilterType) {
            querys.push('tagFilterType=' + filter.tagFilterType);
        }

        if (filter.keyword) {
            querys.push('keyword=' + encodeURIComponent(filter.keyword));
        }
    }

    if (analysisType === StatisticsAnalysisType.CategoricalAnalysis
        && filter.chartDataType !== ChartDataType.AccountTotalAssets.type
        && filter.chartDataType !== ChartDataType.AccountTotalLiabilities.type) {
        querys.push('dateType=' + filter.categoricalChartDateType);

        if (filter.categoricalChartDateType === DateRange.Custom.type) {
            querys.push('minTime=' + filter.categoricalChartStartTime);
            querys.push('maxTime=' + filter.categoricalChartEndTime);
        }
    } else if ((analysisType === StatisticsAnalysisType.TrendAnalysis || analysisType === StatisticsAnalysisType.AssetTrends) && dateRange) {
        querys.push('dateType=' + dateRange.dateType);
        querys.push('minTime=' + dateRange.minTime);
        querys.push('maxTime=' + dateRange.maxTime);
    }

    return querys.join('&');
}
