<template>
    <v-chart autoresize class="radar-chart-container" :class="{ 'transition-in': skeleton }" :option="chartOptions" />
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { useTheme } from 'vuetify';

import { useI18n } from '@/locales/helpers.ts';

import type { ColorValue } from '@/core/color.ts';
import { DEFAULT_CHART_COLORS } from '@/consts/color.ts';

import { isNumber } from '@/lib/common.ts';
import { getDisplayColor } from '@/lib/color.ts';
import { getChartThemePalette } from '@/lib/chartTheme.ts';

interface RadarChartData {
    totalValidValue: number;
    maxValue: number;
    indicators: RadarChartDataItem[];
    values: number[];
    tooltip: string;
}

interface RadarChartDataItem {
    name: string;
    max: number;
    color: string;
}

const props = defineProps<{
    skeleton?: boolean;
    items: Record<string, unknown>[];
    nameField: string;
    valueField: string;
    percentField?: string;
    colorField?: string;
    hiddenField?: string;
    minValidPercent?: number;
    defaultCurrency?: string;
    showValue?: boolean;
    showPercent?: boolean;
}>();

const theme = useTheme();

const { formatAmountToLocalizedNumeralsWithCurrency, formatPercentToLocalizedNumerals } = useI18n();

const chartTheme = computed(() => getChartThemePalette(theme.global.name.value));

const radarData = computed<RadarChartData>(() => {
    let totalValidValue = 0;
    let maxValue = 0;
    const indicators: RadarChartDataItem[] = [];
    const values: number[] = [];
    let tooltip = '';

    if (props.items.length) {
        for (const item of props.items) {
            const value = item[props.valueField];

            if (isNumber(value) && value > 0 && (!props.hiddenField || !item[props.hiddenField])) {
                totalValidValue += value;

                if (value > maxValue) {
                    maxValue = value;
                }
            }
        }

        for (const item of props.items) {
            const value = item[props.valueField];
            const percent = props.percentField ? item[props.percentField] : -1;

            if (isNumber(value) && value > 0 &&
                (!props.hiddenField || !item[props.hiddenField]) &&
                (!props.minValidPercent || value / totalValidValue > props.minValidPercent)) {
                const name = item[props.nameField] as string;
                const color = getDisplayColor((props.colorField && item[props.colorField]) ? item[props.colorField] as ColorValue : DEFAULT_CHART_COLORS[indicators.length % DEFAULT_CHART_COLORS.length]);

                const finalPercent = (isNumber(percent) && percent >= 0) ? percent : (value / totalValidValue * 100);
                const displayPercent = formatPercentToLocalizedNumerals(finalPercent, 2, '&lt;0.01');
                const displayValue = formatAmountToLocalizedNumeralsWithCurrency(value, props.defaultCurrency);

                indicators.push({
                    name: name,
                    max: maxValue,
                    color: chartTheme.value.text
                });

                values.push(value);

                tooltip += '<div><span class="chart-pointer" style="background-color: ' + color + '"></span>';
                tooltip += `<span>${name}</span>`;

                if (props.showValue && props.showPercent) {
                    tooltip += `<span class="ms-1" style="float: inline-end">(${displayPercent})</span><span class="ms-5" style="float: inline-end">${displayValue}</span>`;
                } else if (props.showValue && !props.showPercent) {
                    tooltip += `<span class="ms-5" style="float: inline-end">${displayValue}</span>`;
                } else if (!props.showValue && props.showPercent) {
                    tooltip += `<span class="ms-5" style="float: inline-end">${displayPercent}</span>`;
                }

                tooltip += '</div>';
            }
        }
    } else {
        for (let i = 0; i < 6; i++) {
            indicators.push({
                name: '',
                max: 0,
                color: chartTheme.value.text
            });
            values.push(0);
        }
    }

    const ret: RadarChartData = {
        totalValidValue: totalValidValue,
        maxValue: maxValue,
        indicators: indicators,
        values: values,
        tooltip: tooltip
    };

    return ret;
});

const chartOptions = computed<object>(() => {
    return {
        tooltip: {
            trigger: 'item',
            backgroundColor: chartTheme.value.tooltipBackground,
            borderColor: chartTheme.value.tooltipBorder,
            textStyle: {
                color: chartTheme.value.tooltipText
            },
            formatter: () => radarData.value.tooltip
        },
        radar: {
            radius: '75%',
            splitNumber: (!props.skeleton && props.items.length) ? 5 : 1,
            splitLine: {
                lineStyle: {
                    color: (!props.skeleton && props.items.length) ? chartTheme.value.grid : chartTheme.value.inactive
                }
            },
            splitArea: {
                areaStyle: {
                    color: (!props.skeleton && props.items.length)
                        ? [chartTheme.value.surface, chartTheme.value.grid]
                        : [chartTheme.value.inactive, chartTheme.value.inactive]
                }
            },
            indicator: radarData.value.indicators
        },
        series: (!props.skeleton && props.items.length) ? [
            {
                type: 'radar',
                data: [
                    {
                        value: radarData.value.values,
                        itemStyle: {
                            color: `rgb(${chartTheme.value.primaryRgb})`
                        },
                        lineStyle: {
                            color: `rgb(${chartTheme.value.primaryRgb})`
                        },
                        areaStyle: {
                            color: `rgba(${chartTheme.value.primaryRgb}, 0.32)`
                        }
                    }
                ],
                top: 0,
                emphasis: {
                    itemStyle: {
                        shadowBlur: 10,
                        shadowOffsetX: 0,
                        shadowColor: 'rgba(0, 0, 0, 0.5)',
                    }
                },
                animation: !props.skeleton
            }
        ] : []
    };
});
</script>

<style scoped>
.radar-chart-container {
    width: 100%;
    height: 460px;
}

@media (min-width: 600px) {
    .radar-chart-container {
        height: 650px;
    }
}

.radar-chart-container.transition-in {
    animation: radar-chart-skeleton-fade-in 2s 1;
}

@keyframes radar-chart-skeleton-fade-in {
    0% {
        opacity: 0;
    }
    20% {
        opacity: 0;
    }
    100% {
        opacity: 1;
    }
}
</style>
