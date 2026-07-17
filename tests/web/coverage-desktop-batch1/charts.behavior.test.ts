import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const itemTypes = {
    IncomeByPrimaryCategory: 1,
    IncomeBySecondaryCategory: 2,
    IncomeByAccount: 3,
    ExpenseByAccount: 4,
    NetCashFlow: 5,
    ExpenseBySecondaryCategory: 6,
    ExpenseByPrimaryCategory: 7
} as const;
const trendTypes = {
    Area: { type: 1 },
    Column: { type: 2 },
    Candlestick: { type: 3 }
} as const;

let mockThemeName = 'light';
let mockDirection = 'ltr';
let mockBalanceItems: any[] = [];
let mockBalanceRanges: string[] = [];
let mockPieValidItems: any;
let mockPieSelectedIndex: any;
let mockMeasureWidth = 70;
let mockCanvasAvailable = true;

jest.mock('vuetify', () => ({
    useTheme: () => ({ global: { name: { value: mockThemeName } } })
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockDirection,
        formatAmountToLocalizedNumeralsWithCurrency: (value: number, currency?: string) => `${currency ?? 'default'}:${value}`,
        formatPercentToLocalizedNumerals: (value: number) => `percent:${value}`
    })
}));
jest.mock('@/stores/user.ts', () => ({
    useUserStore: () => ({
        currentUserExpenseAmountColor: 'expense',
        currentUserIncomeAmountColor: 'income'
    })
}));
jest.mock('@/models/transaction.ts', () => ({
    TransactionCategoricalOverviewAnalysisDataItemType: itemTypes
}));
jest.mock('@/core/base.ts', () => ({
    values: (value: Record<string, unknown>) => Object.values(value),
    itemAndIndex: (items: unknown[]) => items.map((item, index) => [item, index])
}));
jest.mock('@/core/text.ts', () => ({ TextDirection: { LTR: 'ltr', RTL: 'rtl' } }));
jest.mock('@/core/theme.ts', () => ({ isDarkApplicationTheme: (value: string) => value === 'dark' }));
jest.mock('@/core/statistics.ts', () => ({ AccountBalanceTrendChartType: trendTypes }));
jest.mock('@/consts/color.ts', () => ({ DEFAULT_CHART_COLORS: ['111111', '222222', '333333', '444444', '555555', '666666'] }));
jest.mock('@/lib/common.ts', () => ({
    isArray: Array.isArray,
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value),
    getObjectOwnFieldCount: (value: object) => Object.keys(value).length
}));
jest.mock('@/lib/ui/common.ts', () => ({
    getExpenseAndIncomeAmountColor: (_expense: string, _income: string, dark: boolean) => ({
        expenseAmountColor: dark ? 'dark-expense' : 'light-expense',
        incomeAmountColor: dark ? 'dark-income' : 'light-income'
    })
}));
jest.mock('@/lib/logger.ts', () => ({ __esModule: true, default: { debug: jest.fn() } }));
jest.mock('@/components/base/AccountBalanceTrendsChartBase.ts', () => {
    const { computed } = jest.requireActual('vue') as any;
    return {
        useAccountBalanceTrendsChartBase: () => ({
            allDataItems: computed(() => mockBalanceItems),
            allDisplayDateRanges: computed(() => mockBalanceRanges)
        })
    };
});
jest.mock('@/components/base/PieChartBase.ts', () => ({
    usePieChartBase: () => ({
        selectedIndex: mockPieSelectedIndex,
        validItems: mockPieValidItems
    })
}));

const SankeyChart = require('@/components/desktop/AccountAndCategorySankeyChart.vue').default as any;
const BalanceChart = require('@/components/desktop/AccountBalanceTrendsChart.vue').default as any;
const PieChart = require('@/components/desktop/PieChart.vue').default as any;
const { ref } = jest.requireActual('vue') as any;

function setup(component: any, props: Record<string, unknown>): { bindings: any; emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = component.setup(props, { attrs: {}, slots: {}, emit, expose: () => undefined });
    return { bindings, emit };
}

