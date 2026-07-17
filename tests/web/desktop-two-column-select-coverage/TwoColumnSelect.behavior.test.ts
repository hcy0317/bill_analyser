import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockTemplateRefs = new Map<string, { value: any }>();
const mockSetChildInputFocus = jest.fn<(...args: any[]) => void>();
const mockScrollToSelectedItem = jest.fn<(...args: any[]) => void>();

function createSlotStub(name: string): any {
    const { defineComponent, h } = jest.requireActual('vue') as any;
    return defineComponent({
        name,
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }),
        ),
    });
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
    };
});
jest.mock('vuetify/components/VSelect', () => ({
    VSelect: createSlotStub('TwoColumnVSelectStub'),
}));
jest.mock('vuetify/components/VTextField', () => ({
    VTextField: createSlotStub('TwoColumnVTextFieldStub'),
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        ti: (text: string, i18n: boolean) => i18n ? `translated:${text}` : String(text ?? ''),
    }),
}));
jest.mock('@/lib/ui/desktop.ts', () => ({
    setChildInputFocus: (...args: any[]) => mockSetChildInputFocus(...args),
    scrollToSelectedItem: (...args: any[]) => mockScrollToSelectedItem(...args),
}));

const TwoColumnSelect = require('@/components/desktop/TwoColumnSelect.vue').default as any;

function createItems(): Record<string, unknown>[] {
    return [
        {
            id: 'group-a',
            title: 'Group Alpha',
            header: 'Header Alpha',
            footer: 'Footer Alpha',
            icon: 'folder-a',
            color: '#123456',
            hidden: false,
            children: [
                {
                    id: 'a-hidden',
                    title: 'Hidden Alpha',
                    header: 'Hidden header',
                    footer: 'Hidden footer',
                    icon: 'hidden-icon',
                    color: '#999999',
                    hidden: true,
                },
                {
                    id: 'a1',
                    title: 'Alpha One',
                    header: 'Alpha header',
                    footer: 'Alpha footer',
                    icon: 'alpha-icon',
                    color: '#abcdef',
                    hidden: false,
                },
                {
                    id: 'duplicate',
                    title: 'Duplicate from Alpha',
                    header: '',
                    footer: '',
                    icon: '',
                    color: '',
                    hidden: false,
                },
                {
                    id: 'unsafe',
                    title: '<img src=x onerror=alert(1)>',
                    header: 'Literal header',
                    footer: 'Literal footer',
                    icon: 'literal-icon',
                    color: '#000000',
                    hidden: false,
                },
            ],
        },
        {
            id: 'group-b',
            title: 'Group Beta',
            header: 'Header Beta',
            footer: 'Footer Beta',
            icon: 'folder-b',
            color: '#654321',
            hidden: false,
            children: [
                {
                    id: 'b-hidden',
                    title: 'Hidden Beta',
                    header: '',
                    footer: '',
                    icon: '',
                    color: '',
                    hidden: true,
                },
                {
                    id: 'b1',
                    title: 'Beta One',
                    header: 'Beta header',
                    footer: 'Beta footer',
                    icon: 'beta-icon',
                    color: '#fedcba',
                    hidden: false,
                },
                {
                    id: 'duplicate',
                    title: 'Duplicate from Beta',
                    header: 'Second duplicate',
                    footer: '',
                    icon: '',
                    color: '',
                    hidden: false,
                },
            ],
        },
        {
            id: 'group-empty',
            title: '',
            header: '',
            footer: '',
            icon: '',
            color: '',
            hidden: false,
            children: [
                {
                    id: 'empty-hidden',
                    title: '',
                    hidden: true,
                },
            ],
        },
        {
            id: 'group-hidden',
            title: 'Hidden Group',
            header: '',
            footer: '',
            icon: '',
            color: '',
            hidden: true,
            children: [{ id: 'hidden-child', title: 'Hidden child', hidden: false }],
        },
    ];
}

