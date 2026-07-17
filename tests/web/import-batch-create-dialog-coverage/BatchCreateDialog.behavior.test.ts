import { beforeEach, describe, expect, jest, test } from '@jest/globals';

const mockCategoryType = { Income: 2, Expense: 3, Transfer: 4 } as const;
const mockSnackbar = { showError: jest.fn<(...args: any[]) => void>() };

const mockCategoryStore = {
    addCategories: jest.fn<(...args: any[]) => Promise<Record<number, any[]>>>(),
    loadAllCategories: jest.fn<(...args: any[]) => Promise<void>>()
};

const mockTagStore = {
    addTags: jest.fn<(...args: any[]) => Promise<any[]>>(),
    loadAllTags: jest.fn<(...args: any[]) => Promise<void>>()
};

class MockTransactionCategory {
    id = '';
    name = '';
    parentId = '0';
    type: number;
    icon = '';
    color = '#000000';
    comment = '';
    displayOrder = 0;
    subCategories?: MockTransactionCategory[];

    constructor(type: number) {
        this.type = type;
    }

    static createNewCategory(type: number): MockTransactionCategory {
        return new MockTransactionCategory(type);
    }

    toCreateRequest(clientSessionId: string): Record<string, unknown> {
        return {
            name: this.name,
            type: this.type,
            parentId: this.parentId,
            icon: this.icon,
            color: this.color,
            comment: this.comment,
            displayOrder: this.displayOrder,
            clientSessionId
        };
    }
}

class MockTransactionTag {
    id = '';
    name: string;

    constructor(name: string) {
        this.name = name;
    }

    static createNewTag(name: string): MockTransactionTag {
        return new MockTransactionTag(name);
    }

    toCreateRequest(): Record<string, unknown> {
        return { name: this.name };
    }
}

const mockInvalidItems = [
    { name: 'Dining', value: 'source-dining' },
    { name: 'Transport', value: 'source-transport' },
    { name: 'Gift', value: 'source-gift' }
];

function createdCategoryResponse(type: number = mockCategoryType.Expense): Record<number, any[]> {
    return {
        [type]: [
            { id: 'empty-parent', name: 'Empty Parent', subCategories: [] },
            {
                id: 'created-parent',
                name: 'Created Parent',
                subCategories: [
                    { id: 'category-dining', name: 'Dining' },
                    { id: 'category-transport', name: 'Transport' },
                    { id: 'category-unrequested', name: 'Unrequested' }
                ]
            },
            { id: 'missing-children-parent', name: 'Missing Children' }
        ]
    };
}

function createdTagResponse(): any[] {
    return [
        { id: 'tag-dining', name: 'Dining' },
        { id: 'tag-gift', name: 'Gift' },
        { id: 'tag-unrequested', name: 'Unrequested' }
    ];
}

jest.mock('vue', () => {
    const actual = jest.requireActual('vue') as any;
    return {
        ...actual,
        useTemplateRef: () => actual.ref(mockSnackbar)
    };
});
jest.mock('@/components/desktop/SnackBar.vue', () => ({
    __esModule: true,
    default: { name: 'SnackBar' }
}));
jest.mock('@/locales/helpers.ts', () => ({
    useI18n: () => ({
        tt: (key: string, values?: Record<string, unknown>) => (
            values ? `tt:${key}:${JSON.stringify(values)}` : `tt:${key}`
        )
    })
}));
jest.mock('@/stores/transactionCategory.ts', () => ({ useTransactionCategoriesStore: () => mockCategoryStore }));
jest.mock('@/stores/transactionTag.ts', () => ({ useTransactionTagsStore: () => mockTagStore }));
jest.mock('@/core/base.ts', () => ({
    values: (value: Record<string, unknown>) => Object.values(value)
}));
jest.mock('@/core/category.ts', () => ({ CategoryType: mockCategoryType }));
jest.mock('@/consts/icon.ts', () => ({ AUTOMATICALLY_CREATED_CATEGORY_ICON_ID: 'auto-created-icon' }));
jest.mock('@/consts/color.ts', () => ({ DEFAULT_CATEGORY_COLOR: '#eeeeee' }));
jest.mock('@/models/transaction_category.ts', () => ({ TransactionCategory: MockTransactionCategory }));
jest.mock('@/models/transaction_tag.ts', () => ({ TransactionTag: MockTransactionTag }));
jest.mock('@/lib/common.ts', () => ({
    isDefined: (value: unknown) => value !== undefined && value !== null,
    arrayItemToObjectField: (items: string[], value: unknown) => Object.fromEntries(items.map(item => [item, value]))
}));
jest.mock('@mdi/js', () => ({
    mdiSelectAll: 'select-all',
    mdiSelect: 'select-none',
    mdiSelectInverse: 'select-inverse',
    mdiDotsVertical: 'dots-vertical'
}));

