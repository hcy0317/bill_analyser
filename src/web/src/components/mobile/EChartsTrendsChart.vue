<template>
    <div class="mobile-echarts-trends-chart-container">
        <v-chart autoresize class="mobile-echarts-trends-chart" :option="chartOptions"
                 @click="clickItem" />
    </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import type { ECElementEvent } from 'echarts/core';
import type { CallbackDataParams } from 'echarts/types/dist/shared';

import { useI18n } from '@/locales/helpers.ts';
import {
    type TrendsChartDateType,
    type CommonTrendsChartProps,
    type TrendsBarChartClickEvent,
    useTrendsChartBase
} from '@/components/base/TrendsChartBase.ts';

import { useEnvironmentsStore } from '@/stores/environment.ts';
import { useUserStore } from '@/stores/user.ts';

import { itemAndIndex } from '@/core/base.ts';
import {
    type Year1BasedMonth,
    type YearMonthDay,
    DateRangeScene
} from '@/core/datetime.ts';
import type { ColorStyleValue } from '@/core/color.ts';
import {
    ChartDataAggregationType,
    TrendChartType,
    AccountBalanceTrendChartType,
    ChartDateAggregationType
} from '@/core/statistics.ts';

import { DEFAULT_CHART_COLORS } from '@/consts/color.ts';

import {
    isArray,
    isNumber
} from '@/lib/common.ts';
import {
    getYearMonthFirstUnixTime,
    getYearMonthLastUnixTime,
    getDateTypeByDateRange,
    getFiscalYearFromUnixTime
} from '@/lib/datetime.ts';
import {
    getDisplayColor
} from '@/lib/color.ts';
import { getTheme } from '@/lib/settings.ts';
import {
    createScrollableLegendTheme,
    resolveMobileChartThemePalette,
    truncateChartLabel
} from '@/lib/chartTheme.ts';

interface MobileEChartsTrendsChartProps<T extends TrendsChartDateType> extends CommonTrendsChartProps<T> {
    skeleton?: boolean;
    type?: number;
}

interface MobileTrendsChartDataItem {
    id: string;
    name: string;
    itemStyle: {
        color: ColorStyleValue;
    };
    type: string;
    areaStyle?: object;
    stack?: string;
    symbolSize?: (data: number) => number;
    animation: boolean;
    data: (number | null | number[])[];
}

const props = defineProps<MobileEChartsTrendsChartProps<TrendsChartDateType>>();

const emit = defineEmits<{
    (e: 'click', value: TrendsBarChartClickEvent): void;
}>();

const {
    formatUnixTimeToShortDate,
    formatUnixTimeToGregorianLikeShortYear,
    formatUnixTimeToGregorianLikeShortYearMonth,
    formatYearQuarterToGregorianLikeYearQuarter,
    formatUnixTimeToGregorianLikeFiscalYear,
    formatAmountToLocalizedNumeralsWithCurrency
} = useI18n();

const { allDateRanges, getItemName } = useTrendsChartBase(props);

const environmentsStore = useEnvironmentsStore();
const userStore = useUserStore();

const selectedLegends = ref<Record<string, boolean>>({});

const chartTheme = computed(() => resolveMobileChartThemePalette(
    getTheme(),
    environmentsStore.framework7DarkMode || false
));

const itemsMap = computed<Record<string, Record<string, unknown>>>(() => {
    const map: Record<string, Record<string, unknown>> = {};

    for (const item of props.items) {
        const id: string = (props.idField && item[props.idField]) ? item[props.idField] as string : getItemName(item[props.nameField] as string);
        const finalItem: Record<string, unknown> = { [props.nameField]: item[props.nameField] };

        if (props.idField) {
            finalItem[props.idField] = item[props.idField];
        }
        if (props.hiddenField) {
            finalItem[props.hiddenField] = item[props.hiddenField];
        }

        map[id] = finalItem;
    }

    return map;
});

function getLegendDisplayName(id: string): string {
    const item = itemsMap.value[id];
    if (item && props.nameField && item[props.nameField]) {
        return getItemName(item[props.nameField] as string);
    }
    return id;
}

