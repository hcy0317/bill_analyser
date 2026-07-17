import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockActualVue = jest.requireActual('vue') as any;
const mockTemplateRefs = new Map<string, any>();
const mockIsNoAvailableTag = jest.fn<(tags: any[], showHidden: boolean) => boolean>();
const mockGetAvailableTagCount = jest.fn<(tags: any[], showHidden: boolean) => number>();
const mockLogger = {
    debug: jest.fn(),
    error: jest.fn(),
};
let mockSlotTag: any;

class MockTransactionTag {
    id = '';
    name = '';
    hidden = false;
    displayOrder = 0;

    constructor(overrides: Record<string, unknown> = {}) {
        Object.assign(this, overrides);
    }

    static createNewTag(): MockTransactionTag {
        return new MockTransactionTag();
    }
}

function createTag(overrides: Record<string, unknown> = {}): MockTransactionTag {
    return new MockTransactionTag({
        id: 'tag-1',
        name: 'Groceries',
        hidden: false,
        displayOrder: 0,
        ...overrides,
    });
}

const mockStore = mockActualVue.reactive({
    allTransactionTags: [
        createTag(),
        createTag({ id: 'tag-hidden', name: 'Archived', hidden: true, displayOrder: 1 }),
    ] as MockTransactionTag[],
    loadAllTags: jest.fn<(...args: any[]) => Promise<void>>(),
    saveTag: jest.fn<(...args: any[]) => Promise<void>>(),
    hideTag: jest.fn<(...args: any[]) => Promise<void>>(),
    deleteTag: jest.fn<(...args: any[]) => Promise<void>>(),
    updateTagDisplayOrders: jest.fn<(...args: any[]) => Promise<void>>(),
    changeTagDisplayOrder: jest.fn<(...args: any[]) => Promise<void>>(),
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
        setup: (_props: unknown, { attrs, slots }: any) => () => mockActualVue.h(
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
    default: createImportedStub('TagsListConfirmStub'),
}));
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: createImportedStub('TagsListSnackBarStub'),
}));
jest.mock('@/components/desktop/SettingsJsonImportExportButton.vue', () => ({
    __esModule: true,
    default: createImportedStub('TagsListImportExportStub'),
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({ tt: (key: string) => `tt:${key}` }),
}));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockStore }));
jest.mock('@/models/transaction_tag.ts', () => ({ TransactionTag: MockTransactionTag }));
jest.mock('@/lib/tag.ts', () => ({
    isNoAvailableTag: (tags: any[], showHidden: boolean) => mockIsNoAvailableTag(tags, showHidden),
    getAvailableTagCount: (tags: any[], showHidden: boolean) => mockGetAvailableTagCount(tags, showHidden),
}));
jest.mock('@/lib/logger.ts', () => ({
    __esModule: true,
    default: mockLogger,
}));

const ListPage = require('@/views/desktop/tags/ListPage.vue').default as any;

function setupPage(): { bindings: any } {
    mockTemplateRefs.clear();
    const bindings = ListPage.setup({}, {
        attrs: {}, slots: {}, emit: jest.fn(), expose: jest.fn(),
    });
    return { bindings };
}

function installRefs(bindings: any): {
    snackbar: { showMessage: jest.Mock; showError: jest.Mock };
    confirm: { open: jest.Mock<(...args: any[]) => Promise<void>> };
} {
    const snackbar = { showMessage: jest.fn(), showError: jest.fn() };
    const confirm = { open: jest.fn<(...args: any[]) => Promise<void>>().mockResolvedValue(undefined) };
    bindings.snackbar.value = snackbar;
    bindings.confirmDialog.value = confirm;
    return { snackbar, confirm };
}

async function flush(times = 8): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
    await mockActualVue.nextTick();
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

