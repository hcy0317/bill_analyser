import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockShowAlert = jest.fn();
const mockShowToast = jest.fn();
const mockRouteBackOnError = jest.fn();
const mockShowLoading = jest.fn();
const mockHideLoading = jest.fn();
const mockOnSwipeoutDeleted = jest.fn();
const mockTemplateHandlers: Array<{ name: string; handler: (...args: any[]) => unknown }> = [];

const mockTextDirection = { LTR: 1, RTL: 2 };

class MockTransactionTag {
    id = '';
    name = '';
    hidden = false;

    constructor(overrides: Record<string, unknown> = {}) {
        Object.assign(this, overrides);
    }

    static createNewTag(): MockTransactionTag {
        return new MockTransactionTag();
    }
}

const visibleTag = new MockTransactionTag({ id: 'food', name: 'Food', hidden: false });
const hiddenTag = new MockTransactionTag({ id: 'old', name: 'Old', hidden: true });

const mockStore = {
    allTransactionTags: [visibleTag, hiddenTag] as MockTransactionTag[],
    transactionTagListStateInvalid: false,
    loadAllTags: jest.fn<(...args: any[]) => Promise<void>>(),
    saveTag: jest.fn<(...args: any[]) => Promise<void>>(),
    hideTag: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteTag: jest.fn<(...args: any[]) => Promise<void>>(),
    updateTagDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeTagDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>()
};

jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCurrentLanguageTextDirection: () => mockTextDirection.LTR
    })
}));
jest.mock('@/lib/ui/mobile.ts', () => ({
    useI18nUIComponents: () => ({
        showAlert: mockShowAlert,
        showToast: mockShowToast,
        routeBackOnError: mockRouteBackOnError
    }),
    showLoading: mockShowLoading,
    hideLoading: mockHideLoading,
    onSwipeoutDeleted: (...args: any[]) => mockOnSwipeoutDeleted(...args)
}));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockStore }));
jest.mock('@/core/text.ts', () => ({ TextDirection: mockTextDirection }));
jest.mock('@/models/transaction_tag.ts', () => ({ TransactionTag: MockTransactionTag }));
jest.mock('@/lib/tag.ts', () => ({
    isNoAvailableTag: (tags: MockTransactionTag[], showHidden: boolean) => !tags.some(tag => showHidden || !tag.hidden),
    getFirstShowingId: (tags: MockTransactionTag[], showHidden: boolean) => tags.find(tag => showHidden || !tag.hidden)?.id ?? null,
    getLastShowingId: (tags: MockTransactionTag[], showHidden: boolean) => [...tags].reverse().find(tag => showHidden || !tag.hidden)?.id ?? null
}));

const TagListPage = require('@/views/mobile/tags/ListPage.vue').default as any;

function setup(): { bindings: any; router: any } {
    const router = { back: jest.fn(), navigate: jest.fn() };
    const bindings = TagListPage.setup({ f7router: router }, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: () => undefined
    });
    return { bindings, router };
}

async function flush(times = 6): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function visitVNode(node: any, handlers: Array<{ name: string; handler: (...args: any[]) => unknown }>): void {
    if (!node) return;
    if (Array.isArray(node)) {
        for (const child of node) visitVNode(child, handlers);
        return;
    }
    if (typeof node !== 'object') return;
    for (const [name, handler] of Object.entries(node.props || {})) {
        if (!name.startsWith('on')) continue;
        if (typeof handler === 'function') handlers.push({ name, handler: handler as (...args: any[]) => unknown });
        if (Array.isArray(handler)) {
            for (const candidate of handler) {
                if (typeof candidate === 'function') handlers.push({ name, handler: candidate });
            }
        }
    }
    if (Array.isArray(node.children)) {
        visitVNode(node.children, handlers);
    } else if (node.children && typeof node.children === 'object') {
        for (const child of Object.values(node.children)) {
            if (typeof child === 'function') {
                try {
                    visitVNode((child as () => unknown)(), handlers);
                } catch {
                    // Framework7 slots may require runtime-owned arguments.
                }
            } else {
                visitVNode(child, handlers);
            }
        }
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockStore.allTransactionTags = [visibleTag, hiddenTag];
    mockStore.transactionTagListStateInvalid = false;
    mockStore.loadAllTags.mockResolvedValue(undefined);
    mockStore.saveTag.mockResolvedValue(undefined);
    mockStore.hideTag.mockResolvedValue(undefined);
    mockStore.deleteTag.mockResolvedValue(undefined);
    mockStore.updateTagDisplayOrders.mockResolvedValue(undefined);
    mockStore.changeTagDisplayOrder.mockResolvedValue(undefined);
    mockTemplateHandlers.length = 0;
});

