import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;

const mockTemplateType = {
    Normal: { type: 1 },
    Schedule: { type: 2 },
} as const;
const mockTextDirection = { LTR: 1, RTL: 2 } as const;

const mockShowAlert = jest.fn<(...args: any[]) => void>();
const mockShowToast = jest.fn<(...args: any[]) => void>();
const mockRouteBackOnError = jest.fn<(...args: any[]) => void>();
const mockShowLoading = jest.fn<(...args: any[]) => void>();
const mockHideLoading = jest.fn<(...args: any[]) => void>();
const mockOnSwipeoutDeleted = jest.fn<(...args: any[]) => void>();

let mockCurrentTextDirection: number = mockTextDirection.LTR;

function createTemplate(overrides: Record<string, unknown> = {}): any {
    return {
        id: 'coffee',
        name: 'Coffee template',
        templateType: mockTemplateType.Normal.type,
        hidden: false,
        displayOrder: 0,
        amountCents: 12_345,
        ...overrides,
    };
}

const mockVisibleTemplate = createTemplate();
const mockHiddenTemplate = createTemplate({
    id: 'archived',
    name: 'Archived template',
    hidden: true,
    displayOrder: 1,
    amountCents: 98_765,
});
const mockScheduledTemplate = createTemplate({
    id: 'rent',
    name: 'Monthly rent',
    templateType: mockTemplateType.Schedule.type,
    amountCents: 98_765,
});

const mockStore = actualVue.reactive({
    allTransactionTemplates: {
        [mockTemplateType.Normal.type]: [mockVisibleTemplate, mockHiddenTemplate],
        [mockTemplateType.Schedule.type]: [mockScheduledTemplate],
    } as Record<number, any[]>,
    transactionTemplateListStatesInvalid: {
        [mockTemplateType.Normal.type]: false,
        [mockTemplateType.Schedule.type]: false,
    } as Record<number, boolean | undefined>,
    loadAllTemplates: jest.fn<(...args: any[]) => Promise<void>>(),
    hideTemplate: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteTemplate: jest.fn<(...args: any[]) => Promise<void>>(),
    updateTemplateDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeTemplateDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>(),
});

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockCurrentTextDirection,
    }),
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError,
    }),
    showLoading: mockShowLoading,
    hideLoading: mockHideLoading,
    onSwipeoutDeleted: (...args: any[]) => mockOnSwipeoutDeleted(...args),
}));
jest.mock('@/stores/transactionTemplate.ts', () => ({
    useTransactionTemplatesStore: () => mockStore,
}));
jest.mock('@/core/text.ts', () => ({ TextDirection: mockTextDirection }));
jest.mock('@/core/template.ts', () => ({ TemplateType: mockTemplateType }));
jest.mock('@/models/transaction_template.ts', () => ({
    TransactionTemplate: class MockTransactionTemplate {},
}));
jest.mock('@/lib/common.ts', () => ({
    isDefined: (value: unknown) => value !== undefined && value !== null,
}));
jest.mock('@/lib/template.ts', () => ({
    isNoAvailableTemplate: (templates: any[], showHidden: boolean) => (
        !templates.some(template => showHidden || !template.hidden)
    ),
    getFirstShowingId: (templates: any[], showHidden: boolean) => (
        templates.find(template => showHidden || !template.hidden)?.id ?? null
    ),
    getLastShowingId: (templates: any[], showHidden: boolean) => (
        [...templates].reverse().find(template => showHidden || !template.hidden)?.id ?? null
    ),
}));

const ListPage = require('@/views/mobile/templates/ListPage.vue').default as any;

