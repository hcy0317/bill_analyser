import type { Ref } from 'vue';

import { DateRange } from '@/core/datetime.ts';
import type {
    TransactionStatisticAssetTrendsResponseItem,
    TransactionStatisticResponse,
    TransactionStatisticTrendsResponseItem
} from '@/models/transaction.ts';
import { isEquals } from '@/lib/common.ts';
import logger from '@/lib/logger.ts';
import services from '@/lib/services.ts';
import type { TransactionStatisticsFilter } from './types.ts';

interface StatisticsLoaderRefs {
    transactionStatisticsFilter: Ref<TransactionStatisticsFilter>;
    transactionCategoryStatisticsData: Ref<TransactionStatisticResponse | null>;
    transactionCategoryTrendsData: Ref<TransactionStatisticTrendsResponseItem[]>;
    transactionAssetTrendsData: Ref<TransactionStatisticAssetTrendsResponseItem[]>;
    transactionStatisticsStateInvalid: Ref<boolean>;
    useTransactionTimezone: () => boolean;
}

interface LoadOptions {
    force: boolean;
}

interface StatisticsLoadError {
    response?: { data?: { message?: string } };
    processed?: boolean;
}

function rejectLoadError(error: unknown, reject: (reason?: unknown) => void): void {
    logger.error('failed to retrieve transaction statistics', error);
    const loadError = error as StatisticsLoadError;

    if (loadError.response?.data?.message) {
        reject({ error: loadError.response.data });
    } else if (!loadError.processed) {
        reject({ message: 'Unable to retrieve transaction statistics' });
    } else {
        reject(error);
    }
}

export function createStatisticsLoaders(refs: StatisticsLoaderRefs) {
    let categoricalAnalysisRequestGeneration = 0;
    let trendAnalysisRequestGeneration = 0;
    let assetTrendsRequestGeneration = 0;

    function invalidateRequests(): void {
        categoricalAnalysisRequestGeneration += 1;
        trendAnalysisRequestGeneration += 1;
        assetTrendsRequestGeneration += 1;
    }

    function loadCategoricalAnalysis({ force }: LoadOptions): Promise<TransactionStatisticResponse> {
        const requestGeneration = ++categoricalAnalysisRequestGeneration;
        return new Promise((resolve, reject) => {
            services.getTransactionStatistics({
                startTime: refs.transactionStatisticsFilter.value.categoricalChartStartTime,
                endTime: refs.transactionStatisticsFilter.value.categoricalChartEndTime,
                tagIds: refs.transactionStatisticsFilter.value.tagIds,
                tagFilterType: refs.transactionStatisticsFilter.value.tagFilterType,
                keyword: refs.transactionStatisticsFilter.value.keyword,
                useTransactionTimezone: refs.useTransactionTimezone()
            }).then(response => {
                const data = response.data;

                if (!data?.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                    return;
                }
                if (requestGeneration !== categoricalAnalysisRequestGeneration) {
                    resolve(data.result);
                    return;
                }
                refs.transactionStatisticsStateInvalid.value = false;
                if (force && isEquals(refs.transactionCategoryStatisticsData.value, data.result)) {
                    reject({ message: 'Data is up to date', isUpToDate: true });
                    return;
                }
                refs.transactionCategoryStatisticsData.value = data.result;
                resolve(data.result);
            }).catch(error => rejectLoadError(error, reject));
        });
    }

    function loadTrendAnalysis({ force }: LoadOptions): Promise<TransactionStatisticTrendsResponseItem[]> {
        const requestGeneration = ++trendAnalysisRequestGeneration;
        return new Promise((resolve, reject) => {
            const filter = refs.transactionStatisticsFilter.value;
            const isAllDateRange = filter.trendChartDateType === DateRange.All.type;

            services.getTransactionStatisticsTrends({
                startYearMonth: isAllDateRange ? '197001' : filter.trendChartStartYearMonth,
                endYearMonth: isAllDateRange ? '197001' : filter.trendChartEndYearMonth,
                tagIds: filter.tagIds,
                tagFilterType: filter.tagFilterType,
                keyword: filter.keyword,
                useTransactionTimezone: refs.useTransactionTimezone()
            }).then(response => {
                const data = response.data;

                if (!data?.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                    return;
                }
                if (requestGeneration !== trendAnalysisRequestGeneration) {
                    resolve(data.result);
                    return;
                }
                refs.transactionStatisticsStateInvalid.value = false;
                if (force && isEquals(refs.transactionCategoryTrendsData.value, data.result)) {
                    reject({ message: 'Data is up to date', isUpToDate: true });
                    return;
                }
                refs.transactionCategoryTrendsData.value = data.result;
                resolve(data.result);
            }).catch(error => rejectLoadError(error, reject));
        });
    }

    function loadAssetTrends({ force }: LoadOptions): Promise<TransactionStatisticAssetTrendsResponseItem[]> {
        const requestGeneration = ++assetTrendsRequestGeneration;
        return new Promise((resolve, reject) => {
            const filter = refs.transactionStatisticsFilter.value;

            services.getTransactionStatisticsAssetTrends({
                startTime: filter.assetTrendsChartStartTime,
                endTime: filter.assetTrendsChartEndTime
            }).then(response => {
                const data = response.data;

                if (!data?.success || !data.result) {
                    reject({ message: 'Unable to retrieve transaction statistics' });
                    return;
                }
                if (requestGeneration !== assetTrendsRequestGeneration) {
                    resolve(data.result);
                    return;
                }
                refs.transactionStatisticsStateInvalid.value = false;
                if (force && isEquals(refs.transactionAssetTrendsData.value, data.result)) {
                    reject({ message: 'Data is up to date', isUpToDate: true });
                    return;
                }
                refs.transactionAssetTrendsData.value = data.result;
                resolve(data.result);
            }).catch(error => rejectLoadError(error, reject));
        });
    }

    return {
        invalidateRequests,
        loadCategoricalAnalysis,
        loadTrendAnalysis,
        loadAssetTrends
    };
}
