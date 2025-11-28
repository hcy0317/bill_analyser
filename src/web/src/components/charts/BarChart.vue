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
    horizontal?: boolean;
    showLabel?: boolean;
    color?: string;
}>(), {
    height: '400px',
    width: '100%',
    horizontal: false,
    showLabel: true,
    color: '#5470c6'
});

// State
const chartContainer = ref<HTMLElement | null>(null);
let chartInstance: ECharts | null = null;

// Methods
const initChart = (): void => {
    if (!chartContainer.value) return;

    chartInstance = echarts.init(chartContainer.value);

    const names = props.data.map(item => item.name);
    const values = props.data.map(item => item.value);

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
            trigger: 'axis',
            axisPointer: {
                type: 'shadow'
            },
            formatter: (params: any) => {
                const param = params[0];
                return `<div style="font-weight: bold; margin-bottom: 5px;">${param.name}</div>
                    <div>${param.marker} 金额: <span style="font-weight: bold;">¥${Math.abs(param.value).toFixed(2)}</span></div>`;
            }
        },
        grid: {
            left: props.horizontal ? '15%' : '3%',
            right: '4%',
            bottom: props.horizontal ? '3%' : '10%',
            containLabel: !props.horizontal
        },
        toolbox: {
            feature: {
                saveAsImage: {
                    title: '保存为图片',
                    pixelRatio: 2
                }
            }
        },
        xAxis: props.horizontal ? {
            type: 'value',
            axisLabel: {
                formatter: '¥{value}'
            }
        } : {
            type: 'category',
            data: names,
            axisLabel: {
                interval: 0,
                rotate: 45
            }
        },
        yAxis: props.horizontal ? {
            type: 'category',
            data: names,
            axisLabel: {
                interval: 0
            }
        } : {
            type: 'value',
            axisLabel: {
                formatter: '¥{value}'
            },
            splitLine: {
                lineStyle: {
                    type: 'dashed'
                }
            }
        },
        series: [
            {
                name: '金额',
                type: 'bar',
                data: values,
                itemStyle: {
                    color: props.color,
                    borderRadius: props.horizontal ? [0, 4, 4, 0] : [4, 4, 0, 0]
                },
                label: props.showLabel ? {
                    show: true,
                    position: props.horizontal ? 'right' : 'top',
                    formatter: (params: any) => `¥${Math.abs(params.value).toFixed(0)}`
                } : undefined,
                emphasis: {
                    itemStyle: {
                        shadowBlur: 10,
                        shadowOffsetX: 0,
                        shadowColor: 'rgba(0, 0, 0, 0.5)'
                    }
                }
            }
        ]
    };

    chartInstance.setOption(option);
};

const updateChart = (): void => {
    if (!chartInstance) return;

    const names = props.data.map(item => item.name);
    const values = props.data.map(item => item.value);

    if (props.horizontal) {
        chartInstance.setOption({
            yAxis: {
                data: names
            },
            series: [
                { data: values }
            ]
        });
    } else {
        chartInstance.setOption({
            xAxis: {
                data: names
            },
            series: [
                { data: values }
            ]
        });
    }
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

watch(() => props.horizontal, () => {
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
