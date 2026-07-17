import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const actualVue = jest.requireActual('vue') as any;
const mockCategoryType = { Income: 2, Expense: 3, Transfer: 4 } as const;
const mockSnackbar = { showError: jest.fn<(...args: any[]) => void>() };
const mockCategorizeAccounts = jest.fn<(...args: any[]) => any[]>();
const mockFindAccountName = jest.fn<(...args: any[]) => string | null>();
const mockPrimaryCategoryName = jest.fn<(...args: any[]) => string>();
const mockSecondaryCategoryName = jest.fn<(...args: any[]) => string>();
let mockSlotTagHidden = false;

const mockSettingsStore = actualVue.reactive({
    appSettings: { showAccountBalance: true },
});
const mockAccountsStore = actualVue.reactive({
    allPlainAccounts: [
        { id: 'cash', name: 'Cash' },
        { id: 'bank', name: 'Bank' },
    ],
    allVisiblePlainAccounts: [{ id: 'cash', name: 'Cash' }],
    loadAllAccounts: jest.fn<(...args: any[]) => Promise<void>>(),
});
const mockCategoryStore = actualVue.reactive({
    allTransactionCategories: {
        [mockCategoryType.Expense]: [{ id: 'expense-food', name: 'Food', subCategories: [] }],
        [mockCategoryType.Income]: [{ id: 'income-salary', name: 'Salary', subCategories: [] }],
        [mockCategoryType.Transfer]: [{ id: 'transfer-main', name: 'Transfer', subCategories: [] }],
    } as Record<number, any[]>,
    hasAvailableExpenseCategories: true,
    hasAvailableIncomeCategories: true,
    hasAvailableTransferCategories: true,
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>(),
});
const mockTagStore = actualVue.reactive({
    allTransactionTags: [
        { id: 'tag-food', name: 'Food', hidden: false },
        { id: 'tag-hidden', name: 'Hidden', hidden: true },
    ],
    loadAllTags: jest.fn<(...args: any[]) => Promise<void>>(),
});

class MockAccount {
    static findAccountNameById(accounts: any[], accountId: string): string | null {
        return mockFindAccountName(accounts, accountId);
    }
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(mockSnackbar),
    };
});
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'BatchReplaceSnackBarStub', render: () => null },
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string) => `tt:${key}`,
        getCategorizedAccountsWithDisplayBalance: (...args: any[]) => mockCategorizeAccounts(...args),
    }),
}));
jest.mock('@/stores/setting.ts', () => ({ useSettingsStore: () => mockSettingsStore }));
jest.mock('@/stores/account.ts', () => ({ useAccountsStore: () => mockAccountsStore }));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/models/account.ts', () => ({ Account: MockAccount }));
jest.mock('@/lib/category.ts', () => ({
    getTransactionPrimaryCategoryName: (...args: any[]) => mockPrimaryCategoryName(...args),
    getTransactionSecondaryCategoryName: (...args: any[]) => mockSecondaryCategoryName(...args),
}));

const BatchReplaceDialog = require(
    '@/views/desktop/transactions/import/dialogs/BatchReplaceDialog.vue'
).default as any;

function setupDialog(): { bindings: any; exposed: Record<string, unknown> } {
    const exposed: Record<string, unknown> = {};
    const bindings = BatchReplaceDialog.setup({}, {
        attrs: {},
        slots: {},
        emit: jest.fn(),
        expose: (value: Record<string, unknown>) => Object.assign(exposed, value),
    });
    return { bindings, exposed };
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

function mountWithHostRenderer(): { app: any; root: any; state: any; vm: any } {
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
        name: 'BatchReplaceSlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h(
                'stub',
                attrs,
                Object.values(slots).flatMap((slot: any) => {
                    try {
                        return slot?.({
                            props: { role: 'option' },
                            item: {
                                title: 'Tag item',
                                value: 'tag-food',
                                raw: { hidden: mockSlotTagHidden },
                            },
                        }) ?? [];
                    } catch {
                        return [];
                    }
                }),
            );
        },
    });
    const app = renderer.createApp(BatchReplaceDialog);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'v-dialog', 'v-card', 'v-card-text', 'v-btn', 'v-progress-circular', 'v-icon',
        'v-tooltip', 'v-row', 'v-col', 'v-autocomplete', 'v-chip', 'v-list-item',
        'v-list-item-title', 'v-switch', 'two-column-select', 'snack-bar',
    ]) {
        app.component(name, SlotHost);
    }
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState, vm };
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