function overviewItem(type: number, id: string, overrides: Record<string, unknown> = {}): any {
    return {
        type,
        id,
        name: `name-${id}`,
        hidden: false,
        totalAmountCents: 100,
        percent: 0.25,
        outflows: [],
        ...overrides
    };
}

function balanceItem(overrides: Record<string, unknown> = {}): any {
    return {
        displayDate: '2026-07-01',
        openingBalanceCents: 100,
        closingBalanceCents: 120,
        minimumBalanceCents: 90,
        maximumBalanceCents: 130,
        medianBalanceCents: 110,
        averageBalanceCents: 112,
        ...overrides
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockThemeName = 'light';
    mockDirection = 'ltr';
    mockBalanceItems = [balanceItem()];
    mockBalanceRanges = ['Jul 1'];
    mockPieSelectedIndex = ref(1);
    mockPieValidItems = ref([
        { id: 'food', name: 'food', displayName: 'Food', displayValue: 'CNY:100', displayPercent: '40%', value: 100, color: '#f00', sourceItem: { id: 'food' } },
        { id: 'rent', name: 'rent', displayName: 'Rent', displayValue: 'CNY:150', displayPercent: '60%', value: 150, color: '#0f0', sourceItem: { id: 'rent' } }
    ]);
    mockMeasureWidth = 70;
    mockCanvasAvailable = true;
    Object.defineProperty(globalThis, 'document', {
        configurable: true,
        value: {
            createElement: () => ({
                getContext: () => mockCanvasAvailable ? {
                    font: '',
                    measureText: () => ({ width: mockMeasureWidth })
                } : null
            })
        }
    });
});

