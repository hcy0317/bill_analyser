import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockChartDataAggregationType = {
    Sum: 'sum',
    Last: 'last'
} as const;
const mockTrendChartType = {
    Area: { type: 1 },
    Column: { type: 2 },
    Bubble: { type: 3 }
} as const;
const mockAccountBalanceTrendChartType = {
    Candlestick: { type: 4 }
} as const;
const mockChartDateAggregationType = {
    Year: { type: 10 },
    FiscalYear: { type: 11 },
    Quarter: { type: 12 },
    Month: { type: 13 },
    Day: { type: 14 }
} as const;

let mockRanges: any[] = [];
let mockDirection = 'ltr';
let mockThemeName = 'light';
let mockMeasureWidth = 70;
let mockCanvasContextAvailable = true;

const mockGetYearMonthFirstUnixTime = jest.fn<(...args: any[]) => number>(() => 130);
const mockGetYearMonthLastUnixTime = jest.fn<(...args: any[]) => number>(() => 170);
const mockGetDateTypeByDateRange = jest.fn<(...args: any[]) => number>(() => 77);
const mockGetFiscalYearFromUnixTime = jest.fn<(...args: any[]) => number>(() => 2024);
const mockSortStatisticsItems = jest.fn((items: any[]) => {
    items.sort((left, right) => (left.displayOrders?.[0] ?? 0) - (right.displayOrders?.[0] ?? 0));
});

jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: { value: mockThemeName } } })
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockDirection,
        formatUnixTimeToShortDate: (value: number) => `day:${value}`,
        formatUnixTimeToGregorianLikeShortYear: (value: number) => `year:${value}`,
        formatUnixTimeToGregorianLikeShortYearMonth: (value: number) => `month:${value}`,
        formatYearQuarterToGregorianLikeYearQuarter: (year: number, quarter: number) => `quarter:${year}-${quarter}`,
        formatUnixTimeToGregorianLikeFiscalYear: (value: number) => `fiscal:${value}`,
        formatAmountToWesternArabicNumeralsWithoutDigitGrouping: (value: number) => `western:${value}`,
        formatAmountToLocalizedNumeralsWithCurrency: (value: number, currency: string) => `${currency}:${value}`
    })
}));

jest.mock('@/components/base/TrendsChartBase.ts', () => {
    const actualVue = jest.requireActual('vue') as any;
    return {
        useTrendsChartBase: (props: any) => ({
            allDateRanges: actualVue.computed(() => mockRanges),
            getItemName: (name: string) => props.translateName ? `translated:${name}` : name
        })
    };
});

jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        currentUserFirstDayOfWeek: 1,
        currentUserFiscalYearStart: 4
    })
}));

jest.mock('@/core/base.ts', () => ({
    itemAndIndex: (items: any[]) => items.map((item, index) => [item, index])
}));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 'ltr', RTL: 'rtl' } }));
jest.mock('@/core/datetime.ts', () => ({ DateRangeScene: { Normal: 'normal' } }));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: (name: string) => name === 'dark' }));
jest.mock('@/core/statistics.ts', () => ({
    ChartDataAggregationType: mockChartDataAggregationType,
    TrendChartType: mockTrendChartType,
    AccountBalanceTrendChartType: mockAccountBalanceTrendChartType,
    ChartDateAggregationType: mockChartDateAggregationType
}));
jest.mock('@/consts/color.ts', () => ({ DEFAULT_CHART_COLORS: ['fallback-0', 'fallback-1'] }));
jest.mock('@/lib/common.ts', () => ({
    isArray: Array.isArray,
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value)
}));
jest.mock('@/lib/datetime.ts', () => ({
    getYearMonthFirstUnixTime: (...args: any[]) => mockGetYearMonthFirstUnixTime(...args),
    getYearMonthLastUnixTime: (...args: any[]) => mockGetYearMonthLastUnixTime(...args),
    getDateTypeByDateRange: (...args: any[]) => mockGetDateTypeByDateRange(...args),
    getFiscalYearFromUnixTime: (...args: any[]) => mockGetFiscalYearFromUnixTime(...args)
}));
jest.mock('@/lib/color.ts', () => ({ getDisplayColor: (value: string) => `display:${value}` }));
jest.mock('@/lib/statistics.ts', () => ({
    sortStatisticsItems: (items: any[], sortingType: number) => {
        expect(sortingType).toBeGreaterThanOrEqual(0);
        mockSortStatisticsItems(items);
    }
}));

const TrendsChart = require('@/components/desktop/TrendsChart.vue').default as any;

function dailyItem(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'food',
        name: 'Food',
        color: 'red',
        hidden: false,
        orders: [2],
        items: [
            { year: 2024, month: 1, day: 2, amount: 100, totalOpeningAmountCents: 90 },
            { year: 2024, month: 1, day: 3, amount: 200, totalOpeningAmountCents: 110 }
        ],
        ...overrides
    };
}

