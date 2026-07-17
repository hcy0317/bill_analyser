import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockAggregations = { Sum: 'sum', Last: 'last' } as const;
const mockDateTypes = {
    Year: { type: 10 },
    FiscalYear: { type: 11 },
    Quarter: { type: 12 },
    Month: { type: 13 },
    Day: { type: 14 }
} as const;

let mockRanges: any[] = [];
const mockFirst = jest.fn<(...args: any[]) => number>(() => 130);
const mockLast = jest.fn<(...args: any[]) => number>(() => 170);
const mockDateType = jest.fn<(...args: any[]) => number>(() => 77);
const mockFiscal = jest.fn<(...args: any[]) => number>(() => 2024);
const mockSort = jest.fn<(items: any[], sortingType: number) => void>((items) => {
    items.sort((left, right) => Number(right.totalAmountCents) - Number(left.totalAmountCents));
});

type TemplateAction = {
    component: string;
    label: string;
    handler: (...args: any[]) => unknown;
};

const mockTemplateActions: TemplateAction[] = [];

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
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
    ChartDateAggregationType: mockDateTypes
}));
jest.mock('@/consts/color.ts', () => ({ DEFAULT_CHART_COLORS: ['fallback-0', 'fallback-1'] }));
jest.mock('@/lib/common.ts', () => ({
    isNumber: (value: unknown) => typeof value === 'number' && Number.isFinite(value)
}));
jest.mock('@/lib/datetime.ts', () => ({
    getYearMonthFirstUnixTime: (...args: any[]) => mockFirst(...args),
    getYearMonthLastUnixTime: (...args: any[]) => mockLast(...args),
    getDateTypeByDateRange: (...args: any[]) => mockDateType(...args),
    getFiscalYearFromUnixTime: (...args: any[]) => mockFiscal(...args)
}));
jest.mock('@/lib/color.ts', () => ({ getDisplayColor: (value: string) => `display:${value}` }));
jest.mock('@/lib/statistics.ts', () => ({
    sortStatisticsItems: (items: any[], sortingType: number) => mockSort(items, sortingType)
}));

const TrendsBarChart = require('@/components/mobile/TrendsBarChart.vue').default as any;
const {
    createRenderer,
    createSSRApp,
    defineComponent,
    h,
    nextTick,
    reactive
} = jest.requireActual('vue') as any;
const { renderToString } = jest.requireActual('vue/server-renderer') as any;

function dailyItem(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'food',
        name: 'Food',
        color: 'red',
        hidden: false,
        displayOrders: [2],
        items: [
            { year: 2024, month: 1, day: 2, amountCents: 100 },
            { year: 2024, month: 1, day: 2, amountCents: 50 }
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
        dataAggregationType: mockAggregations.Sum,
        dateAggregationType: mockDateTypes.Year.type,
        idField: 'id',
        nameField: 'name',
        valueField: 'amountCents',
        colorField: 'color',
        hiddenField: 'hidden',
        displayOrdersField: 'displayOrders',
        translateName: true,
        defaultCurrency: 'CNY',
        enableClickItem: true,
        loading: false,
        ...overrides
    };
}

function monthlyProps(overrides: Record<string, unknown> = {}): any {
    return dailyProps({
        chartMode: 'monthly',
        items: [{
            id: 'salary',
            name: 'Salary',
            hidden: false,
            items: [
                { year: 2024, month1base: 1, amountCents: 100 },
                { year: 2024, month1base: 1, amountCents: 250 }
            ]
        }],
        startTime: undefined,
        endTime: undefined,
        startYearMonth: { year: 2024, month1base: 1 },
        endYearMonth: { year: 2024, month1base: 12 },
        dataAggregationType: mockAggregations.Last,
        colorField: undefined,
        displayOrdersField: undefined,
        translateName: false,
        ...overrides
    });
}

function setupChart(chartProps: any): { bindings: any; emit: jest.Mock } {
    const emit = jest.fn();
    const bindings = TrendsBarChart.setup(chartProps, {
        attrs: {},
        slots: {},
        emit,
        expose: () => undefined
    });
    return { bindings, emit };
}