describe('AccountAndCategorySankeyChart production-loaded behavior', () => {
    test('combines valid nodes and links while filtering hidden, empty, and net-flow nodes', () => {
        const netFlow = overviewItem(itemTypes.NetCashFlow, 'net');
        const expenseAccount = overviewItem(itemTypes.ExpenseByAccount, 'cash', {
            outflows: [
                { relatedItem: netFlow, amountCents: -30 },
                { relatedItem: overviewItem(itemTypes.ExpenseBySecondaryCategory, 'food'), amountCents: -40 },
                { relatedItem: overviewItem(itemTypes.ExpenseBySecondaryCategory, 'food'), amountCents: -10 }
            ]
        });
        const { bindings } = setup(SankeyChart, {
            skeleton: false,
            defaultCurrency: 'CNY',
            enableClickItem: true,
            items: [
                overviewItem(itemTypes.IncomeByPrimaryCategory, 'salary', {
                    outflows: [{ relatedItem: null, amountCents: 9 }]
                }),
                overviewItem(itemTypes.IncomeByAccount, 'bank', { percent: undefined }),
                expenseAccount,
                overviewItem(itemTypes.ExpenseByPrimaryCategory, 'hidden', { hidden: true }),
                overviewItem(itemTypes.ExpenseByPrimaryCategory, 'empty', { totalAmountCents: 0 }),
                netFlow,
                overviewItem(999, 'unknown')
            ]
        });

        expect(bindings.sankeyData.value.nodes).toHaveLength(3);
        expect(bindings.sankeyData.value.nodes[1]).toEqual(expect.objectContaining({
            itemStyle: { color: '#aaa', opacity: 0.5 }
        }));
        expect(bindings.sankeyData.value.nodes[2]).toEqual(expect.objectContaining({ accountNetCashFlow: -30 }));
        expect(bindings.sankeyData.value.links).toEqual(expect.arrayContaining([
            expect.objectContaining({ sourceItemId: 'cash', targetItemId: 'food', value: 50 }),
            expect.objectContaining({ sourceItemId: 'cash', targetItemId: 'net', value: 30 })
        ]));
        expect(bindings.chartOptions.value.series[0]).toEqual(expect.objectContaining({ animation: true }));
    });

    test('formats every node family, edge, percentages, balances, dark colors, and labels', () => {
        mockThemeName = 'dark';
        const types = Object.values(itemTypes);
        const items = types.map((type, index) => overviewItem(type, `id-${index}`, {
            outflows: type === itemTypes.ExpenseByAccount
                ? [{ relatedItem: overviewItem(itemTypes.NetCashFlow, 'net'), amountCents: 25 }]
                : []
        }));
        const { bindings } = setup(SankeyChart, { skeleton: true, defaultCurrency: 'USD', enableClickItem: true, items });
        const options = bindings.chartOptions.value as any;
        expect(options.tooltip.backgroundColor).toBe('#333');
        expect(options.series[0].animation).toBe(false);
        expect(options.series[0].levels[0].itemStyle.color).toBe('dark-income');
        expect(options.series[0].levels[5].itemStyle.color).toBe('dark-expense');

        const nodeFormatter = options.tooltip.formatter;
        for (const node of bindings.sankeyData.value.nodes) {
            const text = nodeFormatter({ dataType: 'node', data: node });
            expect(text).toContain(node.displayName);
            expect(text).toContain('USD:100');
        }
        const account = bindings.sankeyData.value.nodes.find((item: any) => item.itemId === 'id-2');
        account.percent = undefined;
        expect(nodeFormatter({ dataType: 'node', data: account })).toContain('tt:Account Balance');
        const expense = bindings.sankeyData.value.nodes.find((item: any) => item.itemId === 'id-3');
        expect(nodeFormatter({ dataType: 'node', data: expense })).toContain('tt:Net Cash Flow');
        const edge = bindings.sankeyData.value.links[0];
        expect(nodeFormatter({ dataType: 'edge', data: edge, value: 'invalid' })).toContain('USD:0');
        expect(nodeFormatter({ dataType: 'edge', data: edge, value: 25 })).toContain('USD:25');
        expect(nodeFormatter({ dataType: 'other' })).toBe('');
        expect(options.series[0].label.formatter({ data: expense })).toBe(expense.displayName);
    });

    test('emits only enabled, valid node and edge clicks', () => {
        const { bindings, emit } = setup(SankeyChart, { enableClickItem: true, items: [] });
        bindings.clickItem({ componentType: 'legend', seriesType: 'sankey' });
        bindings.clickItem({ componentType: 'series', seriesType: 'line' });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey' });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'node', data: { itemType: 'netCashFlow', itemId: 'net' } });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'node', data: { itemType: 'account', itemId: 'cash' } });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'edge', data: { sourceItemType: 'netCashFlow' } });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'edge', data: { sourceItemType: 'account', sourceItemId: 'same', targetItemType: 'account', targetItemId: 'same' } });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'edge', data: { sourceItemType: 'account', sourceItemId: 'cash', targetItemType: 'category', targetItemId: 'food' } });
        bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'edge', data: { sourceItemType: 'account', sourceItemId: 'cash', targetItemType: 'netCashFlow', targetItemId: 'net' } });
        expect(emit.mock.calls).toEqual([
            ['click', 'account', 'cash'],
            ['click', 'account', 'same'],
            ['click', 'account', 'cash', 'category', 'food']
        ]);

        const disabled = setup(SankeyChart, { enableClickItem: false, items: [] });
        disabled.bindings.clickItem({ componentType: 'series', seriesType: 'sankey', dataType: 'node', data: { itemType: 'account', itemId: 'cash' } });
        expect(disabled.emit).not.toHaveBeenCalled();
    });
});

