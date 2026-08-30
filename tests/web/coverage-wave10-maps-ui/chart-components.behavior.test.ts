/* eslint-disable @typescript-eslint/no-require-imports, @typescript-eslint/no-explicit-any */
import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockChartInstances: any[] = [];
const mockEchartsInit = jest.fn((container: unknown) => {
    const instance = {
        container,
        setOption: jest.fn(),
        resize: jest.fn(),
        dispose: jest.fn()
    };
    mockChartInstances.push(instance);
    return instance;
});
let mockThemeName = 'light';
let mockMobileDark = false;
let mockPieItems: any[] = [];

const lightChartPalette = {
    primaryRgb: '25, 118, 210', onPrimary: '#fff', surface: '#fff', text: '#333', muted: '#666',
    grid: '#e1e6f2', border: '#ddd', tooltipBackground: '#fff', tooltipText: '#333',
    tooltipBorder: '#fff', inactive: '#aaa'
};
const darkChartPalette = {
    primaryRgb: '144, 202, 249', onPrimary: '#111', surface: '#222', text: '#eee', muted: '#888',
    grid: '#4f4f4f', border: '#555', tooltipBackground: '#333', tooltipText: '#eee',
    tooltipBorder: '#333', inactive: '#555'
};

jest.mock('echarts', () => ({ init: (container: unknown) => mockEchartsInit(container) }));
jest.mock('vuetify', () => ({ useTheme: () => ({ global: { name: { get value() { return mockThemeName; } } } }) }));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: number, currency?: string) => `${currency || 'CNY'}:${value}`,
        formatPercentToLocalizedNumerals: (value: number, precision: number, low: string) => `percent:${value}:${precision}:${low}`
    })
}));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: (name: string) => name === 'dark' }));
jest.mock('@/stores/environment.ts', () => ({
    useEnvironmentsStore: () => ({ get framework7DarkMode() { return mockMobileDark; } })
}));
jest.mock('@/lib/settings.ts', () => ({ getTheme: () => 'auto' }));
jest.mock('@/lib/chartTheme.ts', () => ({
    getChartThemePalette: () => mockThemeName === 'dark' ? darkChartPalette : lightChartPalette,
    resolveMobileChartThemePalette: (_theme: string, darkMode: boolean) => darkMode ? darkChartPalette : lightChartPalette,
    truncateChartLabel: (value: string, maxLength: number) => value.length > maxLength ? value.slice(0, maxLength) + '…' : value,
    createScrollableLegendTheme: (palette: typeof lightChartPalette, options: { mobile?: boolean; fontSize?: number } = {}) => ({
        type: 'scroll', left: 4, right: 4, tooltip: { show: true },
        textStyle: { color: palette.text, fontSize: options.fontSize ?? (options.mobile ? 11 : 12) },
        pageIconColor: palette.muted, pageIconInactiveColor: palette.inactive,
        pageTextStyle: { color: palette.text }
    })
}));
jest.mock('@/consts/color.ts', () => ({ DEFAULT_CHART_COLORS: ['fallback-red', 'fallback-blue'] }));
jest.mock('@/lib/common.ts', () => ({
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value)
}));
jest.mock('@/lib/color.ts', () => ({ getDisplayColor: (value: unknown) => `display:${String(value)}` }));
jest.mock('@/components/base/PieChartBase.ts', () => {
    const vue = jest.requireActual('vue') as any;
    return {
        usePieChartBase: () => ({
            selectedIndex: vue.ref(0),
            validItems: vue.computed(() => mockPieItems)
        })
    };
});

const TrendChart = require('@/components/charts/TrendChart.vue').default as any;
const BarChart = require('@/components/charts/BarChart.vue').default as any;
const DesktopPieChart = require('@/components/charts/PieChart.vue').default as any;
const RadarChart = require('@/components/desktop/RadarChart.vue').default as any;
const MobilePieChart = require('@/components/mobile/PieChart.vue').default as any;
const EChartsPieChart = require('@/components/mobile/EChartsPieChart.vue').default as any;
const { nextTick, reactive } = jest.requireActual('vue') as any;
jest.spyOn(console, 'warn').mockImplementation(() => undefined);