function mountWithHostRenderer(): { app: any; root: any; state: any } {
    const { createRenderer, defineComponent, h } = mockActualVue;
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
        name: 'TagsListSlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => {
                    try {
                        return slot?.({ element: mockSlotTag, item: mockSlotTag }) ?? [];
                    } catch {
                        return [];
                    }
                }),
            );
        },
    });
    const app = renderer.createApp(ListPage);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'v-row', 'v-col', 'v-card', 'v-btn', 'v-progress-circular', 'v-icon', 'v-tooltip',
        'v-spacer', 'v-menu', 'v-list', 'v-list-item', 'v-table', 'v-skeleton-loader',
        'v-badge', 'v-text-field', 'draggable-list', 'settings-json-import-export-button',
        'confirm-dialog', 'snack-bar',
    ]) {
        app.component(name, SlotHost);
    }
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState };
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
    mockSlotTag = createTag();
    mockStore.allTransactionTags = [
        createTag(),
        createTag({ id: 'tag-hidden', name: 'Archived', hidden: true, displayOrder: 1 }),
    ];
    mockStore.loadAllTags.mockResolvedValue(undefined);
    mockStore.saveTag.mockResolvedValue(undefined);
    mockStore.hideTag.mockResolvedValue(undefined);
    mockStore.deleteTag.mockResolvedValue(undefined);
    mockStore.updateTagDisplayOrders.mockResolvedValue(undefined);
    mockStore.changeTagDisplayOrder.mockResolvedValue(undefined);
    mockIsNoAvailableTag.mockImplementation((tags, showHidden) => (
        !tags.some(tag => showHidden || !tag.hidden)
    ));
    mockGetAvailableTagCount.mockImplementation((tags, showHidden) => (
        tags.filter(tag => showHidden || !tag.hidden).length
    ));
});

describe('desktop tag ListPage production-loaded initialization and editing', () => {
    test('loads tags and derives visibility, editing, and modification state', async () => {
        const { bindings } = setupPage();
        expect(bindings.loading.value).toBe(true);
        expect(bindings.tags.value).toHaveLength(2);
        expect(bindings.noAvailableTag.value).toBe(false);
        expect(bindings.availableTagCount.value).toBe(1);
        expect(bindings.hasEditingTag.value).toBe(false);

        await flush();
        expect(mockStore.loadAllTags).toHaveBeenCalledWith({ force: false });
        expect(bindings.loading.value).toBe(false);

        const empty = MockTransactionTag.createNewTag();
        expect(bindings.isTagModified(empty)).toBe(false);
        empty.name = 'New tag';
        expect(bindings.isTagModified(empty)).toBe(true);

        const existing = mockStore.allTransactionTags[0];
        bindings.edit(existing);
        expect(bindings.editingTag.value).toMatchObject({ id: 'tag-1', name: 'Groceries' });
        expect(bindings.hasEditingTag.value).toBe(true);
        expect(bindings.isTagModified(existing)).toBe(false);
        bindings.editingTag.value.name = 'Food';
        expect(bindings.isTagModified(existing)).toBe(true);
        bindings.editingTag.value.name = '';
        expect(bindings.isTagModified(existing)).toBe(false);

        bindings.showHidden.value = true;
        expect(bindings.availableTagCount.value).toBe(2);
        mockStore.allTransactionTags = [createTag({ hidden: true })];
        bindings.showHidden.value = false;
        expect(bindings.noAvailableTag.value).toBe(true);
    });

    test('reports only unprocessed initialization failures', async () => {
        mockStore.loadAllTags.mockRejectedValueOnce({ processed: false, message: 'initial load failed' });
        const first = setupPage();
        const firstRefs = installRefs(first.bindings);
        await flush();
        expect(first.bindings.loading.value).toBe(false);
        expect(firstRefs.snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({
            message: 'initial load failed',
        }));

        mockStore.loadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled' });
        const second = setupPage();
        const secondRefs = installRefs(second.bindings);
        await flush();
        expect(second.bindings.loading.value).toBe(false);
        expect(secondRefs.snackbar.showError).not.toHaveBeenCalled();
    });

    test('adds, edits, and cancels both new and existing tag drafts', async () => {
        const { bindings } = setupPage();
        await flush();
        const existing = mockStore.allTransactionTags[0];

        bindings.add();
        expect(bindings.newTag.value).toEqual(expect.objectContaining({ id: '', name: '' }));
        expect(bindings.hasEditingTag.value).toBe(true);
        bindings.cancelSave(bindings.newTag.value);
        expect(bindings.newTag.value).toBeNull();

        bindings.edit(existing);
        expect(bindings.editingTag.value).toMatchObject({ id: 'tag-1', name: 'Groceries' });
        bindings.cancelSave(existing);
        expect(bindings.editingTag.value).toMatchObject({ id: '', name: '' });
        expect(bindings.hasEditingTag.value).toBe(false);
    });
});