function findHostNode(node: any, predicate: (candidate: any) => boolean, seen = new Set<any>()): any {
    if (!node || typeof node !== 'object' || seen.has(node)) return undefined;
    seen.add(node);
    if (predicate(node)) return node;
    for (const child of node.children ?? []) {
        const match = findHostNode(child, predicate, seen);
        if (match) return match;
    }
    return undefined;
}

beforeEach(() => {
    jest.clearAllMocks();
    mockSlotTagHidden = false;
    mockSettingsStore.appSettings.showAccountBalance = true;
    mockAccountsStore.allPlainAccounts = [
        { id: 'cash', name: 'Cash' },
        { id: 'bank', name: 'Bank' },
    ];
    mockAccountsStore.allVisiblePlainAccounts = [{ id: 'cash', name: 'Cash' }];
    mockCategoryStore.allTransactionCategories = {
        [mockCategoryType.Expense]: [{ id: 'expense-food', name: 'Food', subCategories: [] }],
        [mockCategoryType.Income]: [{ id: 'income-salary', name: 'Salary', subCategories: [] }],
        [mockCategoryType.Transfer]: [{ id: 'transfer-main', name: 'Transfer', subCategories: [] }],
    };
    mockCategoryStore.hasAvailableExpenseCategories = true;
    mockCategoryStore.hasAvailableIncomeCategories = true;
    mockCategoryStore.hasAvailableTransferCategories = true;
    mockTagStore.allTransactionTags = [
        { id: 'tag-food', name: 'Food', hidden: false },
        { id: 'tag-hidden', name: 'Hidden', hidden: true },
    ];
    mockAccountsStore.loadAllAccounts.mockResolvedValue(undefined);
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockTagStore.loadAllTags.mockResolvedValue(undefined);
    mockCategorizeAccounts.mockImplementation((accounts, showBalance) => (
        accounts.map((account: any) => ({ ...account, displayBalance: showBalance ? '¥100' : '' }))
    ));
    mockFindAccountName.mockImplementation((accounts, accountId) => (
        accounts.find((account: any) => account.id === accountId)?.name ?? null
    ));
    mockPrimaryCategoryName.mockImplementation(value => `primary:${String(value)}`);
    mockSecondaryCategoryName.mockImplementation(value => `secondary:${String(value)}`);
});