function setup(path = '/template/list'): { bindings: any; router: any } {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const bindings = ListPage.setup(
        { f7route: { path }, f7router: router },
        { attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn() },
    );
    return { bindings, router };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await actualVue.nextTick();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(path = '/template/list'): {
    app: any;
    root: any;
    router: any;
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
        name: 'MobileTemplateSlotHost',
        inheritAttrs: false,
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => slot?.() ?? []),
            );
        },
    });
    const router = { back: jest.fn(), navigate: jest.fn() };
    const app = renderer.createApp(ListPage, { f7route: { path }, f7router: router });
    app.config.warnHandler = () => undefined;
    for (const name of [
        'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
        'f7-list', 'f7-list-item', 'f7-icon', 'f7-badge', 'f7-swipeout-actions',
        'f7-swipeout-button', 'f7-actions', 'f7-actions-group', 'f7-actions-button',
        'f7-actions-label',
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, router, state: vm.$.setupState };
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
        for (const candidate of Array.isArray(value) ? value : [value]) {
            if (typeof candidate === 'function') {
                callbacks.push({ name, callback: candidate as (...args: any[]) => unknown });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

async function invokeHostCallbacks(
    callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }>,
): Promise<void> {
    for (const { name, callback } of callbacks) {
        try {
            if (name === 'onSortable:sort') {
                callback({ el: { id: 'template_coffee' }, from: 0, to: 1 });
            } else if (name === 'onPtr:refresh') {
                callback(jest.fn());
            } else {
                callback();
            }
        } catch {
            // Generated Framework7 wrappers accept heterogeneous payloads and state.
        }
        await flush(2);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCurrentTextDirection = mockTextDirection.LTR;
    mockStore.allTransactionTemplates = {
        [mockTemplateType.Normal.type]: [mockVisibleTemplate, mockHiddenTemplate],
        [mockTemplateType.Schedule.type]: [mockScheduledTemplate],
    };
    mockStore.transactionTemplateListStatesInvalid = {
        [mockTemplateType.Normal.type]: false,
        [mockTemplateType.Schedule.type]: false,
    };
    mockStore.loadAllTemplates.mockResolvedValue(undefined);
    mockStore.hideTemplate.mockResolvedValue(undefined);
    mockStore.deleteTemplate.mockResolvedValue(undefined);
    mockStore.updateTemplateDisplayOrders.mockResolvedValue(undefined);
    mockStore.changeTemplateDisplayOrder.mockResolvedValue(undefined);
});

describe('mobile template ListPage state, loading, and navigation', () => {
    test('initializes normal and schedule routes and projects visibility without converting cents', async () => {
        const normal = setup();
        expect(normal.bindings.loading.value).toBe(true);
        expect(normal.bindings.templateType.value).toBe(mockTemplateType.Normal.type);
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 1, force: false });
        expect(normal.bindings.loading.value).toBe(false);
        expect(normal.bindings.templates.value).toStrictEqual([mockVisibleTemplate, mockHiddenTemplate]);
        expect(normal.bindings.templates.value[0].amountCents).toBe(12_345);
        expect(normal.bindings.textDirection.value).toBe(mockTextDirection.LTR);
        expect(normal.bindings.firstShowingId.value).toBe('coffee');
        expect(normal.bindings.lastShowingId.value).toBe('coffee');
        expect(normal.bindings.noAvailableTemplate.value).toBe(false);
        expect(normal.bindings.getTemplateDomId(mockVisibleTemplate)).toBe('template_coffee');
        expect(normal.bindings.parseTemplateIdFromDomId('template_coffee')).toBe('coffee');
        expect(normal.bindings.parseTemplateIdFromDomId('')).toBeNull();
        expect(normal.bindings.parseTemplateIdFromDomId('category_coffee')).toBeNull();
        expect(normal.bindings.parseTemplateIdFromDomId('template_')).toBe('');

        normal.bindings.showHidden.value = true;
        expect(normal.bindings.lastShowingId.value).toBe('archived');

        const schedule = setup('/schedule/list');
        await flush();
        expect(schedule.bindings.templateType.value).toBe(mockTemplateType.Schedule.type);
        expect(schedule.bindings.templates.value[0].amountCents).toBe(98_765);
        expect(mockStore.loadAllTemplates).toHaveBeenLastCalledWith({ templateType: 2, force: false });

        const fallback = setup('/unknown/list');
        expect(fallback.bindings.templateType.value).toBe(mockTemplateType.Normal.type);
        await flush();
    });

    test('returns empty or hidden-only computed state and supports RTL direction', () => {
        mockCurrentTextDirection = mockTextDirection.RTL;
        mockStore.allTransactionTemplates = {
            [mockTemplateType.Normal.type]: [mockHiddenTemplate],
        };
        const hiddenOnly = setup();
        expect(hiddenOnly.bindings.textDirection.value).toBe(mockTextDirection.RTL);
        expect(hiddenOnly.bindings.noAvailableTemplate.value).toBe(true);
        expect(hiddenOnly.bindings.firstShowingId.value).toBeNull();
        expect(hiddenOnly.bindings.lastShowingId.value).toBeNull();
        hiddenOnly.bindings.showHidden.value = true;
        expect(hiddenOnly.bindings.firstShowingId.value).toBe('archived');
        expect(hiddenOnly.bindings.lastShowingId.value).toBe('archived');

        hiddenOnly.bindings.templateType.value = 999;
        expect(hiddenOnly.bindings.templates.value).toStrictEqual([]);
        expect(hiddenOnly.bindings.noAvailableTemplate.value).toBe(true);
    });

    test('handles processed, readable, and raw initialization failures', async () => {
        mockStore.loadAllTemplates.mockRejectedValueOnce({ processed: true, message: 'handled init' });
        const handled = setup();
        await flush();
        expect(handled.bindings.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalledWith('handled init');

        mockStore.loadAllTemplates.mockRejectedValueOnce({ processed: false, message: 'init failed' });
        const readable = setup();
        await flush();
        expect(readable.bindings.loadingError.value).toMatchObject({ message: 'init failed' });
        expect(mockShowToast).toHaveBeenCalledWith('init failed');

        mockStore.loadAllTemplates.mockRejectedValueOnce('raw init failure');
        const raw = setup();
        await flush();
        expect(raw.bindings.loadingError.value).toBe('raw init failure');
        expect(mockShowToast).toHaveBeenCalledWith('raw init failure');
    });

    test('reloads normally or by pull-to-refresh and blocks while sorting', async () => {
        const { bindings } = setup();
        await flush();
        mockStore.loadAllTemplates.mockClear();

        bindings.reload();
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 1, force: false });

        const done = jest.fn();
        bindings.reload(done);
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenLastCalledWith({ templateType: 1, force: true });
        expect(done).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Template list has been updated');

        bindings.sortable.value = true;
        mockStore.loadAllTemplates.mockClear();
        const blocked = jest.fn();
        bindings.reload(blocked);
        expect(blocked).toHaveBeenCalled();
        expect(mockStore.loadAllTemplates).not.toHaveBeenCalled();
        bindings.sortable.value = false;

        mockStore.loadAllTemplates.mockRejectedValueOnce({ processed: true, message: 'handled reload' });
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled reload');

        mockStore.loadAllTemplates.mockRejectedValueOnce({ processed: false, message: 'reload failed' });
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('reload failed');

        mockStore.loadAllTemplates.mockRejectedValueOnce('raw reload failure');
        bindings.reload();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw reload failure');
    });

    test('navigates to editing and refreshes stale or undefined list state after page entry', async () => {
        const { bindings, router } = setup('/schedule/list');
        await flush();
        bindings.edit(mockScheduledTemplate);
        expect(router.navigate).toHaveBeenCalledWith('/template/edit?id=rent&templateType=2');
        expect(mockScheduledTemplate.amountCents).toBe(98_765);

        mockStore.loadAllTemplates.mockClear();
        mockStore.transactionTemplateListStatesInvalid[2] = true;
        bindings.loading.value = true;
        bindings.onPageAfterIn();
        expect(mockStore.loadAllTemplates).not.toHaveBeenCalled();

        bindings.loading.value = false;
        bindings.onPageAfterIn();
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 2, force: false });
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);

        mockStore.loadAllTemplates.mockClear();
        mockStore.transactionTemplateListStatesInvalid[2] = false;
        bindings.onPageAfterIn();
        expect(mockStore.loadAllTemplates).not.toHaveBeenCalled();

        mockStore.transactionTemplateListStatesInvalid[2] = undefined;
        bindings.onPageAfterIn();
        await flush();
        expect(mockStore.loadAllTemplates).toHaveBeenCalledWith({ templateType: 2, force: false });
    });
});