describe('mobile tag ListPage lifecycle and editing', () => {
    test('initializes visibility state, identity helpers, and modification checks', async () => {
        const { bindings } = setup();
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(mockStore.loadAllTags).toHaveBeenCalledWith({ force: false });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.textDirection.value).toBe(mockTextDirection.LTR);
        expect(bindings.tags.value).toStrictEqual([visibleTag, hiddenTag]);
        expect(bindings.firstShowingId.value).toBe('food');
        expect(bindings.lastShowingId.value).toBe('food');
        expect(bindings.noAvailableTag.value).toBe(false);
        expect(bindings.hasEditingTag.value).toBe(false);
        expect(bindings.getTagDomId(visibleTag)).toBe('tag_food');
        expect(bindings.parseTagIdFromDomId('tag_food')).toBe('food');
        expect(bindings.parseTagIdFromDomId('')).toBeNull();
        expect(bindings.parseTagIdFromDomId('account_food')).toBeNull();
        expect(bindings.isTagModified(new MockTransactionTag())).toBe(false);
        expect(bindings.isTagModified(new MockTransactionTag({ name: 'New' }))).toBe(true);
        bindings.editingTag.value = new MockTransactionTag({ id: 'food', name: 'Food' });
        expect(bindings.isTagModified(visibleTag)).toBe(false);
        bindings.editingTag.value.name = 'Meals';
        expect(bindings.isTagModified(visibleTag)).toBe(true);
        bindings.editingTag.value.name = '';
        expect(bindings.isTagModified(visibleTag)).toBe(false);
        bindings.showHidden.value = true;
        expect(bindings.firstShowingId.value).toBe('food');
        expect(bindings.lastShowingId.value).toBe('old');
        mockStore.allTransactionTags = [hiddenTag];
        const hiddenOnly = setup();
        expect(hiddenOnly.bindings.noAvailableTag.value).toBe(true);
    });

    test('handles processed, readable, and raw initialization failures', async () => {
        mockStore.loadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const handled = setup();
        await flush();
        expect(handled.bindings.loading.value).toBe(false);
        expect(mockShowToast).not.toHaveBeenCalledWith('handled');

        mockStore.loadAllTags.mockRejectedValueOnce({ processed: false, message: 'load failed' });
        const readable = setup();
        await flush();
        expect(readable.bindings.loadingError.value).toMatchObject({ message: 'load failed' });
        expect(mockShowToast).toHaveBeenCalledWith('load failed');

        mockStore.loadAllTags.mockRejectedValueOnce('raw failure');
        setup();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw failure');
    });

    test('reloads normally or by pull-to-refresh and blocks while editing or sorting', async () => {
        const { bindings } = setup();
        await flush();
        mockStore.loadAllTags.mockClear();
        bindings.reload();
        await flush();
        expect(mockStore.loadAllTags).toHaveBeenCalledWith({ force: false });

        const done = jest.fn();
        bindings.reload(done);
        await flush();
        expect(mockStore.loadAllTags).toHaveBeenLastCalledWith({ force: true });
        expect(done).toHaveBeenCalled();
        expect(mockShowToast).toHaveBeenCalledWith('Tag list has been updated');

        bindings.sortable.value = true;
        mockStore.loadAllTags.mockClear();
        const blocked = jest.fn();
        bindings.reload(blocked);
        expect(blocked).toHaveBeenCalled();
        expect(mockStore.loadAllTags).not.toHaveBeenCalled();
        bindings.sortable.value = false;
        bindings.add();
        bindings.reload(blocked);
        expect(mockStore.loadAllTags).not.toHaveBeenCalled();
        bindings.newTag.value = null;

        mockStore.loadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled reload' });
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled reload');
        mockStore.loadAllTags.mockRejectedValueOnce({ processed: false, message: 'reload failed' });
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('reload failed');
        mockStore.loadAllTags.mockRejectedValueOnce('raw reload');
        bindings.reload(jest.fn());
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw reload');
    });

    test('adds, edits, saves, cancels, and reports save failures', async () => {
        const { bindings } = setup();
        await flush();
        bindings.add();
        expect(bindings.newTag.value).toEqual(expect.objectContaining({ id: '', name: '' }));
        bindings.newTag.value.name = 'New';
        bindings.save(bindings.newTag.value);
        await flush();
        expect(mockShowLoading).toHaveBeenCalled();
        expect(mockStore.saveTag).toHaveBeenCalledWith({ tag: expect.objectContaining({ name: 'New' }) });
        expect(bindings.newTag.value).toBeNull();
        expect(mockHideLoading).toHaveBeenCalled();

        bindings.edit(visibleTag);
        expect(bindings.editingTag.value).toEqual(expect.objectContaining({ id: 'food', name: 'Food' }));
        bindings.editingTag.value.name = 'Meals';
        bindings.save(visibleTag);
        await flush();
        expect(bindings.editingTag.value.id).toBe('');
        expect(bindings.editingTag.value.name).toBe('');

        bindings.edit(visibleTag);
        bindings.cancelSave(visibleTag);
        expect(bindings.editingTag.value.id).toBe('');
        bindings.add();
        bindings.cancelSave(bindings.newTag.value);
        expect(bindings.newTag.value).toBeNull();

        mockStore.saveTag.mockRejectedValueOnce({ processed: true, message: 'handled save' });
        bindings.save(visibleTag);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled save');
        mockStore.saveTag.mockRejectedValueOnce({ processed: false, message: 'save failed' });
        bindings.save(visibleTag);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('save failed');
        mockStore.saveTag.mockRejectedValueOnce('raw save');
        bindings.save(visibleTag);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw save');
    });
});