function textFromVNodes(value: unknown): string {
    if (value === null || value === undefined || typeof value === 'boolean') return '';
    if (typeof value === 'string' || typeof value === 'number') return String(value);
    if (Array.isArray(value)) return value.map(textFromVNodes).join(' ');
    if (typeof value === 'object') {
        const vnode = value as { children?: unknown; props?: Record<string, unknown> };
        return [textFromVNodes(vnode.children), textFromVNodes(vnode.props?.['title'])].filter(Boolean).join(' ');
    }
    return '';
}

function createF7Stub(component: string): any {
    return defineComponent({
        inheritAttrs: false,
        props: {
            virtualList: { type: Boolean, default: false },
            virtualListParams: { type: Object, default: undefined }
        },
        setup: (stubProps: any, { attrs, slots }: any) => () => {
            const virtualListParams = stubProps.virtualListParams as {
                items?: unknown[];
                renderExternal?: (virtualList: unknown, data: { items: unknown[]; topPosition: number }) => void;
            } | undefined;
            if (stubProps.virtualList && virtualListParams?.renderExternal) {
                virtualListParams.renderExternal({}, {
                    items: virtualListParams.items || [],
                    topPosition: 0
                });
            }
            const children = Object.values(slots).flatMap(slot => (
                typeof slot === 'function' ? (slot as any)({}) : []
            ));
            if (typeof attrs.onClick === 'function') {
                mockTemplateActions.push({
                    component,
                    label: [textFromVNodes(children), textFromVNodes(attrs.title)].filter(Boolean).join(' '),
                    handler: attrs.onClick
                });
            }
            return h(`${component}-stub`, attrs, children);
        }
    });
}

function registerF7Stubs(app: any): void {
    for (const name of ['f7-list', 'f7-list-item', 'f7-icon', 'f7-progressbar']) {
        app.component(name, createF7Stub(name));
    }
    app.config.warnHandler = () => undefined;
}

async function renderChart(chartProps: any): Promise<string> {
    mockTemplateActions.length = 0;
    const app = createSSRApp(TrendsBarChart, chartProps);
    registerF7Stubs(app);
    return renderToString(app);
}

type HostNode = {
    type: string;
    props: Record<string, unknown>;
    children: HostNode[];
    parent: HostNode | null;
    text: string;
    style: Record<string, unknown>;
};

function hostNode(type: string, text = ''): HostNode {
    return { type, props: {}, children: [], parent: null, text, style: {} };
}

function createMemoryRenderer(): any {
    return createRenderer({
        patchProp(node: HostNode, key: string, _previous: unknown, next: unknown) {
            if (next === null || next === undefined) delete node.props[key];
            else node.props[key] = next;
        },
        insert(child: HostNode, parent: HostNode, anchor: HostNode | null = null) {
            child.parent = parent;
            const anchorIndex = anchor ? parent.children.indexOf(anchor) : -1;
            if (anchorIndex >= 0) parent.children.splice(anchorIndex, 0, child);
            else parent.children.push(child);
        },
        remove(child: HostNode) {
            if (!child.parent) return;
            const index = child.parent.children.indexOf(child);
            if (index >= 0) child.parent.children.splice(index, 1);
            child.parent = null;
        },
        createElement(type: string) {
            return hostNode(type);
        },
        createText(text: string) {
            return hostNode('#text', text);
        },
        createComment(text: string) {
            return hostNode('#comment', text);
        },
        setText(node: HostNode, text: string) {
            node.text = text;
        },
        setElementText(node: HostNode, text: string) {
            const child = hostNode('#text', text);
            child.parent = node;
            node.children = [child];
        },
        parentNode(node: HostNode) {
            return node.parent;
        },
        nextSibling(node: HostNode) {
            if (!node.parent) return null;
            const index = node.parent.children.indexOf(node);
            return node.parent.children[index + 1] || null;
        },
        querySelector() {
            return null;
        },
        setScopeId() {},
        cloneNode(node: HostNode) {
            return { ...node, props: { ...node.props }, children: [...node.children], parent: null };
        },
        insertStaticContent(content: string, parent: HostNode, anchor: HostNode | null) {
            const node = hostNode('#static', content);
            node.parent = parent;
            const anchorIndex = anchor ? parent.children.indexOf(anchor) : -1;
            if (anchorIndex >= 0) parent.children.splice(anchorIndex, 0, node);
            else parent.children.push(node);
            return [node, node];
        }
    });
}