function setup(component: any, props: Record<string, unknown>) {
    const emit = jest.fn();
    const exposed: any = {};
    const bindings = component.setup(props, {
        attrs: {}, slots: {}, emit,
        expose: (value: Record<string, any>) => Object.assign(exposed, value)
    });
    return { bindings, emit, exposed };
}

function latestChart() {
    return mockChartInstances.at(-1)!;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockChartInstances.length = 0;
    mockThemeName = 'light';
    mockMobileDark = false;
    mockPieItems = [];
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: {
            documentElement: { classList: { contains: () => false } },
            body: { classList: { contains: () => false } }
        }
    });
});

describe('TrendChart production ECharts contract', () => {
    function props(overrides: Record<string, unknown> = {}) {
        return reactive({
            title: 'Cash flow',
            data: [
                { date: '2026-01-01', income: 120.5, expense: -20.25, net: 100.25 },
                { date: '2026-01-02', income: undefined, expense: undefined, net: undefined }
            ],
            height: '320px', width: '90%', showLegend: true, showTooltip: true, showZoom: true,
            ...overrides
        });
    }

    test('builds titled income/expense/net series and formats tooltip and date labels', () => {
        const mounted = setup(TrendChart, props());
        expect(() => mounted.bindings.initChart()).not.toThrow();
        mounted.bindings.chartContainer.value = { id: 'trend' };
        mounted.exposed.refresh();
        const option = latestChart().setOption.mock.calls[0][0] as any;
        expect(option.title.text).toBe('Cash flow');
        expect(option.legend).toEqual(expect.objectContaining({ data: ['收入', '支出', '净收入'], top: 30 }));
        expect(option.grid.bottom).toBe('15%');
        expect(option.series.map((series: any) => series.data)).toEqual([[120.5, 0], [20.25, 0], [100.25, 0]]);
        expect(option.xAxis.axisLabel.formatter('2026-01-02')).toBe('01-02');
        expect(option.tooltip.formatter([
            { axisValue: '2026-01-01', seriesName: '收入', marker: '●', value: 120.5 },
            { axisValue: '2026-01-01', seriesName: '支出', marker: '●', value: 20.25 }
        ])).toContain('¥-20.25');
        mounted.exposed.resize();
        expect(latestChart().resize).toHaveBeenCalled();
    });

    test('omits optional chrome, updates changed data and guards before initialization', async () => {
        const chartProps = props({ title: undefined, showLegend: false, showTooltip: false, showZoom: false });
        const mounted = setup(TrendChart, chartProps);
        expect(() => mounted.bindings.updateChart()).not.toThrow();
        expect(() => mounted.exposed.resize()).not.toThrow();
        mounted.bindings.chartContainer.value = {};
        mounted.exposed.refresh();
        const option = latestChart().setOption.mock.calls[0][0] as any;
        expect(option).toEqual(expect.objectContaining({ title: undefined, legend: undefined, tooltip: undefined, dataZoom: undefined }));
        expect(option.grid.bottom).toBe('3%');

        chartProps.data = [{ date: '2026-02-01', income: 5, expense: -7, net: -2 }];
        await nextTick();
        expect(latestChart().setOption).toHaveBeenCalledWith({
            xAxis: { data: ['2026-02-01'] },
            series: [{ data: [5] }, { data: [7] }, { data: [-2] }]
        });
        chartProps.data = [];
        await nextTick();
        expect(latestChart().setOption).toHaveBeenCalledTimes(2);
    });
});

