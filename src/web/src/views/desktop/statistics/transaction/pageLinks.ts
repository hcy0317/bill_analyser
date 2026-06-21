import type { TimeRangeAndDateType } from '@/core/datetime.ts';
import type { StatisticsAnalysisType } from '@/core/statistics.ts';

interface StatisticsPageLinkSource {
    getTransactionStatisticsPageParams(
        analysisType: StatisticsAnalysisType,
        trendDateAggregationType: number,
        assetTrendsDateAggregationType: number
    ): string;
}

interface TransactionListLinkSource {
    getTransactionListPageParams(
        analysisType: StatisticsAnalysisType,
        itemId: string,
        dateRange?: TimeRangeAndDateType
    ): string;
}

/**
 * 构造统计页筛选链接，保留当前分析类型、聚合粒度和筛选条件。
 */
export function getFilterLinkUrl(
    statisticsStore: StatisticsPageLinkSource,
    analysisType: StatisticsAnalysisType,
    trendDateAggregationType: number,
    assetTrendsDateAggregationType: number
): string {
    return `/statistics/transaction?${statisticsStore.getTransactionStatisticsPageParams(analysisType, trendDateAggregationType, assetTrendsDateAggregationType)}`;
}

/**
 * 构造统计图表条目到交易列表的钻取链接。
 */
export function getTransactionItemLinkUrl(
    statisticsStore: TransactionListLinkSource,
    analysisType: StatisticsAnalysisType,
    itemId: string,
    dateRange?: TimeRangeAndDateType
): string {
    return `/transaction/list?${statisticsStore.getTransactionListPageParams(analysisType, itemId, dateRange)}`;
}
