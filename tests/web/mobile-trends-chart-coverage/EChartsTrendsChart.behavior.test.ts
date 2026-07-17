import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockAggregations = { Sum: 'sum', Last: 'last' } as const;
const mockTrendTypes = { Area: { type: 1 }, Column: { type: 2 }, Bubble: { type: 3 } } as const;
const mockAccountTypes = { Candlestick: { type: 4 } } as const;
const mockDateTypes = {
    Year: { type: 10 }, FiscalYear: { type: 11 }, Quarter: { type: 12 }, Month: { type: 13 }, Day: { type: 14 }
} as const;

let mockRanges: any[] = [];
let mockDark = false;
const mockFirst = jest.fn<(...args: any[]) => number>(() => 130);
const mockLast = jest.fn<(...args: any[]) => number>(() => 170);
const mockDateType = jest.fn<(...args: any[]) => number>(() => 77);
const mockFiscal = jest.fn<(...args: any[]) => number>(() => 2024);

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        formatUnixTimeToShortDate: (value: number) => `day:${value}`,
        formatUnixTimeToGregorianLikeShortYear: (value: number) => `year:${value}`,
        formatUnixTimeToGregorianLikeShortYearMonth: (value: number) => `month:${value}`,
        formatYearQuarterToGregorianLikeYearQuarter: (year: number, quarter: number) => `quarter:${year}-${quarter}`,
        formatUnixTimeToGregorianLikeFiscalYear: (value: number) => `fiscal:${value}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: number, currency: string) => `${currency}:${value}`
    })
}));
jest.mock('@/components/base/TrendsChartBase.ts', () => {
    const vue = jest.requireActual('vue') as any;
    return {
        useTrendsChartBase: (props: any) => ({
            allDateRanges: vue.computed(() => mockRanges),
            getItemName: (name: string) => props.translateName ? `translated:${name}` : name
        })
    };
});
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({ currentUserFirstDayOfWeek: 1, currentUserFiscalYearStart: 4 })
}));
jest.mock('@/core/base.ts', () => ({ itemAndIndex: (items: any[]) => items.map((item, index) => [item, index]) }));
jest.mock('@/core/datetime.ts', () => ({ DateRangeScene: { Normal: 'normal' } }));
jest.mock('@/core/statistics.ts', () => ({
    ChartDataAggregationType: mockAggregations,
    TrendChartType: mockTrendTypes,
    AccountBalanceTrendChartType: mockAccountTypes,
    ChartDateAggregationType: mockDateTypes
}));
jest.mock('@/consts/color.ts', () => ({ DEFAULT_CHART_COLORS: ['fallback-0', 'fallback-1'] }));
jest.mock('@/lib/common.ts', () => ({
    isArray: Array.isArray,
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value)
}));
jest.mock('@/lib/datetime.ts', () => ({
    getYearMonthFirstUnixTime: (...args: any[]) => mockFirst(...args),
    getYearMonthLastUnixTime: (...args: any[]) => mockLast(...args),
    getDateTypeByDateRange: (...args: any[]) => mockDateType(...args),
    getFiscalYearFromUnixTime: (...args: any[]) => mockFiscal(...args)
}));
jest.mock('@/lib/color.ts', () => ({ getDisplayColor: (value: string) => `display:${value}` }));

const EChartsTrendsChart = require('@/components/mobile/EChartsTrendsChart.vue').default as any;

function item(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'food', name: 'Food', color: 'red', hidden: false,
        items: [
            { year: 2024, month: 1, day: 2, amount: 100, totalOpeningAmount: 90 },
            { year: 2024, month: 1, day: 3, amount: 200, totalOpeningAmount: 110 }
        ],
        ...overrides
    };
}

function props(overrides: Record<string, unknown> = {}): any {
    return {
        chartMode: 'daily', items: [item()], stacked: false,
        startTime: 120, endTime: 180, startYearMonth: undefined, endYearMonth: undefined,
        fiscalYearStart: 4, sortingType: 0,
        dataAggregationType: mockAggregations.Sum,
        dateAggregationType: mockDateTypes.Year.type,
        idField: 'id', nameField: 'name', valueField: 'amount', colorField: 'color', hiddenField: 'hidden',
        translateName: true, defaultCurrency: 'CNY', enableClickItem: true,
        skeleton: false, type: mockTrendTypes.Area.type,
        ...overrides
    };
}

function monthlyProps(overrides: Record<string, unknown> = {}): any {
    return props({
        chartMode: 'monthly',
        items: [{ id: 'salary', name: 'Salary', items: [
            { year: 2024, month1base: 1, amount: 100 },
            { year: 2024, month1base: 1, amount: 250 }
        ] }],
        startTime: undefined, endTime: undefined,
        startYearMonth: { year: 2024, month1base: 1 },
        endYearMonth: { year: 2024, month1base: 12 },
        dataAggregationType: mockAggregations.Last,
        colorField: undefined, hiddenField: undefined, translateName: false, type: undefined,
        ...overrides
    });
}

function setupChart(chartProps: any): { bindings: any; emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = EChartsTrendsChart.setup(chartProps, {
        attrs: {}, slots: {}, emit, expose: () => undefined
    });
    return { bindings, emit };
}

function installDocument(): void {
    const classList = { contains: () => mockDark };
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: { documentElement: { classList }, body: { classList } }
    });
}

beforeEach(() => {
    jest.clearAllMocks();
    mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 199 }];
    mockDark = false;
    mockFirst.mockReturnValue(130);
    mockLast.mockReturnValue(170);
    mockDateType.mockReturnValue(77);
    mockFiscal.mockReturnValue(2024);
    installDocument();
});

describe('mobile EChartsTrendsChart production-loaded behavior', () => {
    test('builds maps, sum area data, tooltip, axes, and clamped daily drilldown', () => {
        const hidden = item({ id: 'hidden', name: 'Hidden', hidden: true, items: [] });
        const { bindings, emit } = setupChart(props({ items: [item(), hidden] }));
        expect(bindings.itemsMap.value).toEqual({
            food: { id: 'food', name: 'Food', hidden: false },
            hidden: { id: 'hidden', name: 'Hidden', hidden: true }
        });
        expect(bindings.allDisplayDateRanges.value).toEqual(['year:100']);
        expect(bindings.allSeries.value).toEqual([expect.objectContaining({
            id: 'food', name: 'food', areaStyle: {}, stack: 'food', data: [300],
            animation: true, itemStyle: { color: 'display:red' }
        })]);
        const options = bindings.chartOptions.value as any;
        expect(options.xAxis[0].data).toEqual(['year:100']);
        expect(options.xAxis[0].axisLabel.rotate).toBe(0);
        expect(options.yAxis[0].axisLabel.formatter(999)).toBe('999');
        expect(options.yAxis[0].axisLabel.formatter(1_000)).toBe('1.0k');
        expect(options.yAxis[0].axisLabel.formatter(-10_000)).toBe('-1.0w');
        expect(options.tooltip.formatter({ seriesName: 'Food', data: 300, name: '2024' })).toBe('2024\nFood  CNY:300');
        expect(options.tooltip.formatter([
            { seriesName: 'Candle', data: [90, 200], name: '2024' },
            { seriesName: 'Zero', data: 0, name: '2024' }
        ])).toContain('Candle  CNY:200');

        bindings.clickItem({ componentType: 'series', seriesId: 'food', dataIndex: 0 });
        expect(emit).toHaveBeenCalledWith('click', {
            itemId: 'food', dateRange: { minTime: 120, maxTime: 180, dateType: 77 }
        });
        expect(mockDateType).toHaveBeenCalledWith(120, 180, 1, 4, 'normal');
    });

    test.each([
        ['fiscal daily', mockDateTypes.FiscalYear.type, { year: 2024, minUnixTime: 10, maxUnixTime: 20 }, 'fiscal:10', 300],
        ['quarter daily', mockDateTypes.Quarter.type, { year: 2024, quarter: 1, minUnixTime: 10, maxUnixTime: 20 }, 'quarter:2024-1', 300],
        ['month daily', mockDateTypes.Month.type, { year: 2024, month0base: 0, minUnixTime: 10, maxUnixTime: 20 }, 'month:10', 300],
        ['day daily', mockDateTypes.Day.type, { year: 2024, month: 1, day: 2, minUnixTime: 10, maxUnixTime: 20 }, 'day:10', 100]
    ])('maps %s', (_name, dateAggregationType, range, display, amount) => {
        mockRanges = [range];
        const bindings = setupChart(props({ dateAggregationType })).bindings;
        expect(bindings.allDisplayDateRanges.value).toEqual([display]);
        expect(bindings.allSeries.value[0].data).toEqual([amount]);
    });

    test.each([
        ['year monthly', mockDateTypes.Year.type, { year: 2024, minUnixTime: 10, maxUnixTime: 20 }, 'year:10'],
        ['fiscal monthly', mockDateTypes.FiscalYear.type, { year: 2024, minUnixTime: 10, maxUnixTime: 20 }, 'fiscal:10'],
        ['quarter monthly', mockDateTypes.Quarter.type, { year: 2024, quarter: 1, minUnixTime: 10, maxUnixTime: 20 }, 'quarter:2024-1'],
        ['month monthly', mockDateTypes.Month.type, { year: 2024, month0base: 0, minUnixTime: 10, maxUnixTime: 20 }, 'month:10']
    ])('maps %s with last aggregation', (_name, dateAggregationType, range, display) => {
        mockRanges = [range];
        const bindings = setupChart(monthlyProps({ dateAggregationType })).bindings;
        expect(bindings.allDisplayDateRanges.value).toEqual([display]);
        expect(bindings.allSeries.value[0].data).toEqual([250]);
    });

    test('builds bubble, column, stacked, fallback color, and candlestick variants', () => {
        mockRanges = [
            { year: 2024, month0base: 0, minUnixTime: 10, maxUnixTime: 20 },
            { year: 2024, month0base: 1, minUnixTime: 21, maxUnixTime: 30 }
        ];
        const bubbleBindings = setupChart(props({
            idField: undefined, colorField: undefined, hiddenField: undefined,
            dateAggregationType: mockDateTypes.Month.type, type: mockTrendTypes.Bubble.type,
            items: [item({ name: 'Bubble', items: [{ year: 2024, month: 2, day: 1, amount: -400 }] })]
        })).bindings;
        expect(bubbleBindings.itemsMap.value).toEqual({ 'translated:Bubble': { name: 'Bubble' } });
        const bubble = bubbleBindings.allSeries.value[0];
        expect(bubble).toEqual(expect.objectContaining({
            id: 'translated:Bubble', type: 'scatter', data: [null, -400],
            itemStyle: { color: 'display:fallback-0' }
        }));
        expect(bubble.symbolSize(-400)).toBe(38);
        expect(setupChart(props({ type: mockTrendTypes.Column.type })).bindings.allSeries.value[0].type).toBe('bar');
        expect(setupChart(props({ stacked: true, type: undefined, skeleton: true })).bindings.allSeries.value[0])
            .toEqual(expect.objectContaining({ stack: 'a', type: 'line', animation: false }));

        mockRanges = [
            { year: 2024, minUnixTime: 100, maxUnixTime: 199 },
            { year: 2025, minUnixTime: 200, maxUnixTime: 299 }
        ];
        const candle = setupChart(props({
            type: mockAccountTypes.Candlestick.type,
            dataAggregationType: mockAggregations.Last,
            items: [item({ items: [
                { year: 2024, month: 1, day: 2, amount: 100, totalOpeningAmount: 90 },
                { year: 2024, month: 1, day: 3, amount: 150, totalOpeningAmount: 200 },
                { year: 2025, month: 1, day: 1, amount: Number.NaN }
            ] })]
        })).bindings.allSeries.value[0];
        expect(candle.data).toEqual([[90, 150, 90, 200], null]);
    });

    test('covers dark and document-free options, dense dates, click guards, and monthly clamp variants', () => {
        mockDark = true;
        mockRanges = Array.from({ length: 9 }, (_, index) => ({ year: 2020 + index, minUnixTime: index, maxUnixTime: index + 1 }));
        const dark = setupChart(props({ items: [] })).bindings;
        expect(dark.isDarkMode.value).toBe(true);
        expect(dark.chartOptions.value).toEqual(expect.objectContaining({
            tooltip: expect.objectContaining({ backgroundColor: '#333' })
        }));
        expect(dark.chartOptions.value.xAxis[0].axisLabel.rotate).toBe(30);
        expect(dark.chartOptions.value.tooltip.formatter([])).toBe('');

        Reflect.deleteProperty(globalThis, 'document');
        const noDocument = setupChart(props()).bindings;
        expect(noDocument.isDarkMode.value).toBe(false);
        installDocument();

        const disabled = setupChart(props({ enableClickItem: false }));
        disabled.bindings.clickItem({ componentType: 'series', seriesId: 'food', dataIndex: 0 });
        disabled.bindings.clickItem({ componentType: 'legend', seriesId: 'food', dataIndex: 0 });
        expect(disabled.emit).not.toHaveBeenCalled();
        const missing = setupChart(props());
        missing.bindings.clickItem({ componentType: 'series', seriesId: 'food', dataIndex: 99 });
        expect(missing.emit).not.toHaveBeenCalled();

        mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 200 }];
        const monthly = setupChart(monthlyProps());
        monthly.bindings.clickItem({ componentType: 'series', seriesId: 'salary', dataIndex: 0 });
        expect(monthly.emit).toHaveBeenCalledWith('click', {
            itemId: 'salary', dateRange: { minTime: 130, maxTime: 170, dateType: 77 }
        });
        mockFirst.mockReturnValueOnce(80);
        mockLast.mockReturnValueOnce(220);
        const unclamped = setupChart(monthlyProps());
        unclamped.bindings.clickItem({ componentType: 'series', seriesId: 'salary', dataIndex: 0 });
        expect(unclamped.emit).toHaveBeenCalledWith('click', {
            itemId: 'salary', dateRange: { minTime: 100, maxTime: 200, dateType: 77 }
        });
    });

    test('SSR renders the production mobile chart template', async () => {
        const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
        const { renderToString } = jest.requireActual('vue/server-renderer') as any;
        const app = createSSRApp(EChartsTrendsChart, props());
        app.component('v-chart', defineComponent({
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => () => h('div', { 'data-testid': 'mobile-trends', class: attrs.class })
        }));
        app.config.warnHandler = () => undefined;
        const html = await renderToString(app);
        expect(html).toContain('data-testid="mobile-trends"');
        expect(html).toContain('mobile-echarts-trends-chart-container');
    });
});