function dailyProps(overrides: Record<string, unknown> = {}): any {
    return {
        chartMode: 'daily',
        items: [dailyItem()],
        stacked: false,
        startTime: 120,
        endTime: 180,
        startYearMonth: undefined,
        endYearMonth: undefined,
        fiscalYearStart: 4,
        sortingType: 0,
        dataAggregationType: mockChartDataAggregationType.Sum,
        dateAggregationType: mockChartDateAggregationType.Year.type,
        idField: 'id',
        nameField: 'name',
        valueField: 'amount',
        colorField: 'color',
        hiddenField: 'hidden',
        displayOrdersField: 'orders',
        translateName: true,
        defaultCurrency: 'CNY',
        enableClickItem: true,
        skeleton: false,
        type: mockTrendChartType.Area.type,
        showValue: true,
        showTotalAmountInTooltip: true,
        ...overrides
    };
}

function monthlyProps(overrides: Record<string, unknown> = {}): any {
    return dailyProps({
        chartMode: 'monthly',
        items: [{
            id: 'salary',
            name: 'Salary',
            items: [
                { year: 2024, month1base: 1, amount: 100 },
                { year: 2024, month1base: 1, amount: 250 }
            ]
        }],
        startTime: undefined,
        endTime: undefined,
        startYearMonth: { year: 2024, month1base: 1 },
        endYearMonth: { year: 2024, month1base: 12 },
        dataAggregationType: mockChartDataAggregationType.Last,
        type: undefined,
        colorField: undefined,
        hiddenField: undefined,
        displayOrdersField: undefined,
        translateName: false,
        ...overrides
    });
}

function setupChart(props: any): { bindings: any; emit: jest.Mock; exposed: Record<string, unknown> } {
    const emit = jest.fn();
    const exposed: Record<string, unknown> = {};
    const bindings = TrendsChart.setup(props, {
        attrs: {},
        slots: {},
        emit,
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value)
    });
    return { bindings, emit, exposed };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 199 }];
    mockDirection = 'ltr';
    mockThemeName = 'light';
    mockMeasureWidth = 70;
    mockCanvasContextAvailable = true;
    mockGetYearMonthFirstUnixTime.mockReturnValue(130);
    mockGetYearMonthLastUnixTime.mockReturnValue(170);
    mockGetDateTypeByDateRange.mockReturnValue(77);
    mockGetFiscalYearFromUnixTime.mockReturnValue(2024);
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: {
            createElement: () => ({
                getContext: () => mockCanvasContextAvailable ? {
                    font: '',
                    measureText: () => ({ width: mockMeasureWidth })
                } : null
            })
        }
    });
});