describe('AccountBalanceTrendsChart production-loaded behavior', () => {
    function props(overrides: Record<string, unknown> = {}): any {
        return { legendName: 'Balance', account: { currency: 'CNY' }, skeleton: false, ...overrides };
    }

    test('builds line, area, column, and candlestick series with expected geometry', () => {
        let bindings = setup(BalanceChart, props()).bindings;
        expect(bindings.allSeries.value[0]).toEqual(expect.objectContaining({ type: 'line', data: [120], animation: true }));
        expect(bindings.chartOptions.value.series[0].barWidth).toBeUndefined();

        bindings = setup(BalanceChart, props({ type: trendTypes.Area.type })).bindings;
        expect(bindings.allSeries.value[0].areaStyle).toEqual({});

        bindings = setup(BalanceChart, props({ type: trendTypes.Column.type })).bindings;
        expect(bindings.chartOptions.value.series[0]).toEqual(expect.objectContaining({ type: 'bar', barWidth: '60%', barGap: '10%' }));

        mockBalanceItems = [balanceItem({ openingBalanceCents: 100, closingBalanceCents: 90, minimumBalanceCents: 95, maximumBalanceCents: 95 })];
        bindings = setup(BalanceChart, props({ type: trendTypes.Candlestick.type, skeleton: true })).bindings;
        expect(bindings.allSeries.value[0]).toEqual(expect.objectContaining({
            type: 'candlestick',
            animation: false,
            data: [[100, 90, 90, 100]],
            itemStyle: expect.objectContaining({ color: 'light-income', color0: 'light-expense' })
        }));
        expect(bindings.chartOptions.value.series[0]).toEqual(expect.objectContaining({ barWidth: '80%', barMinHeight: 1 }));
    });

    test('measures axes at lower, content, capped, and missing-canvas bounds', () => {
        mockBalanceItems = [balanceItem(), balanceItem()];
        let bindings = setup(BalanceChart, props()).bindings;
        expect(bindings.yAxisWidth.value).toBe(90);
        mockMeasureWidth = 100;
        bindings = setup(BalanceChart, props()).bindings;
        expect(bindings.yAxisWidth.value).toBe(120);
        mockMeasureWidth = 250;
        bindings = setup(BalanceChart, props()).bindings;
        expect(bindings.yAxisWidth.value).toBe(270);
        mockCanvasAvailable = false;
        bindings = setup(BalanceChart, props()).bindings;
        expect(bindings.yAxisWidth.value).toBe(90);
        mockBalanceItems = [];
        bindings = setup(BalanceChart, props()).bindings;
        expect(bindings.yAxisWidth.value).toBe(90);
    });

    test('formats regular and candlestick tooltips, axis labels, RTL, and dark theme', () => {
        mockDirection = 'rtl';
        mockThemeName = 'dark';
        let bindings = setup(BalanceChart, props()).bindings;
        let options = bindings.chartOptions.value as any;
        expect(options.xAxis[0].inverse).toBe(true);
        expect(options.tooltip.backgroundColor).toBe('#333');
        expect(options.tooltip.formatter([{ data: 120, name: 'Jul 1' }])).toContain('CNY:120');
        expect(options.yAxis[0].axisLabel.formatter('123')).toBe('CNY:123');
        expect(options.yAxis[0].axisPointer.label.formatter({ value: 12.9 })).toBe('CNY:12');

        bindings = setup(BalanceChart, props({ type: trendTypes.Candlestick.type })).bindings;
        options = bindings.chartOptions.value as any;
        const tooltip = options.tooltip.formatter([{ dataIndex: 0, name: 'Jul 1' }]);
        for (const label of ['Opening Balance', 'Closing Balance', 'Minimum Balance', 'Maximum Balance', 'Median Balance', 'Average Balance']) {
            expect(tooltip).toContain(`tt:${label}`);
        }
    });
});

