import { describe, expect, test } from '@jest/globals';

import { ChartDateAggregationType, AccountBalanceTrendChartType } from '@/core/statistics.ts';
import {
    chartDataDateAggregationTypeIconMap,
    chartTypeIconMap
} from '@/views/desktop/accounts/list/dialogs/reconciliationStatementDialogOptions.ts';

describe('reconciliation statement dialog options', () => {
    test('exposes chart and date aggregation icon maps', () => {
        expect(chartTypeIconMap[AccountBalanceTrendChartType.Column.type]).toBeTruthy();
        expect(chartTypeIconMap[AccountBalanceTrendChartType.Area.type]).toBeTruthy();
        expect(chartDataDateAggregationTypeIconMap[ChartDateAggregationType.Day.type]).toBeTruthy();
        expect(chartDataDateAggregationTypeIconMap[ChartDateAggregationType.FiscalYear.type]).toBeTruthy();
    });
});