describe('desktop tag ListPage production-loaded reload and saves', () => {
    test('blocks reload during edits and handles success and all error ownership paths', async () => {
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        mockStore.loadAllTags.mockClear();

        bindings.add();
        bindings.reload();
        expect(mockStore.loadAllTags).not.toHaveBeenCalled();
        bindings.cancelSave(bindings.newTag.value);
        bindings.edit(mockStore.allTransactionTags[0]);
        bindings.reload();
        expect(mockStore.loadAllTags).not.toHaveBeenCalled();
        bindings.cancelSave(bindings.editingTag.value);

        bindings.displayOrderModified.value = true;
        bindings.reload();
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(mockStore.loadAllTags).toHaveBeenCalledWith({ force: true });
        expect(bindings.loading.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(snackbar.showMessage).toHaveBeenCalledWith('Tag list has been updated');

        bindings.displayOrderModified.value = true;
        mockStore.loadAllTags.mockRejectedValueOnce({ isUpToDate: true, processed: true });
        bindings.reload();
        await flush();
        expect(bindings.displayOrderModified.value).toBe(false);
        expect(snackbar.showError).not.toHaveBeenCalled();

        bindings.displayOrderModified.value = true;
        mockStore.loadAllTags.mockRejectedValueOnce({ isUpToDate: false, processed: false, message: 'reload failed' });
        bindings.reload();
        await flush();
        expect(bindings.loading.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(true);
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'reload failed' }));

        mockStore.loadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled reload' });
        bindings.reload();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
    });

    test('saves an edit, reloads the list, and exposes the pending state', async () => {
        const pending = deferred<void>();
        mockStore.saveTag.mockReturnValueOnce(pending.promise);
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        const existing = mockStore.allTransactionTags[0];
        bindings.edit(existing);
        bindings.editingTag.value.name = 'Food';

        bindings.save(bindings.editingTag.value);
        expect(bindings.updating.value).toBe(true);
        expect(bindings.tagUpdating.value['tag-1']).toBe(true);
        expect(mockStore.saveTag).toHaveBeenCalledWith({ tag: expect.objectContaining({ name: 'Food' }) });
        pending.resolve(undefined);
        await flush();
        expect(bindings.updating.value).toBe(false);
        expect(bindings.tagUpdating.value['tag-1']).toBe(false);
        expect(bindings.editingTag.value).toMatchObject({ id: '', name: '' });
        expect(mockStore.loadAllTags).toHaveBeenLastCalledWith({ force: true });
        expect(snackbar.showMessage).toHaveBeenCalledWith('Tag saved successfully');
    });

    test('saves a new tag and logs a post-save reload failure', async () => {
        const { bindings } = setupPage();
        await flush();
        installRefs(bindings);
        mockStore.loadAllTags.mockRejectedValueOnce(new Error('post-save reload failed'));

        bindings.add();
        bindings.newTag.value.name = 'Travel';
        bindings.save(bindings.newTag.value);
        await flush();
        expect(mockStore.saveTag).toHaveBeenCalledWith({
            tag: expect.objectContaining({ id: '', name: 'Travel' }),
        });
        expect(bindings.tagUpdating.value['']).toBe(false);
        expect(bindings.newTag.value).toBeNull();
        expect(mockLogger.error).toHaveBeenCalledWith(
            '[ListPage] Failed to reload tags after save',
            expect.objectContaining({ message: 'post-save reload failed' }),
        );
    });

    test('clears pending save state and reports only unprocessed save failures', async () => {
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        const existing = mockStore.allTransactionTags[0];

        mockStore.saveTag.mockRejectedValueOnce({ processed: false, message: 'save failed' });
        bindings.save(existing);
        await flush();
        expect(bindings.updating.value).toBe(false);
        expect(bindings.tagUpdating.value['tag-1']).toBe(false);
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'save failed' }));

        mockStore.saveTag.mockRejectedValueOnce({ processed: true, message: 'handled save' });
        bindings.save(existing);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
    });
});

