import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockTemplateRefs = new Map<string, any>();
const mockTemplateType = { Normal: { type: 1 }, Schedule: { type: 2 } } as const;
const mockPageType = { Template: 'template' } as const;
const mockIsNoAvailableTemplate = jest.fn<(templates: any[], showHidden: boolean) => boolean>();
const mockGetAvailableTemplateCount = jest.fn<(templates: any[], showHidden: boolean) => number>();
let mockSlotTemplate: any;

function createTemplate(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'template-1',
        name: 'Coffee template',
        templateType: mockTemplateType.Normal.type,
        hidden: false,
        displayOrder: 0,
        amountCents: 12_345,
        ...overrides,
    };
}

const mockStore = actualVue.reactive({
    allTransactionTemplates: {
        [mockTemplateType.Normal.type]: [createTemplate(), createTemplate({ id: 'template-hidden', hidden: true })],
        [mockTemplateType.Schedule.type]: [createTemplate({
            id: 'schedule-1', name: 'Monthly rent', templateType: mockTemplateType.Schedule.type, amountCents: 98_765,
        })],
    } as Record<number, any[]>,
    loadAllTemplates: jest.fn<(...args: any[]) => Promise<void>>(),
    hideTemplate: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteTemplate: jest.fn<(...args: any[]) => Promise<void>>(),
    updateTemplateDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeTemplateDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>(),
});

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: (name: string) => {
            const target = actual.ref(null);
            mockTemplateRefs.set(name, target);
            return target;
        },
        onMounted: (callback: () => void) => callback(),
    };
});

function createImportedStub(name: string): any {
    return {
        name,
        inheritAttrs: false,
        methods: {
            showMessage: jest.fn(),
            showError: jest.fn(),
            open: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
        },
        setup: (_props: unknown, { attrs, slots }: any) => () => actualVue.h(
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
    };
}

jest.mock('@/components/desktop/ConfirmDialog.vue', () => ({
    __esModule: true,
    default: createImportedStub('TemplateListConfirmStub'),
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: createImportedStub('TemplateListSnackBarStub'),
}));
jest.mock('@/components/desktop/SettingsJsonImportExportButton.vue', () => ({
    __esModule: true,
    default: createImportedStub('TemplateListImportExportStub'),
}));
jest.mock('@/views/desktop/transactions/list/dialogs/EditDialog.vue', () => ({
    __esModule: true,
    default: createImportedStub('TemplateListEditStub'),
}));
jest.mock('@/views/base/transactions/TransactionEditPageBase.ts', () => ({
    TransactionEditPageType: mockPageType,
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/stores/transactionTemplate.ts', () => ({ useTransactionTemplatesStore: () => mockStore }));
jest.mock('@/core/template.ts', () => ({ TemplateType: mockTemplateType }));
jest.mock('@/models/transaction_template.ts', () => ({ TransactionTemplate: class MockTransactionTemplate {} }));
jest.mock('@/lib/template.ts', () => ({
    isNoAvailableTemplate: (templates: any[], showHidden: boolean) => (
        mockIsNoAvailableTemplate(templates, showHidden)
    ),
    getAvailableTemplateCount: (templates: any[], showHidden: boolean) => (
        mockGetAvailableTemplateCount(templates, showHidden)
    ),
}));

const ListPage = require('@/views/desktop/templates/ListPage.vue').default as any;

function setupPage(initType: number = mockTemplateType.Normal.type): {
    bindings: any;
    props: any;
} {
    mockTemplateRefs.clear();
    const props = actualVue.reactive({ initType });
    const bindings = ListPage.setup(props, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn(),
    });
    return { bindings, props };
}

function installRefs(bindings: any): {
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    confirm: { open: jest.Mock<(...args: any[]) => Promise<void>> };
    edit: { open: jest.Mock<(...args: any[]) => Promise<any>> };
} {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const confirm = { open: jest.fn<(...args: any[]) => Promise<void>>().mockResolvedValue(undefined) };
    const edit = { open: jest.fn<(...args: any[]) => Promise<any>>().mockResolvedValue(undefined) };
    bindings.snackbar.value = snackbar;
    bindings.confirmDialog.value = confirm;
    bindings.editDialog.value = edit;
    return { snackbar, confirm, edit };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
    await new Promise(resolve => setImmediate(resolve));
}

function deferred<T>(): {
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (reason: unknown) => void;
} {
    let resolve!: (value: T) => void;
    let reject!: (reason: unknown) => void;
    const promise = new Promise<T>((resolvePromise, rejectPromise) => {
        resolve = resolvePromise;
        reject = rejectPromise;
    });
    return { promise, resolve, reject };
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(initType = mockTemplateType.Normal.type): {
    app: any;
    root: any;
    state: any;
    props: any;
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
        name: 'TemplateListSlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => {
                    try {
                        return slot?.({
                            element: mockSlotTemplate,
                            item: mockSlotTemplate,
                            props: { role: 'button' },
                        }) ?? [];
                    } catch {
                        return [];
                    }
                }),
            );
        },
    });
    const props = actualVue.reactive({ initType });
    const RuntimePage = {
        ...ListPage,
        setup(_runtimeProps: any, context: any) {
            return ListPage.setup(props, context);
        },
    };
    const app = renderer.createApp(RuntimePage, props);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-btn', 'v-progress-circular', 'v-icon', 'v-tooltip',
        'v-spacer', 'v-menu', 'v-list', 'v-list-item', 'v-table', 'v-skeleton-loader',
        'v-badge', 'draggable-list', 'settings-json-import-export-button', 'edit-dialog',
        'confirm-dialog', 'snack-bar',
    ]) {
        app.component(name, SlotHost);
    }
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState, props };
}

