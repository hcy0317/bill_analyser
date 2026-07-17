/* eslint-disable @typescript-eslint/no-explicit-any, @typescript-eslint/no-require-imports */
import { afterAll, beforeAll, beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const { proxyRefs, reactive } = actualVue;

const mockTemplateRefs = new Map<string, { value: any }>();
const mockSearchbarClear = jest.fn();
const mockScrollToSelectedItem = jest.fn();
const mockScrollSheetToTop = jest.fn();
let consoleWarnSpy: jest.SpiedFunction<typeof console.warn>;

jest.mock('vue', () => ({
    ...actualVue,
    useTemplateRef: (name: string) => {
        const value = actualVue.ref(
            name === 'sheet'
                ? { $el: { id: 'sheet-element' } }
                : { clear: mockSearchbarClear }
        );
        mockTemplateRefs.set(name, value);
        return value;
    }
}));

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        ti: (value: string, enabled: boolean) => enabled ? `i18n:${value}` : String(value)
    })
}));

jest.mock('@/lib/ui/mobile.ts', () => ({
    scrollToSelectedItem: (...args: unknown[]) => mockScrollToSelectedItem(...args),
    scrollSheetToTop: (...args: unknown[]) => mockScrollSheetToTop(...args)
}));

const TreeViewSelectionSheet = require('@/components/mobile/TreeViewSelectionSheet.vue').default as any;
const TwoColumnListItemSelectionSheet = require('@/components/mobile/TwoColumnListItemSelectionSheet.vue').default as any;

function setup(component: any, props: Record<string, unknown>): {
    props: Record<string, unknown>;
    bindings: any;
    emit: jest.Mock;
} {
    const reactiveProps = reactive(props);
    const emit = jest.fn();
    const bindings = component.setup(reactiveProps, {
        attrs: {},
        slots: {},
        emit,
        expose: jest.fn()
    });
    return { props: reactiveProps, bindings, emit };
}

function render(component: any, props: Record<string, unknown>, bindings: any): any {
    return component.render({}, [], props, proxyRefs(bindings), {}, {});
}

function collectVNodeHandlers(node: any, handlers: Array<(event: any) => unknown> = []): Array<(event: any) => unknown> {
    if (!node) return handlers;
    if (Array.isArray(node)) {
        for (const child of node) collectVNodeHandlers(child, handlers);
        return handlers;
    }
    if (typeof node !== 'object') return handlers;

    for (const [name, candidate] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        for (const handler of Array.isArray(candidate) ? candidate : [candidate]) {
            if (typeof handler === 'function') handlers.push(handler as (event: any) => unknown);
        }
    }

    if (Array.isArray(node.children)) {
        collectVNodeHandlers(node.children, handlers);
    } else if (node.children && typeof node.children === 'object') {
        for (const child of Object.values(node.children)) {
            if (typeof child === 'function') {
                try {
                    collectVNodeHandlers((child as () => unknown)(), handlers);
                } catch {
                    // Framework-owned scoped slots can require component-specific slot arguments.
                }
            } else {
                collectVNodeHandlers(child, handlers);
            }
        }
    }

    return handlers;
}

async function renderToHtml(component: any, props: Record<string, unknown>): Promise<string> {
    const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
    const { renderToString } = jest.requireActual('vue/server-renderer') as any;
    const Stub = defineComponent({
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'div',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            })
        )
    });
    const app = createSSRApp(component, props);
    for (const name of [
        'f7-sheet', 'f7-toolbar', 'f7-link', 'f7-searchbar', 'f7-page-content',
        'f7-list', 'f7-list-item', 'f7-treeview', 'f7-treeview-item', 'f7-icon', 'ItemIcon'
    ]) {
        app.component(name, Stub);
    }
    app.config.warnHandler = () => undefined;
    return renderToString(app);
}