function createProps(overrides: Record<string, unknown> = {}): Record<string, any> {
    return actualVue.reactive({
        modelValue: 'a1',
        density: 'compact',
        variant: 'outlined',
        disabled: false,
        readonly: false,
        label: 'Category',
        items: createItems(),
        primaryKeyField: 'id',
        primaryValueField: 'id',
        primaryTitleField: 'title',
        primaryTitleI18n: true,
        primaryHeaderField: 'header',
        primaryHeaderI18n: false,
        primaryFooterField: 'footer',
        primaryFooterI18n: true,
        primaryIconField: 'icon',
        primaryIconType: 'category',
        primaryColorField: 'color',
        primaryHiddenField: 'hidden',
        primarySubItemsField: 'children',
        secondaryKeyField: 'id',
        secondaryValueField: 'id',
        secondaryTitleField: 'title',
        secondaryTitleI18n: true,
        secondaryHeaderField: 'header',
        secondaryHeaderI18n: true,
        secondaryFooterField: 'footer',
        secondaryFooterI18n: false,
        secondaryIconField: 'icon',
        secondaryIconType: 'category',
        secondaryColorField: 'color',
        secondaryHiddenField: 'hidden',
        enableFilter: true,
        filterPlaceholder: 'Find item',
        filterNoItemsText: 'Nothing found',
        noItemText: 'No choice',
        autoUpdateMenuPosition: false,
        showSelectionPrimaryText: true,
        showSelectionSecondaryIcon: true,
        customSelectionPrimaryText: '',
        customSelectionSecondaryText: '',
        primaryActionTitle: 'Add primary',
        primaryActionIcon: 'primary-plus',
        primaryActionDisabled: false,
        secondaryActionTitle: 'Add secondary',
        secondaryActionIcon: 'secondary-plus',
        secondaryActionDisabled: false,
        ...overrides,
    });
}

function setup(overrides: Record<string, unknown> = {}): {
    bindings: any;
    emit: jest.Mock;
    props: Record<string, any>;
} {
    const props = createProps(overrides);
    const emit = jest.fn();
    mockTemplateRefs.clear();
    const bindings = TwoColumnSelect.setup(props, {
        attrs: {}, slots: {}, emit, expose: jest.fn(),
    });
    return { bindings, emit, props };
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(overrides: Record<string, unknown> = {}): {
    app: any;
    root: any;
    state: any;
} {
    const { createRenderer, defineComponent, h } = actualVue;
    const renderer = createRenderer({
        patchProp(node: any, key: string, _previous: unknown, value: unknown) {
            node.props[key] = value;
        },
        insert(child: any, parent: any, anchor: any = null) {
            child.parent = parent;
            if (!anchor) {
                parent.children.push(child);
                return;
            }
            const index = parent.children.indexOf(anchor);
            parent.children.splice(index < 0 ? parent.children.length : index, 0, child);
        },
        remove(child: any) {
            const index = child.parent?.children.indexOf(child) ?? -1;
            if (index >= 0) child.parent.children.splice(index, 1);
        },
        createElement(type: string) {
            return createHostNode(type);
        },
        createText(text: string) {
            return createHostNode('#text', text);
        },
        createComment(text: string) {
            return createHostNode('#comment', text);
        },
        setText(node: any, text: string) {
            node.text = text;
        },
        setElementText(node: any, text: string) {
            node.text = text;
            node.children = [];
        },
        parentNode(node: any) {
            return node.parent;
        },
        nextSibling(node: any) {
            const siblings = node.parent?.children ?? [];
            return siblings[siblings.indexOf(node) + 1] ?? null;
        },
        querySelector() {
            return null;
        },
        setScopeId(node: any, scopeId: string) {
            node.props[scopeId] = '';
        },
        cloneNode(node: any) {
            return { ...node, children: [...node.children], props: { ...node.props }, parent: null };
        },
        insertStaticContent(content: string, parent: any, anchor: any) {
            const node = createHostNode('#static', content);
            node.parent = parent;
            const index = anchor ? parent.children.indexOf(anchor) : -1;
            parent.children.splice(index < 0 ? parent.children.length : index, 0, node);
            return [node, node];
        },
    });
    const SlotHost = defineComponent({
        name: 'TwoColumnGlobalSlotStub',
        inheritAttrs: false,
        setup: (_props: unknown, { attrs, slots }: any) => () => h(
            'stub',
            attrs,
            Object.values(slots).flatMap((slot: any) => {
                try {
                    return slot?.({}) ?? [];
                } catch {
                    return [];
                }
            }),
        ),
    });
    const props = createProps(overrides);
    const app = renderer.createApp(TwoColumnSelect, props);
    app.config.warnHandler = () => undefined;
    for (const name of ['v-icon', 'v-list', 'v-list-item', 'v-divider', 'ItemIcon']) {
        app.component(name, SlotHost);
    }
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
}

function walkHostNodes(node: any, visit: (node: any) => void, seen = new Set<any>()): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    visit(node);
    for (const child of node.children ?? []) walkHostNodes(child, visit, seen);
}

