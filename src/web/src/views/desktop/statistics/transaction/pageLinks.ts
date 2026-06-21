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

export function getFilterLinkUrl(
    statisticsStore: StatisticsPageLinkSource,
    analysisType: StatisticsAnalysisType,
    trendDateAggregationType: number,
    assetTrendsDateAggregationType: number
): string {
    return `/statistics/transaction?${statisticsStore.getTransactionStatisticsPageParams(analysisType, trendDateAggregationType, assetTrendsDateAggregationType)}`;
}

export function getTransactionItemLinkUrl(
    statisticsStore: TransactionListLinkSource,
    analysisType: StatisticsAnalysisType,
    itemId: string,
    dateRange?: TimeRangeAndDateType
): string {
    return `/transaction/list?${statisticsStore.getTransactionListPageParams(analysisType, itemId, dateRange)}`;
}