describe('desktop tag ListPage production-loaded mutations and sorting', () => {
    test('tracks hide state and reports only unprocessed failures', async () => {
        const pending = deferred<void>();
        mockStore.hideTag.mockReturnValueOnce(pending.promise);
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        const tag = mockStore.allTransactionTags[0];

        bindings.hide(tag, true);
        expect(bindings.updating.value).toBe(true);
        expect(bindings.tagHiding.value['tag-1']).toBe(true);
        expect(mockStore.hideTag).toHaveBeenCalledWith({ tag, hidden: true });
        pending.resolve(undefined);
        await flush();
        expect(bindings.updating.value).toBe(false);
        expect(bindings.tagHiding.value['tag-1']).toBe(false);
        expect(mockLogger.debug).toHaveBeenCalledWith('[ListPage] Tag hidden successfully: tag-1');

        mockStore.hideTag.mockRejectedValueOnce({ processed: false, message: 'hide failed' });
        bindings.hide(tag, false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'hide failed' }));
        expect(mockLogger.error).toHaveBeenCalledWith(
            '[ListPage] Failed to hide tag: tag-1',
            expect.objectContaining({ message: 'hide failed' }),
        );

        mockStore.hideTag.mockRejectedValueOnce({ processed: true, message: 'handled hide' });
        bindings.hide(tag, false);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
    });

    test('requires confirmation before delete and covers success and failure ownership', async () => {
        const { bindings } = setupPage();
        await flush();
        const { confirm, snackbar } = installRefs(bindings);
        const tag = mockStore.allTransactionTags[0];

        bindings.remove(tag);
        expect(confirm.open).toHaveBeenCalledWith('Are you sure you want to delete this tag?');
        await flush();
        expect(mockStore.deleteTag).toHaveBeenCalledWith({ tag });
        expect(bindings.updating.value).toBe(false);
        expect(bindings.tagRemoving.value['tag-1']).toBe(false);
        expect(mockLogger.debug).toHaveBeenCalledWith('[ListPage] Tag removed successfully: tag-1');

        mockStore.deleteTag.mockRejectedValueOnce({ processed: false, message: 'delete failed' });
        bindings.remove(tag);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'delete failed' }));
        expect(mockLogger.error).toHaveBeenCalledWith(
            '[ListPage] Failed to remove tag: tag-1',
            expect.objectContaining({ message: 'delete failed' }),
        );

        mockStore.deleteTag.mockRejectedValueOnce({ processed: true, message: 'handled delete' });
        bindings.remove(tag);
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);

        bindings.confirmDialog.value = null;
        expect(() => bindings.remove(tag)).not.toThrow();
    });

    test('persists changed order and handles processed and visible failures', async () => {
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);
        mockStore.updateTagDisplayOrders.mockClear();

        bindings.saveSortResult();
        expect(mockStore.updateTagDisplayOrders).not.toHaveBeenCalled();

        bindings.displayOrderModified.value = true;
        bindings.saveSortResult();
        expect(bindings.loading.value).toBe(true);
        await flush();
        expect(mockStore.updateTagDisplayOrders).toHaveBeenCalledWith();
        expect(bindings.loading.value).toBe(false);
        expect(bindings.displayOrderModified.value).toBe(false);

        bindings.displayOrderModified.value = true;
        mockStore.updateTagDisplayOrders.mockRejectedValueOnce({ processed: false, message: 'sort failed' });
        bindings.saveSortResult();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'sort failed' }));
        expect(bindings.displayOrderModified.value).toBe(true);

        mockStore.updateTagDisplayOrders.mockRejectedValueOnce({ processed: true, message: 'handled sort' });
        bindings.saveSortResult();
        await flush();
        expect(snackbar.showError).toHaveBeenCalledTimes(1);
        expect(bindings.loading.value).toBe(false);
    });

    test('validates move events, records valid moves, and exposes move failures', async () => {
        const { bindings } = setupPage();
        await flush();
        const { snackbar } = installRefs(bindings);

        bindings.onMove(null);
        bindings.onMove({});
        expect(mockStore.changeTagDisplayOrder).not.toHaveBeenCalled();
        bindings.onMove({ moved: { element: null, oldIndex: 0, newIndex: 1 } });
        bindings.onMove({ moved: { element: {}, oldIndex: 0, newIndex: 1 } });
        expect(snackbar.showMessage).toHaveBeenCalledTimes(2);
        expect(snackbar.showMessage).toHaveBeenLastCalledWith('Unable to move tag');

        bindings.onMove({ moved: { element: { id: 'tag-1' }, oldIndex: 0, newIndex: 1 } });
        await flush();
        expect(mockStore.changeTagDisplayOrder).toHaveBeenCalledWith({
            tagId: 'tag-1', from: 0, to: 1,
        });
        expect(bindings.displayOrderModified.value).toBe(true);

        mockStore.changeTagDisplayOrder.mockRejectedValueOnce({ message: 'move failed' });
        bindings.onMove({ moved: { element: { id: 'tag-1' }, oldIndex: 1, newIndex: 0 } });
        await flush();
        expect(snackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'move failed' }));
    });
});