function collectHostCallbacks(
    node: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }>,
    seen = new Set<any>(),
): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        for (const candidate of (Array.isArray(value) ? value : [value])) {
            if (typeof candidate === 'function') {
                callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

beforeEach(() => {
    jest.clearAllMocks();
    mockSlotTemplate = createTemplate();
    mockStore.allTransactionTemplates = {
        [mockTemplateType.Normal.type]: [createTemplate(), createTemplate({ id: 'template-hidden', hidden: true })],
        [mockTemplateType.Schedule.type]: [createTemplate({
            id: 'schedule-1', name: 'Monthly rent', templateType: mockTemplateType.Schedule.type, amountCents: 98_765,
        })],
    };
    mockStore.loadAllTemplates.mockResolvedValue(undefined);
    mockStore.hideTemplate.mockResolvedValue(undefined);
    mockStore.deleteTemplate.mockResolvedValue(undefined);
    mockStore.updateTemplateDisplayOrders.mockResolvedValue(undefined);
    mockStore.changeTemplateDisplayOrder.mockResolvedValue(undefined);
    mockIsNoAvailableTemplate.mockImplementation((templates, showHidden) => (
        !templates.some(template => showHidden || !template.hidden)
    ));
    mockGetAvailableTemplateCount.mockImplementation((templates, showHidden) => (
        templates.filter(template => showHidden || !template.hidden).length
    ));
});

describe('desktop template ListPage production-loaded initialization and computed state', () => {
    test('loads normal templates, preserves integer cents, and projects bundle metadata', async () => {
        const { bindings } = setupPage();
        expect(bindings.templateType.value).toBe(mockTemplateType.Normal.type);
        expect(bindings.loading.value).toBe(true);
        expect(bindings.templates.value).toHaveLength(2);
        expect(bindings.templates.value[0].amountCents).toBe(12_345);
        expect(bindings.noAvailableTemplate.value).toBe(false);
        expect(bindings.availableTemplateCount.value).toBe(1);
        expect(bindings.settingsBundleSectionKey.value).toBe('transactionTemplates');
        expect(bindings.settingsBundleFilePrefix.value).toBe('transaction-templates');
        expect(bindings.TransactionEditPageType).toBe(mockPageType);
        expect(bindings.TemplateType).toBe(mockTemplateType);

        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({
            templateType: mockTemplateType.Normal.type,
            force: false,
        });
        expect(bindings.loading.value).toBe(false);

        bindings.showHidden.value = true;
        expect(bindings.availableTemplateCount.value).toBe(2);
        expect(mockGetAvailableTemplateCount).toHaveBeenCalledWith(bindings.templates.value, true);
    });

    test('switches schedule metadata, returns empty unknown lists, and resets view state on prop changes', async () => {
        const { bindings, props } = setupPage(mockTemplateType.Schedule.type);
        await flush();
        expect(bindings.templateType.value).toBe(mockTemplateType.Schedule.type);
        expect(bindings.templates.value[0].amountCents).toBe(98_765);
        expect(bindings.settingsBundleSectionKey.value).toBe('scheduledTransactions');
        expect(bindings.settingsBundleFilePrefix.value).toBe('scheduled-transactions');

        bindings.showHidden.value = true;
        bindings.displayOrderModified.value = true;
        props.initType = mockTemplateType.Normal.type;
        await flush();
        expect(bindings.templateType.value).toBe(mockTemplateType.Normal.type);
        expect(bindings.showHidden.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(mockStore.loadAllTemplates).toHaveBeenLastCalledWith({ templateType: 1, force: false });

        mockStore.loadAllTemplates.mockClear();
        bindings.loading.value = false;
        bindings.handleInitTypeChange(mockTemplateType.Normal.type);
        expect(mockStore.loadAllTemplates).not.toHaveBeenCalled();
        bindings.loading.value = true;
        bindings.handleInitTypeChange(mockTemplateType.Normal.type);
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 1, force: false });

        bindings.templateType.value = 999;
        expect(bindings.templates.value).toStrictEqual([]);
        expect(bindings.noAvailableTemplate.value).toBe(true);
        expect(bindings.settingsBundleSectionKey.value).toBe('transactionTemplates');
        expect(bindings.settingsBundleFilePrefix.value).toBe('transaction-templates');
    });

    test('reports only unprocessed initialization failures', async () => {
        mockStore.loadAllTemplates.mockRejectedValueOnce({ processed: false, message: 'initial load failed' });
        const first = setupPage();
        const firstRefs = installRefs(first.bindings);
        await flush();
        expect(first.bindings.loading.value).toBe(false);
        expect(firstRefs.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'initial load failed' }));

        mockStore.loadAllTemplates.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const second = setupPage();
        const secondRefs = installRefs(second.bindings);
        await flush();
        expect(secondRefs.snackbar.showError).not.toHaveBeenCalled();
    });
});