describe('BatchReplaceDialog production-loaded state and open contracts', () => {
    test('projects account, category, tag, availability, and localized display state', () => {
        const { bindings, exposed } = setupDialog();

        expect(exposed).toEqual({ open: bindings.open });
        expect(bindings.showAccountBalance.value).toBe(true);
        expect(bindings.allAccounts.value.map((item: any) => item.id)).toStrictEqual(['cash', 'bank']);
        expect(bindings.allVisibleAccounts.value.map((item: any) => item.id)).toStrictEqual(['cash']);
        expect(bindings.allVisibleCategorizedAccounts.value).toStrictEqual([
            { id: 'cash', name: 'Cash', displayBalance: '¥100' },
        ]);
        expect(mockCategorizeAccounts).toHaveBeenCalledWith(bindings.allVisibleAccounts.value, true);
        expect(bindings.allCategories.value).toStrictEqual(mockCategoryStore.allTransactionCategories);
        expect(bindings.allTags.value).toStrictEqual(mockTagStore.allTransactionTags);
        expect(bindings.hasAvailableExpenseCategories.value).toBe(true);
        expect(bindings.hasAvailableIncomeCategories.value).toBe(true);
        expect(bindings.hasAvailableTransferCategories.value).toBe(true);

        mockSettingsStore.appSettings.showAccountBalance = false;
        expect(bindings.showAccountBalance.value).toBe(false);
        expect(bindings.allVisibleCategorizedAccounts.value[0].displayBalance).toBe('');
        expect(bindings.getAccountDisplayName('cash')).toBe('Cash');
        expect(bindings.getAccountDisplayName('missing')).toBe('');
        expect(bindings.getAccountDisplayName()).toBe('tt:None');
    });

    test('resets stale state and routes invalid/source item inputs only to supported modes', async () => {
        const { bindings } = setupDialog();
        bindings.sourceItem.value = 'stale-source';
        bindings.targetItem.value = 'stale-target';
        bindings.removeTag.value = true;

        const invalidItems = [{ name: 'Legacy', value: 'legacy-id' }];
        const invalidPending = bindings.open({
            mode: 'replaceInvalidItems', type: 'account', invalidItems,
        });
        expect(bindings.mode.value).toBe('replaceInvalidItems');
        expect(bindings.type.value).toBe('account');
        expect(bindings.invalidItems.value).toStrictEqual(invalidItems);
        expect(bindings.allSourceTagItems.value).toBeUndefined();
        expect(bindings.sourceItem.value).toBeUndefined();
        expect(bindings.targetItem.value).toBeUndefined();
        expect(bindings.removeTag.value).toBe(false);
        expect(bindings.showState.value).toBe(true);
        bindings.cancel();
        await expect(invalidPending).rejects.toBeUndefined();

        const sourceTags = [{ name: 'Old Tag', value: 'tag-old' }];
        const tagPending = bindings.open({
            mode: 'batchReplace', type: 'tag', allSourceTagItems: sourceTags,
        });
        expect(bindings.invalidItems.value).toBeUndefined();
        expect(bindings.allSourceTagItems.value).toStrictEqual(sourceTags);
        bindings.cancel();
        await expect(tagPending).rejects.toBeUndefined();

        const addPending = bindings.open({ mode: 'batchAdd', type: 'tag' });
        expect(bindings.invalidItems.value).toBeUndefined();
        expect(bindings.allSourceTagItems.value).toBeUndefined();
        bindings.cancel();
        await expect(addPending).rejects.toBeUndefined();

        bindings.cancel();
        expect(bindings.showState.value).toBe(false);
    });
});

describe('BatchReplaceDialog production-loaded confirmation and cancellation', () => {
    test('resolves batch category/account replacement with target identifiers only', async () => {
        const { bindings } = setupDialog();
        for (const type of ['expenseCategory', 'incomeCategory', 'transferCategory', 'account', 'destinationAccount']) {
            const pending = bindings.open({ mode: 'batchReplace', type });
            bindings.sourceItem.value = 'ignored-source';
            bindings.targetItem.value = `${type}-target`;
            bindings.confirm();
            await expect(pending).resolves.toStrictEqual({ targetItem: `${type}-target` });
            expect(bindings.showState.value).toBe(false);
        }
    });

    test('resolves tag replace, tag add, invalid replacement, empty IDs, and remove-tag semantics', async () => {
        const { bindings } = setupDialog();

        let pending = bindings.open({ mode: 'batchReplace', type: 'tag' });
        bindings.sourceItem.value = 'tag-old';
        bindings.targetItem.value = 'tag-new';
        bindings.confirm();
        await expect(pending).resolves.toStrictEqual({ sourceItem: 'tag-old', targetItem: 'tag-new' });

        pending = bindings.open({ mode: 'batchAdd', type: 'tag' });
        bindings.targetItem.value = '';
        bindings.confirm();
        await expect(pending).resolves.toStrictEqual({ targetItem: '' });

        pending = bindings.open({ mode: 'replaceInvalidItems', type: 'expenseCategory' });
        bindings.sourceItem.value = '';
        bindings.targetItem.value = '';
        bindings.confirm();
        await expect(pending).resolves.toStrictEqual({ sourceItem: '', targetItem: '' });

        pending = bindings.open({ mode: 'batchReplace', type: 'tag' });
        bindings.sourceItem.value = 'tag-remove';
        bindings.targetItem.value = 'must-clear';
        bindings.removeTag.value = true;
        await flush();
        expect(bindings.targetItem.value).toBeUndefined();
        bindings.confirm();
        await expect(pending).resolves.toStrictEqual({ sourceItem: 'tag-remove', targetItem: undefined });

        bindings.targetItem.value = 'kept-when-false';
        bindings.removeTag.value = false;
        await flush();
        expect(bindings.targetItem.value).toBe('kept-when-false');
    });

    test('safely closes when callbacks are absent or the runtime mode is unsupported', () => {
        const { bindings } = setupDialog();

        bindings.confirm();
        expect(bindings.showState.value).toBe(false);
        bindings.cancel();
        expect(bindings.showState.value).toBe(false);
        bindings.showState.value = true;
        bindings.mode.value = 'unsupported';
        bindings.type.value = 'tag';
        bindings.confirm();
        expect(bindings.showState.value).toBe(false);
    });
});