describe('desktop TrendsChart production-loaded aggregations', () => {
    test('builds translated maps, summed area series, tooltip, axes, export, and daily click ranges', () => {
        const hidden = dailyItem({ id: 'hidden', name: 'Hidden', hidden: true, items: [] });
        const { bindings, emit, exposed } = setupChart(dailyProps({ items: [dailyItem(), hidden] }));

        expect(bindings.itemsMap.value).toEqual({
            food: { id: 'food', name: 'Food', hidden: false, orders: [2] },
            hidden: { id: 'hidden', name: 'Hidden', hidden: true, orders: [2] }
        });
        expect(bindings.allDisplayDateRanges.value).toEqual(['year:100']);
        expect(bindings.allSeries.value).toEqual([
            expect.objectContaining({
                id: 'food',
                name: 'food',
                areaStyle: {},
                stack: 'food',
                animation: true,
                data: [300],
                itemStyle: { color: 'display:red' }
            })
        ]);

        mockMeasureWidth = 100;
        expect(bindings.yAxisWidth.value).toBe(120);
        const options = bindings.chartOptions.value as any;
        expect(options.xAxis[0]).toEqual(expect.objectContaining({ data: ['year:100'], inverse: false }));
        expect(options.legend.formatter('food')).toBe('translated:Food');
        expect(options.legend.formatter('missing')).toBe('missing');
        expect(options.yAxis[0].axisLabel.formatter('123')).toBe('CNY:123');
        expect(options.yAxis[0].axisPointer.label.formatter({ value: 12.9 })).toBe('CNY:12');

        const tooltip = options.tooltip.formatter([
            { seriesId: 'food', color: '#f00', data: 300, name: '2024' },
            { seriesId: 'hidden', color: '#000', data: 0, name: '2024' }
        ]);
        expect(tooltip).toContain('2024');
        expect(tooltip).toContain('translated:Food');
        expect(tooltip).toContain('tt:Total Amount');
        expect(tooltip).not.toContain('translated:Hidden');

        bindings.clickItem({ componentType: 'series', seriesId: 'food', dataIndex: 0 });
        expect(emit).toHaveBeenCalledWith('click', {
            itemId: 'food',
            dateRange: { minTime: 120, maxTime: 180, dateType: 77 }
        });
        expect(mockGetDateTypeByDateRange).toHaveBeenCalledWith(120, 180, 1, 4, 'normal');

        expect((exposed['exportData'] as () => unknown)()).toEqual({
            headers: ['tt:Date', 'translated:Food'],
            data: [['year:100', 'western:300']]
        });
        bindings.onLegendSelectChanged({ selected: { food: false } });
        expect(bindings.selectedLegends.value).toEqual({ food: false });
    });

    test.each([
        ['fiscal daily', mockChartDateAggregationType.FiscalYear.type, { year: 2024, minUnixTime: 10, maxUnixTime: 20 }, 'fiscal:10', 300],
        ['quarter daily', mockChartDateAggregationType.Quarter.type, { year: 2024, quarter: 1, minUnixTime: 10, maxUnixTime: 20 }, 'quarter:2024-1', 300],
        ['month daily', mockChartDateAggregationType.Month.type, { year: 2024, month0base: 0, minUnixTime: 10, maxUnixTime: 20 }, 'month:10', 300],
        ['day daily', mockChartDateAggregationType.Day.type, { year: 2024, month: 1, day: 2, minUnixTime: 10, maxUnixTime: 20 }, 'day:10', 100]
    ])('maps %s ranges and values', (_name, dateAggregationType, range, display, amount) => {
        mockRanges = [range];
        const { bindings } = setupChart(dailyProps({ dateAggregationType }));
        expect(bindings.allDisplayDateRanges.value).toEqual([display]);
        expect(bindings.allSeries.value[0].data).toEqual([amount]);
    });

    test.each([
        ['year monthly', mockChartDateAggregationType.Year.type, { year: 2024, minUnixTime: 10, maxUnixTime: 20 }, 'year:10'],
        ['fiscal monthly', mockChartDateAggregationType.FiscalYear.type, { year: 2024, minUnixTime: 10, maxUnixTime: 20 }, 'fiscal:10'],
        ['quarter monthly', mockChartDateAggregationType.Quarter.type, { year: 2024, quarter: 1, minUnixTime: 10, maxUnixTime: 20 }, 'quarter:2024-1'],
        ['month monthly', mockChartDateAggregationType.Month.type, { year: 2024, month0base: 0, minUnixTime: 10, maxUnixTime: 20 }, 'month:10']
    ])('maps %s ranges using last-value aggregation', (_name, dateAggregationType, range, display) => {
        mockRanges = [range];
        const { bindings } = setupChart(monthlyProps({ dateAggregationType }));
        expect(bindings.allDisplayDateRanges.value).toEqual([display]);
        expect(bindings.allSeries.value[0]).toEqual(expect.objectContaining({
            name: 'salary',
            stack: 'salary',
            data: [250]
        }));
    });
});