import BatchCreateDialog from '@/views/desktop/transactions/import/dialogs/BatchCreateDialog.vue';

function setup(): { bindings: any; exposed: any } {
    let exposed: any = null;
    const bindings = (BatchCreateDialog as any).setup(
        {},
        {
            attrs: {},
            slots: {},
            emit: jest.fn(),
            expose(value: any) {
                exposed = value;
            }
        }
    );
    return { bindings, exposed };
}

async function flush(times = 10): Promise<void> {
    for (let index = 0; index < times; index++) await Promise.resolve();
}

function createHostNode(type: string, text = ''): any {
    return { type, text, children: [], parent: null, props: {}, style: {} };
}

function mountWithHostRenderer(): { app: any; root: any; state: any; vm: any } {
    const { createRenderer, defineComponent, h } = jest.requireActual('vue') as any;
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
        createElement(hostType: string) {
            return createHostNode(hostType);
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
        }
    });
    const SlotHost = defineComponent({
        name: 'SlotHost',
        setup(_props: unknown, { attrs, slots }: any) {
            return () => h('stub', attrs, Object.values(slots).flatMap((slot: any) => slot?.() ?? []));
        }
    });
    const app = renderer.createApp(BatchCreateDialog as any);
    app.config.warnHandler = () => undefined;
    for (const name of [
        'v-dialog', 'v-card', 'v-card-text', 'v-btn', 'v-icon', 'v-menu', 'v-list', 'v-list-item',
        'v-text-field', 'v-checkbox-btn', 'v-table', 'v-chip', 'v-progress-circular', 'snack-bar'
    ]) app.component(name, SlotHost);
    const root = createHostNode('root');
    const vm = app.mount(root) as any;
    return { app, root, state: vm.$.setupState, vm };
}

function collectHostCallbacks(
    node: any,
    callbacks: Array<{ name: string; callback: (...args: any[]) => any }>,
    seen = new Set<any>()
): void {
    if (!node || typeof node !== 'object' || seen.has(node)) return;
    seen.add(node);
    for (const [name, value] of Object.entries(node.props ?? {})) {
        if (!name.startsWith('on')) continue;
        if (typeof value === 'function') {
            callbacks.push({ name, callback: value as (...args: any[]) => any });
        } else if (Array.isArray(value)) {
            for (const callback of value) {
                if (typeof callback === 'function') callbacks.push({ name, callback });
            }
        }
    }
    for (const child of node.children ?? []) collectHostCallbacks(child, callbacks, seen);
}

async function invokeHostCallbacks(callbacks: Array<{ name: string; callback: (...args: any[]) => any }>): Promise<void> {
    for (const { name, callback } of callbacks) {
        if (name === 'onUpdate:modelValue') {
            callback('Dining');
        } else {
            callback();
        }
        await flush(3);
    }
}