const allDisplayDateRanges = computed<string[]>(() => {
    const result: string[] = [];

    for (const dateRange of allDateRanges.value) {
        if (props.dateAggregationType === ChartDateAggregationType.Year.type) {
            result.push(formatUnixTimeToGregorianLikeShortYear(dateRange.minUnixTime));
        } else if (props.dateAggregationType === ChartDateAggregationType.FiscalYear.type && 'year' in dateRange) {
            result.push(formatUnixTimeToGregorianLikeFiscalYear(dateRange.minUnixTime));
        } else if (props.dateAggregationType === ChartDateAggregationType.Quarter.type && 'quarter' in dateRange) {
            result.push(formatYearQuarterToGregorianLikeYearQuarter(dateRange.year, dateRange.quarter));
        } else if (props.dateAggregationType === ChartDateAggregationType.Month.type) {
            result.push(formatUnixTimeToGregorianLikeShortYearMonth(dateRange.minUnixTime));
        } else if (props.dateAggregationType === ChartDateAggregationType.Day.type && props.chartMode === 'daily') {
            result.push(formatUnixTimeToShortDate(dateRange.minUnixTime));
        }
    }

    return result;
});

const allSeries = computed<MobileTrendsChartDataItem[]>(() => {
    const result: MobileTrendsChartDataItem[] = [];
    let maxAmount = 0;

    for (const [item, index] of itemAndIndex(props.items)) {
        if (props.hiddenField && item[props.hiddenField]) {
            continue;
        }

        const allAmounts: (number | null | number[])[] = [];
        const dateRangeAmountMap: Record<string, (Year1BasedMonth | YearMonthDay)[]> = {};

        for (const dataItem of item.items) {
            let dateRangeKey = '';

            if (props.chartMode === 'daily' && 'month' in dataItem) {
                if (props.dateAggregationType === ChartDateAggregationType.Year.type) {
                    dateRangeKey = dataItem.year.toString();
                } else if (props.dateAggregationType === ChartDateAggregationType.FiscalYear.type) {
                    const fiscalYear = getFiscalYearFromUnixTime(
                        getYearMonthFirstUnixTime({ year: dataItem.year, month1base: dataItem.month }),
                        props.fiscalYearStart
                    );
                    dateRangeKey = fiscalYear.toString();
                } else if (props.dateAggregationType === ChartDateAggregationType.Quarter.type) {
                    dateRangeKey = `${dataItem.year}-${Math.floor((dataItem.month - 1) / 3) + 1}`;
                } else if (props.dateAggregationType === ChartDateAggregationType.Month.type) {
                    dateRangeKey = `${dataItem.year}-${dataItem.month}`;
                } else {
                    dateRangeKey = `${dataItem.year}-${dataItem.month}-${dataItem.day}`;
                }
            } else if (props.chartMode === 'monthly' && 'month1base' in dataItem) {
                if (props.dateAggregationType === ChartDateAggregationType.Year.type) {
                    dateRangeKey = dataItem.year.toString();
                } else if (props.dateAggregationType === ChartDateAggregationType.FiscalYear.type) {
                    const fiscalYear = getFiscalYearFromUnixTime(
                        getYearMonthFirstUnixTime({ year: dataItem.year, month1base: dataItem.month1base }),
                        props.fiscalYearStart
                    );
                    dateRangeKey = fiscalYear.toString();
                } else if (props.dateAggregationType === ChartDateAggregationType.Quarter.type) {
                    dateRangeKey = `${dataItem.year}-${Math.floor((dataItem.month1base - 1) / 3) + 1}`;
                } else {
                    dateRangeKey = `${dataItem.year}-${dataItem.month1base}`;
                }
            }

            const dataItems = dateRangeAmountMap[dateRangeKey] || [];
            dataItems.push(dataItem);
            dateRangeAmountMap[dateRangeKey] = dataItems;
        }

        for (const dateRange of allDateRanges.value) {
            let dateRangeKey = '';

            if (props.dateAggregationType === ChartDateAggregationType.Year.type) {
                dateRangeKey = dateRange.year.toString();
            } else if (props.dateAggregationType === ChartDateAggregationType.FiscalYear.type && 'year' in dateRange) {
                dateRangeKey = dateRange.year.toString();
            } else if (props.dateAggregationType === ChartDateAggregationType.Quarter.type && 'quarter' in dateRange) {
                dateRangeKey = `${dateRange.year}-${dateRange.quarter}`;
            } else if (props.dateAggregationType === ChartDateAggregationType.Month.type && 'month0base' in dateRange) {
                dateRangeKey = `${dateRange.year}-${dateRange.month0base + 1}`;
            } else if (props.dateAggregationType === ChartDateAggregationType.Day.type && 'day' in dateRange && props.chartMode === 'daily') {
                dateRangeKey = `${dateRange.year}-${dateRange.month}-${dateRange.day}`;
            }

            let amount = 0;
            let open = 0;
            let close = 0;
            let min = Number.MAX_SAFE_INTEGER;
            let max = Number.MIN_SAFE_INTEGER;
            let hasData = false;
            const dataItems = dateRangeAmountMap[dateRangeKey];

            if (isArray(dataItems)) {
                for (let i = 0; i < dataItems.length; i++) {
                    const dataItem = dataItems[i];
                    const value = (dataItem as unknown as Record<string, unknown>)[props.valueField] as number;
                    const openingValue = (dataItem as unknown as Record<string, unknown>)['totalOpeningAmount'] as number;

                    if (isNumber(value)) {
                        hasData = true;
                        if (props.dataAggregationType === ChartDataAggregationType.Sum) {
                            amount += value;
                        } else if (props.dataAggregationType === ChartDataAggregationType.Last) {
                            amount = value;
                        }

                        if (props.type === AccountBalanceTrendChartType.Candlestick.type) {
                            if (i === 0) {
                                open = isNumber(openingValue) ? openingValue : value;
                            }
                            if (i === dataItems.length - 1) {
                                close = value;
                            }
                            if (value < min) min = value;
                            if (value > max) max = value;
                            if (isNumber(openingValue)) {
                                if (openingValue < min) min = openingValue;
                                if (openingValue > max) max = openingValue;
                            }
                        }
                    }
                }
            }

            if (Math.abs(amount) > maxAmount) {
                maxAmount = Math.abs(amount);
            }

            if (props.type === TrendChartType.Bubble.type && amount === 0) {
                allAmounts.push(null);
            } else if (props.type === AccountBalanceTrendChartType.Candlestick.type) {
                if (hasData) {
                    allAmounts.push([open, close, min, max]);
                } else {
                    allAmounts.push(null);
                }
            } else {
                allAmounts.push(amount);
            }
        }

        const finalItem: MobileTrendsChartDataItem = {
            id: (props.idField && item[props.idField]) ? item[props.idField] as string : getItemName(item[props.nameField] as string),
            name: (props.idField && item[props.idField]) ? item[props.idField] as string : getItemName(item[props.nameField] as string),
            itemStyle: {
                color: getDisplayColor(props.colorField && item[props.colorField] ? item[props.colorField] as string : DEFAULT_CHART_COLORS[index % DEFAULT_CHART_COLORS.length]),
            },
            type: 'line',
            animation: !props.skeleton,
            data: allAmounts
        };

        if (props.stacked) {
            finalItem.stack = 'a';
        } else if (props.idField && item[props.idField]) {
            finalItem.stack = item[props.idField] as string;
        }

        if (props.type === TrendChartType.Area.type) {
            finalItem.areaStyle = {};
        } else if (props.type === TrendChartType.Column.type) {
            finalItem.type = 'bar';
        } else if (props.type === TrendChartType.Bubble.type) {
            finalItem.type = 'scatter';
            finalItem.symbolSize = (data: number): number => {
                return Math.sqrt(Math.abs(data)) / Math.sqrt(maxAmount) * 30 + 8;
            };
        }

        result.push(finalItem);
    }

    return result;
});