describe('desktop TrendsChart production-loaded chart modes and defensive branches', () => {
    test('builds bubble, column, stacked, fallback-name, and candlestick series', () => {
        mockRanges = [
            { year: 2024, month0base: 0, minUnixTime: 10, maxUnixTime: 20 },
            { year: 2024, month0base: 1, minUnixTime: 21, maxUnixTime: 30 }
        ];
        const bubbleBindings = setupChart(dailyProps({
            idField: undefined,
            colorField: undefined,
            hiddenField: undefined,
            displayOrdersField: undefined,
            dateAggregationType: mockChartDateAggregationType.Month.type,
            type: mockTrendChartType.Bubble.type,
            items: [dailyItem({
                name: 'Bubble',
                items: [{ year: 2024, month: 2, day: 1, amount: -400 }]
            })]
        })).bindings;
        expect(bubbleBindings.itemsMap.value).toEqual({
            'translated:Bubble': { name: 'Bubble' }
        });
        const bubble = bubbleBindings.allSeries.value[0];
        expect(bubble).toEqual(expect.objectContaining({
            id: 'translated:Bubble',
            name: 'translated:Bubble',
            type: 'scatter',
            data: [null, -400],
            itemStyle: { color: 'display:fallback-0' }
        }));
        expect(bubble.symbolSize(-400)).toBe(65);

        const column = setupChart(dailyProps({ type: mockTrendChartType.Column.type })).bindings.allSeries.value[0];
        expect(column.type).toBe('bar');

        const stacked = setupChart(dailyProps({ stacked: true, type: undefined, skeleton: true })).bindings.allSeries.value[0];
        expect(stacked).toEqual(expect.objectContaining({ stack: 'a', type: 'line', animation: false }));

        mockRanges = [
            { year: 2024, minUnixTime: 100, maxUnixTime: 199 },
            { year: 2025, minUnixTime: 200, maxUnixTime: 299 }
        ];
        const candleBindings = setupChart(dailyProps({
            type: mockAccountBalanceTrendChartType.Candlestick.type,
            dataAggregationType: mockChartDataAggregationType.Last,
            items: [dailyItem({
                items: [
                    { year: 2024, month: 1, day: 2, amount: 100, totalOpeningAmountCents: 90 },
                    { year: 2024, month: 1, day: 3, amount: 150, totalOpeningAmountCents: 200 },
                    { year: 2025, month: 1, day: 4, amount: Number.NaN, totalOpeningAmountCents: undefined }
                ]
            })]
        })).bindings;
        expect(candleBindings.allSeries.value[0].data).toEqual([[90, 150, 90, 200], null]);
        expect(candleBindings.yAxisWidth.value).toBe(90);
        expect(candleBindings.exportData()).toEqual({
            headers: ['tt:Date', 'translated:Food'],
            data: [
                ['year:100', 'western:150'],
                ['year:200', 'western:0']
            ]
        });
    });

    test('covers dark RTL tooltip, zero totals, canvas bounds, and no-series defaults', () => {
        mockThemeName = 'dark';
        mockDirection = 'rtl';
        mockMeasureWidth = 250;
        const { bindings } = setupChart(dailyProps());
        expect(bindings.yAxisWidth.value).toBe(270);
        const options = bindings.chartOptions.value as any;
        expect(options.xAxis[0].inverse).toBe(true);
        expect(options.tooltip.backgroundColor).toBe('#333');
        expect(options.tooltip.formatter([
            { seriesId: 'food', color: '#f00', data: [1], name: '' },
            { seriesId: 'food', color: '#f00', data: [1, undefined], name: '' }
        ])).toContain('<div></div>');
        expect(options.tooltip.formatter([
            { seriesId: 'food', color: '#f00', data: 0, name: '' }
        ])).toContain('translated:Food');

        mockCanvasContextAvailable = false;
        const noContext = setupChart(dailyProps()).bindings;
        expect(noContext.yAxisWidth.value).toBe(90);

        const empty = setupChart(dailyProps({ items: [] })).bindings;
        expect(empty.allSeries.value).toEqual([]);
        expect(empty.yAxisWidth.value).toBe(90);
        expect(empty.chartOptions.value.tooltip.formatter([])).toContain('<div></div>');
    });

    test('guards clicks and clamps monthly ranges before emitting', () => {
        const disabled = setupChart(dailyProps({ enableClickItem: false }));
        disabled.bindings.clickItem({ componentType: 'series', seriesId: 'food', dataIndex: 0 });
        disabled.bindings.clickItem({ componentType: 'legend', seriesId: 'food', dataIndex: 0 });
        expect(disabled.emit).not.toHaveBeenCalled();

        const missingRange = setupChart(dailyProps());
        missingRange.bindings.clickItem({ componentType: 'series', seriesId: 'food', dataIndex: 99 });
        expect(missingRange.emit).not.toHaveBeenCalled();

        mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 200 }];
        const monthly = setupChart(monthlyProps());
        monthly.bindings.clickItem({ componentType: 'series', seriesId: 'salary', dataIndex: 0 });
        expect(monthly.emit).toHaveBeenCalledWith('click', {
            itemId: 'salary',
            dateRange: { minTime: 130, maxTime: 170, dateType: 77 }
        });

        mockGetYearMonthFirstUnixTime.mockReturnValueOnce(80);
        mockGetYearMonthLastUnixTime.mockReturnValueOnce(220);
        const unclamped = setupChart(monthlyProps());
        unclamped.bindings.clickItem({ componentType: 'series', seriesId: 'salary', dataIndex: 0 });
        expect(unclamped.emit).toHaveBeenCalledWith('click', {
            itemId: 'salary',
            dateRange: { minTime: 100, maxTime: 200, dateType: 77 }
        });
    });

    test('SSR renders the production template with the chart option contract', async () => {
        const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
        const { renderToString } = jest.requireActual('vue/server-renderer') as any;
        const app = createSSRApp(TrendsChart, dailyProps());
        app.component('v-chart', defineComponent({
            inheritAttrs: false,
            setup: (_props: unknown, { attrs }: any) => () => h('div', {
                'data-testid': 'trends-chart',
                class: attrs.class
            })
        }));
        app.config.warnHandler = () => undefined;
        const html = await renderToString(app);
        expect(html).toContain('data-testid="trends-chart"');
        expect(html).toContain('trends-chart-container');
    });
});