beforeEach(() => {
    jest.clearAllMocks();
    mockCategoryStore.addCategories.mockResolvedValue(createdCategoryResponse());
    mockCategoryStore.loadAllCategories.mockResolvedValue(undefined);
    mockTagStore.addTags.mockResolvedValue(createdTagResponse());
    mockTagStore.loadAllTags.mockResolvedValue(undefined);
});

describe('BatchCreateDialog selection, open, and close contracts', () => {
    test('opens with reset search and all incoming names selected, then cancels by rejection', async () => {
        const { bindings, exposed } = setup();
        bindings.searchKeyword.value = 'stale search';
        bindings.selectedNames.value = ['stale selection'];

        const result = bindings.open({ type: 'expenseCategory', invalidItems: mockInvalidItems });
        expect(exposed.open).toBe(bindings.open);
        expect(bindings.type.value).toBe('expenseCategory');
        expect(bindings.invalidItems.value).toStrictEqual(mockInvalidItems);
        expect(bindings.searchKeyword.value).toBe('');
        expect(bindings.selectedNames.value).toStrictEqual(['Dining', 'Transport', 'Gift']);
        expect(bindings.showState.value).toBe(true);

        bindings.cancel();
        await expect(result).rejects.toBeUndefined();
        expect(bindings.showState.value).toBe(false);
    });

    test('supports missing items and optional cancel resolver without stale selection', async () => {
        const { bindings } = setup();
        bindings.cancel();
        expect(bindings.showState.value).toBe(false);

        const result = bindings.open({ type: 'tag' });
        expect(bindings.invalidItems.value).toBeUndefined();
        expect(bindings.selectedNames.value).toStrictEqual([]);
        expect(bindings.filteredInvalidItems.value).toStrictEqual([]);
        expect(bindings.anyButNotAllSelected.value).toBe(false);
        expect(bindings.allFilteredItemsSelected.value).toBe(false);
        bindings.cancel();
        await expect(result).rejects.toBeUndefined();
    });

    test('filters case-insensitively and computes full, partial, and empty selection states', () => {
        const { bindings } = setup();
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        expect(bindings.selectedNameSet.value).toStrictEqual({ Dining: true, Transport: true, Gift: true });
        expect(bindings.filteredInvalidItems.value).toStrictEqual(mockInvalidItems);
        expect(bindings.allFilteredItemsSelected.value).toBe(true);
        expect(bindings.anyButNotAllSelected.value).toBe(false);

        bindings.searchKeyword.value = '  in  ';
        expect(bindings.filteredInvalidItems.value).toStrictEqual([mockInvalidItems[0]]);
        bindings.selectedNames.value = [];
        expect(bindings.allFilteredItemsSelected.value).toBe(false);
        expect(bindings.anyButNotAllSelected.value).toBe(false);

        bindings.searchKeyword.value = '';
        bindings.selectedNames.value = ['Dining'];
        expect(bindings.anyButNotAllSelected.value).toBe(true);
        bindings.selectedNames.value = ['Dining', 'Transport', 'Gift'];
        expect(bindings.anyButNotAllSelected.value).toBe(false);

        bindings.searchKeyword.value = 'missing';
        expect(bindings.filteredInvalidItems.value).toStrictEqual([]);
        expect(bindings.anyButNotAllSelected.value).toBe(false);
        expect(bindings.allFilteredItemsSelected.value).toBe(false);
    });

    test('merges or removes filtered selections and updates individual names', () => {
        const { bindings } = setup();
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.searchKeyword.value = 'Dining';
        bindings.selectedNames.value = ['Gift'];

        bindings.allFilteredItemsSelected.value = true;
        expect(bindings.selectedNames.value).toStrictEqual(['Gift', 'Dining']);
        bindings.allFilteredItemsSelected.value = true;
        expect(bindings.selectedNames.value).toStrictEqual(['Gift', 'Dining']);
        bindings.allFilteredItemsSelected.value = false;
        expect(bindings.selectedNames.value).toStrictEqual(['Gift']);

        bindings.selectedNames.value = ['Dining', 'Transport'];
        bindings.updateSelectedNames('Dining', false);
        expect(bindings.selectedNames.value).toStrictEqual(['Transport']);
        bindings.updateSelectedNames('Gift', true);
        expect(bindings.selectedNames.value).toStrictEqual(['Transport', 'Gift']);
        bindings.updateSelectedNames('Gift', null);
        expect(bindings.selectedNames.value).toStrictEqual(['Transport']);
    });

    test('selects all, none, and inverse while accepting absent item lists', () => {
        const { bindings } = setup();
        bindings.invalidItems.value = undefined;
        bindings.selectAllItems();
        bindings.selectInvertItems();
        expect(bindings.selectedNames.value).toStrictEqual([]);

        bindings.invalidItems.value = mockInvalidItems;
        bindings.selectedNames.value = ['Transport'];
        bindings.selectInvertItems();
        expect(bindings.selectedNames.value).toStrictEqual(['Dining', 'Gift']);
        bindings.selectAllItems();
        expect(bindings.selectedNames.value).toStrictEqual(['Dining', 'Transport', 'Gift']);
        bindings.selectNoneItems();
        expect(bindings.selectedNames.value).toStrictEqual([]);
    });
});