const chartOptions = computed<object>(() => {
    return {
        tooltip: {
            trigger: 'axis',
            confine: true,
            renderMode: 'richText',
            backgroundColor: chartTheme.value.tooltipBackground,
            borderColor: chartTheme.value.tooltipBorder,
            textStyle: {
                color: chartTheme.value.tooltipText,
                fontSize: 11
            },
            formatter: (params: CallbackDataParams | CallbackDataParams[]) => {
                const items = isArray(params) ? params : [params];
                const tooltipLines: string[] = [];

                if (items.length && items[0] && items[0].name) {
                    tooltipLines.push(String(items[0].name));
                }

                for (const param of items) {
                    let amount = 0;
                    if (isArray(param.data)) {
                        const dataArray = param.data as number[];
                        if (dataArray.length >= 2) {
                            amount = dataArray[1] ?? 0;
                        }
                    } else {
                        amount = param.data as number;
                    }

                    if (amount !== 0) {
                        const value = formatAmountToLocalizedNumeralsWithCurrency(amount, props.defaultCurrency);
                        tooltipLines.push(`${getLegendDisplayName(String(param.seriesName || ''))}  ${value}`);
                    }
                }

                return tooltipLines.join('\n');
            }
        },
        legend: {
            orient: 'horizontal',
            ...createScrollableLegendTheme(chartTheme.value, { mobile: true, fontSize: 10 }),
            top: 0,
            data: allSeries.value.map(item => item.name),
            selected: selectedLegends.value,
            formatter: (id: string) => truncateChartLabel(getLegendDisplayName(id), 18),
            tooltip: {
                show: true,
                formatter: (params: { name?: string }) => getLegendDisplayName(params.name || '')
            }
        },
        grid: {
            left: 8,
            right: 8,
            top: 36,
            bottom: 28,
            containLabel: true
        },
        xAxis: [
            {
                type: 'category',
                data: allDisplayDateRanges.value,
                axisLabel: {
                    color: chartTheme.value.muted,
                    fontSize: 9,
                    interval: 'auto',
                    rotate: allDisplayDateRanges.value.length > 8 ? 30 : 0
                }
            }
        ],
        yAxis: [
            {
                type: 'value',
                axisLabel: {
                    color: chartTheme.value.muted,
                    fontSize: 9,
                    formatter: (value: number) => {
                        if (Math.abs(value) >= 10000) {
                            return (value / 10000).toFixed(1) + 'w';
                        }
                        if (Math.abs(value) >= 1000) {
                            return (value / 1000).toFixed(1) + 'k';
                        }
                        return value.toString();
                    }
                },
                splitLine: {
                    lineStyle: {
                        color: chartTheme.value.grid,
                    }
                }
            }
        ],
        series: allSeries.value
    };
});

