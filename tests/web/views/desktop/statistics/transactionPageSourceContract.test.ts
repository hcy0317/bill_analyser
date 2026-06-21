import { describe, expect, test } from '@jest/globals';

import { readSource } from '../../../helpers/vueSource';

describe('desktop transaction statistics page source contract', () => {
    const readTransactionPageSource = () => readSource('src/views/desktop/statistics/TransactionPage.vue');

    test('keeps the statistics page facade wired to store, export dialog, and filter params', () => {
        const source = readTransactionPageSource();

        [
            "import ExportDialog from '@/views/desktop/statistics/transaction/dialogs/ExportDialog.vue'",
            "import { type TransactionStatisticsPartialFilter, useStatisticsStore } from '@/stores/statistics.ts'",
            'const statisticsStore = useStatisticsStore()',
            "const exportDialog = useTemplateRef<ExportDialogType>('exportDialog')",
            'statisticsStore.initTransactionStatisticsFilter',
            'statisticsStore.updateTransactionStatisticsFilter',
            'statisticsStore.getTransactionStatisticsPageParams',
            'statisticsStore.getTransactionListPageParams',
            'statisticsStore.loadCategoricalAnalysis',
            'statisticsStore.loadTrendAnalysis',
            'statisticsStore.loadAssetTrends'
        ].forEach(requiredSource => {
            expect(source).toContain(requiredSource);
        });
    });

    test('keeps chart drilldown, date controls, and export actions attached', () => {
        const source = readTransactionPageSource();

        [
            'function getFilterLinkUrl(): string',
            'function getTransactionItemLinkUrl(itemId: string, dateRange?: TimeRangeAndDateType): string',
            'function setAnalysisType(type: StatisticsAnalysisType): void',
            'function setTrendDateAggregationType(type: number): void',
            'function setAssetTrendsDateAggregationType(type: number): void',
            'function setDateFilter(dateType: number): void',
            'function setCustomDateFilter(startTime: number | TextualYearMonth, endTime: number | TextualYearMonth): void',
            'function exportResults(): void',
            "function onClickSankeyChartItem(sourceItemType: 'account' | 'category'",
            'function onClickPieChartItem(item: Record<string, unknown>): void',
            'function onClickTrendChartItem(item: { itemId: string, dateRange: TimeRangeAndDateType }): void',
            'router.push(getFilterLinkUrl())',
            'router.push(getTransactionItemLinkUrl'
        ].forEach(requiredSource => {
            expect(source).toContain(requiredSource);
        });
    });
});