describe('BarChart vertical/horizontal behavior', () => {
    function props(overrides: Record<string, unknown> = {}) {
        return reactive({
            title: 'By category',
            data: [{ name: 'Food', value: -123.45 }, { name: 'Salary', value: 200 }],
            height: '400px', width: '100%', horizontal: false, showLabel: true, color: '#123456',
            ...overrides
        });
    }

    test('builds vertical axes, positive display labels and tooltip', () => {
        const mounted = setup(BarChart, props());
        mounted.bindings.chartContainer.value = {};
        mounted.exposed.refresh();
        const option = latestChart().setOption.mock.calls[0][0] as any;
        expect(option.title.text).toBe('By category');
        expect(option.xAxis).toEqual(expect.objectContaining({ type: 'category', data: ['Food', 'Salary'] }));
        expect(option.yAxis.type).toBe('value');
        expect(option.grid).toEqual(expect.objectContaining({ left: '3%', bottom: '10%', containLabel: true }));
        expect(option.series[0].itemStyle).toEqual({ color: '#123456', borderRadius: [4, 4, 0, 0] });
        expect(option.series[0].label.formatter({ value: -123.45 })).toBe('¥123');
        expect(option.tooltip.formatter([{ name: 'Food', marker: '●', value: -123.45 }])).toContain('¥123.45');
    });

    test('builds horizontal/no-label mode and updates the correct axis', async () => {
        const chartProps = props({ title: undefined, horizontal: true, showLabel: false });
        const mounted = setup(BarChart, chartProps);
        expect(() => mounted.bindings.updateChart()).not.toThrow();
        expect(() => mounted.bindings.resizeChart()).not.toThrow();
        mounted.bindings.chartContainer.value = {};
        mounted.exposed.refresh();
        const first = latestChart();
        const option = first.setOption.mock.calls[0][0] as any;
        expect(option.title).toBeUndefined();
        expect(option.xAxis.type).toBe('value');
        expect(option.yAxis.data).toEqual(['Food', 'Salary']);
        expect(option.series[0].label).toBeUndefined();
        expect(option.series[0].itemStyle.borderRadius).toEqual([0, 4, 4, 0]);

        chartProps.data = [{ name: 'Rent', value: 88 }];
        await nextTick();
        expect(first.setOption).toHaveBeenCalledWith({ yAxis: { data: ['Rent'] }, series: [{ data: [88] }] });
        chartProps.horizontal = false;
        await nextTick();
        expect(mockEchartsInit).toHaveBeenCalledTimes(2);
        mounted.exposed.resize();
        expect(latestChart().resize).toHaveBeenCalled();
    });
});

describe('desktop PieChart legend, palette and doughnut behavior', () => {
    function props(overrides: Record<string, unknown> = {}) {
        return reactive({
            title: 'Spending',
            data: [{ name: 'Food', value: 25 }, { name: 'Rent', value: 75 }],
            height: '400px', width: '100%', showLegend: true, showLabel: true, type: 'pie',
            ...overrides
        });
    }

    test('formats tooltip, legend percentages and unknown/zero-total names', () => {
        const mounted = setup(DesktopPieChart, props());
        mounted.bindings.chartContainer.value = {};
        mounted.exposed.refresh();
        const option = latestChart().setOption.mock.calls[0][0] as any;
        expect(option.series[0].radius).toBe('70%');
        expect(option.series[0].data[0]).toEqual(expect.objectContaining({
            name: 'Food', value: 25, itemStyle: { color: '#5470c6' }
        }));
        expect(option.legend.formatter('Food')).toBe('Food: 25.0%');
        expect(option.legend.formatter('Missing')).toBe('Missing');
        expect(option.tooltip.formatter({ name: 'Food', value: 25, percent: 25 })).toContain('25.00%');

        const zero = setup(DesktopPieChart, props({ data: [{ name: 'Zero', value: 0 }] }));
        zero.bindings.chartContainer.value = {};
        zero.exposed.refresh();
        expect(latestChart().setOption.mock.calls[0][0].legend.formatter('Zero')).toBe('Zero: 0%');
    });

    test('supports doughnut without legend/labels, updates palette and guards empty data', async () => {
        const chartProps = props({ title: undefined, showLegend: false, showLabel: false, type: 'doughnut' });
        const mounted = setup(DesktopPieChart, chartProps);
        expect(() => mounted.bindings.updateChart()).not.toThrow();
        mounted.bindings.chartContainer.value = {};
        mounted.exposed.refresh();
        const first = latestChart();
        const option = first.setOption.mock.calls[0][0] as any;
        expect(option).toEqual(expect.objectContaining({ title: undefined, legend: undefined }));
        expect(option.series[0].radius).toEqual(['40%', '70%']);
        expect(option.series[0].label).toEqual({ show: false });
        expect(option.series[0].labelLine).toEqual({ show: false });

        chartProps.data = Array.from({ length: 11 }, (_, index) => ({ name: `Item ${index}`, value: index }));
        await nextTick();
        const update = first.setOption.mock.calls.at(-1)![0];
        expect(update.series[0].data[10].itemStyle.color).toBe('#5470c6');
        chartProps.data = [];
        await nextTick();
        expect(first.setOption.mock.calls.at(-1)![0]).toBe(update);
        chartProps.type = 'pie';
        await nextTick();
        expect(mockEchartsInit).toHaveBeenCalledTimes(2);
    });
});

