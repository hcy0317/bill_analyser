import type { TextualYearMonth } from '@/core/datetime.ts';
import type {
    TransactionAssetTrendsAnalysisDataAmount,
    TransactionAssetTrendsAnalysisDataItem,
    TransactionStatisticDataItemType,
    TransactionTrendsAnalysisDataAmount,
    TransactionTrendsAnalysisDataItem
} from '@/models/transaction.ts';

export interface WritableTransactionCategoricalAnalysisData {
    totalAmountCents: number;
    totalNonNegativeAmountCents: number;
    items: Record<string, WritableTransactionCategoricalAnalysisDataItem>;
}

export interface WritableTransactionCategoricalAnalysisDataItem extends Record<string, unknown> {
    name: string;
    type: TransactionStatisticDataItemType;
    id: string;
    icon: string;
    color: string;
    hidden: boolean;
    displayOrders: number[];
    totalAmountCents: number;
    percent?: number;
}

export interface WritableTransactionTrendsAnalysisDataItem extends Record<string, unknown>, TransactionTrendsAnalysisDataItem {
    name: string;
    type: TransactionStatisticDataItemType;
    id: string;
    icon: string;
    color: string;
    hidden: boolean;
    displayOrders: number[];
    totalAmountCents: number;
    items: TransactionTrendsAnalysisDataAmount[];
}

export interface WritableTransactionAssetTrendsAnalysisDataItem extends Record<string, unknown>, TransactionAssetTrendsAnalysisDataItem {
    name: string;
    type: TransactionStatisticDataItemType;
    id: string;
    icon: string;
    color: string;
    hidden: boolean;
    displayOrders: number[];
    totalAmountCents: number;
    totalOpeningAmountCents?: number;
    items: TransactionAssetTrendsAnalysisDataAmount[];
}

export interface TransactionStatisticsPartialFilter {
    chartDataType?: number;
    categoricalChartType?: number;
    categoricalChartDateType?: number;
    categoricalChartStartTime?: number;
    categoricalChartEndTime?: number;
    trendChartType?: number;
    trendChartDateType?: number;
    trendChartStartYearMonth?: TextualYearMonth | '';
    trendChartEndYearMonth?: TextualYearMonth | '';
    assetTrendsChartType?: number;
    assetTrendsChartDateType?: number;
    assetTrendsChartStartTime?: number;
    assetTrendsChartEndTime?: number;
    filterAccountIds?: Record<string, boolean>;
    filterCategoryIds?: Record<string, boolean>;
    tagIds?: string;
    tagFilterType?: number;
    keyword?: string;
    sortingType?: number;
}

export interface TransactionStatisticsFilter extends TransactionStatisticsPartialFilter {
    chartDataType: number;
    categoricalChartType: number;
    categoricalChartDateType: number;
    categoricalChartStartTime: number;
    categoricalChartEndTime: number;
    trendChartType: number;
    trendChartDateType: number;
    trendChartStartYearMonth: TextualYearMonth | '';
    trendChartEndYearMonth: TextualYearMonth | '';
    assetTrendsChartType: number;
    assetTrendsChartDateType: number;
    assetTrendsChartStartTime: number;
    assetTrendsChartEndTime: number;
    filterAccountIds: Record<string, boolean>;
    filterCategoryIds: Record<string, boolean>;
    tagIds: string;
    tagFilterType: number;
    keyword: string;
    sortingType: number;
}
