<template>
    <v-chart autoresize class="account-balance-trends-chart-container" :class="{ 'transition-in': skeleton }" :option="chartOptions"/>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTheme } from 'vuetify';
import type { CallbackDataParams } from 'echarts/types/dist/shared';

import { useI18n } from '@/locales/helpers.ts';
import {
    type AccountBalanceTrendsChartItem,
    type CommonAccountBalanceTrendsChartProps,
    useAccountBalanceTrendsChartBase
} from '@/components/base/AccountBalanceTrendsChartBase.ts'

import { useUserStore } from '@/stores/user.ts';

import { type NameValue, itemAndIndex } from '@/core/base.ts';
import { TextDirection } from '@/core/text.ts';
import type { ColorStyleValue } from '@/core/color.ts';
import { ThemeType } from '@/core/theme.ts';
import { AccountBalanceTrendChartType } from '@/core/statistics.ts';
import { DEFAULT_CHART_COLORS } from '@/consts/color.ts';

import { isArray } from '@/lib/common.ts';
import { getExpenseAndIncomeAmountColor } from '@/lib/ui/common.ts';

interface DesktopAccountBalanceTrendsChartProps extends CommonAccountBalanceTrendsChartProps {
    legendName: string;
    skeleton?: boolean;
    type?: number;
}

interface AccountBalanceTrendsChartDataItem {
    id: string;
    name: string;
    itemStyle: {
        color: ColorStyleValue;
        color0?: ColorStyleValue;
        borderColor?: ColorStyleValue;
        borderColor0?: ColorStyleValue;
    };
    selected: boolean;
    type: string;
    areaStyle?: object;
    stack: string;
    animation: boolean;
    data: (number | number[])[];
}

const props = defineProps<DesktopAccountBalanceTrendsChartProps>();

const theme = useTheme();
const { tt, getCurrentLanguageTextDirection, formatAmountToLocalizedNumeralsWithCurrency } = useI18n();
const { allDataItems, allDisplayDateRanges } = useAccountBalanceTrendsChartBase(props);

const userStore = useUserStore();

const textDirection = computed<TextDirection>(() => getCurrentLanguageTextDirection());
const isDarkMode = computed<boolean>(() => theme.global.name.value === ThemeType.Dark);

const allSeries = computed<AccountBalanceTrendsChartDataItem[]>(() => {
    console.log(`[AccountBalanceTrendsChart] allSeries计算开始 - type=${props.type}, dataItems数量=${allDataItems.value?.length || 0}`);

    const series: AccountBalanceTrendsChartDataItem = {
        id: 'accountBalance',
        name: props.legendName,
        itemStyle: {
            color: `#${DEFAULT_CHART_COLORS[0]}`
        },
        selected: true,
        type: 'line',
        stack: 'a',
        animation: !props.skeleton,
        data: []
    };

    if (props.type === AccountBalanceTrendChartType.Area.type) {
        series.areaStyle = {};
    } else if (props.type === AccountBalanceTrendChartType.Column.type) {
        series.type = 'bar';
    } else if (props.type === AccountBalanceTrendChartType.Candlestick.type) {
        console.log(`[AccountBalanceTrendsChart] 配置K线图样式`);
        const expenseIncomeAmountColor = getExpenseAndIncomeAmountColor(userStore.currentUserExpenseAmountColor, userStore.currentUserIncomeAmountColor, isDarkMode.value);
        series.type = 'candlestick';
        series.itemStyle.color = expenseIncomeAmountColor.incomeAmountColor;
        series.itemStyle.color0 = expenseIncomeAmountColor.expenseAmountColor;
        series.itemStyle.borderColor = expenseIncomeAmountColor.incomeAmountColor;
        series.itemStyle.borderColor0 = expenseIncomeAmountColor.expenseAmountColor;
    }

    for (const item of allDataItems.value) {
        if (props.type === AccountBalanceTrendChartType.Candlestick.type) {
            // K线图数据格式: [开盘, 收盘, 最低, 最高]
            // 确保最低价和最高价包含开盘价和收盘价，避免柱体高度为0
            const lowest = Math.min(item.openingBalance, item.closingBalance, item.minimumBalance);
            const highest = Math.max(item.openingBalance, item.closingBalance, item.maximumBalance);

            const candlestickData = [
                item.openingBalance,
                item.closingBalance,
                lowest,
                highest
            ];
            series.data.push(candlestickData);

            console.log(`[AccountBalanceTrendsChart] ${item.displayDate} K线数据: [开=${candlestickData[0]}, 收=${candlestickData[1]}, 低=${candlestickData[2]}, 高=${candlestickData[3]}], 高度=${highest - lowest}`);
        } else {
            series.data.push(item.closingBalance);
        }
    }

    console.log(`[AccountBalanceTrendsChart] allSeries计算完成 - series.type=${series.type}, data点数=${series.data.length}`);
    return [series];
});

