<template>
    <div class="mobile-echarts-pie-chart-container">
        <v-chart autoresize class="mobile-echarts-pie-chart" :class="{ 'transition-in': skeleton }" :option="chartOptions"
                 @click="clickItem" />
    </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

import type { ECElementEvent } from 'echarts/core';
import type { CallbackDataParams } from 'echarts/types/dist/shared';

import { useI18n } from '@/locales/helpers.ts';
import { type CommonPieChartDataItem, type CommonPieChartProps, usePieChartBase } from '@/components/base/PieChartBase.ts';

import { useEnvironmentsStore } from '@/stores/environment.ts';

import type { ColorStyleValue } from '@/core/color.ts';

import { getTheme } from '@/lib/settings.ts';
import {
    createScrollableLegendTheme,
    resolveMobileChartThemePalette,
    truncateChartLabel
} from '@/lib/chartTheme.ts';

interface MobileEChartsPieChartDataItem extends CommonPieChartDataItem {
    itemStyle: {
        color: ColorStyleValue;
    };
    selected: boolean;
}

const props = defineProps<CommonPieChartProps>();

const emit = defineEmits<{
    (e: 'click', value: Record<string, unknown>): void;
}>();

const { formatAmountToLocalizedNumeralsWithCurrency } = useI18n();
const { validItems } = usePieChartBase(props);
const environmentsStore = useEnvironmentsStore();

const chartTheme = computed(() => resolveMobileChartThemePalette(
    getTheme(),
    environmentsStore.framework7DarkMode || false
));

const seriesData = computed<MobileEChartsPieChartDataItem[]>(() => {
    const ret: MobileEChartsPieChartDataItem[] = [];

    for (const item of validItems.value) {
        ret.push({
            ...item,
            itemStyle: {
                color: item.color,
            },
            selected: true
        });
    }

    return ret;
});

const chartOptions = computed<object>(() => {
    return {
        tooltip: {
            trigger: 'item',
            confine: true,
            renderMode: 'richText',
            backgroundColor: chartTheme.value.tooltipBackground,
            borderColor: chartTheme.value.tooltipBorder,
            textStyle: {
                color: chartTheme.value.tooltipText,
                fontSize: 12
            },
            formatter: (params: CallbackDataParams) => {
                const dataItem = params.data as MobileEChartsPieChartDataItem;
                const name = dataItem ? dataItem.displayName : '';
                const value = dataItem?.displayValue || formatAmountToLocalizedNumeralsWithCurrency(params.value as number);
                const percent = dataItem?.displayPercent || (params.percent + '%');

                const tooltipLines: string[] = [];

                if (name) {
                    tooltipLines.push(name);
                }

                if (props.showValue && props.showPercent) {
                    tooltipLines.push(`${value} (${percent})`);
                } else if (props.showValue && !props.showPercent) {
                    tooltipLines.push(value);
                } else if (!props.showValue && props.showPercent) {
                    tooltipLines.push(percent);
                }

                return tooltipLines.join('\n');
            }
        },
        legend: {
            orient: 'horizontal',
            ...createScrollableLegendTheme(chartTheme.value, { mobile: true }),
            top: 0,
            data: seriesData.value.map(item => item.displayName),
            formatter: (name: string) => truncateChartLabel(name, 18),
            tooltip: {
                show: true,
                formatter: (params: { name?: string }) => params.name || ''
            },
        },
        series: [
            {
                type: 'pie',
                data: seriesData.value,
                top: 28,
                bottom: 8,
                center: ['50%', '56%'],
                radius: ['42%', '82%'],
                avoidLabelOverlap: true,
                emphasis: {
                    itemStyle: {
                        shadowBlur: 8,
                        shadowOffsetX: 0,
                        shadowColor: 'rgba(0, 0, 0, 0.4)',
                    },
                    label: {
                        show: true,
                        fontSize: 12,
                        fontWeight: 'bold'
                    }
                },
                label: {
                    show: true,
                    color: chartTheme.value.text,
                    fontSize: 10,
                    formatter: (params: CallbackDataParams) => {
                        const dataItem = params.data as MobileEChartsPieChartDataItem;
                        if (!dataItem) {
                            return '';
                        }
                        if (dataItem.actualPercent < 0.05) {
                            return '';
                        }
                        return truncateChartLabel(dataItem.displayName, 14);
                    }
                },
                labelLine: {
                    length: 8,
                    length2: 8
                },
                animation: !props.skeleton
            }
        ]
    };
});

function clickItem(e: ECElementEvent): void {
    if (!props.enableClickItem || e.componentType !== 'series' || e.seriesType !== 'pie') {
        return;
    }

    if (!e.data) {
        return;
    }

    const data = e.data as object;

    if ('sourceItem' in data) {
        emit('click', data.sourceItem as Record<string, unknown>);
    }
}
</script>

<style scoped>
.mobile-echarts-pie-chart-container {
    width: 100%;
    min-height: 430px;
    height: min(58vh, 520px);
    padding: 0 4px;
}

.mobile-echarts-pie-chart {
    width: 100%;
    height: 100%;
}

.mobile-echarts-pie-chart.transition-in {
    animation: mobile-echarts-pie-chart-fade-in 1.5s 1;
}

@keyframes mobile-echarts-pie-chart-fade-in {
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