describe('mobile tag ListPage hide, delete, and sorting', () => {
    test('hides tags and reports service failures', async () => {
        const { bindings } = setup();
        bindings.hide(visibleTag, true);
        await flush();
        expect(mockStore.hideTag).toHaveBeenCalledWith({ tag: visibleTag, hidden: true });
        expect(mockHideLoading).toHaveBeenCalled();
        mockStore.hideTag.mockRejectedValueOnce({ processed: true, message: 'handled hide' });
        bindings.hide(visibleTag, false);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled hide');
        mockStore.hideTag.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(visibleTag, false);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('hide failed');
        mockStore.hideTag.mockRejectedValueOnce('raw hide');
        bindings.hide(visibleTag, false);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw hide');
    });

    test('prompts and deletes with exact swipeout completion', async () => {
        const { bindings } = setup();
        bindings.remove(null, false);
        expect(mockShowAlert).toHaveBeenCalledWith('An error occurred');
        bindings.remove(visibleTag, false);
        expect(bindings.tagToDelete.value).toStrictEqual(visibleTag);
        expect(bindings.showDeleteActionSheet.value).toBe(true);
        mockStore.deleteTag.mockImplementationOnce(async ({ beforeResolve }: any) => beforeResolve('done'));
        bindings.remove(visibleTag, true);
        await flush();
        expect(mockOnSwipeoutDeleted).toHaveBeenCalledWith('tag_food', 'done');
        expect(bindings.tagToDelete.value).toBeNull();
        expect(bindings.showDeleteActionSheet.value).toBe(false);

        mockStore.deleteTag.mockRejectedValueOnce({ processed: true, message: 'handled delete' });
        bindings.remove(visibleTag, true);
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled delete');
        mockStore.deleteTag.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(visibleTag, true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('delete failed');
        mockStore.deleteTag.mockRejectedValueOnce('raw delete');
        bindings.remove(visibleTag, true);
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw delete');
    });

    test('sorts, persists changed order, and keeps failed changes retryable', async () => {
        const { bindings } = setup();
        bindings.setSortable();
        expect(bindings.sortable.value).toBe(true);
        expect(bindings.showHidden.value).toBe(true);
        expect(bindings.displayOrderModified.value).toBe(false);
        bindings.setSortable();
        bindings.saveSortResult();
        expect(bindings.sortable.value).toBe(false);
        expect(mockStore.updateTagDisplayOrders).not.toHaveBeenCalled();

        bindings.setSortable();
        bindings.displayOrderModified.value = true;
        bindings.saveSortResult();
        await flush();
        expect(mockStore.updateTagDisplayOrders).toHaveBeenCalled();
        expect(bindings.displayOrderSaving.value).toBe(false);
        expect(bindings.sortable.value).toBe(false);
        expect(bindings.showHidden.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);

        bindings.setSortable();
        bindings.displayOrderModified.value = true;
        mockStore.updateTagDisplayOrders.mockRejectedValueOnce({ processed: true, message: 'handled sort' });
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).not.toHaveBeenCalledWith('handled sort');
        mockStore.updateTagDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('sort failed');
        mockStore.updateTagDisplayOrders.mockRejectedValueOnce('raw sort');
        bindings.saveSortResult();
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw sort');
    });

    test('validates sortable DOM ids and records successful moves', async () => {
        const { bindings } = setup();
        bindings.onSort(null);
        bindings.onSort({ el: { id: '' }, from: 1, to: 2 });
        bindings.onSort({ el: { id: 'account_food' }, from: 1, to: 2 });
        expect(mockShowToast).toHaveBeenCalledTimes(3);
        bindings.onSort({ el: { id: 'tag_food' }, from: 2, to: 4 });
        await flush();
        expect(mockStore.changeTagDisplayOrder).toHaveBeenCalledWith({ tagId: 'food', from: 2, to: 4 });
        expect(bindings.displayOrderModified.value).toBe(true);
        mockStore.changeTagDisplayOrder.mockRejectedValueOnce(new Error('move failed'));
        bindings.onSort({ el: { id: 'tag_food' }, from: 1, to: 2 });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('move failed');
        mockStore.changeTagDisplayOrder.mockRejectedValueOnce('raw move');
        bindings.onSort({ el: { id: 'tag_food' }, from: 1, to: 2 });
        await flush();
        expect(mockShowToast).toHaveBeenCalledWith('raw move');
    });
});