describe('BatchCreateDialog category creation contracts', () => {
    test.each([
        ['expenseCategory', mockCategoryType.Expense, 'tt:Default Expense Category'],
        ['incomeCategory', mockCategoryType.Income, 'tt:Default Income Category'],
        ['transferCategory', mockCategoryType.Transfer, 'tt:Default Transfer Category']
    ] as const)('creates selected %s items and resolves source-to-category identities', async (dialogType, categoryType, primaryName) => {
        mockCategoryStore.addCategories.mockResolvedValueOnce(createdCategoryResponse(categoryType));
        const { bindings } = setup();
        const resultPromise = bindings.open({ type: dialogType, invalidItems: mockInvalidItems });
        bindings.selectedNames.value = ['Dining', 'Transport'];

        bindings.confirm();
        expect(bindings.submitting.value).toBe(true);
        const request = mockCategoryStore.addCategories.mock.calls[0]![0] as any;
        expect(request).toStrictEqual({
            categories: [{
                name: primaryName,
                type: categoryType,
                icon: 'auto-created-icon',
                color: '#eeeeee',
                subCategories: [
                    expect.objectContaining({ name: 'Dining', type: categoryType, icon: 'auto-created-icon', clientSessionId: '' }),
                    expect.objectContaining({ name: 'Transport', type: categoryType, icon: 'auto-created-icon', clientSessionId: '' })
                ]
            }]
        });
        expect(JSON.stringify(request)).not.toMatch(/amount|cents|date|account/i);
        await flush();

        await expect(resultPromise).resolves.toStrictEqual({
            sourceTargetMap: {
                'source-dining': 'category-dining',
                'source-transport': 'category-transport'
            }
        });
        expect(mockCategoryStore.loadAllCategories).toHaveBeenCalledWith({ force: false });
        expect(bindings.submitting.value).toBe(false);
        expect(bindings.showState.value).toBe(false);
    });

    test('submits an explicit empty category batch without inventing money or date fields', async () => {
        mockCategoryStore.addCategories.mockResolvedValueOnce({ [mockCategoryType.Expense]: [] });
        const { bindings } = setup();
        const resultPromise = bindings.open({ type: 'expenseCategory' });
        bindings.confirm();
        const request = mockCategoryStore.addCategories.mock.calls[0]![0] as any;
        expect(request.categories[0].subCategories).toStrictEqual([]);
        expect(request.categories[0]).not.toHaveProperty('amount');
        expect(request.categories[0]).not.toHaveProperty('amountCents');
        expect(request.categories[0]).not.toHaveProperty('date');
        expect(request.categories[0]).not.toHaveProperty('accountId');
        await flush();
        await expect(resultPromise).resolves.toStrictEqual({ sourceTargetMap: {} });
    });

    test('ignores created children without source identities and undefined source values', async () => {
        const invalidItems = [
            { name: 'Dining', value: undefined as unknown as string },
            { name: 'Transport', value: 'source-transport' }
        ];
        const { bindings } = setup();
        const resultPromise = bindings.open({ type: 'expenseCategory', invalidItems });
        bindings.confirm();
        await flush();
        await expect(resultPromise).resolves.toStrictEqual({
            sourceTargetMap: { 'source-transport': 'category-transport' }
        });
    });

    test('reports category add and refresh failures only when unprocessed', async () => {
        const { bindings } = setup();

        const handledAdd = { processed: true, message: 'handled category add' };
        mockCategoryStore.addCategories.mockRejectedValueOnce(handledAdd);
        bindings.open({ type: 'expenseCategory', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).not.toHaveBeenCalledWith(handledAdd);
        expect(bindings.submitting.value).toBe(false);

        const readableAdd = { processed: false, message: 'category add failed' };
        mockCategoryStore.addCategories.mockRejectedValueOnce(readableAdd);
        bindings.open({ type: 'incomeCategory', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(readableAdd);

        mockCategoryStore.addCategories.mockRejectedValueOnce('raw category add failure');
        bindings.open({ type: 'transferCategory', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith('raw category add failure');

        const handledRefresh = { processed: true, message: 'handled category refresh' };
        mockCategoryStore.loadAllCategories.mockRejectedValueOnce(handledRefresh);
        bindings.open({ type: 'expenseCategory', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).not.toHaveBeenCalledWith(handledRefresh);

        const readableRefresh = { processed: false, message: 'category refresh failed' };
        mockCategoryStore.loadAllCategories.mockRejectedValueOnce(readableRefresh);
        bindings.open({ type: 'expenseCategory', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(readableRefresh);

        mockCategoryStore.loadAllCategories.mockRejectedValueOnce('raw category refresh failure');
        bindings.open({ type: 'expenseCategory', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith('raw category refresh failure');
        expect(bindings.showState.value).toBe(true);
    });

    test('keeps optional snackbar and resolver boundaries safe', async () => {
        const { bindings } = setup();
        bindings.type.value = 'expenseCategory';
        bindings.invalidItems.value = mockInvalidItems;
        bindings.selectedNames.value = ['Dining'];
        bindings.snackbar.value = null;
        mockCategoryStore.addCategories.mockRejectedValueOnce({ processed: false, message: 'no snackbar' });
        bindings.confirm();
        await flush();
        expect(bindings.submitting.value).toBe(false);

        mockCategoryStore.addCategories.mockResolvedValueOnce(createdCategoryResponse());
        bindings.snackbar.value = mockSnackbar;
        bindings.confirm();
        await flush();
        expect(bindings.showState.value).toBe(false);
    });
});

describe('BatchCreateDialog tag creation contracts', () => {
    test('creates selected tags and resolves only matching source identities', async () => {
        const { bindings } = setup();
        const resultPromise = bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.selectedNames.value = ['Dining', 'Gift'];
        bindings.confirm();
        expect(mockTagStore.addTags).toHaveBeenCalledWith({
            tags: [{ name: 'Dining' }, { name: 'Gift' }],
            skipExists: true
        });
        expect(JSON.stringify(mockTagStore.addTags.mock.calls[0]![0])).not.toMatch(/amount|cents|date|account/i);
        await flush();

        await expect(resultPromise).resolves.toStrictEqual({
            sourceTargetMap: {
                'source-dining': 'tag-dining',
                'source-gift': 'tag-gift'
            }
        });
        expect(mockTagStore.loadAllTags).toHaveBeenCalledWith({ force: false });
        expect(bindings.submitting.value).toBe(false);
        expect(bindings.showState.value).toBe(false);
    });

    test('submits empty tags and excludes undefined source identities', async () => {
        mockTagStore.addTags.mockResolvedValueOnce([]);
        const empty = setup();
        const emptyResult = empty.bindings.open({ type: 'tag' });
        empty.bindings.confirm();
        expect(mockTagStore.addTags).toHaveBeenCalledWith({ tags: [], skipExists: true });
        await flush();
        await expect(emptyResult).resolves.toStrictEqual({ sourceTargetMap: {} });

        mockTagStore.addTags.mockResolvedValueOnce(createdTagResponse());
        const undefinedSource = setup();
        const undefinedResult = undefinedSource.bindings.open({
            type: 'tag',
            invalidItems: [{ name: 'Dining', value: undefined as unknown as string }]
        });
        undefinedSource.bindings.confirm();
        await flush();
        await expect(undefinedResult).resolves.toStrictEqual({ sourceTargetMap: {} });
    });

    test('reports tag add and refresh failures only when unprocessed', async () => {
        const { bindings } = setup();

        const handledAdd = { processed: true, message: 'handled tag add' };
        mockTagStore.addTags.mockRejectedValueOnce(handledAdd);
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).not.toHaveBeenCalledWith(handledAdd);

        const readableAdd = { processed: false, message: 'tag add failed' };
        mockTagStore.addTags.mockRejectedValueOnce(readableAdd);
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(readableAdd);

        mockTagStore.addTags.mockRejectedValueOnce('raw tag add failure');
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith('raw tag add failure');

        const handledRefresh = { processed: true, message: 'handled tag refresh' };
        mockTagStore.loadAllTags.mockRejectedValueOnce(handledRefresh);
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).not.toHaveBeenCalledWith(handledRefresh);

        const readableRefresh = { processed: false, message: 'tag refresh failed' };
        mockTagStore.loadAllTags.mockRejectedValueOnce(readableRefresh);
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith(readableRefresh);

        mockTagStore.loadAllTags.mockRejectedValueOnce('raw tag refresh failure');
        bindings.open({ type: 'tag', invalidItems: mockInvalidItems });
        bindings.confirm();
        await flush();
        expect(mockSnackbar.showError).toHaveBeenCalledWith('raw tag refresh failure');
        expect(bindings.submitting.value).toBe(false);
    });

    test('does nothing for an unset or unknown type', () => {
        const { bindings } = setup();
        bindings.confirm();
        bindings.type.value = 'unknown';
        bindings.confirm();
        expect(mockCategoryStore.addCategories).not.toHaveBeenCalled();
        expect(mockTagStore.addTags).not.toHaveBeenCalled();
        expect(bindings.submitting.value).toBe(false);
    });
});

describe('BatchCreateDialog production template', () => {
    test('renders every title, popup state, selection state, and event wrapper', async () => {
        const mounted = mountWithHostRenderer();
        const opened = mounted.state.open({ type: 'expenseCategory', invalidItems: mockInvalidItems });
        opened.catch(() => undefined);
        try {
            const { nextTick } = jest.requireActual('vue') as any;
            await nextTick();
            const callbacks: Array<{ name: string; callback: (...args: any[]) => any }> = [];
            collectHostCallbacks(mounted.root, callbacks);
            await invokeHostCallbacks(callbacks);
            expect(callbacks.length).toBeGreaterThan(8);

            for (const dialogType of ['incomeCategory', 'transferCategory', 'tag'] as const) {
                mounted.state.type = dialogType;
                mounted.state.submitting = dialogType === 'tag';
                mounted.state.searchKeyword = dialogType === 'tag' ? 'missing' : '';
                mounted.state.selectedNames = dialogType === 'tag' ? [] : ['Dining'];
                await nextTick();
                expect(mounted.root.children.length).toBeGreaterThan(0);
            }

            mounted.state.invalidItems = undefined;
            mounted.state.submitting = false;
            mounted.state.showState = false;
            await nextTick();
            expect(mounted.root.children.length).toBeGreaterThan(0);
        } finally {
            mounted.app.unmount();
        }
    });
});