describe('BatchReplaceDialog production-loaded reload branches', () => {
    test('reloads each category, account, and tag source with force and ignores unknown types', async () => {
        const { bindings } = setupDialog();

        for (const type of ['expenseCategory', 'incomeCategory', 'transferCategory']) {
            bindings.type.value = type;
            bindings.reload();
            await flush();
        }
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledTimes(3);
        expect(mockCategoryStore.loadAllCategories).toHaveBeenLastCalledWith({ force: true });

        for (const type of ['account', 'destinationAccount']) {
            bindings.type.value = type;
            bindings.reload();
            await flush();
        }
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenCalledTimes(2);
        expect(mockAccountsStore.loadAllAccounts).toHaveBeenLastCalledWith({ force: true });

        bindings.type.value = 'tag';
        bindings.reload();
        await flush();
        expect(mockTagStore.loadAllTags).toHaveBeenCalledWith({ force: true });

        const callCounts = [
            mockCategoryStore.loadAllCategories.mock.calls.length,
            mockAccountsStore.loadAllAccounts.mock.calls.length,
            mockTagStore.loadAllTags.mock.calls.length,
        ];
        bindings.type.value = 'unsupported';
        bindings.reload();
        expect([
            mockCategoryStore.loadAllCategories.mock.calls.length,
            mockAccountsStore.loadAllAccounts.mock.calls.length,
            mockTagStore.loadAllTags.mock.calls.length,
        ]).toStrictEqual(callCounts);
        expect(bindings.loading.value).toBe(false);
    });

    test('tracks pending reloads and reports only unprocessed store failures', async () => {
        const categoryPending = deferred<void>();
        mockCategoryStore.loadAllCategories.mockReturnValueOnce(categoryPending.promise);
        const { bindings } = setupDialog();
        bindings.type.value = 'expenseCategory';
        bindings.reload();
        expect(bindings.loading.value).toBe(true);
        categoryPending.resolve(undefined);
        await flush();
        expect(bindings.loading.value).toBe(false);

        mockCategoryStore.loadAllCategories.mockRejectedValueOnce({ processed: false, message: 'category failed' });
        bindings.reload();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'category failed' }));
        mockCategoryStore.loadAllCategories.mockRejectedValueOnce({ processed: true, message: 'handled category' });
        bindings.reload();
        await flush();

        bindings.type.value = 'account';
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: false, message: 'account failed' });
        bindings.reload();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'account failed' }));
        mockAccountsStore.loadAllAccounts.mockRejectedValueOnce({ processed: true, message: 'handled account' });
        bindings.reload();
        await flush();

        bindings.type.value = 'tag';
        mockTagStore.loadAllTags.mockRejectedValueOnce({ processed: false, message: 'tag failed' });
        bindings.reload();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(expect.objectContaining({ message: 'tag failed' }));
        mockTagStore.loadAllTags.mockRejectedValueOnce({ processed: true, message: 'handled tag' });
        bindings.reload();
        await flush();

        expect(mockSnackbar.showError).toHaveBeenCalledTimes(3);
        expect(bindings.loading.value).toBe(false);
    });
});