function treeProps(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        modelValue: 'cash',
        show: true,
        primaryKeyField: 'id',
        primaryTitleField: 'title',
        primaryTitleI18n: true,
        primaryIconField: 'icon',
        primaryIconType: 'account',
        primaryColorField: 'color',
        primaryHiddenField: 'hidden',
        primarySubItemsField: 'children',
        secondaryKeyField: 'id',
        secondaryValueField: 'id',
        secondaryTitleField: 'title',
        secondaryTitleI18n: true,
        secondaryIconField: 'icon',
        secondaryIconType: 'category',
        secondaryColorField: 'color',
        secondaryHiddenField: 'hidden',
        enableFilter: true,
        filterPlaceholder: 'Search',
        filterNoItemsText: 'Nothing found',
        items: [
            {
                id: 'accounts',
                title: 'Accounts',
                icon: 'wallet',
                color: '#111111',
                children: [
                    { id: 'cash', title: 'Cash', icon: 'cash', color: '#222222' },
                    { id: 'card', title: 'Card', icon: 'card', color: '#333333' },
                    { id: 'secret', title: 'Secret', hidden: true }
                ]
            },
            { id: 'empty', title: 'Empty', icon: 'empty', children: [] },
            { id: 'hidden', title: 'Hidden', hidden: true, children: [{ id: 'hidden-child', title: 'Hidden child' }] }
        ],
        ...overrides
    };
}

function columnProps(overrides: Record<string, unknown> = {}): Record<string, unknown> {
    return {
        ...treeProps(),
        primaryValueField: 'id',
        primaryHeaderField: 'header',
        primaryHeaderI18n: true,
        primaryFooterField: 'footer',
        primaryFooterI18n: true,
        secondaryHeaderField: 'header',
        secondaryHeaderI18n: true,
        secondaryFooterField: 'footer',
        secondaryFooterI18n: true,
        items: [
            {
                id: 'accounts',
                title: 'Accounts',
                header: 'Assets',
                footer: 'Primary accounts',
                icon: 'wallet',
                color: '#111111',
                children: [
                    { id: 'cash', title: 'Cash', header: 'Liquid', footer: 'Wallet', icon: 'cash', color: '#222222' },
                    { id: 'card', title: 'Card', header: 'Credit', footer: 'Bank', icon: 'card', color: '#333333' },
                    { id: 'secret', title: 'Secret', hidden: true }
                ]
            },
            {
                id: 'budgets',
                title: 'Budgets',
                header: 'Planning',
                footer: 'Monthly',
                icon: 'budget',
                children: [{ id: 'food', title: 'Food', header: 'Living', footer: 'Meals', icon: 'food' }]
            }
        ],
        ...overrides
    };
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
    Object.defineProperty(globalThis, 'window', {
        configurable: true,
        value: { innerHeight: 720 }
    });
});

beforeAll(() => {
    consoleWarnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
});

afterAll(() => {
    consoleWarnSpy.mockRestore();
});

