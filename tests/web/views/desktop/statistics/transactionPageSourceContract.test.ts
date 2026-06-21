import { describe, expect, test } from '@jest/globals';

import { readSource } from '../../../helpers/vueSource';

describe('desktop transaction statistics page source contract', () => {
    const readTransactionPageDomainSource = () => [
        'src/views/desktop/statistics/TransactionPage.vue',
        'src/views/desktop/statistics/transaction/TransactionPage.template.html',
        'src/views/desktop/statistics/transaction/pageLinks.ts'
    ].map(readSource).join('\n');

    test('keeps the statistics page facade wired to store, export dialog, and filter params', () => {
        const source = readTransactionPageDomainSource();

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
        const source = readTransactionPageDomainSource();

        [
            'function getFilterLinkUrl(',
            'function getTransactionItemLinkUrl(',
            'function setAnalysisType(type: StatisticsAnalysisType): void',
            'function setTrendDateAggregationType(type: number): void',
            'function setAssetTrendsDateAggregationType(type: number): void',
            'function setDateFilter(dateType: number): void',
            'function setCustomDateFilter(startTime: number | TextualYearMonth, endTime: number | TextualYearMonth): void',
            'function exportResults(): void',
            "function onClickSankeyChartItem(sourceItemType: 'account' | 'category'",
            'function onClickPieChartItem(item: Record<string, unknown>): void',
            'function onClickTrendChartItem(item: { itemId: string, dateRange: TimeRangeAndDateType }): void',
            'router.push(getFilterLinkUrl(statisticsStore',
            'router.push(getTransactionItemLinkUrl'
        ].forEach(requiredSource => {
            expect(source).toContain(requiredSource);
        });
    });
});