describe('mobile tag ListPage navigation and template', () => {
    test('refreshes invalid state after page entry and always invokes route recovery', async () => {
        const { bindings, router } = setup();
        await flush();
        mockStore.loadAllTags.mockClear();
        mockStore.transactionTagListStateInvalid = true;
        bindings.loading.value = true;
        bindings.onPageAfterIn();
        expect(mockStore.loadAllTags).not.toHaveBeenCalled();
        bindings.loading.value = false;
        bindings.onPageAfterIn();
        await flush();
        expect(mockStore.loadAllTags).toHaveBeenCalledWith({ force: false });
        expect(mockRouteBackOnError).toHaveBeenCalledWith(router, bindings.loadingError);
    });

    test('executes production template branches and event wrappers', async () => {
        const warnSpy = jest.spyOn(console, 'warn').mockImplementation(() => undefined);
        try {
            const { proxyRefs } = jest.requireActual('vue') as any;
            const { bindings } = setup();
            await flush();
            const scenarios = ['loading', 'normal', 'editing', 'sortable', 'empty'];
            for (const scenario of scenarios) {
                bindings.loading.value = scenario === 'loading';
                bindings.sortable.value = scenario === 'sortable';
                bindings.showHidden.value = scenario === 'sortable';
                bindings.newTag.value = scenario === 'editing' ? new MockTransactionTag({ name: 'Draft' }) : null;
                bindings.editingTag.value = scenario === 'editing'
                    ? new MockTransactionTag({ id: 'food', name: 'Meals' })
                    : MockTransactionTag.createNewTag();
                mockStore.allTransactionTags = scenario === 'empty' ? [] : [visibleTag, hiddenTag];
                const exposed = proxyRefs(bindings);
                const vnode = TagListPage.render(exposed, [], {}, exposed, {}, {});
                const handlers: Array<{ name: string; handler: (...args: any[]) => unknown }> = [];
                visitVNode(vnode, handlers);
                expect(handlers.length).toBeGreaterThan(2);
                for (const { name, handler } of handlers) {
                    try {
                        if (name === 'onSortable:sort') handler({ el: { id: 'tag_food' }, from: 1, to: 2 });
                        else if (name === 'onPtr:refresh') handler(jest.fn());
                        else if (name.startsWith('onUpdate:')) handler(true);
                        else handler();
                    } catch {
                        // Generated handlers intentionally accept heterogeneous payloads.
                    }
                    await flush(2);
                }
            }

            const { createSSRApp, defineComponent, h } = jest.requireActual('vue') as any;
            const { renderToString } = jest.requireActual('vue/server-renderer') as any;
            const RuntimePage = {
                ...TagListPage,
                setup(props: any, context: any) {
                    const bindings = TagListPage.setup(props, context);
                    bindings.loading.value = false;
                    bindings.editingTag.value = new MockTransactionTag({ id: 'food', name: 'Meals' });
                    bindings.newTag.value = new MockTransactionTag({ name: 'Draft' });
                    return bindings;
                }
            };
            const Stub = defineComponent({
                inheritAttrs: false,
                setup: (_props: unknown, { attrs, slots }: any) => {
                    for (const [name, handler] of Object.entries(attrs)) {
                        if (name.startsWith('on') && typeof handler === 'function') {
                            mockTemplateHandlers.push({ name, handler: handler as (...args: any[]) => unknown });
                        }
                    }
                    return () => h('div', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
                }
            });
            mockStore.allTransactionTags = [visibleTag, hiddenTag];
            const app = createSSRApp(RuntimePage, { f7router: { back: jest.fn(), navigate: jest.fn() } });
            for (const name of [
                'f7-page', 'f7-navbar', 'f7-nav-left', 'f7-nav-title', 'f7-nav-right', 'f7-link',
                'f7-list', 'f7-list-item', 'f7-icon', 'f7-badge', 'f7-input', 'f7-button',
                'f7-swipeout-actions', 'f7-swipeout-button', 'f7-actions', 'f7-actions-group',
                'f7-actions-button', 'f7-actions-label'
            ]) app.component(name, Stub);
            app.config.warnHandler = () => undefined;
            const html = await renderToString(app);
            expect(html).toContain('tag-item-list');
            for (const { name, handler } of mockTemplateHandlers) {
                try {
                    if (name.startsWith('onUpdate:')) handler('Updated');
                    else handler();
                } catch {
                    // SSR-captured handlers intentionally accept different payloads.
                }
                await flush(2);
            }
        } finally {
            warnSpy.mockRestore();
        }
    });
});