describe('mobile TreeViewSelectionSheet production behavior', () => {
    test('derives all height classes and finds selected or filtered secondary values', () => {
        expect(setup(TreeViewSelectionSheet, treeProps()).bindings.heightClass.value)
            .toBe('tree-view-selection-default-sheet');
        expect(setup(TreeViewSelectionSheet, treeProps({
            items: Array.from({ length: 3 }, (_, index) => ({ id: index, title: String(index), children: [] }))
        })).bindings.heightClass.value).toBe('tree-view-selection-large-sheet');
        expect(setup(TreeViewSelectionSheet, treeProps({
            items: Array.from({ length: 7 }, (_, index) => ({ id: index, title: String(index), children: [] }))
        })).bindings.heightClass.value).toBe('tree-view-selection-huge-sheet');

        const { bindings } = setup(TreeViewSelectionSheet, treeProps());
        const accounts = treeProps()['items'] as any[];
        expect(bindings.isPrimaryItemHasSecondaryValue(accounts[0])).toBe(true);
        expect(bindings.isPrimaryItemHasSecondaryValue(accounts[1])).toBe(false);
        bindings.currentValue.value = 'missing';
        bindings.filterContent.value = 'card';
        expect(bindings.isPrimaryItemHasSecondaryValue(accounts[0])).toBe(true);
        bindings.filterContent.value = 'absent';
        expect(bindings.isPrimaryItemHasSecondaryValue(accounts[0])).toBe(false);

        const child = { title: 'Object child' };
        const objectSetup = setup(TreeViewSelectionSheet, treeProps({
            modelValue: child,
            secondaryValueField: undefined,
            secondaryHiddenField: undefined,
            items: [{ title: 'Objects', children: [child] }]
        }));
        expect(objectSetup.bindings.isPrimaryItemHasSecondaryValue(
            (objectSetup.props['items'] as any[])[0]
        )).toBe(true);
    });

    test('selects, focuses, restores, closes, and tolerates absent template refs', () => {
        const props = treeProps();
        const { bindings, emit } = setup(TreeViewSelectionSheet, props);
        const card = (props['items'] as any[])[0].children[1];
        bindings.onSecondaryItemClicked(card);
        expect(bindings.currentValue.value).toBe('card');
        expect(emit.mock.calls).toEqual([
            ['update:modelValue', 'card'],
            ['update:show', false]
        ]);

        bindings.onSearchBarFocus();
        expect(mockScrollSheetToTop).toHaveBeenCalledWith({ id: 'sheet-element' }, 720);
        bindings.currentValue.value = 'card';
        bindings.onSheetOpen({ $el: { id: 'opened-tree-sheet' } });
        expect(bindings.currentValue.value).toBe('cash');
        expect(mockScrollToSelectedItem).toHaveBeenCalledWith(
            { id: 'opened-tree-sheet' },
            '.page-content',
            '.treeview-item .treeview-item-selected'
        );

        bindings.filterContent.value = 'cash';
        bindings.onSheetClosed();
        expect(bindings.filterContent.value).toBe('');
        expect(mockSearchbarClear).toHaveBeenCalledTimes(1);
        expect(emit).toHaveBeenLastCalledWith('update:show', false);

        bindings.sheet.value = null;
        bindings.searchbar.value = null;
        expect(() => bindings.onSearchBarFocus()).not.toThrow();
        expect(() => bindings.onSheetClosed()).not.toThrow();
    });

    test('renders populated, filtered-empty, icon-free, and generated event paths', () => {
        const populated = setup(TreeViewSelectionSheet, treeProps());
        const vnode = render(TreeViewSelectionSheet, populated.props, populated.bindings);
        expect(vnode).toBeDefined();
        const handlers = collectVNodeHandlers(vnode);
        expect(handlers.length).toBeGreaterThan(4);
        for (const handler of handlers) {
            handler({ target: { value: 'card' }, $el: { id: 'template-tree-sheet' } });
        }
        expect(populated.emit).toHaveBeenCalled();

        populated.bindings.filterContent.value = 'no-match';
        expect(render(TreeViewSelectionSheet, populated.props, populated.bindings)).toBeDefined();

        const iconFree = setup(TreeViewSelectionSheet, treeProps({
            enableFilter: false,
            primaryKeyField: undefined,
            primaryTitleField: undefined,
            primaryIconField: undefined,
            primaryColorField: undefined,
            secondaryKeyField: undefined,
            secondaryValueField: undefined,
            secondaryTitleField: undefined,
            secondaryIconField: undefined,
            secondaryColorField: undefined,
            items: [{ children: ['raw child'] }],
            modelValue: 'raw child'
        }));
        expect(render(TreeViewSelectionSheet, iconFree.props, iconFree.bindings)).toBeDefined();
    });

    test('SSR executes populated and icon-free production slots', async () => {
        expect(await renderToHtml(TreeViewSelectionSheet, treeProps())).toContain('tree-view-selection-default-sheet');
        expect(await renderToHtml(TreeViewSelectionSheet, treeProps({
            primaryIconField: undefined,
            primaryColorField: undefined,
            secondaryIconField: undefined,
            secondaryColorField: undefined,
            items: [{ title: 'Raw', children: [{ title: 'Raw child' }] }]
        }))).toContain('Raw');
    });
});

