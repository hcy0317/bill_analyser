import { AccountBalanceTrendChartType, ChartDateAggregationType } from '@/core/statistics.ts';
import {
    mdiCalendarMonthOutline,
    mdiCalendarTodayOutline,
    mdiChartAreasplineVariant,
    mdiChartBar,
    mdiChartWaterfall,
    mdiLayersTripleOutline
} from '@mdi/js';

export const chartTypeIconMap: Record<number, string> = {
    [AccountBalanceTrendChartType.Column.type]: mdiChartBar,
    [AccountBalanceTrendChartType.Area.type]: mdiChartAreasplineVariant,
    [AccountBalanceTrendChartType.Candlestick.type]: mdiChartWaterfall,
};

export const chartDataDateAggregationTypeIconMap: Record<number, string> = {
    [ChartDateAggregationType.Day.type]: mdiCalendarTodayOutline,
    [ChartDateAggregationType.Month.type]: mdiCalendarMonthOutline,
    [ChartDateAggregationType.Quarter.type]: mdiLayersTripleOutline,
    [ChartDateAggregationType.Year.type]: mdiLayersTripleOutline,
    [ChartDateAggregationType.FiscalYear.type]: mdiLayersTripleOutline,
};