function flattenHost(root: HostNode): HostNode[] {
    return [root, ...root.children.flatMap(flattenHost)];
}

function hostText(root: HostNode): string {
    return flattenHost(root).map(node => node.text).join(' ');
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateActions.length = 0;
    mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 199 }];
    mockFirst.mockReturnValue(130);
    mockLast.mockReturnValue(170);
    mockDateType.mockReturnValue(77);
    mockFiscal.mockReturnValue(2024);
});

describe('mobile TrendsBarChart production-loaded behavior', () => {
    test('covers empty, single, multi-series, positive, negative, zero, invalid, hidden, and fallback metadata', () => {
        mockRanges = [];
        const empty = setupChart(dailyProps({ items: [] })).bindings;
        expect(empty.allDisplayDataItems.value).toEqual({ data: [], legends: [] });
        expect(empty.useVirtualList.value).toBe(true);

        mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 199 }];
        const values = setupChart(dailyProps({
            items: [
                dailyItem(),
                dailyItem({
                    id: 'refund',
                    name: '',
                    color: '',
                    displayOrders: [],
                    items: [
                        { year: 2024, month: 1, day: 2, amountCents: -50 },
                        { year: 2024, month: 1, day: 2, amountCents: Number.NaN }
                    ]
                }),
                dailyItem({ id: 'zero', name: 'Zero', color: undefined, displayOrders: null, items: [
                    { year: 2024, month: 1, day: 2, amountCents: 0 }
                ] }),
                dailyItem({ id: 'invalid', name: 'Invalid', items: [
                    { year: 2024, month: 1, day: 2, amountCents: Number.NaN }
                ] }),
                dailyItem({ id: 'hidden', hidden: true, items: [] })
            ]
        })).bindings;

        expect(values.useVirtualList.value).toBe(false);
        expect(values.allDisplayDataItems.value.legends).toEqual([
            { id: 'food', name: 'translated:Food', color: 'display:red', displayOrders: [2] },
            { id: 'refund', name: 'refund', color: 'display:fallback-1', displayOrders: [] },
            { id: 'zero', name: 'translated:Zero', color: 'display:fallback-0', displayOrders: [0] },
            { id: 'invalid', name: 'translated:Invalid', color: 'display:red', displayOrders: [2] }
        ]);
        expect(values.allDisplayDataItems.value.data[0]).toEqual(expect.objectContaining({
            totalAmountCents: 100,
            totalPositiveAmountCents: 150,
            maxAmount: 150,
            percent: 100
        }));
        expect(values.allDisplayDataItems.value.data[0].items.map((entry: any) => entry.totalAmountCents))
            .toEqual([150, 0, 0, -50]);
        expect(mockSort).toHaveBeenCalledWith(expect.any(Array), 0);

        const fallbackIdentity = setupChart(dailyProps({
            idField: undefined,
            items: [dailyItem({ name: 'Named' })]
        })).bindings;
        expect(fallbackIdentity.allDisplayDataItems.value.legends[0].id).toBe('translated:Named');
    });

    test.each([
        ['year', mockDateTypes.Year.type, { year: 2024, minUnixTime: 100, maxUnixTime: 199 }, 'year:100'],
        ['fiscal year', mockDateTypes.FiscalYear.type, { year: 2024, minUnixTime: 100, maxUnixTime: 199 }, 'fiscal:100'],
        ['quarter', mockDateTypes.Quarter.type, { year: 2024, quarter: 1, minUnixTime: 100, maxUnixTime: 199 }, 'quarter:2024-1'],
        ['month', mockDateTypes.Month.type, { year: 2024, month0base: 0, minUnixTime: 100, maxUnixTime: 199 }, 'month:100'],
        ['day', mockDateTypes.Day.type, { year: 2024, month: 1, day: 2, minUnixTime: 100, maxUnixTime: 199 }, 'day:100']
    ])('maps daily %s ranges and labels', (_name, dateAggregationType, range, label) => {
        mockRanges = [range];
        const bindings = setupChart(dailyProps({ dateAggregationType })).bindings;
        expect(bindings.allDisplayDataItems.value.data[0]).toEqual(expect.objectContaining({
            displayDateRange: label,
            totalAmountCents: 150,
            percent: 100
        }));
    });

    test.each([
        ['year', mockDateTypes.Year.type, { year: 2024, minUnixTime: 100, maxUnixTime: 199 }, 'year:100'],
        ['fiscal year', mockDateTypes.FiscalYear.type, { year: 2024, minUnixTime: 100, maxUnixTime: 199 }, 'fiscal:100'],
        ['quarter', mockDateTypes.Quarter.type, { year: 2024, quarter: 1, minUnixTime: 100, maxUnixTime: 199 }, 'quarter:2024-1'],
        ['month', mockDateTypes.Month.type, { year: 2024, month0base: 0, minUnixTime: 100, maxUnixTime: 199 }, 'month:100']
    ])('maps monthly %s ranges with last-value aggregation', (_name, dateAggregationType, range, label) => {
        mockRanges = [range];
        const bindings = setupChart(monthlyProps({ dateAggregationType })).bindings;
        expect(bindings.allDisplayDataItems.value.data[0]).toEqual(expect.objectContaining({
            displayDateRange: label,
            totalAmountCents: 250,
            percent: 100
        }));
    });

    test('legend toggling, virtual updates, and daily/monthly click drilldown preserve identities and clamps', () => {
        const visibleSecond = dailyItem({ id: 'salary', name: 'Salary', color: 'blue' });
        const hidden = dailyItem({ id: 'hidden', hidden: true });
        const daily = setupChart(dailyProps({ items: [dailyItem(), visibleSecond, hidden] }));
        const row = daily.bindings.allDisplayDataItems.value.data[0];

        daily.bindings.clickItem(row);
        expect(daily.emit).toHaveBeenLastCalledWith('click', {
            itemId: 'food,salary',
            dateRange: { minTime: 120, maxTime: 180, dateType: 77 }
        });

        daily.bindings.toggleLegend(daily.bindings.allDisplayDataItems.value.legends[1]);
        expect(daily.bindings.unselectedLegends.value).toEqual({ salary: true });
        daily.bindings.clickItem(daily.bindings.allDisplayDataItems.value.data[0]);
        expect(daily.emit).toHaveBeenLastCalledWith('click', {
            itemId: 'food',
            dateRange: { minTime: 120, maxTime: 180, dateType: 77 }
        });
        daily.bindings.toggleLegend({ id: 'salary' });
        expect(daily.bindings.unselectedLegends.value).toEqual({});

        const virtualItems = [daily.bindings.allDisplayDataItems.value.data[0]];
        daily.bindings.renderExternal({}, { items: virtualItems, topPosition: 42 });
        expect(daily.bindings.virtualDataItems.value).toEqual({ items: virtualItems, topPosition: 42 });

        const unclamped = setupChart(dailyProps({ startTime: 80, endTime: 220 }));
        unclamped.bindings.clickItem(unclamped.bindings.allDisplayDataItems.value.data[0]);
        expect(unclamped.emit).toHaveBeenLastCalledWith('click', {
            itemId: 'food',
            dateRange: { minTime: 100, maxTime: 199, dateType: 77 }
        });

        const monthly = setupChart(monthlyProps());
        monthly.bindings.clickItem(monthly.bindings.allDisplayDataItems.value.data[0]);
        expect(monthly.emit).toHaveBeenLastCalledWith('click', {
            itemId: 'salary',
            dateRange: { minTime: 130, maxTime: 170, dateType: 77 }
        });
        mockFirst.mockReturnValueOnce(80);
        mockLast.mockReturnValueOnce(220);
        const monthlyUnclamped = setupChart(monthlyProps());
        monthlyUnclamped.bindings.clickItem(monthlyUnclamped.bindings.allDisplayDataItems.value.data[0]);
        expect(monthlyUnclamped.emit).toHaveBeenLastCalledWith('click', {
            itemId: 'salary',
            dateRange: { minTime: 100, maxTime: 199, dateType: 77 }
        });

        const noHiddenField = setupChart(dailyProps({ hiddenField: undefined }));
        noHiddenField.bindings.clickItem(noHiddenField.bindings.allDisplayDataItems.value.data[0]);
        expect(noHiddenField.emit).toHaveBeenLastCalledWith('click', expect.objectContaining({ itemId: '' }));
        const nameIdentity = setupChart(dailyProps({ idField: undefined }));
        nameIdentity.bindings.clickItem(nameIdentity.bindings.allDisplayDataItems.value.data[0]);
        expect(nameIdentity.emit).toHaveBeenLastCalledWith('click', expect.objectContaining({
            itemId: 'translated:Food'
        }));
        expect(mockDateType).toHaveBeenCalledWith(expect.any(Number), expect.any(Number), 1, 4, 'normal');
    });

    test('SSR covers loading, no-data, multi-legend, stacked, non-stacked, colors, formatting, and template click', async () => {
        const loadingHtml = await renderChart(dailyProps({ loading: true }));
        expect(loadingHtml).toContain('skeleton-text');
        expect(loadingHtml).toContain('0.00 USD');

        mockRanges = [];
        const emptyHtml = await renderChart(dailyProps({ items: [] }));
        expect(emptyHtml).toContain('tt:No transaction data');

        mockRanges = [{ year: 2024, minUnixTime: 100, maxUnixTime: 199 }];
        const nonStackedHtml = await renderChart(dailyProps({
            items: [dailyItem(), dailyItem({ id: 'salary', name: 'Salary', color: 'blue' })]
        }));
        expect(nonStackedHtml).toContain('translated:Food');
        expect(nonStackedHtml).toContain('translated:Salary');
        expect(nonStackedHtml).toContain('display:red');
        expect(nonStackedHtml).toContain('statistics-list-item-non-stacked');
        const rowClick = mockTemplateActions.find(action => (
            action.component === 'f7-list-item' && action.label.includes('year:100')
        ));
        expect(rowClick).toBeDefined();
        await rowClick!.handler();

        const stackedHtml = await renderChart(dailyProps({ stacked: true }));
        expect(stackedHtml).toContain('statistics-list-item-stacked');
        expect(stackedHtml).toContain('CNY:150');
        expect(stackedHtml).toContain('display:red');
    });

    test('custom renderer mounts, reacts to deep item updates, dispatches template events, and unmounts cleanly', async () => {
        const renderer = createMemoryRenderer();
        const root = hostNode('root');
        const state = reactive(dailyProps({
            items: [dailyItem(), dailyItem({ id: 'salary', name: 'Salary', color: 'blue' })],
            stacked: true
        }));
        const Root = defineComponent({ setup: () => () => h(TrendsBarChart, state) });
        const app = renderer.createApp(Root);
        registerF7Stubs(app);

        expect(() => app.mount(root)).not.toThrow();
        expect(hostText(root)).toContain('CNY:300');

        const legend = flattenHost(root).find(node => (
            node.type === 'div'
            && String(node.props['class'] || '').includes('trends-bar-chart-legend')
            && typeof node.props['onClick'] === 'function'
        ));
        expect(legend).toBeDefined();
        (legend!.props['onClick'] as () => void)();
        await nextTick();
        expect(String(legend!.props['class'])).toContain('trends-bar-chart-legend-unselected');

        const row = flattenHost(root).find(node => (
            node.type === 'f7-list-item-stub'
            && String(node.props['class'] || '').includes('statistics-list-item')
            && typeof node.props['onClick'] === 'function'
        ));
        expect(row).toBeDefined();
        (row!.props['onClick'] as () => void)();

        state.items[1].items[0].amountCents = 400;
        await nextTick();
        expect(hostText(root)).toContain('CNY:450');

        expect(() => app.unmount()).not.toThrow();
        expect(root.children).toEqual([]);
    });
});