function collectHostCallbacks(node: any): Array<{ name: string; callback: (...args: any[]) => unknown }> {
    const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
    walkHostNodes(node, current => {
        for (const [name, value] of Object.entries(current.props ?? {})) {
            if (!name.startsWith('on')) continue;
            for (const candidate of Array.isArray(value) ? value : [value]) {
                if (typeof candidate === 'function') {
                    callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
                }
            }
        }
    });
    return callbacks;
}

async function invokeHostCallbacks(
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }>,
): Promise<void> {
    for (const { name, callback } of callbacks) {
        try {
            if (name === 'onUpdate:modelValue') callback('b1');
            else if (name === 'onUpdate:menu') callback(true);
            else if (name === 'onUpdate:focused') callback(true);
            else callback();
        } catch {
            // Generated wrappers accept different event payloads.
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockTemplateRefs.clear();
});

describe('desktop TwoColumnSelect props, filtering, and computed selection', () => {
    test('maps defaults, selected text, icons, duplicate ids, and reactive model changes', () => {
        const { bindings, props } = setup();
        expect(bindings.currentPrimaryValue.value).toBe('group-a');
        expect(bindings.currentSecondaryValue.value).toBe('a1');
        expect(bindings.selectedPrimaryItem.value).toBe(props['items'][0]);
        expect(bindings.selectedSecondaryItem.value).toBe((props['items'][0].children as any[])[1]);
        expect(bindings.selectionPrimaryItemText.value).toBe('translated:Group Alpha');
        expect(bindings.selectionSecondaryItemText.value).toBe('translated:Alpha One');
        expect(bindings.noSelectionText.value).toBe('No choice');
        expect(bindings.primaryActionIcon.value).toBe('primary-plus');
        expect(bindings.secondaryActionIcon.value).toBe('secondary-plus');
        expect(bindings.isSecondarySelected((props['items'][0].children as any[])[1])).toBe(true);
        expect(bindings.isSecondarySelected((props['items'][1].children as any[])[1])).toBe(false);

        props['modelValue'] = 'duplicate';
        expect(bindings.currentPrimaryValue.value).toBe('group-a');
        expect(bindings.selectionSecondaryItemText.value).toBe('translated:Duplicate from Alpha');

        props['modelValue'] = 'b1';
        expect(bindings.currentPrimaryValue.value).toBe('group-b');
        expect(bindings.selectionPrimaryItemText.value).toBe('translated:Group Beta');
        expect(bindings.selectionSecondaryItemText.value).toBe('translated:Beta One');

        props['modelValue'] = 'missing';
        expect(bindings.selectedPrimaryItem.value).toBeNull();
        expect(bindings.selectedSecondaryItem.value).toBeNull();
        expect(bindings.selectionPrimaryItemText.value).toBe('No choice');
        expect(bindings.selectionSecondaryItemText.value).toBe('No choice');
    });

    test('filters both columns, excludes hidden values, and returns empty results safely', () => {
        const { bindings, props } = setup();
        expect(bindings.filteredItems.value).toHaveLength(3);
        expect(bindings.filteredSubItems.value.map((item: any) => item.id)).toEqual([
            'a1', 'duplicate', 'unsafe',
        ]);

        bindings.filterContent.value = 'beta one';
        expect(bindings.filteredItems.value.map((item: any) => item.id)).toEqual(['group-b']);
        expect(bindings.filteredSubItems.value).toStrictEqual([]);

        props['modelValue'] = 'b1';
        expect(bindings.filteredSubItems.value.map((item: any) => item.id)).toEqual(['b1']);

        bindings.filterContent.value = 'group beta';
        expect(bindings.filteredItems.value.map((item: any) => item.id)).toEqual(['group-b']);
        expect(bindings.filteredSubItems.value.map((item: any) => item.id)).toEqual(['b1', 'duplicate']);

        bindings.filterContent.value = 'not present';
        expect(bindings.filteredItems.value).toStrictEqual([]);
        expect(bindings.filteredSubItems.value).toStrictEqual([]);

        const empty = setup({ items: [], modelValue: '' });
        expect(empty.bindings.filteredItems.value).toStrictEqual([]);
        expect(empty.bindings.filteredSubItems.value).toStrictEqual([]);
        expect(empty.bindings.selectedPrimaryItem.value).toBeNull();
        expect(empty.bindings.noSelectionText.value).toBe('No choice');
    });

    test('supports title fallbacks, default labels/icons, and literal unsafe-looking strings', () => {
        const defaults = setup({
            modelValue: 'unsafe',
            noItemText: undefined,
            primaryActionIcon: undefined,
            secondaryActionIcon: undefined,
        });
        expect(defaults.bindings.noSelectionText.value).toBe('tt:None');
        expect(defaults.bindings.primaryActionIcon.value).toBeTruthy();
        expect(defaults.bindings.secondaryActionIcon.value).toBeTruthy();
        expect(defaults.bindings.selectionSecondaryItemText.value).toBe(
            'translated:<img src=x onerror=alert(1)>',
        );

        const withoutTitles = setup({
            primaryTitleField: undefined,
            secondaryTitleField: undefined,
        });
        expect(withoutTitles.bindings.selectionPrimaryItemText.value).toBe('group-a');
        expect(withoutTitles.bindings.selectionSecondaryItemText.value).toBe('a1');

        const emptyTitles = setup({ modelValue: 'empty-hidden', primaryHiddenField: undefined, secondaryHiddenField: undefined });
        expect(emptyTitles.bindings.selectionPrimaryItemText.value).toBe('translated:No choice');
        expect(emptyTitles.bindings.selectionSecondaryItemText.value).toBe('translated:No choice');
    });
});

describe('desktop TwoColumnSelect selection, actions, focus, and menu positioning', () => {
    test('selects parent/child ids, closes the menu, and emits all public events', async () => {
        const { bindings, emit, props } = setup();
        bindings.menuState.value = true;
        bindings.currentSecondaryValue.value = 'b1';
        expect(bindings.menuState.value).toBe(false);
        expect(emit).toHaveBeenCalledWith('update:modelValue', 'b1');

        emit.mockClear();
        bindings.currentPrimaryValue.value = 'group-b';
        expect(emit).toHaveBeenCalledWith('update:modelValue', 'b1');

        emit.mockClear();
        bindings.currentPrimaryValue.value = 'missing-group';
        expect(emit).not.toHaveBeenCalled();

        bindings.currentPrimaryValue.value = 'group-empty';
        expect(emit).not.toHaveBeenCalled();

        bindings.onPrimaryItemClicked(props['items'][1]);
        expect(emit).toHaveBeenCalledWith('update:modelValue', 'b1');

        emit.mockClear();
        bindings.menuState.value = true;
        bindings.onSecondaryItemClicked((props['items'][0].children as any[])[2]);
        expect(emit).toHaveBeenCalledWith('update:modelValue', 'duplicate');
        expect(bindings.menuState.value).toBe(false);

        emit.mockClear();
        props['modelValue'] = 'a1';
        bindings.menuState.value = true;
        bindings.onPrimaryActionClicked();
        expect(bindings.menuState.value).toBe(false);
        expect(emit).toHaveBeenCalledWith('primary-action', props['items'][0]);

        bindings.menuState.value = true;
        bindings.onSecondaryActionClicked();
        expect(bindings.menuState.value).toBe(false);
        expect(emit).toHaveBeenCalledWith('secondary-action', props['items'][0]);
        expect(emit.mock.calls.every(call => typeof call[0] === 'string')).toBe(true);
    });

    test('does not emit a child value when the secondary value field is absent', () => {
        const { bindings, emit, props } = setup({ secondaryValueField: undefined });
        bindings.currentPrimaryValue.value = 'group-b';
        expect(emit).not.toHaveBeenCalled();
        bindings.onPrimaryItemClicked(props['items'][1]);
        expect(emit).not.toHaveBeenCalled();
    });

    test('scrolls on menu open and focuses only a valid focused input', async () => {
        const { bindings } = setup();
        const menuParent = { id: 'menu-parent' };
        bindings.dropdownMenu.value = { parentElement: menuParent };

        bindings.onMenuStateChanged(false);
        await flush();
        expect(mockScrollToSelectedItem).not.toHaveBeenCalled();

        bindings.onMenuStateChanged(true);
        await flush();
        expect(mockScrollToSelectedItem).toHaveBeenNthCalledWith(
            1, menuParent, '.primary-list-container', '.primary-list-item-selected',
        );
        expect(mockScrollToSelectedItem).toHaveBeenNthCalledWith(
            2, menuParent, '.secondary-list-container', '.secondary-list-item-selected',
        );

        bindings.dropdownMenu.value = null;
        bindings.onMenuStateChanged(true);
        await flush();
        expect(mockScrollToSelectedItem).toHaveBeenCalledTimes(2);

        bindings.onInputFocused(null, true);
        bindings.onInputFocused(undefined, true);
        bindings.onInputFocused({ $el: { id: 'input' } }, false);
        await flush();
        expect(mockSetChildInputFocus).not.toHaveBeenCalled();

        const input = { $el: { id: 'input' } };
        bindings.onInputFocused(input, true);
        await flush();
        expect(mockSetChildInputFocus).toHaveBeenCalledWith(input.$el, 'input');
    });

    test('updates menu top only when auto-positioning has complete geometry and room', async () => {
        const disabled = setup({ autoUpdateMenuPosition: false });
        disabled.bindings.updateMenuPosition();
        await flush();

        const enabled = setup({ autoUpdateMenuPosition: true });
        enabled.bindings.updateMenuPosition();
        await flush();

        const style: Record<string, string> = {};
        const selectMenu = {
            style,
            getBoundingClientRect: () => ({ height: 200 }),
        };
        enabled.bindings.twoColumnMainSelect.value = {
            $el: { getBoundingClientRect: () => ({ top: 100, height: 30 }) },
        };
        enabled.bindings.dropdownMenu.value = {
            parentElement: { parentElement: selectMenu },
        };
        const previousDocument = (globalThis as any).document;
        const documentElement = { scrollHeight: 1_000 };
        Object.defineProperty(globalThis, 'document', {
            configurable: true,
            writable: true,
            value: { documentElement },
        });
        try {
            enabled.bindings.updateMenuPosition();
            await flush();
            expect(style['top']).toBe('130px');

            style['top'] = '';
            documentElement.scrollHeight = 300;
            enabled.bindings.updateMenuPosition();
            await flush();
            expect(style['top']).toBe('');
        } finally {
            if (previousDocument === undefined) {
                delete (globalThis as any).document;
            } else {
                (globalThis as any).document = previousDocument;
            }
        }
    });
});

describe('desktop TwoColumnSelect production template', () => {
    test('renders full and minimal branches and executes generated event wrappers', async () => {
        const full = mountWithHostRenderer({
            disabled: true,
            readonly: true,
            modelValue: 'unsafe',
        });
        try {
            await flush();
            const callbacks = collectHostCallbacks(full.root);
            expect(callbacks.some(item => item.name === 'onUpdate:modelValue')).toBe(true);
            expect(callbacks.some(item => item.name === 'onUpdate:menu')).toBe(true);
            expect(callbacks.some(item => item.name === 'onUpdate:focused')).toBe(true);
            expect(callbacks.some(item => item.name === 'onClick')).toBe(true);
            await invokeHostCallbacks(callbacks);

            const types: string[] = [];
            const texts: string[] = [];
            const props: Array<Record<string, unknown>> = [];
            walkHostNodes(full.root, node => {
                types.push(node.type);
                if (node.text) texts.push(node.text);
                props.push(node.props ?? {});
            });
            expect(types).not.toContain('img');
            expect(texts).toContain('translated:<img src=x onerror=alert(1)>');
            expect(props.some(item => item['disabled'] === true)).toBe(true);
            expect(props.some(item => item['readonly'] === true)).toBe(true);
        } finally {
            full.app.unmount();
        }

        const custom = mountWithHostRenderer({
            customSelectionPrimaryText: 'Custom primary',
            customSelectionSecondaryText: 'Custom secondary',
            showSelectionPrimaryText: false,
            showSelectionSecondaryIcon: false,
            primaryKeyField: undefined,
            secondaryKeyField: undefined,
            primaryTitleField: undefined,
            secondaryTitleField: undefined,
            primaryHeaderField: undefined,
            primaryFooterField: undefined,
            secondaryHeaderField: undefined,
            secondaryFooterField: undefined,
            primaryIconField: undefined,
            secondaryIconField: undefined,
            primaryActionTitle: undefined,
            secondaryActionTitle: undefined,
            enableFilter: false,
        });
        try {
            await flush();
            expect(custom.root.children.length).toBeGreaterThan(0);
        } finally {
            custom.app.unmount();
        }

        const empty = mountWithHostRenderer({
            items: [],
            modelValue: '',
            customSelectionPrimaryText: '',
            customSelectionSecondaryText: '',
            noItemText: undefined,
        });
        try {
            await flush();
            expect(empty.state.noSelectionText).toBe('tt:None');
            expect(empty.state.filteredItems).toStrictEqual([]);
        } finally {
            empty.app.unmount();
        }
    });
});