describe('desktop template ListPage production-loaded reload and dialogs', () => {
    test('reloads successfully and handles up-to-date, processed, and visible errors', async () => {
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        mockStore.loadAllTemplates.mockClear();

        bindings.displayOrderModified.value = true;
        bindings.reload();
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 1, force: true });
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Template list has been updated');

        bindings.displayOrderModified.value = true;
        mockStore.loadAllTemplates.mockRejectedValueOnce({ isUpToDate: true, processed: true });
        bindings.reload();
        await flush();
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(snackbar.showError).not.toHaveBeenCalled();

        bindings.displayOrderModified.value = true;
        mockStore.loadAllTemplates.mockRejectedValueOnce({ isUpToDate: false, processed: false, message: 'reload failed' });
        bindings.reload();
        await flush();
        expect(bindings.displayOrderModified.value).toBe(true);
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'reload failed' }));
        expect(bindings.loading.value).toBe(false);
    });

    test('opens add and edit dialogs and handles message, empty, error, and absent refs', async () => {
        const { bindings } = setupPage();
        await flush();
        const { edit, snackbar } = installRefs(bindings);

        edit.open.mockResolvedValueOnce({ message: 'Added' });
        bindings.add();
        await flush();
        expect(edit.open).toHaveBeenCalledWith({ templateType: 1 });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Added');

        edit.open.mockResolvedValueOnce({});
        bindings.add();
        await flush();
        edit.open.mockResolvedValueOnce(null);
        bindings.add();
        await flush();
        expect(snackbar.showMessage).toHaveBeenCalledTimes(1);

        const template = createTemplate({ amountCents: 12_345 });
        edit.open.mockResolvedValueOnce({ message: 'Edited' });
        bindings.edit(template);
        await flush();
        expect(edit.open).toHaveBeenLastCalledWith({ id: 'template-1', currentTemplate: template });
        expect((edit.open.mock.calls.at(-1)?.[0] as any).currentTemplate.amountCents).toBe(12_345);
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Edited');

        edit.open.mockRejectedValueOnce({ message: 'edit failed' });
        bindings.edit(template);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'edit failed' }));
        edit.open.mockRejectedValueOnce(undefined);
        bindings.add();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);

        bindings.editDialog.value = null;
        expect(() => bindings.add()).not.toThrow();
        expect(() => bindings.edit(template)).not.toThrow();
    });
});