describe('PieChart production-loaded behavior', () => {
    function props(overrides: Record<string, unknown> = {}): any {
        return {
            items: [
                { id: 'food', label: 'Food' },
                { id: 'rent', label: 'Rent' },
                { label: 'Other' }
            ],
            idField: 'id',
            nameField: 'label',
            showValue: true,
            showPercent: true,
            enableClickItem: true,
            ...overrides
        };
    }

    test('maps items, builds series, computes angle, legend labels, and dark skeleton options', () => {
        mockThemeName = 'dark';
        const { bindings } = setup(PieChart, props({ skeleton: true }));
        expect(bindings.itemsMap.value).toEqual(expect.objectContaining({ food: expect.objectContaining({ label: 'Food' }), Other: expect.objectContaining({ label: 'Other' }) }));
        expect(bindings.seriesData.value).toEqual(expect.arrayContaining([
            expect.objectContaining({ id: 'food', itemStyle: { color: '#f00' }, selected: true })
        ]));
        expect(bindings.firstItemAndHalfCurrentItemTotalPercent.value).toBeCloseTo(0.7);
        const options = bindings.chartOptions.value as any;
        expect(options.tooltip.backgroundColor).toBe('#333');
        expect(options.series[0].animation).toBe(false);
        expect(options.legend.formatter('food')).toBe('Food');
        expect(options.legend.formatter('missing')).toBe('missing');
        expect(options.series[0].label.formatter({ data: mockPieValidItems.value[0] })).toBe('Food');
        expect(options.series[0].label.formatter({ data: null })).toBe('');
    });

    test('formats all tooltip visibility combinations and selected-legend percentages', () => {
        const cases = [
            [{ showValue: true, showPercent: true }, 'CNY:100', '40%'],
            [{ showValue: true, showPercent: false }, 'CNY:100', null],
            [{ showValue: false, showPercent: true }, null, '40%'],
            [{ showValue: false, showPercent: false }, null, null]
        ] as const;
        for (const [flags, included, maybePercent] of cases) {
            const { bindings } = setup(PieChart, props(flags));
            const text = (bindings.chartOptions.value as any).tooltip.formatter({
                data: mockPieValidItems.value[0], value: 100, percent: 40, color: '#f00'
            });
            if (included) expect(text).toContain(included); else expect(text).not.toContain('CNY:100');
            if (maybePercent) expect(text).toContain(maybePercent); else expect(text).not.toContain('40%');
        }

        const { bindings } = setup(PieChart, props());
        bindings.onLegendSelectChanged({ selected: { food: true, rent: false } });
        expect(bindings.hasUnselectedItem.value).toBe(true);
        const fallback = (bindings.chartOptions.value as any).tooltip.formatter({ data: null, value: 55, percent: 22, color: '#123' });
        expect(fallback).toContain('default:55');
        expect(fallback).toContain('22%');
    });

    test('moves selection and emits only valid pie source clicks', () => {
        const { bindings, emit } = setup(PieChart, props());
        bindings.clickItem({ componentType: 'legend', seriesType: 'pie' });
        bindings.clickItem({ componentType: 'series', seriesType: 'line' });
        bindings.clickItem({ componentType: 'series', seriesType: 'pie', event: { target: { currentStates: ['emphasis'] } }, dataIndex: 0 });
        expect(mockPieSelectedIndex.value).toBe(0);
        bindings.clickItem({ componentType: 'series', seriesType: 'pie' });
        bindings.clickItem({ componentType: 'series', seriesType: 'pie', data: { value: 1 } });
        bindings.clickItem({ componentType: 'series', seriesType: 'pie', data: { sourceItem: { id: 'food' } } });
        expect(emit).toHaveBeenCalledWith('click', { id: 'food' });

        mockPieSelectedIndex.value = 1;
        bindings.onLegendSelectChanged({ selected: { food: true, rent: false } });
        expect(mockPieSelectedIndex.value).toBe(0);
        bindings.onLegendSelectChanged({ selected: { food: false, rent: false } });
        expect(mockPieSelectedIndex.value).toBe(0);
        mockPieSelectedIndex.value = 99;
        bindings.onLegendSelectChanged({ selected: { food: false, rent: true } });
        expect(mockPieSelectedIndex.value).toBe(1);

        const disabled = setup(PieChart, props({ enableClickItem: false }));
        disabled.bindings.clickItem({ componentType: 'series', seriesType: 'pie', data: { sourceItem: { id: 'rent' } } });
        expect(disabled.emit).not.toHaveBeenCalled();
    });
});
