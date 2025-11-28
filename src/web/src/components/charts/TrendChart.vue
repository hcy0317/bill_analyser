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
    data: Array<{ date: string; income?: number; expense?: number; net?: number }>;
    height?: string;
    width?: string;
    showLegend?: boolean;
    showTooltip?: boolean;
    showZoom?: boolean;
}>(), {
    height: '400px',
    width: '100%',
    showLegend: true,
    showTooltip: true,
    showZoom: true
});

// State
const chartContainer = ref<HTMLElement | null>(null);
let chartInstance: ECharts | null = null;

// Methods
const initChart = (): void => {
    if (!chartContainer.value) return;

    chartInstance = echarts.init(chartContainer.value);

    const dates = props.data.map(item => item.date);
    const incomeData = props.data.map(item => item.income || 0);
    const expenseData = props.data.map(item => Math.abs(item.expense || 0));
    const netData = props.data.map(item => item.net || 0);

    const option: EChartsOption = {
        title: props.title ? {
            text: props.title,
            left: 'center',
            textStyle: {
                fontSize: 16,
                fontWeight: 'normal'
            }
        } : undefined,
        tooltip: props.showTooltip ? {
            trigger: 'axis',
            axisPointer: {
                type: 'cross',
                label: {
                    backgroundColor: '#6a7985'
                }
            },
            formatter: (params: any) => {
                let result = `<div style="font-weight: bold; margin-bottom: 5px;">${params[0].axisValue}</div>`;
                params.forEach((param: any) => {
                    const value = param.seriesName === '支出' ? -param.value : param.value;
                    result += `<div style="margin: 3px 0;">
                        ${param.marker} ${param.seriesName}: 
                        <span style="font-weight: bold;">¥${value.toFixed(2)}</span>
                    </div>`;
                });
                return result;
            }
        } : undefined,
        legend: props.showLegend ? {
            data: ['收入', '支出', '净收入'],
            top: props.title ? 30 : 0
        } : undefined,
        grid: {
            left: '3%',
            right: '4%',
            bottom: props.showZoom ? '15%' : '3%',
            containLabel: true
        },
        toolbox: {
            feature: {
                saveAsImage: {
                    title: '保存为图片',
                    pixelRatio: 2
                },
                dataZoom: {
                    yAxisIndex: 'none',
                    title: {
                        zoom: '区域缩放',
                        back: '还原'
                    }
                },
                restore: {
                    title: '还原'
                }
            }
        },
        xAxis: {
            type: 'category',
            boundaryGap: false,
            data: dates,
            axisLabel: {
                rotate: 45,
                formatter: (value: string) => {
                    // 格式化日期显示
                    return value.substring(5); // 只显示月-日
                }
            }
        },
        yAxis: {
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
        dataZoom: props.showZoom ? [
            {
                type: 'inside',
                start: 0,
                end: 100
            },
            {
                start: 0,
                end: 100
            }
        ] : undefined,
        series: [
            {
                name: '收入',
                type: 'line',
                smooth: true,
                symbol: 'circle',
                symbolSize: 6,
                data: incomeData,
                itemStyle: {
                    color: '#4caf50'
                },
                areaStyle: {
                    color: {
                        type: 'linear',
                        x: 0,
                        y: 0,
                        x2: 0,
                        y2: 1,
                        colorStops: [
                            { offset: 0, color: 'rgba(76, 175, 80, 0.3)' },
                            { offset: 1, color: 'rgba(76, 175, 80, 0.05)' }
                        ]
                    }
                }
            },
            {
                name: '支出',
                type: 'line',
                smooth: true,
                symbol: 'circle',
                symbolSize: 6,
                data: expenseData,
                itemStyle: {
                    color: '#f44336'
                },
                areaStyle: {
                    color: {
                        type: 'linear',
                        x: 0,
                        y: 0,
                        x2: 0,
                        y2: 1,
                        colorStops: [
                            { offset: 0, color: 'rgba(244, 67, 54, 0.3)' },
                            { offset: 1, color: 'rgba(244, 67, 54, 0.05)' }
                        ]
                    }
                }
            },
            {
                name: '净收入',
                type: 'line',
                smooth: true,
                symbol: 'diamond',
                symbolSize: 8,
                data: netData,
                itemStyle: {
                    color: '#2196f3'
                },
                lineStyle: {
                    width: 3,
                    type: 'solid'
                }
            }
        ]
    };

    chartInstance.setOption(option);
};

const updateChart = (): void => {
    if (!chartInstance) return;
    
    const dates = props.data.map(item => item.date);
    const incomeData = props.data.map(item => item.income || 0);
    const expenseData = props.data.map(item => Math.abs(item.expense || 0));
    const netData = props.data.map(item => item.net || 0);

    chartInstance.setOption({
        xAxis: {
            data: dates
        },
        series: [
            { data: incomeData },
            { data: expenseData },
            { data: netData }
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

// Expose
defineExpose({
    refresh: initChart,
    resize: resizeChart
});
</script>

<style scoped>
/* ECharts容器样式 */
</style>