describe('mobile template ListPage hide, delete, and sorting', () => {
    test('hides or restores templates and preserves integer cents through every outcome', async () => {
        const { bindings } = setup();
        bindings.hide(mockVisibleTemplate, true);
        expect(mockShowLoading).toHaveBeenCalled();
        expect(mockStore.hideTemplate).toHaveBeenCalledWith({
            template: mockVisibleTemplate,
            hidden: true,
        });
        expect((mockStore.hideTemplate.mock.calls[0]?.[0] as any).template.amountCents).toBe(12_345);
        await flush();
        expect(mockHideLoading).toHaveBeenCalled();

        mockStore.hideTemplate.mockRejectedValueOnce({ processed: true, message: 'handled hide' });
        bindings.hide(mockVisibleTemplate, false);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled hide');

        mockStore.hideTemplate.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(mockVisibleTemplate, false);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('hide failed');

        mockStore.hideTemplate.mockRejectedValueOnce('raw hide failure');
        bindings.hide(mockVisibleTemplate, false);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw hide failure');
    });

    test('prompts, deletes with swipeout completion, and reports every failure shape', async () => {
        const { bindings } = setup();
        bindings.remove(null, false);
        expect(mockShowAlert).toHaveBeenCalledWith('An error occurred');

        bindings.remove(mockHiddenTemplate, false);
        expect(bindings.templateToDelete.value).toStrictEqual(mockHiddenTemplate);
        expect(bindings.showDeleteActionSheet.value).toBe(true);

        mockStore.deleteTemplate.mockImplementationOnce(async ({ beforeResolve }: any) => {
            beforeResolve('done-callback');
        });
        bindings.remove(mockHiddenTemplate, true);
        await flush();
        expect(mockOnSwipeoutDeleted).toHaveBeenCalledWith('template_archived', 'done-callback');
        expect((mockStore.deleteTemplate.mock.calls[0]?.[0] as any).template.amountCents).toBe(98_765);
        expect(bindings.templateToDelete.value).toBeNull();
        expect(bindings.showDeleteActionSheet.value).toBe(false);
        expect(mockHideLoading).toHaveBeenCalled();

        mockStore.deleteTemplate.mockRejectedValueOnce({ processed: true, message: 'handled delete' });
        bindings.remove(mockVisibleTemplate, true);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled delete');

        mockStore.deleteTemplate.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(mockVisibleTemplate, true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('delete failed');

        mockStore.deleteTemplate.mockRejectedValueOnce('raw delete failure');
        bindings.remove(mockVisibleTemplate, true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw delete failure');
    });

    test('enters sorting and saves changed or unchanged order across all outcomes', async () => {
        const { bindings } = setup('/schedule/list');
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);
        expect(bindings.showHidden.value).toBe(true);
        expect(bindings.displayOrderModified.value).toBe(false);
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);

        bindings.saveSortResult();
        expect(bindings.sortable.value).toBe(false);
        expect(bindings.showHidden.value).toBe(false);
        expect(mockStore.updateTemplateDisplayOrders).not.toHaveBeenCalled();

        bindings.setSortable();
        bindings.displayOrderModified.value = true;
        bindings.saveSortResult();
        expect(bindings.displayOrderSaving.value).toBe(true);
        await flush();
        expect(mockStore.updateTemplateDisplayOrders).toHaveBeenCalledWith({ templateType: 2 });
        expect(bindings.displayOrderSaving.value).toBe(false);
        expect(bindings.sortable.value).toBe(false);
        expect(bindings.showHidden.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);

        bindings.setSortable();
        bindings.displayOrderModified.value = true;
        mockStore.updateTemplateDisplayOrders.mockRejectedValueOnce({ processed: true, message: 'handled sort' });
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled sort');
        expect(bindings.displayOrderSaving.value).toBe(false);

        mockStore.updateTemplateDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('sort failed');

        mockStore.updateTemplateDisplayOrders.mockRejectedValueOnce('raw sort failure');
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw sort failure');
    });

    test('validates sortable DOM ids and records successful display-order changes', async () => {
        const { bindings } = setup();
        bindings.onSort(null);
        bindings.onSort({});
        bindings.onSort({ el: {} });
        bindings.onSort({ el: { id: 'category_coffee' }, from: 1, to: 2 });
        bindings.onSort({ el: { id: 'template_' }, from: 1, to: 2 });
        expect(mockShowToast).toHaveBeenCalledTimes(5);

        bindings.onSort({ el: { id: 'template_coffee' }, from: 2, to: 4 });
        await flush();
        expect(mockStore.changeTemplateDisplayOrder).toHaveBeenCalledWith({
            templateType: 1,
            templateId: 'coffee',
            from: 2,
            to: 4,
        });
        expect(bindings.displayOrderModified.value).toBe(true);

        mockStore.changeTemplateDisplayOrder.mockRejectedValueOnce(new Error('move failed'));
        bindings.onSort({ el: { id: 'template_coffee' }, from: 1, to: 2 });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('move failed');

        mockStore.changeTemplateDisplayOrder.mockRejectedValueOnce('raw move failure');
        bindings.onSort({ el: { id: 'template_coffee' }, from: 1, to: 2 });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw move failure');
    });
});