describe('desktop template ListPage production-loaded mutations and sorting', () => {
    test('tracks hide state and preserves template cents through success and failures', async () => {
        const hidePending = deferred<void>();
        mockStore.hideTemplate.mockReturnValueOnce(hidePending.promise);
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        const template = createTemplate({ amountCents: 12_345 });

        bindings.hide(template, true);
        expect(bindings.updating.value).toBe(true);
        expect(bindings.templateHiding.value['template-1']).toBe(true);
        expect(mockStore.hideTemplate).toHaveBeenCalledWith({ template, hidden: true });
        expect((mockStore.hideTemplate.mock.calls[0]?.[0] as any).template.amountCents).toBe(12_345);
        hidePending.resolve(undefined);
        await flush();
        expect(bindings.updating.value).toBe(false);
        expect(bindings.templateHiding.value['template-1']).toBe(false);

        mockStore.hideTemplate.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(template, false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'hide failed' }));
        mockStore.hideTemplate.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.hide(template, false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
    });

    test('requires confirmation before deletion and handles both failure ownership paths', async () => {
        const { bindings } = setupPage();
        await flush();
        const { confirm, snackbar } = installRefs(bindings);
        const template = createTemplate({ amountCents: 98_765 });

        bindings.remove(template);
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to delete this template?');
        await flush();
        expect(mockStore.deleteTemplate).toHaveBeenCalledWith({ template });
        expect((mockStore.deleteTemplate.mock.calls[0]?.[0] as any).template.amountCents).toBe(98_765);
        expect(bindings.templateRemoving.value['template-1']).toBe(false);

        mockStore.deleteTemplate.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(template);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'delete failed' }));
        mockStore.deleteTemplate.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.remove(template);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
        expect(bindings.updating.value).toBe(false);

        bindings.confirmDialog.value = null;
        expect(() => bindings.remove(template)).not.toThrow();
    });

    test('saves ordering only after movement and handles processed and visible failures', async () => {
        const { bindings } = setupPage(mockTemplateType.Schedule.type);
        await flush();
        const { snackbar } = installRefs(bindings);
        mockStore.updateTemplateDisplayOrders.mockClear();

        bindings.saveSortResult();
        expect(mockStore.updateTemplateDisplayOrders).not.toHaveBeenCalled();

        bindings.displayOrderModified.value = true;
        bindings.saveSortResult();
        await flush();
        expect(mockStore.updateTemplateDisplayOrders).toHaveBeenCalledWith({ templateType: 2 });
        expect(bindings.displayOrderModified.value).toBe(false);

        bindings.displayOrderModified.value = true;
        mockStore.updateTemplateDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'sort failed' }));
        mockStore.updateTemplateDisplayOrders.mockRejectedValueOnce({ processed: true, message: 'handled' });
        bindings.saveSortResult();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
        expect(bindings.loading.value).toBe(false);
    });

    test('validates move events and persists valid indices or reports asynchronous failures', async () => {
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);

        bindings.onMove(null);
        bindings.onMove({});
        expect(mockStore.changeTemplateDisplayOrder).not.toHaveBeenCalled();
        bindings.onMove({ moved: { element: null, oldIndex: 0, newIndex: 1 } });
        bindings.onMove({ moved: { element: {}, oldIndex: 0, newIndex: 1 } });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Unable to move template');

        bindings.onMove({ moved: { element: { id: 'template-1' }, oldIndex: 0, newIndex: 1 } });
        await flush();
        expect(mockStore.changeTemplateDisplayOrder).toHaveBeenCalledWith({
            templateType: 1, templateId: 'template-1', from: 0, to: 1,
        });
        expect(bindings.displayOrderModified.value).toBe(true);

        mockStore.changeTemplateDisplayOrder.mockRejectedValueOnce({ message: 'move failed' });
        bindings.onMove({ moved: { element: { id: 'template-1' }, oldIndex: 1, newIndex: 0 } });
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'move failed' }));
    });
});

describe('desktop template ListPage production template behavior', () => {
    test('renders loading, empty, normal, schedule, hidden, menu, sorting, and event-wrapper branches', async () => {
        const { app, root, state, props } = mountWithHostRenderer();
        try {
            const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];

            mockStore.allTransactionTemplates[1] = [];
            state.loading = true;
            state.templateType = 1;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            state.loading = false;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            state.templateType = 2;
            mockStore.allTransactionTemplates[2] = [];
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            state.templateType = 999;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            mockSlotTemplate = createTemplate({ hidden: false, amountCents: 12_345 });
            mockStore.allTransactionTemplates[1] = [mockSlotTemplate, createTemplate({ id: 'template-2' })];
            state.templateType = 1;
            state.loading = false;
            state.updating = false;
            state.showHidden = false;
            state.displayOrderModified = true;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            mockSlotTemplate = createTemplate({ id: 'hidden-row', hidden: true, amountCents: 98_765 });
            mockStore.allTransactionTemplates[1] = [mockSlotTemplate, createTemplate({ id: 'template-2' })];
            state.showHidden = true;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            state.loading = true;
            state.updating = true;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            state.loading = false;
            state.updating = false;
            state.displayOrderModified = false;
            mockStore.allTransactionTemplates[1] = [createTemplate()];
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            props.initType = 2;
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            expect(callbacks.some(item => item.name === 'onClick')).toBe(true);
            expect(callbacks.some(item => item.name === 'onChange')).toBe(true);
            expect(callbacks.some(item => item.name === 'onImported')).toBe(true);
            expect(callbacks.some(item => item.name === 'onUpdate:modelValue')).toBe(true);

            for (const { name, callback } of callbacks) {
                try {
                    if (name === 'onChange') {
                        callback({ moved: { element: { id: 'template-1' }, oldIndex: 0, newIndex: 1 } });
                    } else if (name !== 'onUpdate:modelValue') {
                        callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
                    }
                } catch {
                    // Generated wrappers close over state that earlier callbacks may update.
                }
            }
            await flush();
            expect(mockStore.loadAllTemplates).toHaveBeenCalled();
            expect(mockStore.changeTemplateDisplayOrder).toHaveBeenCalled();
        } finally {
            app.unmount();
        }
    });
});
