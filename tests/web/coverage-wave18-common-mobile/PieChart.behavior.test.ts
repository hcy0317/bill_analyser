/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
let mockValidItems: any[] = [];
let mockSelectedIndex = actualVue.ref(0);
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` })
}));
jest.mock('@/components/base/PieChartBase.ts', () => ({
    usePieChartBase: () => ({
        selectedIndex: mockSelectedIndex,
        validItems: actualVue.computed(() => mockValidItems)
    })
}));

const PieChart = require('@/components/mobile/PieChart.vue').default as any;

function pieItem(value: number, actualPercent: number, overrides: Record<string, unknown> = {}): any {
    return {
        id: `item-${value}`,
        name: `item-${value}`,
        displayName: `Item ${value}`,
        value,
        percent: actualPercent * 100,
        actualPercent,
        color: `color-${value}`,
        sourceItem: { id: `source-${value}` },
        displayPercent: `${actualPercent * 100}%`,
        displayValue: `CNY:${value}`,
        ...overrides
    };
}

function props(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        items: [],
        nameField: 'name',
        valueField: 'value',
        skeleton: false,
        showValue: true,
        showPercent: true,
        enableClickItem: true,
        showCenterText: true,
        showSelectedItemInfo: true,
        centerTextBackground: 'purple',
        ...overrides
    };
}

function setup(input: Record<string, unknown>): {
    bindings: any;
    emit: jest.Mock;
    props: Record<string, unknown>;
} {
    const reactiveProps = actualVue.reactive(input);
    const emit = jest.fn();
    const bindings = PieChart.setup(reactiveProps, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    return { bindings, emit, props: reactiveProps };
}

function render(runtime: ReturnType<typeof setup>): unknown {
    return PieChart.render(
        {}, [], runtime.props, actualVue.proxyRefs(runtime.bindings), {}, {}
    );
}

async function renderToHtml(input: Record<string, unknown>): Promise<string> {
    const { createSSRApp, defineComponent, h } = actualVue;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => slot?.({}) ?? [])
        )
    });
    const app = createSSRApp(PieChart, input);
    for (const name of ['f7-link', 'f7-icon', 'f7-chip']) app.component(name, Stub);
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

function collectHandlers(node: unknown, handlers: Array<() => unknown> = []): Array<() => unknown> {
    if (node === null || node === undefined || typeof node === 'boolean') return handlers;
    if (Array.isArray(node)) {
        for (const child of node) collectHandlers(child, handlers);
        return handlers;
    }
    if (typeof node !== 'object') return handlers;
    const vnode = node as any;
    for (const [name, candidate] of Object.entries(vnode.props ?? {})) {
        if (!name.startsWith('on')) continue;
        for (const handler of Array.isArray(candidate) ? candidate : [candidate]) {
            if (typeof handler === 'function') handlers.push(handler as () => unknown);
        }
    }
    if (Array.isArray(vnode.children)) collectHandlers(vnode.children, handlers);
    if (vnode.children && typeof vnode.children === 'object' && !Array.isArray(vnode.children)) {
        for (const slot of Object.values(vnode.children)) {
            if (typeof slot !== 'function') continue;
            try {
                collectHandlers((slot as any)({}), handlers);
            } catch {
                // Framework7 slots do not all accept the same scoped payload.
            }
        }
    }
    return handlers;
}

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

beforeEach(() => {
    jest.clearAllMocks();
    mockSelectedIndex = actualVue.ref(0);
    mockValidItems = [];
});

describe('mobile PieChart production-loaded behavior', () => {
    test('computes totals, colors, segment geometry, previous offsets, and selected offsets', () => {
        const first = pieItem(60, 0.6);
        const zero = pieItem(0, 0);
        const third = pieItem(40, 0.4);
        mockValidItems = [first, zero, third];
        mockSelectedIndex.value = 2;
        const runtime = setup(props());

        expect(runtime.bindings.totalValidValue.value).toBe(100);
        expect(runtime.bindings.getColorStyle('red')).toEqual({ color: 'red' });
        expect(runtime.bindings.getColorStyle('blue', '--border')).toEqual({
            color: 'blue', '--border': 'blue'
        });
        const dash = runtime.bindings.getItemStrokeDash(first).split(' ').map(Number);
        expect(dash[0]).toBeCloseTo(60 * Math.PI);
        expect(dash[1]).toBeCloseTo(40 * Math.PI);
        expect(runtime.bindings.getItemDashOffset(first, mockValidItems)).toBe(25 * Math.PI);
        expect(runtime.bindings.getItemDashOffset(third, mockValidItems, 10)).toBeCloseTo(65 * Math.PI + 10);
        expect(runtime.bindings.itemCommonDashOffset.value).toBeCloseTo(-70 * Math.PI);
        expect(runtime.bindings.selectedItem.value).toBe(third);
    });

    test('handles zero totals, invalid indices, wrapping selection, direct selection, and click guards', () => {
        mockValidItems = [pieItem(0, 0), pieItem(0, 0)];
        const zero = setup(props());
        expect(zero.bindings.totalValidValue.value).toBe(0);
        expect(zero.bindings.itemCommonDashOffset.value).toBe(0);
        expect(zero.bindings.getItemDashOffset(mockValidItems[0], mockValidItems, 0)).toBe(25 * Math.PI);

        mockValidItems = [pieItem(75, 0.75), pieItem(25, 0.25)];
        const runtime = setup(props());
        runtime.bindings.switchSelectedItem(-1);
        expect(runtime.bindings.selectedIndex.value).toBe(1);
        runtime.bindings.switchSelectedItem(1);
        expect(runtime.bindings.selectedIndex.value).toBe(0);
        runtime.bindings.switchSelectedIndex(1);
        expect(runtime.bindings.selectedItem.value).toBe(mockValidItems[1]);
        runtime.bindings.switchSelectedIndex(-1);
        expect(runtime.bindings.selectedItem.value).toBe(mockValidItems[0]);
        runtime.bindings.switchSelectedIndex(99);
        expect(runtime.bindings.selectedItem.value).toBe(mockValidItems[0]);
        runtime.bindings.clickItem(mockValidItems[0]);
        expect(runtime.emit).toHaveBeenCalledWith('click', { id: 'source-75' });

        const disabled = setup(props({ enableClickItem: false }));
        disabled.bindings.clickItem(mockValidItems[0]);
        expect(disabled.emit).not.toHaveBeenCalled();

        mockValidItems = [];
        const empty = setup(props());
        expect(empty.bindings.selectedItem.value).toBeNull();
    });

    test('executes populated, skeleton, empty, and hidden template paths and their events', async () => {
        mockValidItems = [pieItem(60, 0.6), pieItem(40, 0.4)];
        expect(await renderToHtml(props())).toContain('pie-chart-text-group');
        const populated = setup(props({ showCenterText: false }));
        const handlers = collectHandlers(render(populated));
        expect(handlers.length).toBeGreaterThan(4);
        for (const handler of handlers) handler();
        expect(populated.emit).toHaveBeenCalledWith('click', expect.any(Object));

        expect(await renderToHtml(props({ skeleton: true, centerTextBackground: undefined })))
            .toContain('skeleton-text');
        const skeleton = setup(props({
            skeleton: true,
            centerTextBackground: undefined,
            showCenterText: false
        }));
        expect(render(skeleton)).toBeDefined();

        mockValidItems = [];
        const empty = setup(props({ showCenterText: false }));
        expect(render(empty)).toBeDefined();

        const hidden = setup(props({
            showCenterText: false,
            showSelectedItemInfo: false,
            showValue: false,
            showPercent: false,
            enableClickItem: false
        }));
        expect(render(hidden)).toBeDefined();
    });
});