describe('mobile TwoColumnListItemSelectionSheet production behavior', () => {
    test('projects primary and secondary selection state and emits close on selection', () => {
        const props = columnProps();
        const { bindings, emit } = setup(TwoColumnListItemSelectionSheet, props);
        expect(bindings.currentPrimaryValue.value).toBe('accounts');
        expect(bindings.selectedPrimaryItem.value).toStrictEqual((props['items'] as any[])[0]);
        expect(bindings.filteredSubItems.value.map((item: any) => item.id)).toEqual(['cash', 'card']);
        expect(bindings.isSecondarySelected((props['items'] as any[])[0].children[0])).toBe(true);
        expect(bindings.isSecondarySelected((props['items'] as any[])[0].children[1])).toBe(false);

        bindings.onPrimaryItemClicked((props['items'] as any[])[1]);
        expect(bindings.currentPrimaryValue.value).toBe('budgets');
        expect(bindings.selectedPrimaryItem.value).toStrictEqual((props['items'] as any[])[1]);
        const food = (props['items'] as any[])[1].children[0];
        bindings.onSecondaryItemClicked(food);
        expect(bindings.currentSecondaryValue.value).toBe('food');
        expect(emit.mock.calls).toEqual([
            ['update:modelValue', 'food'],
            ['update:show', false]
        ]);

        bindings.close();
        expect(emit).toHaveBeenLastCalledWith('update:show', false);
    });

    test('restores both columns, scrolls each selection, focuses, and clears on close', () => {
        const { bindings, emit } = setup(TwoColumnListItemSelectionSheet, columnProps());
        bindings.currentPrimaryValue.value = 'budgets';
        bindings.currentSecondaryValue.value = 'food';
        bindings.onSheetOpen({ $el: { id: 'opened-column-sheet' } });
        expect(bindings.currentPrimaryValue.value).toBe('accounts');
        expect(bindings.currentSecondaryValue.value).toBe('cash');
        expect(mockScrollToSelectedItem.mock.calls).toEqual([
            [{ id: 'opened-column-sheet' }, '.primary-list-container', 'li.primary-list-item-selected'],
            [{ id: 'opened-column-sheet' }, '.secondary-list-container', 'li.secondary-list-item-selected']
        ]);

        bindings.onSearchBarFocus();
        expect(mockScrollSheetToTop).toHaveBeenCalledWith({ id: 'sheet-element' }, 720);
        bindings.filterContent.value = 'cash';
        bindings.onSheetClosed();
        expect(bindings.filterContent.value).toBe('');
        expect(mockSearchbarClear).toHaveBeenCalledTimes(1);
        expect(emit).toHaveBeenLastCalledWith('update:show', false);

        bindings.sheet.value = null;
        bindings.searchbar.value = null;
        bindings.onSearchBarFocus();
        bindings.onSheetClosed();
    });

    test('renders field-rich, empty, object-backed, and generated event paths', () => {
        const populated = setup(TwoColumnListItemSelectionSheet, columnProps());
        const vnode = render(TwoColumnListItemSelectionSheet, populated.props, populated.bindings);
        expect(vnode).toBeDefined();
        const handlers = collectVNodeHandlers(vnode);
        expect(handlers.length).toBeGreaterThan(5);
        for (const handler of handlers) {
            handler({ target: { value: 'food' }, $el: { id: 'template-column-sheet' } });
        }
        expect(populated.emit).toHaveBeenCalled();

        populated.bindings.filterContent.value = 'no-match';
        expect(render(TwoColumnListItemSelectionSheet, populated.props, populated.bindings)).toBeDefined();

        const primary = { children: [{ title: 'Raw child' }] };
        const objectBacked = setup(TwoColumnListItemSelectionSheet, columnProps({
            modelValue: primary.children[0],
            primaryValueField: undefined,
            primaryKeyField: undefined,
            primaryTitleField: undefined,
            primaryHeaderField: undefined,
            primaryFooterField: undefined,
            primaryIconField: undefined,
            primaryColorField: undefined,
            secondaryValueField: undefined,
            secondaryKeyField: undefined,
            secondaryTitleField: undefined,
            secondaryHeaderField: undefined,
            secondaryFooterField: undefined,
            secondaryIconField: undefined,
            secondaryColorField: undefined,
            enableFilter: false,
            items: [primary]
        }));
        expect(render(TwoColumnListItemSelectionSheet, objectBacked.props, objectBacked.bindings)).toBeDefined();
        expect(objectBacked.bindings.selectedPrimaryItem.value).toStrictEqual(primary);
    });

    test('SSR executes field-rich and icon-free production slots', async () => {
        expect(await renderToHtml(TwoColumnListItemSelectionSheet, columnProps())).toContain('primary-list-container');
        expect(await renderToHtml(TwoColumnListItemSelectionSheet, columnProps({
            primaryIconField: undefined,
            primaryColorField: undefined,
            secondaryIconField: undefined,
            secondaryColorField: undefined
        }))).toContain('secondary-list-container');
    });
});