describe('RadarChart values, thresholds, display modes and themes', () => {
    function props(overrides: Record<string, unknown> = {}) {
        return reactive({
            skeleton: false,
            items: [
                { name: 'Food', value: 60, percent: 61, color: 'red', hidden: false },
                { name: 'Rent', value: 35, percent: undefined, color: '', hidden: false },
                { name: 'Tiny', value: 5, percent: -1, hidden: false },
                { name: 'Hidden', value: 100, hidden: true },
                { name: 'Zero', value: 0, hidden: false },
                { name: 'Invalid', value: Number.NaN, hidden: false }
            ],
            nameField: 'name', valueField: 'value', percentField: 'percent', colorField: 'color', hiddenField: 'hidden',
            minValidPercent: 0.05, defaultCurrency: 'USD', showValue: true, showPercent: true,
            ...overrides
        });
    }

    test('excludes hidden/invalid/threshold values and formats value-plus-percent tooltip', () => {
        const mounted = setup(RadarChart, props());
        expect(mounted.bindings.radarData.value).toEqual(expect.objectContaining({
            totalValidValue: 100,
            maxValue: 60,
            values: [60, 35]
        }));
        expect(mounted.bindings.radarData.value.indicators).toEqual([
            { name: 'Food', max: 60, color: '#333' },
            { name: 'Rent', max: 60, color: '#333' }
        ]);
        expect(mounted.bindings.radarData.value.tooltip).toContain('USD:60');
        expect(mounted.bindings.radarData.value.tooltip).toContain('percent:61');
        expect(mounted.bindings.radarData.value.tooltip).toContain('display:red');
        expect(mounted.bindings.radarData.value.tooltip).toContain('display:fallback-blue');
        expect(mounted.bindings.chartOptions.value.tooltip.formatter()).toBe(mounted.bindings.radarData.value.tooltip);
        expect(mounted.bindings.chartOptions.value.radar.splitNumber).toBe(5);
        expect(mounted.bindings.chartOptions.value.series[0].data[0].value).toEqual([60, 35]);
    });

    test.each([
        [true, false, 'USD:60', false],
        [false, true, 'percent:61', false],
        [false, false, '</div>', true]
    ])('supports showValue=%s and showPercent=%s', (showValue, showPercent, text, excludesCurrency) => {
        const mounted = setup(RadarChart, props({ showValue, showPercent, minValidPercent: undefined }));
        expect(mounted.bindings.radarData.value.tooltip).toContain(text);
        if (excludesCurrency) expect(mounted.bindings.radarData.value.tooltip).not.toContain('USD:60');
    });

    test('renders six empty skeleton axes and dark chart colors', () => {
        mockThemeName = 'dark';
        const mounted = setup(RadarChart, props({ skeleton: true, items: [] }));
        expect(mounted.bindings.chartTheme.value.tooltipBackground).toBe('#333');
        expect(mounted.bindings.radarData.value.indicators).toHaveLength(6);
        expect(mounted.bindings.radarData.value.values).toEqual([0, 0, 0, 0, 0, 0]);
        const option = mounted.bindings.chartOptions.value as any;
        expect(option.tooltip.backgroundColor).toBe('#333');
        expect(option.radar.splitNumber).toBe(1);
        expect(option.radar.splitArea.areaStyle.color).toEqual(['#555', '#555']);
        expect(option.series).toEqual([]);
    });

    test('renders signed account flow amounts as magnitudes and skips an empty effective series', () => {
        const signed = setup(RadarChart, props({
            minValidPercent: undefined,
            items: [
                { name: 'Expense account', value: -60, hidden: false },
                { name: 'Zero', value: 0, hidden: false },
                { name: 'Hidden', value: -90, hidden: true },
            ],
        }));

        expect(signed.bindings.radarData.value).toEqual(expect.objectContaining({
            totalValidValue: 60,
            maxValue: 60,
            values: [60],
        }));
        expect(signed.bindings.chartOptions.value.series[0].data[0].value).toEqual([60]);

        const noEffectiveItems = setup(RadarChart, props({
            items: [
                { name: 'Zero', value: 0, hidden: false },
                { name: 'Hidden', value: 100, hidden: true },
            ],
        }));
        expect(noEffectiveItems.bindings.radarData.value.indicators).toEqual([]);
        expect(noEffectiveItems.bindings.chartOptions.value.series).toEqual([]);
    });
});

