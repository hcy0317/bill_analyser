<template>
    <div ref="chartContainer" :style="{ height: height, width: width }"></div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import * as echarts from 'echarts';
import type { EChartsOption, ECharts } from 'echarts';

// Props
const props = withDefaults(defineProps<{
    title?: string;
    data: Array<{ name: string; value: number }>;
    height?: string;
    width?: string;
    showLegend?: boolean;
    showLabel?: boolean;
    type?: 'pie' | 'doughnut';
}>(), {
    height: '400px',
    width: '100%',
    showLegend: true,
    showLabel: true,
    type: 'pie'
});

// State
const chartContainer = ref<HTMLElement | null>(null);
let chartInstance: ECharts | null = null;

// Color palette
const colorPalette = [
    '#5470c6', '#91cc75', '#fac858', '#ee6666', '#73c0de',
    '#3ba272', '#fc8452', '#9a60b4', '#ea7ccc', '#546570'
];

// Methods
const initChart = (): void => {
    if (!chartContainer.value) return;

    chartInstance = echarts.init(chartContainer.value);

    const option: EChartsOption = {
        title: props.title ? {
            text: props.title,
            left: 'center',
            textStyle: {
                fontSize: 16,
                fontWeight: 'normal'
            }
        } : undefined,
        tooltip: {
            trigger: 'item',
            formatter: (params: any) => {
                return `<div style="font-weight: bold; margin-bottom: 5px;">${params.name}</div>
                    <div>金额: <span style="font-weight: bold;">¥${params.value.toFixed(2)}</span></div>
                    <div>占比: <span style="font-weight: bold;">${params.percent.toFixed(2)}%</span></div>`;
            }
        },
        legend: props.showLegend ? {
            orient: 'vertical',
            left: 'left',
            top: 'middle',
            formatter: (name: string) => {
                const item = props.data.find(d => d.name === name);
                if (item) {
                    const total = props.data.reduce((sum, d) => sum + d.value, 0);
                    const percent = total > 0 ? ((item.value / total) * 100).toFixed(1) : 0;
                    return `${name}: ${percent}%`;
                }
                return name;
            }
        } : undefined,
        toolbox: {
            feature: {
                saveAsImage: {
                    title: '保存为图片',
                    pixelRatio: 2
                }
            }
        },
        series: [
            {
                name: '分类统计',
                type: 'pie',
                radius: props.type === 'doughnut' ? ['40%', '70%'] : '70%',
                center: ['60%', '50%'],
                data: props.data.map((item, index) => ({
                    name: item.name,
                    value: item.value,
                    itemStyle: {
                        color: colorPalette[index % colorPalette.length]
                    }
                })),
                emphasis: {
                    itemStyle: {
                        shadowBlur: 10,
                        shadowOffsetX: 0,
                        shadowColor: 'rgba(0, 0, 0, 0.5)'
                    }
                },
                label: props.showLabel ? {
                    formatter: '{b}: ¥{c}\n({d}%)',
                    fontSize: 12
                } : {
                    show: false
                },
                labelLine: props.showLabel ? {
                    show: true
                } : {
                    show: false
                }
            }
        ]
    };

    chartInstance.setOption(option);
};

const updateChart = (): void => {
    if (!chartInstance) return;

    chartInstance.setOption({
        series: [
            {
                data: props.data.map((item, index) => ({
                    name: item.name,
                    value: item.value,
                    itemStyle: {
                        color: colorPalette[index % colorPalette.length]
                    }
                }))
            }
        ]
    });
};

const resizeChart = (): void => {
    if (chartInstance) {
        chartInstance.resize();
    }
};

// Lifecycle
onMounted(() => {
    initChart();
    window.addEventListener('resize', resizeChart);
});

onUnmounted(() => {
    window.removeEventListener('resize', resizeChart);
    if (chartInstance) {
        chartInstance.dispose();
        chartInstance = null;
    }
});

watch(() => props.data, () => {
    if (chartInstance && props.data.length > 0) {
        updateChart();
    }
}, { deep: true });

watch(() => props.type, () => {
    if (chartInstance) {
        initChart();
    }
});

// Expose
defineExpose({
    refresh: initChart,
    resize: resizeChart
});
</script>

<style scoped>
/* ECharts容器样式 */
</style>