describe('mobile template ListPage production template', () => {
    test('renders loading, empty, schedule, sheet, swipeout, and sorting wrappers', async () => {
        const mounted = mountWithHostRenderer();
        try {
            await flush();
            await actualVue.nextTick();

            mounted.state.templateToDelete = mockHiddenTemplate;
            mounted.state.showDeleteActionSheet = true;
            mounted.state.showMoreActionSheet = true;
            const normalCallbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            collectHostCallbacks(mounted.root, normalCallbacks);
            await invokeHostCallbacks(normalCallbacks);
            expect(normalCallbacks.length).toBeGreaterThan(8);

            mounted.state.sortable = true;
            mounted.state.showHidden = true;
            mounted.state.displayOrderModified = true;
            mounted.state.displayOrderSaving = true;
            await actualVue.nextTick();
            const sortableCallbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            collectHostCallbacks(mounted.root, sortableCallbacks);
            await invokeHostCallbacks(sortableCallbacks);
            expect(sortableCallbacks.length).toBeGreaterThan(5);

            mounted.state.loading = true;
            await actualVue.nextTick();
            mounted.state.loading = false;
            mounted.state.sortable = false;
            mounted.state.showHidden = false;
            mockStore.allTransactionTemplates = { [mockTemplateType.Normal.type]: [] };
            await actualVue.nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);

            mounted.state.templateType = 999;
            await actualVue.nextTick();
        } finally {
            mounted.app.unmount();
        }

        mockCurrentTextDirection = mockTextDirection.RTL;
        const schedule = mountWithHostRenderer('/schedule/list');
        try {
            await flush();
            schedule.state.showHidden = true;
            schedule.state.sortable = true;
            await actualVue.nextTick();
            const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            collectHostCallbacks(schedule.root, callbacks);
            await invokeHostCallbacks(callbacks);
            expect(schedule.state.templateType).toBe(mockTemplateType.Schedule.type);
            expect(schedule.state.textDirection).toBe(mockTextDirection.RTL);
            expect(mockScheduledTemplate.amountCents).toBe(98_765);
        } finally {
            schedule.app.unmount();
        }
    });
});