describe('mobile SVG PieChart interaction and geometry', () => {
    function props(overrides: Record<string, unknown> = {}) {
        return reactive({
            items: [], valueField: 'value', nameField: 'name', colorField: 'color', hiddenField: 'hidden',
            skeleton: false, showValue: true, showPercent: true, enableClickItem: true,
            showCenterText: true, showSelectedItemInfo: true, centerTextBackground: 'purple',
            ...overrides
        });
    }

    test('computes segment geometry, selected item, wrap-around and click event', () => {
        mockPieItems = [
            { value: 60, actualPercent: 0.6, color: 'red', displayName: 'Food', displayValue: '60', displayPercent: '60%', sourceItem: { id: 'food' } },
            { value: 40, actualPercent: 0.4, color: 'blue', displayName: 'Rent', displayValue: '40', displayPercent: '40%', sourceItem: { id: 'rent' } }
        ];
        const mounted = setup(MobilePieChart, props());
        expect(mounted.bindings.totalValidValue.value).toBe(100);
        expect(mounted.bindings.selectedItem.value.sourceItem.id).toBe('food');
        expect(mounted.bindings.getColorStyle('red')).toEqual({ color: 'red' });
        expect(mounted.bindings.getColorStyle('red', '--border')).toEqual({ color: 'red', '--border': 'red' });
        const dash = mounted.bindings.getItemStrokeDash(mockPieItems[0]).split(' ').map(Number);
        expect(dash[0]).toBeCloseTo(60 * Math.PI);
        expect(dash[1]).toBeCloseTo(40 * Math.PI);
        expect(mounted.bindings.getItemDashOffset(mockPieItems[0], mockPieItems)).toBe(25 * Math.PI);
        expect(mounted.bindings.getItemDashOffset(mockPieItems[1], mockPieItems, 10)).toBeCloseTo(65 * Math.PI + 10);
        expect(mounted.bindings.itemCommonDashOffset.value).toBeCloseTo(-20 * Math.PI);

        mounted.bindings.switchSelectedItem(-1);
        expect(mounted.bindings.selectedIndex.value).toBe(1);
        expect(mounted.bindings.itemCommonDashOffset.value).toBeCloseTo(-70 * Math.PI);
        mounted.bindings.switchSelectedItem(1);
        expect(mounted.bindings.selectedIndex.value).toBe(0);
        mounted.bindings.switchSelectedIndex(99);
        expect(mounted.bindings.selectedItem.value.sourceItem.id).toBe('food');
        mounted.bindings.switchSelectedIndex(-1);
        expect(mounted.bindings.selectedItem.value.sourceItem.id).toBe('food');
        mounted.bindings.clickItem(mockPieItems[0]);
        expect(mounted.emit).toHaveBeenCalledWith('click', { id: 'food' });
    });

    test('handles empty/zero pies and disabled click', () => {
        mockPieItems = [];
        const empty = setup(MobilePieChart, props({ enableClickItem: false }));
        expect(empty.bindings.totalValidValue.value).toBe(0);
        expect(empty.bindings.itemCommonDashOffset.value).toBe(0);
        expect(empty.bindings.selectedItem.value).toBeNull();
        empty.bindings.clickItem({ sourceItem: { id: 'ignored' } });
        expect(empty.emit).not.toHaveBeenCalled();

        mockPieItems = [{ value: 0, actualPercent: 0, color: 'gray', sourceItem: {} }];
        const zero = setup(MobilePieChart, props());
        expect(zero.bindings.itemCommonDashOffset.value).toBe(0);
        expect(zero.bindings.getItemDashOffset(mockPieItems[0], mockPieItems, 0)).toBe(25 * Math.PI);
    });
});

