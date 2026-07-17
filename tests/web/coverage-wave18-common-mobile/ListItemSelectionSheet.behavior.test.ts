/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockScrollToSelectedItem = jest.fn<(...args: any[]) => void>();
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        ti: (value: string, enabled: boolean) => enabled ? `i18n:${value}` : String(value)
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    scrollToSelectedItem: (...args: any[]) => mockScrollToSelectedItem(...args)
}));

const ListItemSelectionSheet = require('@/components/mobile/ListItemSelectionSheet.vue').default as any;

function itemProps(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        modelValue: 'cash',
        valueType: 'item',
        keyField: 'id',
        valueField: 'id',
        titleField: 'title',
        titleI18n: true,
        afterField: 'after',
        afterI18n: true,
        iconField: 'icon',
        iconType: 'account',
        colorField: 'color',
        hiddenField: 'hidden',
        items: [
            { id: 'cash', title: 'Cash', after: 'Wallet', icon: 'cash', color: '#111' },
            { id: 'card', title: 'Card', after: 'Bank', icon: 'card', color: '#222' },
            { id: 'hidden', title: 'Hidden', hidden: true },
            0
        ],
        show: true,
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
    const bindings = ListItemSelectionSheet.setup(reactiveProps, {
        attrs: {}, slots: {}, emit, expose: jest.fn()
    });
    return { bindings, emit, props: reactiveProps };
}

function render(runtime: ReturnType<typeof setup>): unknown {
    return ListItemSelectionSheet.render(
        {}, [], runtime.props, actualVue.proxyRefs(runtime.bindings), {}, {}
    );
}

function collectHandlers(node: unknown, handlers: Array<(event?: unknown) => unknown> = []): Array<(event?: unknown) => unknown> {
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
            if (typeof handler === 'function') handlers.push(handler as (event?: unknown) => unknown);
        }
    }
    if (Array.isArray(vnode.children)) collectHandlers(vnode.children, handlers);
    if (vnode.children && typeof vnode.children === 'object' && !Array.isArray(vnode.children)) {
        for (const slot of Object.values(vnode.children)) {
            if (typeof slot !== 'function') continue;
            try {
                collectHandlers((slot as any)({}), handlers);
            } catch {
                // Component-owned scoped slots can require a more specific runtime payload.
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
});

describe('ListItemSelectionSheet production-loaded behavior', () => {
    test('derives all heights and item/index selection and value modes', () => {
        expect(setup(itemProps()).bindings.heightClass.value).toBe('');
        expect(setup(itemProps({ items: Array.from({ length: 7 }, (_, index) => index) }))
            .bindings.heightClass.value).toBe('list-item-selection-large-sheet');
        expect(setup(itemProps({ items: Array.from({ length: 11 }, (_, index) => index) }))
            .bindings.heightClass.value).toBe('list-item-selection-huge-sheet');

        const runtime = setup(itemProps());
        const cash = (runtime.props['items'] as any[])[0];
        const card = (runtime.props['items'] as any[])[1];
        expect(runtime.bindings.isSelected(cash, 0)).toBe(true);
        expect(runtime.bindings.isSelected(card, 1)).toBe(false);
        expect(runtime.bindings.getItemValue(cash, 0, 'id', 'item')).toBe('cash');
        expect(runtime.bindings.getItemValue(cash, 0, undefined, 'item')).toBe(cash);
        expect(runtime.bindings.getItemValue(cash, 3, 'id', 'index')).toBe(3);

        const objectValue = { title: 'Object' };
        const objectRuntime = setup(itemProps({
            modelValue: objectValue,
            valueField: undefined,
            keyField: undefined,
            titleField: '',
            items: [objectValue]
        }));
        expect(objectRuntime.bindings.isSelected(
            (objectRuntime.props['items'] as any[])[0],
            0
        )).toBe(true);

        const indexRuntime = setup(itemProps({ modelValue: 1, valueType: 'index' }));
        expect(indexRuntime.bindings.isSelected(card, 1)).toBe(true);
        expect(indexRuntime.bindings.isSelected(cash, 0)).toBe(false);
    });

    test('selects every value mode, restores on open, scrolls, and closes', () => {
        const runtime = setup(itemProps());
        const card = (runtime.props['items'] as any[])[1];
        runtime.bindings.onItemClicked(card, 1);
        expect(runtime.bindings.currentValue.value).toBe('card');
        expect(runtime.emit.mock.calls).toEqual([
            ['update:modelValue', 'card'],
            ['update:show', false]
        ]);

        runtime.bindings.currentValue.value = 'stale';
        runtime.bindings.onSheetOpen({ $el: { id: 'sheet' } });
        expect(runtime.bindings.currentValue.value).toBe('cash');
        expect(mockScrollToSelectedItem).toHaveBeenCalledWith(
            { id: 'sheet' }, '.page-content', 'li.list-item-selected'
        );
        runtime.bindings.onSheetClosed();
        expect(runtime.emit).toHaveBeenLastCalledWith('update:show', false);

        const objectValue = { title: 'Object' };
        const objectRuntime = setup(itemProps({
            modelValue: null,
            valueField: undefined,
            items: [objectValue]
        }));
        objectRuntime.bindings.onItemClicked(objectValue, 0);
        expect(objectRuntime.emit).toHaveBeenCalledWith('update:modelValue', objectValue);

        const indexRuntime = setup(itemProps({ modelValue: 0, valueType: 'index' }));
        indexRuntime.bindings.onItemClicked({}, 2);
        expect(indexRuntime.emit).toHaveBeenCalledWith('update:modelValue', 2);
    });

    test('executes rich, field-free, hidden, and generated template event branches', () => {
        const rich = setup(itemProps());
        const handlers = collectHandlers(render(rich));
        expect(handlers.length).toBeGreaterThan(3);
        for (const handler of handlers) handler({ $el: { id: 'template-sheet' } });
        expect(rich.emit).toHaveBeenCalled();

        const fieldFree = setup(itemProps({
            modelValue: 'plain',
            valueType: 'item',
            keyField: undefined,
            valueField: undefined,
            titleField: '',
            titleI18n: false,
            afterField: undefined,
            afterI18n: false,
            iconField: undefined,
            colorField: undefined,
            hiddenField: undefined,
            items: ['plain']
        }));
        expect(render(fieldFree)).toBeDefined();
    });
});