describe('desktop tag ListPage production template behavior', () => {
    test('mounts template-backed states and keeps every event target callable', async () => {
        const { app, root, state } = mountWithHostRenderer();
        try {
            const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            mockSlotTag = createTag();
            mockStore.allTransactionTags = [mockSlotTag, createTag({ id: 'tag-2', name: 'Dining' })];
            state.loading = false;
            state.updating = false;
            state.showHidden = false;
            state.displayOrderModified = true;
            state.editingTag = MockTransactionTag.createNewTag();
            state.newTag = null;
            await mockActualVue.nextTick();
            collectHostCallbacks(root, callbacks);

            expect(root).toBeDefined();
            expect([
                state.add,
                state.edit,
                state.save,
                state.cancelSave,
                state.reload,
                state.saveSortResult,
                state.hide,
                state.remove,
                state.onMove,
            ]).toEqual(expect.arrayContaining([expect.any(Function)]));

            state.snackbar = { showMessage: jest.fn(), showError: jest.fn() };
            state.confirmDialog = {
                open: jest.fn<() => Promise<void>>().mockResolvedValue(undefined),
            };
            state.add();
            expect(state.newTag).toMatchObject({ id: '', name: '' });
            state.newTag.name = 'Travel';
            state.save(state.newTag);
            await flush();
            state.edit(mockSlotTag);
            expect(state.editingTag).toMatchObject({ id: 'tag-1', name: 'Groceries' });
            state.cancelSave(state.editingTag);
            state.showHidden = true;
            state.showHidden = false;
            state.onMove({ moved: { element: mockSlotTag, oldIndex: 0, newIndex: 1 } });
            await flush();
            state.saveSortResult();
            await flush();
            state.hide(mockSlotTag, true);
            await flush();
            state.remove(mockSlotTag);
            await flush();
            state.reload();
            await flush();
            expect(mockStore.loadAllTags).toHaveBeenCalled();
            expect(mockStore.saveTag).toHaveBeenCalled();
            expect(mockStore.changeTagDisplayOrder).toHaveBeenCalled();
            expect(mockStore.updateTagDisplayOrders).toHaveBeenCalled();
            expect(mockStore.hideTag).toHaveBeenCalled();
            expect(mockStore.deleteTag).toHaveBeenCalled();
        } finally {
            app.unmount();
        }
    });
});