const yAxisWidth = computed<number>(() => {
    let maxValue = Number.MIN_SAFE_INTEGER;
    let minValue = Number.MAX_SAFE_INTEGER;
    let width = 90;

    if (!allSeries.value || !allSeries.value.length) {
        return width;
    }

    for (const series of allSeries.value) {
        for (const data of series.data) {
            let value: number;

            if (isArray(data)) {
                value = data[1] as number; // for candlestick, use closing balance
            } else {
                value = data as number; // for line or bar chart
            }

            if (value > maxValue) {
                maxValue = value;
            }

            if (value < minValue) {
                minValue = value;
            }
        }
    }

    const maxValueText = formatAmountToLocalizedNumeralsWithCurrency(maxValue, props.account.currency);
    const minValueText = formatAmountToLocalizedNumeralsWithCurrency(minValue, props.account.currency);
    const maxLengthText = maxValueText.length > minValueText.length ? maxValueText : minValueText;

    const canvas = document.createElement('canvas');
    const context = canvas.getContext('2d');

    if (context) {
        context.font = '12px Arial';

        const textMetrics = context.measureText(maxLengthText);
        const actualWidth = Math.round(textMetrics.width) + 20;

        if (actualWidth >= 200) {
            width = 200;
        } if (actualWidth > 90) {
            width = actualWidth;
        }
    }

    return width;
});

const chartOptions = computed<object>(() => {
    return {
        tooltip: {
            trigger: 'axis',
            axisPointer: {
                type: 'cross',
                label: {
                    backgroundColor: isDarkMode.value ? '#333' : '#fff',
                    color: isDarkMode.value ? '#eee' : '#333'
                },
            },
            backgroundColor: isDarkMode.value ? '#333' : '#fff',
            borderColor: isDarkMode.value ? '#333' : '#fff',
            textStyle: {
                color: isDarkMode.value ? '#eee' : '#333'
            },
            formatter: (params: CallbackDataParams[]) => {
                if (props.type === AccountBalanceTrendChartType.Candlestick.type) {
                    const dataIndex = params[0]!.dataIndex;
                    const dataItem = allDataItems.value[dataIndex] as AccountBalanceTrendsChartItem;
                    const displayItems: NameValue[] = [
                        {
                            name: tt('Opening Balance'),
                            value: formatAmountToLocalizedNumeralsWithCurrency(dataItem.openingBalance, props.account.currency)
                        },
                        {
                            name: tt('Closing Balance'),
                            value: formatAmountToLocalizedNumeralsWithCurrency(dataItem.closingBalance, props.account.currency)
                        },
                        {
                            name: tt('Minimum Balance'),
                            value: formatAmountToLocalizedNumeralsWithCurrency(dataItem.minimumBalance, props.account.currency)
                        },
                        {
                            name: tt('Maximum Balance'),
                            value: formatAmountToLocalizedNumeralsWithCurrency(dataItem.maximumBalance, props.account.currency)
                        },
                        {
                            name: tt('Median Balance'),
                            value: formatAmountToLocalizedNumeralsWithCurrency(dataItem.medianBalance, props.account.currency)
                        },
                        {
                            name: tt('Average Balance'),
                            value: formatAmountToLocalizedNumeralsWithCurrency(dataItem.averageBalance, props.account.currency)
                        }
                    ];

                    let tooltip = `${params[0]!.name} ${props.legendName}<br/>`;

                    for (const [displayItem, index] of itemAndIndex(displayItems)) {
                        tooltip += `<div><span class="chart-pointer" style="background-color: #${DEFAULT_CHART_COLORS[index]}"></span>`
                            + `<span>${displayItem.name}</span><span class="ms-5" style="float: inline-end">${displayItem.value}</span><br/>`
                            + `</div>`;
                    }

                    return tooltip;
                } else {
                    const amount = params[0]!.data as number;
                    const value = formatAmountToLocalizedNumeralsWithCurrency(amount, props.account.currency);

                    return `${params[0]!.name}<br/>`
                        + '<div><span class="chart-pointer" style="background-color: #' + DEFAULT_CHART_COLORS[0] + '"></span>'
                        + `<span>${props.legendName}</span><span class="ms-5" style="float: inline-end">${value}</span><br/>`
                        + '</div>';
                }
            }
        },
        grid: {
            left: yAxisWidth.value,
            right: 20,
            top: 0,
            bottom: 20
        },
        xAxis: [
            {
                type: 'category',
                data: allDisplayDateRanges.value,
                inverse: textDirection.value === TextDirection.RTL,
                axisLabel: {
                    color: isDarkMode.value ? '#888' : '#666'
                }
            }
        ],
        yAxis: [
            {
                type: 'value',
                scale: true,
                axisLabel: {
                    color: isDarkMode.value ? '#888' : '#666',
                    formatter: (value: string) => {
                        return formatAmountToLocalizedNumeralsWithCurrency(parseInt(value), props.account.currency);
                    }
                },
                axisPointer: {
                    label: {
                        formatter: (params: CallbackDataParams) => {
                            return formatAmountToLocalizedNumeralsWithCurrency(Math.trunc(params.value as number), props.account.currency);
                        }
                    }
                },
                splitLine: {
                    lineStyle: {
                        color: isDarkMode.value ? '#4f4f4f' : '#e1e6f2',
                    }
                }
            }
        ],
        series: allSeries.value.map(series => {
            if (series.type === 'candlestick') {
                // K线图：减少柱体间距，barWidth增大使柱体更明显
                return {
                    ...series,
                    barWidth: '80%',  // 增大柱体宽度，默认为'70%'
                    barMinHeight: 1,  // 设置最小高度，确保无变动天也能显示
                    itemStyle: {
                        ...series.itemStyle,
                        borderWidth: 1  // 边框宽度
                    }
                };
            } else if (series.type === 'bar') {
                // 柱状图：减少间距
                return {
                    ...series,
                    barWidth: '60%',
                    barGap: '10%'  // 减少柱子间距
                };
            }
            return series;
        })
    };
});
</script>

<style scoped>
.account-balance-trends-chart-container {
    width: 100%;
    height: 400px;
    margin-top: 10px;
}
</style>