describe('mobile EChartsPieChart tooltip, labels and click boundary', () => {
    function props(overrides: Record<string, unknown> = {}) {
        return reactive({
            items: [], valueField: 'value', nameField: 'name', colorField: 'color', hiddenField: 'hidden',
            skeleton: false, showValue: true, showPercent: true, enableClickItem: true,
            ...overrides
        });
    }

    test('projects display items and formats light tooltip and label thresholds', () => {
        mockPieItems = [
            { value: 60, actualPercent: 0.6, color: 'red', displayName: 'Food', displayValue: 'USD:60', displayPercent: '60%', sourceItem: { id: 'food' } },
            { value: 1, actualPercent: 0.01, color: 'gray', displayName: 'Tiny', displayValue: '', displayPercent: '', sourceItem: { id: 'tiny' } }
        ];
        const mounted = setup(EChartsPieChart, props());
        expect(mounted.bindings.chartTheme.value.tooltipBackground).toBe('#fff');
        expect(mounted.bindings.seriesData.value[0]).toEqual(expect.objectContaining({
            itemStyle: { color: 'red' }, selected: true
        }));
        const option = mounted.bindings.chartOptions.value as any;
        expect(option.tooltip.backgroundColor).toBe('#fff');
        expect(option.tooltip.formatter({ data: mounted.bindings.seriesData.value[0], value: 60, percent: 60 })).toBe('Food\nUSD:60 (60%)');
        expect(option.tooltip.formatter({ data: mounted.bindings.seriesData.value[1], value: 1, percent: 1 })).toBe('Tiny\nCNY:1 (1%)');
        expect(option.legend.formatter('Food')).toBe('Food');
        expect(option.legend.tooltip.formatter({ name: 'Food' })).toBe('Food');
        expect(option.series[0].label.formatter({ data: mounted.bindings.seriesData.value[0] })).toBe('Food');
        expect(option.series[0].label.formatter({ data: mounted.bindings.seriesData.value[1] })).toBe('');
        expect(option.series[0].label.formatter({ data: null })).toBe('');
    });

    test.each([
        [true, false, 'CNY:9'],
        [false, true, '9%'],
        [false, false, '']
    ])('formats fallback showValue=%s showPercent=%s', (showValue, showPercent, expected) => {
        mockPieItems = [];
        const mounted = setup(EChartsPieChart, props({ showValue, showPercent }));
        const formatter = mounted.bindings.chartOptions.value.tooltip.formatter;
        expect(formatter({ data: null, value: 9, percent: 9 })).toBe(expected);
    });

    test('uses active mobile dark theme and emits only valid pie-series source items', () => {
        mockMobileDark = true;
        Object.defineProperty(globalThis, 'document', {
            configurable: true,
            value: {
                documentElement: { classList: { contains: (name: string) => name === 'theme-dark' } },
                body: { classList: { contains: () => false } }
            }
        });
        mockPieItems = [{ value: 10, actualPercent: 1, color: 'red', displayName: 'One', sourceItem: { id: 'one' } }];
        const mounted = setup(EChartsPieChart, props({ skeleton: true }));
        expect(mounted.bindings.chartTheme.value.tooltipBackground).toBe('#333');
        expect(mounted.bindings.chartOptions.value.tooltip.backgroundColor).toBe('#333');
        expect(mounted.bindings.chartOptions.value.series[0].animation).toBe(false);

        for (const event of [
            { componentType: 'legend', seriesType: 'pie', data: mounted.bindings.seriesData.value[0] },
            { componentType: 'series', seriesType: 'line', data: mounted.bindings.seriesData.value[0] },
            { componentType: 'series', seriesType: 'pie', data: null },
            { componentType: 'series', seriesType: 'pie', data: { value: 1 } }
        ]) mounted.bindings.clickItem(event);
        expect(mounted.emit).not.toHaveBeenCalled();
        mounted.bindings.clickItem({ componentType: 'series', seriesType: 'pie', data: mounted.bindings.seriesData.value[0] });
        expect(mounted.emit).toHaveBeenCalledWith('click', { id: 'one' });

        const disabled = setup(EChartsPieChart, props({ enableClickItem: false }));
        disabled.bindings.clickItem({ componentType: 'series', seriesType: 'pie', data: { sourceItem: { id: 'ignored' } } });
        expect(disabled.emit).not.toHaveBeenCalled();
    });
});