function clickItem(e: ECElementEvent): void {
    if (!props.enableClickItem || e.componentType !== 'series') {
        return;
    }

    const id = e.seriesId as string;
    const item = itemsMap.value[id] as Record<string, unknown>;
    const itemId = props.idField ? item[props.idField] as string : '';
    const dateRange = allDateRanges.value[e.dataIndex];

    if (!dateRange) {
        return;
    }

    let minUnixTime = dateRange.minUnixTime;
    let maxUnixTime = dateRange.maxUnixTime;

    if (props.chartMode === 'daily') {
        if (props.startTime && props.startTime > minUnixTime) {
            minUnixTime = props.startTime;
        }
        if (props.endTime && props.endTime < maxUnixTime) {
            maxUnixTime = props.endTime;
        }
    } else if (props.chartMode === 'monthly') {
        if (props.startYearMonth) {
            const startMinUnixTime = getYearMonthFirstUnixTime(props.startYearMonth);
            if (startMinUnixTime > minUnixTime) {
                minUnixTime = startMinUnixTime;
            }
        }
        if (props.endYearMonth) {
            const endMaxUnixTime = getYearMonthLastUnixTime(props.endYearMonth);
            if (endMaxUnixTime < maxUnixTime) {
                maxUnixTime = endMaxUnixTime;
            }
        }
    }

    const dateRangeType = getDateTypeByDateRange(minUnixTime, maxUnixTime, userStore.currentUserFirstDayOfWeek, userStore.currentUserFiscalYearStart, DateRangeScene.Normal);

    emit('click', {
        itemId: itemId,
        dateRange: {
            minTime: minUnixTime,
            maxTime: maxUnixTime,
            dateType: dateRangeType
        }
    });
}
</script>

<style scoped>
.mobile-echarts-trends-chart-container {
    width: 100%;
    height: 380px;
    padding: 4px;
}

.mobile-echarts-trends-chart {
    width: 100%;
    height: 100%;
}
</style>