describe('BatchReplaceDialog production template behavior', () => {
    test('renders every mode/type, persistent/disabled state, scoped item branch, and event wrapper', async () => {
        const { app, root, state } = mountWithHostRenderer();
        try {
            const callbacks: Array<{ name: string; callback: (...args: any[]) => unknown }> = [];
            const combinations = [
                ['batchReplace', 'expenseCategory'],
                ['batchReplace', 'incomeCategory'],
                ['batchReplace', 'transferCategory'],
                ['batchReplace', 'account'],
                ['batchReplace', 'destinationAccount'],
                ['batchReplace', 'tag'],
                ['batchAdd', 'tag'],
                ['replaceInvalidItems', 'expenseCategory'],
                ['replaceInvalidItems', 'incomeCategory'],
                ['replaceInvalidItems', 'transferCategory'],
                ['replaceInvalidItems', 'account'],
                ['replaceInvalidItems', 'tag'],
            ];
            for (const [mode, type] of combinations) {
                state.mode = mode;
                state.type = type;
                state.loading = false;
                state.sourceItem = type === 'tag' ? 'source-tag' : undefined;
                state.targetItem = `${type}-target`;
                await actualVue.nextTick();
                collectHostCallbacks(root, callbacks);
            }

            state.mode = 'replaceInvalidItems';
            state.type = 'tag';
            state.sourceItem = undefined;
            state.targetItem = undefined;
            state.removeTag = false;
            await actualVue.nextTick();
            let confirmNode = findHostNode(root, node => node.props?.onClick === state.confirm);
            expect(confirmNode?.props.disabled).toBe(true);

            state.sourceItem = '';
            state.targetItem = '';
            await actualVue.nextTick();
            confirmNode = findHostNode(root, node => node.props?.onClick === state.confirm);
            expect(confirmNode?.props.disabled).toBe(false);

            state.loading = true;
            await actualVue.nextTick();
            confirmNode = findHostNode(root, node => node.props?.onClick === state.confirm);
            expect(confirmNode?.props.disabled).toBe(true);

            state.loading = false;
            state.sourceItem = 'source-tag';
            state.removeTag = true;
            await actualVue.nextTick();
            confirmNode = findHostNode(root, node => node.props?.onClick === state.confirm);
            expect(confirmNode?.props.disabled).toBe(false);

            mockSlotTagHidden = true;
            state.targetItem = 'force-hidden-slot-render';
            await actualVue.nextTick();

            const pending = state.open({
                mode: 'batchReplace', type: 'tag', allSourceTagItems: [{ name: 'Old', value: 'old' }],
            });
            pending.catch(() => undefined);
            await actualVue.nextTick();
            collectHostCallbacks(root, callbacks);
            expect(callbacks.some(item => item.name === 'onClick')).toBe(true);
            expect(callbacks.some(item => item.name === 'onUpdate:modelValue')).toBe(true);

            for (const { name, callback } of callbacks) {
                try {
                    if (name === 'onUpdate:modelValue') {
                        callback('selected-id');
                    } else {
                        callback({ preventDefault: jest.fn(), stopPropagation: jest.fn() });
                    }
                } catch {
                    // V-model and dialog callbacks have heterogeneous payload contracts.
                }
            }
            await flush();
            expect(mockCategoryStore.loadAllCategories.mock.calls.length
                + mockAccountsStore.loadAllAccounts.mock.calls.length
                + mockTagStore.loadAllTags.mock.calls.length).toBeGreaterThan(0);
        } finally {
            app.unmount();
        }
    });
});
